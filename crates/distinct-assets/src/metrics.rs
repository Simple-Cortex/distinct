//! Advisory geometric metrics for SVG paths (D-G).
//!
//! Uses `kurbo` 0.13 to compute per-path and whole-logo metrics from
//! SVG path data. All metrics are labelled `advisory_metrics` in output;
//! LP-12 (binding quality gates) stays deferred.

use kurbo::{BezPath, ParamCurveArclen, Shape};

/// Accuracy for kurbo arc-length and area computations.
const ACCURACY: f64 = 0.25;

/// Metrics for a single SVG path.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PathMetrics {
    /// Number of path elements (MoveTo, LineTo, QuadTo, CurveTo, ClosePath).
    pub node_count: usize,
    /// Number of segments (LineTo, QuadTo, CurveTo, ClosePath — excludes the initial MoveTo).
    pub segment_count: usize,
    /// Total arc length of the path.
    pub length: f64,
    /// Signed area enclosed by the path (positive = clockwise in SVG coords).
    pub area: f64,
    /// Bounding box: [x_min, y_min, x_max, y_max].
    pub bbox: [f64; 4],
}

/// Whole-logo advisory metrics.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LogoMetrics {
    /// Per-path metrics.
    pub paths: Vec<PathMetrics>,
    /// Bounding-box centre offset from the viewBox centre: [dx, dy].
    pub bbox_center_offset: [f64; 2],
    /// Total bounding box of all paths: [x_min, y_min, x_max, y_max].
    pub total_bbox: [f64; 4],
}

/// Parse an SVG `d` attribute into a `BezPath`.
///
/// Returns `None` if the path data is malformed.
pub fn parse_path_d(d: &str) -> Option<BezPath> {
    BezPath::from_svg(d).ok()
}

/// Compute metrics for a single SVG path `d` string.
pub fn path_metrics(d: &str) -> Option<PathMetrics> {
    let path = parse_path_d(d)?;
    Some(compute_path_metrics(&path))
}

/// Compute metrics from a parsed BezPath.
fn compute_path_metrics(path: &BezPath) -> PathMetrics {
    let elements = path.iter().collect::<Vec<_>>();
    let node_count = elements.len();

    // Segments exclude the initial MoveTo
    let segment_count = path.segments().count();

    // Arc length: sum of segment arclens
    let length: f64 = path.segments().map(|seg| seg.arclen(ACCURACY)).sum();

    // Area: sum of segment areas (signed)
    let area: f64 = path.segments().map(|seg| seg.area()).sum();

    // Bounding box
    let bbox_rect = path.bounding_box();
    let bbox = [bbox_rect.x0, bbox_rect.y0, bbox_rect.x1, bbox_rect.y1];

    PathMetrics {
        node_count,
        segment_count,
        length: round4(length),
        area: round4(area),
        bbox: [
            round4(bbox[0]),
            round4(bbox[1]),
            round4(bbox[2]),
            round4(bbox[3]),
        ],
    }
}

/// Compute whole-logo metrics from a list of path `d` strings and a viewBox.
///
/// `viewbox` is `[min_x, min_y, width, height]` from the SVG `viewBox` attribute.
pub fn logo_metrics(path_d_strings: &[&str], viewbox: [f64; 4]) -> LogoMetrics {
    let paths: Vec<PathMetrics> = path_d_strings
        .iter()
        .filter_map(|d| path_metrics(d))
        .collect();

    // Compute total bbox and center offset
    let total_bbox = if paths.is_empty() {
        [0.0, 0.0, 0.0, 0.0]
    } else {
        let mut x_min = f64::INFINITY;
        let mut y_min = f64::INFINITY;
        let mut x_max = f64::NEG_INFINITY;
        let mut y_max = f64::NEG_INFINITY;
        for p in &paths {
            x_min = x_min.min(p.bbox[0]);
            y_min = y_min.min(p.bbox[1]);
            x_max = x_max.max(p.bbox[2]);
            y_max = y_max.max(p.bbox[3]);
        }
        [round4(x_min), round4(y_min), round4(x_max), round4(y_max)]
    };

    // ViewBox centre
    let vb_cx = viewbox[0] + viewbox[2] / 2.0;
    let vb_cy = viewbox[1] + viewbox[3] / 2.0;

    // Bbox centre
    let bbox_cx = (total_bbox[0] + total_bbox[2]) / 2.0;
    let bbox_cy = (total_bbox[1] + total_bbox[3]) / 2.0;

    let bbox_center_offset = [round4(bbox_cx - vb_cx), round4(bbox_cy - vb_cy)];

    LogoMetrics {
        paths,
        bbox_center_offset,
        total_bbox,
    }
}

fn round4(v: f64) -> f64 {
    (v * 10000.0).round() / 10000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_rect_path() {
        // A 100x100 rectangle at origin
        let d = "M 0 0 L 100 0 L 100 100 L 0 100 Z";
        let m = path_metrics(d).unwrap();
        assert_eq!(m.node_count, 5); // M, L, L, L, Z
        assert_eq!(m.segment_count, 4); // L, L, L, Z
        assert!(
            m.length > 399.0 && m.length < 401.0,
            "perimeter ~400, got {}",
            m.length
        );
        assert!(
            m.area.abs() > 9900.0 && m.area.abs() < 10100.0,
            "area ~10000, got {}",
            m.area
        );
        assert_eq!(m.bbox, [0.0, 0.0, 100.0, 100.0]);
    }

    #[test]
    fn line_path_has_zero_area() {
        let d = "M 0 0 L 100 100";
        let m = path_metrics(d).unwrap();
        assert!(
            m.area.abs() < 1.0,
            "line should have ~zero area, got {}",
            m.area
        );
        assert!(
            m.length > 140.0 && m.length < 142.0,
            "diagonal ~141.4, got {}",
            m.length
        );
    }

    #[test]
    fn invalid_path_returns_none() {
        assert!(path_metrics("not a path").is_none());
        // Empty string parses to an empty BezPath (valid but zero metrics)
        let empty = path_metrics("");
        // kurbo accepts empty strings; just verify it doesn't panic
        let _ = empty;
    }

    #[test]
    fn logo_metrics_centered() {
        // A centered rectangle in a 200x200 viewBox
        let paths = &["M 50 50 L 150 50 L 150 150 L 50 150 Z"];
        let viewbox = [0.0, 0.0, 200.0, 200.0];
        let lm = logo_metrics(paths, viewbox);
        assert_eq!(lm.paths.len(), 1);
        assert!(
            lm.bbox_center_offset[0].abs() < 0.01,
            "centered bbox should have ~zero x offset, got {}",
            lm.bbox_center_offset[0]
        );
        assert!(
            lm.bbox_center_offset[1].abs() < 0.01,
            "centered bbox should have ~zero y offset, got {}",
            lm.bbox_center_offset[1]
        );
    }

    #[test]
    fn logo_metrics_offset_bbox() {
        // An off-center rectangle
        let paths = &["M 10 10 L 60 10 L 60 60 L 10 60 Z"];
        let viewbox = [0.0, 0.0, 100.0, 100.0];
        let lm = logo_metrics(paths, viewbox);
        // bbox center is (35, 35), viewbox center is (50, 50)
        assert!(
            (lm.bbox_center_offset[0] - (-15.0)).abs() < 0.1,
            "x offset should be -15, got {}",
            lm.bbox_center_offset[0]
        );
        assert!(
            (lm.bbox_center_offset[1] - (-15.0)).abs() < 0.1,
            "y offset should be -15, got {}",
            lm.bbox_center_offset[1]
        );
    }

    #[test]
    fn cubic_curve_path() {
        let d = "M 0 0 C 33 100, 66 0, 100 100";
        let m = path_metrics(d).unwrap();
        assert_eq!(m.node_count, 2); // M, C
        assert_eq!(m.segment_count, 1); // C
        assert!(
            m.length > 100.0,
            "curve should be longer than straight line"
        );
    }
}
