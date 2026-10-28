//! Colour analysis for logo review (D-G).
//!
//! In-house implementations:
//! - WCAG 2.x relative luminance and contrast ratio
//! - APCA Lc (SAPC-APCA 0.0.98G constants, labelled `apca_advisory`)
//! - Machado 2009 colour-vision deficiency simulation at severity 1.0
//!   (protanopia, deuteranopia, tritanopia; labelled `simulation`)
//!
//! All findings are advisory; none produce a pass/fail verdict.

use palette::Srgb;

/// An sRGB colour with 8-bit channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct Colour {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Colour {
    /// Parse a hex colour string (`#rrggbb` or `#rgb`).
    pub fn from_hex(hex: &str) -> Result<Self, String> {
        let hex = hex.strip_prefix('#').unwrap_or(hex);
        match hex.len() {
            3 => {
                let r = u8::from_str_radix(&hex[0..1], 16).map_err(|e| e.to_string())?;
                let g = u8::from_str_radix(&hex[1..2], 16).map_err(|e| e.to_string())?;
                let b = u8::from_str_radix(&hex[2..3], 16).map_err(|e| e.to_string())?;
                Ok(Self {
                    r: r * 17,
                    g: g * 17,
                    b: b * 17,
                })
            }
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).map_err(|e| e.to_string())?;
                let g = u8::from_str_radix(&hex[2..4], 16).map_err(|e| e.to_string())?;
                let b = u8::from_str_radix(&hex[4..6], 16).map_err(|e| e.to_string())?;
                Ok(Self { r, g, b })
            }
            _ => Err(format!("invalid hex colour: #{hex}")),
        }
    }

    /// Convert to linear sRGB components [0.0, 1.0].
    fn to_linear(self) -> [f64; 3] {
        let srgb = Srgb::new(
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
        );
        let linear: palette::LinSrgb = srgb.into_linear();
        [linear.red as f64, linear.green as f64, linear.blue as f64]
    }
}

// ---------------------------------------------------------------------------
// WCAG 2.x relative luminance and contrast ratio
// ---------------------------------------------------------------------------

/// WCAG 2.x relative luminance (0.0 = black, 1.0 = white).
///
/// Uses the sRGB linearisation specified by WCAG 2.1.
pub fn wcag_luminance(c: Colour) -> f64 {
    let lin = c.to_linear();
    0.2126 * lin[0] + 0.7152 * lin[1] + 0.0722 * lin[2]
}

/// WCAG 2.x contrast ratio between two colours.
///
/// Returns a value in [1.0, 21.0]. The lighter colour is always divided
/// by the darker (after adding the 0.05 offset).
pub fn wcag_contrast(fg: Colour, bg: Colour) -> f64 {
    let l1 = wcag_luminance(fg);
    let l2 = wcag_luminance(bg);
    let (lighter, darker) = if l1 >= l2 { (l1, l2) } else { (l2, l1) };
    (lighter + 0.05) / (darker + 0.05)
}

// ---------------------------------------------------------------------------
// APCA (Accessible Perceptual Contrast Algorithm) 0.0.98G
// ---------------------------------------------------------------------------

/// APCA Lc contrast value (SAPC-APCA 0.0.98G).
///
/// Returns a signed value: positive means the text is lighter than the
/// background, negative means darker. The magnitude is the perceptual
/// contrast. Labelled `apca_advisory` in output.
///
/// Constants from the published SAPC-APCA 0.0.98G specification.
pub fn apca_lc(text: Colour, bg: Colour) -> f64 {
    let text_y = sapc_luminance(text);
    let bg_y = sapc_luminance(bg);

    const S_SOFT_CLF: f64 = 0.022;
    const S_SOFT_FLR: f64 = 0.001;
    const S_SCALE: f64 = 1.14;
    const S_OFFSET: f64 = 0.001_204_891_357_670_328_4;

    const D_SOFT_CLF: f64 = 0.03;
    const D_SOFT_FLR: f64 = 0.001;
    const D_SCALE: f64 = 1.14;
    const D_OFFSET: f64 = 0.000_399_999_999_999_999_96;

    let sapc = if bg_y > text_y {
        // Normal polarity (dark text on light bg)
        let b = bg_y.powf(0.56) - S_OFFSET;
        let t = text_y.powf(0.57) - S_OFFSET;
        let delta = b - t;
        if delta < S_SOFT_CLF {
            0.0
        } else {
            delta * S_SCALE
        }
    } else {
        // Reverse polarity (light text on dark bg)
        let t = text_y.powf(0.62) - D_OFFSET;
        let b = bg_y.powf(0.65) - D_OFFSET;
        let delta = t - b;
        if delta < D_SOFT_CLF {
            0.0
        } else {
            -(delta * D_SCALE)
        }
    };

    // Soft floor: use the polarity-appropriate threshold
    let floor = if sapc >= 0.0 { S_SOFT_FLR } else { D_SOFT_FLR };
    if sapc.abs() < floor {
        0.0
    } else {
        sapc * 100.0
    }
}

/// SAPC luminance from sRGB, using the APCA-specific exponents.
fn sapc_luminance(c: Colour) -> f64 {
    let lin = c.to_linear();
    // SAPC uses its own weighting of linear sRGB
    0.2126729 * lin[0] + 0.7151522 * lin[1] + 0.0721750 * lin[2]
}

// ---------------------------------------------------------------------------
// Machado 2009 colour-vision deficiency simulation (severity 1.0)
// ---------------------------------------------------------------------------

/// Type of colour-vision deficiency to simulate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CvdKind {
    Protanopia,
    Deuteranopia,
    Tritanopia,
}

/// Simulate colour-vision deficiency using Machado 2009 matrices at severity 1.0.
///
/// Returns the simulated sRGB colour. The matrices are applied in linear
/// sRGB space, then clamped back to [0, 255].
pub fn simulate_cvd(c: Colour, kind: CvdKind) -> Colour {
    let lin = c.to_linear();
    let m = match kind {
        CvdKind::Protanopia => [
            [0.15220, 1.05258, -0.20478],
            [0.11450, 0.78628, 0.09922],
            [-0.00388, -0.04811, 1.05199],
        ],
        CvdKind::Deuteranopia => [
            [0.36732, 0.86065, -0.22797],
            [0.28008, 0.67250, 0.04741],
            [-0.01182, 0.04294, 0.96888],
        ],
        CvdKind::Tritanopia => [
            [1.25553, -0.07675, -0.17878],
            [-0.07841, 1.14081, -0.06240],
            [0.00473, 0.04008, 0.95519],
        ],
    };

    let r = (m[0][0] * lin[0] + m[0][1] * lin[1] + m[0][2] * lin[2]).clamp(0.0, 1.0);
    let g = (m[1][0] * lin[0] + m[1][1] * lin[1] + m[1][2] * lin[2]).clamp(0.0, 1.0);
    let b = (m[2][0] * lin[0] + m[2][1] * lin[1] + m[2][2] * lin[2]).clamp(0.0, 1.0);

    // Convert back to sRGB 8-bit
    let srgb = Srgb::new(r as f32, g as f32, b as f32);
    let encoded: Srgb<u8> = srgb.into_format();
    Colour {
        r: encoded.red,
        g: encoded.green,
        b: encoded.blue,
    }
}

/// Result of checking a pair of colours.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ColourCheckResult {
    pub foreground: Colour,
    pub background: Colour,
    /// WCAG 2.x contrast ratio [1.0, 21.0].
    pub wcag_contrast_ratio: f64,
    /// WCAG relative luminance of foreground.
    pub fg_luminance: f64,
    /// WCAG relative luminance of background.
    pub bg_luminance: f64,
    /// APCA Lc value (labelled `apca_advisory`).
    pub apca_lc: f64,
    /// Machado 2.0 CVD simulations at severity 1.0 (labelled `simulation`).
    pub cvd_simulations: CvdSimulations,
}

/// CVD simulation results for a colour pair.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CvdSimulations {
    pub protanopia: SimulatedPair,
    pub deuteranopia: SimulatedPair,
    pub tritanopia: SimulatedPair,
}

/// A simulated foreground/background pair under CVD.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SimulatedPair {
    pub foreground: Colour,
    pub background: Colour,
    pub wcag_contrast_ratio: f64,
}

/// Check a foreground/background pair against all colour analyses.
pub fn check_pair(fg: Colour, bg: Colour) -> ColourCheckResult {
    let wcag_cr = wcag_contrast(fg, bg);
    let fg_lum = wcag_luminance(fg);
    let bg_lum = wcag_luminance(bg);
    let apca = apca_lc(fg, bg);

    let prot_fg = simulate_cvd(fg, CvdKind::Protanopia);
    let prot_bg = simulate_cvd(bg, CvdKind::Protanopia);
    let deut_fg = simulate_cvd(fg, CvdKind::Deuteranopia);
    let deut_bg = simulate_cvd(bg, CvdKind::Deuteranopia);
    let trit_fg = simulate_cvd(fg, CvdKind::Tritanopia);
    let trit_bg = simulate_cvd(bg, CvdKind::Tritanopia);

    ColourCheckResult {
        foreground: fg,
        background: bg,
        wcag_contrast_ratio: (wcag_cr * 100.0).round() / 100.0,
        fg_luminance: (fg_lum * 10000.0).round() / 10000.0,
        bg_luminance: (bg_lum * 10000.0).round() / 10000.0,
        apca_lc: (apca * 100.0).round() / 100.0,
        cvd_simulations: CvdSimulations {
            protanopia: SimulatedPair {
                foreground: prot_fg,
                background: prot_bg,
                wcag_contrast_ratio: (wcag_contrast(prot_fg, prot_bg) * 100.0).round() / 100.0,
            },
            deuteranopia: SimulatedPair {
                foreground: deut_fg,
                background: deut_bg,
                wcag_contrast_ratio: (wcag_contrast(deut_fg, deut_bg) * 100.0).round() / 100.0,
            },
            tritanopia: SimulatedPair {
                foreground: trit_fg,
                background: trit_bg,
                wcag_contrast_ratio: (wcag_contrast(trit_fg, trit_bg) * 100.0).round() / 100.0,
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wcag_black_white_contrast() {
        let black = Colour { r: 0, g: 0, b: 0 };
        let white = Colour {
            r: 255,
            g: 255,
            b: 255,
        };
        let ratio = wcag_contrast(black, white);
        assert!(
            (ratio - 21.0).abs() < 0.1,
            "black/white contrast should be ~21.0, got {ratio}"
        );
    }

    #[test]
    fn wcag_777_white_contrast() {
        let grey = Colour::from_hex("#777777").unwrap();
        let white = Colour {
            r: 255,
            g: 255,
            b: 255,
        };
        let ratio = wcag_contrast(grey, white);
        assert!(
            (ratio - 4.48).abs() < 0.05,
            "#777/white contrast should be ~4.48, got {ratio}"
        );
    }

    #[test]
    fn wcag_same_colour_is_one() {
        let c = Colour::from_hex("#808080").unwrap();
        let ratio = wcag_contrast(c, c);
        assert!(
            (ratio - 1.0).abs() < 0.01,
            "same colour contrast should be 1.0, got {ratio}"
        );
    }

    #[test]
    fn hex_parsing_short() {
        let c = Colour::from_hex("#f00").unwrap();
        assert_eq!(c.r, 255);
        assert_eq!(c.g, 0);
        assert_eq!(c.b, 0);
    }

    #[test]
    fn hex_parsing_long() {
        let c = Colour::from_hex("#00ff00").unwrap();
        assert_eq!(c.r, 0);
        assert_eq!(c.g, 255);
        assert_eq!(c.b, 0);
    }

    #[test]
    fn hex_parsing_invalid() {
        assert!(Colour::from_hex("#xyz").is_err());
        assert!(Colour::from_hex("#12345").is_err());
    }

    #[test]
    fn apca_black_on_white_positive() {
        let black = Colour { r: 0, g: 0, b: 0 };
        let white = Colour {
            r: 255,
            g: 255,
            b: 255,
        };
        let lc = apca_lc(black, white);
        assert!(lc > 0.0, "black on white should have positive Lc, got {lc}");
        assert!(lc > 100.0, "black on white APCA should be high, got {lc}");
    }

    #[test]
    fn apca_same_colour_near_zero() {
        let grey = Colour::from_hex("#808080").unwrap();
        let lc = apca_lc(grey, grey);
        assert!(
            lc.abs() < 1.0,
            "same colour APCA should be near zero, got {lc}"
        );
    }

    #[test]
    fn cvd_protanopia_shifts_red() {
        let red = Colour { r: 255, g: 0, b: 0 };
        let sim = simulate_cvd(red, CvdKind::Protanopia);
        // Protanopia should reduce the red channel significantly
        assert!(
            sim.r < 200,
            "protanopia simulation of red should reduce red channel, got {}",
            sim.r
        );
    }

    #[test]
    fn cvd_black_stays_black() {
        let black = Colour { r: 0, g: 0, b: 0 };
        for kind in [
            CvdKind::Protanopia,
            CvdKind::Deuteranopia,
            CvdKind::Tritanopia,
        ] {
            let sim = simulate_cvd(black, kind);
            assert_eq!(sim.r, 0);
            assert_eq!(sim.g, 0);
            assert_eq!(sim.b, 0);
        }
    }

    #[test]
    fn check_pair_produces_all_fields() {
        let fg = Colour::from_hex("#333333").unwrap();
        let bg = Colour::from_hex("#ffffff").unwrap();
        let result = check_pair(fg, bg);
        assert!(result.wcag_contrast_ratio > 1.0);
        assert!(result.apca_lc.abs() > 0.0);
        assert!(result.cvd_simulations.protanopia.wcag_contrast_ratio >= 1.0);
        assert!(result.cvd_simulations.deuteranopia.wcag_contrast_ratio >= 1.0);
        assert!(result.cvd_simulations.tritanopia.wcag_contrast_ratio >= 1.0);
    }
}
