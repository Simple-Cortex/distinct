//! SVG rendering worker (D-F).
//!
//! Uses resvg 0.49 + usvg + tiny-skia to render SVG files to PNG.
//! No text/fonts, no external images, no raster images — only the
//! supported static-SVG profile from D-E.

/// Maximum renders per job.
pub const MAX_RENDERS: usize = 24;
/// Maximum pixels per side.
pub const MAX_SIDE_PX: u32 = 4096;
/// Maximum total megapixels.
pub const MAX_MEGAPIXELS: f64 = 16.0;

/// A single render request within a job.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct RenderSpec {
    pub size_px: u32,
    pub background: String,
}

/// Input job for the render worker (read from stdin as JSON).
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct RenderJob {
    pub svg_path: String,
    pub out_dir: String,
    pub renders: Vec<RenderSpec>,
}

/// One rendered output in the manifest.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RenderOutput {
    pub file: String,
    pub sha256: String,
    pub width: u32,
    pub height: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub advisory: Option<AdvisoryRaster>,
}

/// Advisory raster checks (D-F): smallest visible feature and contrast.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct AdvisoryRaster {
    pub min_feature_px: u32,
    pub contrast_ratio: f64,
}

/// Result of a render job.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RenderManifest {
    pub outputs: Vec<RenderOutput>,
    pub renderer: String,
    pub svg_hash: String,
}

/// Errors from the render worker.
#[derive(Debug)]
pub enum RenderError {
    TooManyRenders { count: usize, max: usize },
    SizeTooLarge { size_px: u32, max: u32 },
    MegapixelLimit { mp: f64, max: f64 },
    SvgParse(String),
    IoError(String),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooManyRenders { count, max } => {
                write!(f, "too many renders: {count} exceeds limit of {max}")
            }
            Self::SizeTooLarge { size_px, max } => {
                write!(f, "size {size_px}px exceeds limit of {max}px per side")
            }
            Self::MegapixelLimit { mp, max } => {
                write!(f, "total megapixels {mp:.1} exceeds limit of {max:.1}")
            }
            Self::SvgParse(e) => write!(f, "SVG parse error: {e}"),
            Self::IoError(e) => write!(f, "I/O error: {e}"),
        }
    }
}

impl std::error::Error for RenderError {}

/// Validate a render job before executing.
pub fn validate_job(job: &RenderJob) -> Result<(), RenderError> {
    if job.renders.len() > MAX_RENDERS {
        return Err(RenderError::TooManyRenders {
            count: job.renders.len(),
            max: MAX_RENDERS,
        });
    }
    for spec in &job.renders {
        if spec.size_px > MAX_SIDE_PX {
            return Err(RenderError::SizeTooLarge {
                size_px: spec.size_px,
                max: MAX_SIDE_PX,
            });
        }
    }
    // Check total megapixels
    let total_px: f64 = job
        .renders
        .iter()
        .map(|s| s.size_px as f64 * s.size_px as f64)
        .sum();
    let total_mp = total_px / 1_000_000.0;
    if total_mp > MAX_MEGAPIXELS {
        return Err(RenderError::MegapixelLimit {
            mp: total_mp,
            max: MAX_MEGAPIXELS,
        });
    }
    Ok(())
}

/// Execute a render job: parse SVG, render each spec, write PNGs.
pub fn execute_job(job: &RenderJob) -> Result<RenderManifest, RenderError> {
    validate_job(job)?;

    // Read SVG
    let svg_data = std::fs::read(&job.svg_path)
        .map_err(|e| RenderError::IoError(format!("{}: {e}", job.svg_path)))?;

    let svg_hash = distinct_core::hash::hash_bytes(&svg_data);

    // Parse with usvg (no text, no external resources)
    let opt = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_data(&svg_data, &opt)
        .map_err(|e| RenderError::SvgParse(e.to_string()))?;

    let svg_size = tree.size();
    let svg_w = svg_size.width() as f64;
    let svg_h = svg_size.height() as f64;

    // Create output directory
    std::fs::create_dir_all(&job.out_dir)
        .map_err(|e| RenderError::IoError(format!("{}: {e}", job.out_dir)))?;

    let mut outputs = Vec::new();

    for (i, spec) in job.renders.iter().enumerate() {
        // Compute scale to fit the SVG into size_px while preserving aspect ratio
        let scale = spec.size_px as f64 / svg_w.max(svg_h);
        let out_w = (svg_w * scale).ceil() as u32;
        let out_h = (svg_h * scale).ceil() as u32;

        let mut pixmap = resvg::tiny_skia::Pixmap::new(out_w, out_h)
            .ok_or_else(|| RenderError::IoError("failed to allocate pixmap".into()))?;

        // Fill background
        match spec.background.as_str() {
            "transparent" => {} // leave transparent
            hex => {
                if let Some(color) = parse_hex_color(hex) {
                    pixmap.fill(color);
                }
            }
        }

        let transform = resvg::tiny_skia::Transform::from_scale(scale as f32, scale as f32);
        resvg::render(&tree, transform, &mut pixmap.as_mut());

        let bg = pixmap_bg(&pixmap, &spec.background);
        let advisory = compute_advisory_raster(&pixmap, bg);

        // Encode PNG
        let png_data = pixmap
            .encode_png()
            .map_err(|e| RenderError::IoError(format!("PNG encode: {e}")))?;

        let png_hash = distinct_core::hash::hash_bytes(&png_data);
        let filename = format!("render_{i}_{}x{}.png", out_w, out_h);
        let out_path = std::path::Path::new(&job.out_dir).join(&filename);

        std::fs::write(&out_path, &png_data)
            .map_err(|e| RenderError::IoError(format!("{:?}: {e}", out_path)))?;

        outputs.push(RenderOutput {
            file: filename,
            sha256: png_hash.as_hex().to_string(),
            width: out_w,
            height: out_h,
            advisory: Some(advisory),
        });
    }

    Ok(RenderManifest {
        outputs,
        renderer: "resvg 0.49".into(),
        svg_hash: svg_hash.as_hex().to_string(),
    })
}

/// Compute advisory raster checks for a rendered pixmap.
///
/// `min_feature_px`: narrowest horizontal or vertical run of foreground pixels.
/// `contrast_ratio`: WCAG contrast between mean foreground colour and background.
pub fn compute_advisory_raster(pixmap: &resvg::tiny_skia::Pixmap, bg: [u8; 4]) -> AdvisoryRaster {
    let w = pixmap.width();
    let h = pixmap.height();
    let data = pixmap.data();

    let is_fg = |idx: usize| -> bool {
        let r = data[idx];
        let g = data[idx + 1];
        let b = data[idx + 2];
        let a = data[idx + 3];
        a > 0 && (r != bg[0] || g != bg[1] || b != bg[2])
    };

    let min_feature_px = compute_min_feature(data, w, h, is_fg);

    let (mut fg_r, mut fg_g, mut fg_b, mut fg_count): (u64, u64, u64, u64) = (0, 0, 0, 0);
    for y in 0..h {
        for x in 0..w {
            let idx = ((y * w + x) * 4) as usize;
            if is_fg(idx) {
                fg_r += data[idx] as u64;
                fg_g += data[idx + 1] as u64;
                fg_b += data[idx + 2] as u64;
                fg_count += 1;
            }
        }
    }

    let contrast_ratio = if fg_count > 0 {
        let fg_mean = crate::colour::Colour {
            r: (fg_r / fg_count) as u8,
            g: (fg_g / fg_count) as u8,
            b: (fg_b / fg_count) as u8,
        };
        let bg_colour = crate::colour::Colour {
            r: bg[0],
            g: bg[1],
            b: bg[2],
        };
        crate::colour::wcag_contrast(fg_mean, bg_colour)
    } else {
        1.0
    };

    AdvisoryRaster {
        min_feature_px,
        contrast_ratio: (contrast_ratio * 100.0).round() / 100.0,
    }
}

fn compute_min_feature(_data: &[u8], w: u32, h: u32, is_fg: impl Fn(usize) -> bool) -> u32 {
    let mut min_run = w.max(h).max(1);

    for y in 0..h {
        let mut run = 0u32;
        for x in 0..w {
            let idx = ((y * w + x) * 4) as usize;
            if is_fg(idx) {
                run += 1;
            } else if run > 0 {
                min_run = min_run.min(run);
                run = 0;
            }
        }
        if run > 0 {
            min_run = min_run.min(run);
        }
    }

    for x in 0..w {
        let mut run = 0u32;
        for y in 0..h {
            let idx = ((y * w + x) * 4) as usize;
            if is_fg(idx) {
                run += 1;
            } else if run > 0 {
                min_run = min_run.min(run);
                run = 0;
            }
        }
        if run > 0 {
            min_run = min_run.min(run);
        }
    }

    if min_run > w.max(h) {
        0
    } else {
        min_run
    }
}

fn pixmap_bg(pixmap: &resvg::tiny_skia::Pixmap, declared: &str) -> [u8; 4] {
    if declared != "transparent" {
        if let Some(hex) = declared.strip_prefix('#') {
            if hex.len() == 6 {
                let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255);
                let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(255);
                let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(255);
                return [r, g, b, 255];
            }
        }
    }
    let data = pixmap.data();
    let w = pixmap.width();
    let h = pixmap.height();
    let corners: [u32; 4] = [0, w - 1, (h - 1) * w, w * h - 1];
    for &px_idx in &corners {
        let byte_idx = (px_idx * 4) as usize;
        let a = data[byte_idx + 3];
        if a > 0 {
            return [data[byte_idx], data[byte_idx + 1], data[byte_idx + 2], a];
        }
    }
    [255, 255, 255, 255]
}

/// Parse a `#rrggbb` hex colour into a tiny-skia Color.
fn parse_hex_color(hex: &str) -> Option<resvg::tiny_skia::Color> {
    let hex = hex.strip_prefix('#').unwrap_or(hex);
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    resvg::tiny_skia::Color::from_rgba8(r, g, b, 255);
    // tiny_skia::Color::from_rgba8 takes premultiplied values; for opaque colours this is fine
    Some(resvg::tiny_skia::Color::from_rgba8(r, g, b, 255))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_rejects_too_many_renders() {
        let job = RenderJob {
            svg_path: "test.svg".into(),
            out_dir: "/tmp/out".into(),
            renders: (0..25)
                .map(|_| RenderSpec {
                    size_px: 100,
                    background: "transparent".into(),
                })
                .collect(),
        };
        let err = validate_job(&job).unwrap_err();
        assert!(matches!(err, RenderError::TooManyRenders { .. }));
    }

    #[test]
    fn validate_rejects_oversize() {
        let job = RenderJob {
            svg_path: "test.svg".into(),
            out_dir: "/tmp/out".into(),
            renders: vec![RenderSpec {
                size_px: 5000,
                background: "transparent".into(),
            }],
        };
        let err = validate_job(&job).unwrap_err();
        assert!(matches!(err, RenderError::SizeTooLarge { .. }));
    }

    #[test]
    fn validate_accepts_valid_job() {
        let job = RenderJob {
            svg_path: "test.svg".into(),
            out_dir: "/tmp/out".into(),
            renders: vec![
                RenderSpec {
                    size_px: 256,
                    background: "transparent".into(),
                },
                RenderSpec {
                    size_px: 512,
                    background: "#ffffff".into(),
                },
            ],
        };
        assert!(validate_job(&job).is_ok());
    }

    #[test]
    fn advisory_on_synthetic_pixmap() {
        let mut p = resvg::tiny_skia::Pixmap::new(20, 20).unwrap();
        p.fill(resvg::tiny_skia::Color::from_rgba8(255, 255, 255, 255));
        for y in 5..15 {
            for x in 5..15 {
                let idx = ((y * 20 + x) * 4) as usize;
                p.data_mut()[idx] = 0;
                p.data_mut()[idx + 1] = 0;
                p.data_mut()[idx + 2] = 0;
                p.data_mut()[idx + 3] = 255;
            }
        }
        let a = compute_advisory_raster(&p, [255, 255, 255, 255]);
        assert_eq!(a.min_feature_px, 10, "10x10 block → min run 10");
        assert!(
            (a.contrast_ratio - 21.0).abs() < 0.1,
            "black on white ≈ 21.0, got {}",
            a.contrast_ratio
        );
    }

    #[test]
    fn advisory_thin_line() {
        let mut p = resvg::tiny_skia::Pixmap::new(20, 20).unwrap();
        p.fill(resvg::tiny_skia::Color::from_rgba8(255, 255, 255, 255));
        for x in 0..20 {
            let idx = ((10 * 20 + x) * 4) as usize;
            p.data_mut()[idx] = 0;
            p.data_mut()[idx + 1] = 0;
            p.data_mut()[idx + 2] = 0;
            p.data_mut()[idx + 3] = 255;
        }
        let a = compute_advisory_raster(&p, [255, 255, 255, 255]);
        assert_eq!(a.min_feature_px, 1, "single-pixel line → min run 1");
    }

    #[test]
    fn render_simple_svg() {
        let dir = std::env::temp_dir().join(format!(
            "distinct-render-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let svg_path = dir.join("test.svg");
        std::fs::write(
            &svg_path,
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
  <rect x="10" y="10" width="80" height="80" fill="blue"/>
</svg>"#,
        )
        .unwrap();

        let out_dir = dir.join("out");
        let job = RenderJob {
            svg_path: svg_path.to_string_lossy().into(),
            out_dir: out_dir.to_string_lossy().into(),
            renders: vec![RenderSpec {
                size_px: 128,
                background: "transparent".into(),
            }],
        };

        let manifest = execute_job(&job).unwrap();
        assert_eq!(manifest.outputs.len(), 1);
        assert_eq!(manifest.renderer, "resvg 0.49");
        assert!(!manifest.svg_hash.is_empty());
        assert!(manifest.outputs[0].width > 0);
        assert!(manifest.outputs[0].height > 0);
        assert!(out_dir.join(&manifest.outputs[0].file).exists());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
