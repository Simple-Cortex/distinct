//! SVG inspection for the limited static-SVG profile (contract §8, D-E).
//!
//! Inspection only — no rendering. The resvg worker is out of scope;
//! `native_proof` is always reported as `"unavailable"`.

use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::collections::BTreeMap;
use std::path::Path;

/// Maximum input file size: 2 MiB.
const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;
/// Maximum element nesting depth.
const MAX_DEPTH: usize = 64;
/// Maximum total element count.
const MAX_ELEMENTS: usize = 100_000;

/// Elements accepted by the limited profile.
const SUPPORTED_ELEMENTS: &[&str] = &[
    "svg", "g", "path", "rect", "circle", "ellipse", "line", "polyline", "polygon", "title",
    "desc", "metadata", "defs",
];

/// Outcome of an SVG inspection.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// All checks passed; unsupported items may still be listed.
    Inspected,
    /// Unsupported elements or attributes were found (reported, not stripped).
    UnsupportedContent,
    /// A structural check failed (malformed XML, missing viewBox, non-finite numbers).
    Invalid,
}

/// One unsupported item found during inspection.
#[derive(Debug, Clone, serde::Serialize)]
pub struct UnsupportedItem {
    /// `"element"` or `"attribute"`.
    pub kind: String,
    /// Slash-separated element path from root (e.g. `"svg/g/script"`).
    pub path: String,
    /// Name of the element or attribute.
    pub name: String,
}

/// One issue found during inspection.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Issue {
    /// `"error"` or `"warning"`.
    pub severity: String,
    /// Human-readable description.
    pub message: String,
}

/// Result of inspecting one SVG file.
#[derive(Debug, Clone, serde::Serialize)]
pub struct InspectResult {
    pub verdict: Verdict,
    /// SHA-256 of the raw file bytes (lowercase 64-hex).
    pub file_hash: String,
    /// Element counts by tag name.
    pub element_counts: BTreeMap<String, usize>,
    /// Unsupported elements and attributes found.
    pub unsupported: Vec<UnsupportedItem>,
    /// Structural issues (missing viewBox, non-finite numbers, etc.).
    pub issues: Vec<Issue>,
    /// Always `"unavailable"` in this alpha (no renderer worker).
    pub native_proof: String,
    /// Total element count.
    pub element_count: usize,
    /// Maximum nesting depth reached.
    pub max_depth_reached: usize,
}

/// Errors that can occur during inspection.
#[derive(Debug, Clone)]
pub enum InspectError {
    /// Input exceeds the 2 MiB cap.
    InputTooLarge { actual: usize, max: usize },
    /// Nesting depth exceeds 64.
    DepthExceeded { depth: usize, max: usize },
    /// Element count exceeds 100 000.
    ElementLimitExceeded { count: usize, max: usize },
    /// I/O error reading the file.
    IoError(String),
    /// XML well-formedness error.
    XmlError(String),
    /// File is not valid UTF-8.
    Utf8Error(String),
}

impl std::fmt::Display for InspectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InputTooLarge { actual, max } => {
                write!(f, "input too large: {actual} bytes exceeds {max} byte cap")
            }
            Self::DepthExceeded { depth, max } => {
                write!(f, "nesting depth {depth} exceeds limit of {max}")
            }
            Self::ElementLimitExceeded { count, max } => {
                write!(f, "element count {count} exceeds limit of {max}")
            }
            Self::IoError(e) => write!(f, "I/O error: {e}"),
            Self::XmlError(e) => write!(f, "XML error: {e}"),
            Self::Utf8Error(e) => write!(f, "UTF-8 error: {e}"),
        }
    }
}

impl std::error::Error for InspectError {}

/// Inspect an SVG file at `file_path` against the limited static-SVG profile.
pub fn inspect_svg(file_path: &Path) -> Result<InspectResult, InspectError> {
    let bytes = std::fs::read(file_path)
        .map_err(|e| InspectError::IoError(format!("{file_path:?}: {e}")))?;

    if bytes.len() > MAX_INPUT_BYTES {
        return Err(InspectError::InputTooLarge {
            actual: bytes.len(),
            max: MAX_INPUT_BYTES,
        });
    }

    let file_hash = distinct_core::hash::hash_bytes(&bytes);

    let input = std::str::from_utf8(&bytes).map_err(|e| InspectError::Utf8Error(e.to_string()))?;

    let mut reader = Reader::from_reader(input.as_bytes());
    reader.config_mut().trim_text_start = true;
    reader.config_mut().trim_text_end = true;
    let mut buf = Vec::new();

    let mut element_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut unsupported: Vec<UnsupportedItem> = Vec::new();
    let mut issues: Vec<Issue> = Vec::new();
    let mut depth: usize = 0;
    let mut max_depth: usize = 0;
    let mut total_elements: usize = 0;
    let mut element_stack: Vec<String> = Vec::new();
    let mut has_viewbox = false;
    let mut svg_seen = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let is_svg = name == "svg";

                depth += 1;
                max_depth = max_depth.max(depth);
                element_stack.push(name.clone());

                if depth > MAX_DEPTH {
                    return Err(InspectError::DepthExceeded {
                        depth,
                        max: MAX_DEPTH,
                    });
                }

                total_elements += 1;
                if total_elements > MAX_ELEMENTS {
                    return Err(InspectError::ElementLimitExceeded {
                        count: total_elements,
                        max: MAX_ELEMENTS,
                    });
                }

                *element_counts.entry(name.clone()).or_insert(0) += 1;

                if is_unsupported_element(&name) {
                    let path = element_stack.join("/");
                    unsupported.push(UnsupportedItem {
                        kind: "element".into(),
                        path,
                        name: name.clone(),
                    });
                }

                scan_attributes(
                    &e.name(),
                    &mut e.attributes().flatten(),
                    &element_stack,
                    &mut unsupported,
                    &mut issues,
                );

                if is_svg {
                    svg_seen = true;
                    check_viewbox_attrs(
                        &mut e.attributes().flatten(),
                        &mut has_viewbox,
                        &mut issues,
                    );
                }
            }
            Ok(Event::Empty(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let is_svg = name == "svg";

                total_elements += 1;
                if total_elements > MAX_ELEMENTS {
                    return Err(InspectError::ElementLimitExceeded {
                        count: total_elements,
                        max: MAX_ELEMENTS,
                    });
                }

                *element_counts.entry(name.clone()).or_insert(0) += 1;

                if is_unsupported_element(&name) {
                    let mut path_elems: Vec<&str> =
                        element_stack.iter().map(|s| s.as_str()).collect();
                    path_elems.push(&name);
                    let path = path_elems.join("/");
                    unsupported.push(UnsupportedItem {
                        kind: "element".into(),
                        path,
                        name: name.clone(),
                    });
                }

                scan_attributes(
                    &e.name(),
                    &mut e.attributes().flatten(),
                    &element_stack,
                    &mut unsupported,
                    &mut issues,
                );

                if is_svg {
                    svg_seen = true;
                    check_viewbox_attrs(
                        &mut e.attributes().flatten(),
                        &mut has_viewbox,
                        &mut issues,
                    );
                }
            }
            Ok(Event::End(_)) => {
                depth = depth.saturating_sub(1);
                element_stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(InspectError::XmlError(format!("XML parse error: {e}")));
            }
            _ => {}
        }
        buf.clear();
    }

    if svg_seen && !has_viewbox {
        issues.push(Issue {
            severity: "error".into(),
            message: "missing viewBox attribute on root <svg>".into(),
        });
    }

    let verdict = if issues.iter().any(|i| i.severity == "error") {
        Verdict::Invalid
    } else if !unsupported.is_empty() {
        Verdict::UnsupportedContent
    } else {
        Verdict::Inspected
    };

    Ok(InspectResult {
        verdict,
        file_hash: file_hash.as_hex().to_string(),
        element_counts,
        unsupported,
        issues,
        native_proof: "unavailable".into(),
        element_count: total_elements,
        max_depth_reached: max_depth,
    })
}

/// Returns true if the element name is in the unsupported set.
fn is_unsupported_element(name: &str) -> bool {
    if SUPPORTED_ELEMENTS.contains(&name) {
        return false;
    }
    // fe* filter primitives are unsupported
    if name.starts_with("fe") && name.len() > 2 {
        let third = name.as_bytes()[2];
        if third.is_ascii_uppercase() {
            return true;
        }
    }
    // Anything not in the supported set is unsupported
    true
}

/// Scan attributes of one element for `on*` event handlers and external hrefs.
fn scan_attributes<'a, I>(
    _elem_name: &quick_xml::name::QName,
    attrs: &mut I,
    element_stack: &[String],
    unsupported: &mut Vec<UnsupportedItem>,
    issues: &mut Vec<Issue>,
) where
    I: Iterator<Item = quick_xml::events::attributes::Attribute<'a>>,
{
    let path = element_stack.join("/");
    for attr in attrs {
        let key_bytes = attr.key.as_ref();
        let key_str = String::from_utf8_lossy(key_bytes);

        // Check for on* event-handler attributes
        if key_str.len() >= 3
            && key_str.as_bytes()[0] == b'o'
            && key_str.as_bytes()[1] == b'n'
            && key_str.as_bytes()[2].is_ascii_lowercase()
        {
            unsupported.push(UnsupportedItem {
                kind: "attribute".into(),
                path: path.clone(),
                name: key_str.to_string(),
            });
        }

        // Check for external href or url() references
        let val = String::from_utf8_lossy(&attr.value);
        if (key_str == "href" || key_str == "xlink:href")
            && (val.contains("http://") || val.contains("https://") || val.contains("url("))
        {
            issues.push(Issue {
                severity: "error".into(),
                message: format!("external href in attribute '{key_str}': {val}"),
            });
        }
        // Check style attribute for url()
        if key_str == "style" && val.contains("url(") {
            issues.push(Issue {
                severity: "error".into(),
                message: format!("external url() in style attribute: {val}"),
            });
        }

        // Check numeric attributes for non-finite values
        check_numeric_attr_finite(&key_str, &val, &path, issues);
    }
}

/// Check if a numeric-looking attribute value is finite.
fn check_numeric_attr_finite(key: &str, val: &str, path: &str, issues: &mut Vec<Issue>) {
    // Only check attributes that are known to be numeric
    let is_numeric_attr = matches!(
        key,
        "x" | "y"
            | "width"
            | "height"
            | "cx"
            | "cy"
            | "r"
            | "rx"
            | "ry"
            | "x1"
            | "y1"
            | "x2"
            | "y2"
            | "dx"
            | "dy"
            | "fx"
            | "fy"
            | "offset"
            | "stroke-width"
            | "stroke-dashoffset"
            | "font-size"
    );
    if !is_numeric_attr {
        return;
    }
    // Try to parse as f64; if it parses to non-finite, report it
    if let Ok(v) = val.trim().parse::<f64>() {
        if !v.is_finite() {
            issues.push(Issue {
                severity: "error".into(),
                message: format!("non-finite value in attribute '{key}' at {path}: {val}"),
            });
        }
    }
    // Also check for literal NaN/Infinity strings
    let lower = val.trim().to_ascii_lowercase();
    if lower.contains("nan") || lower.contains("infinity") || lower.contains("inf") {
        issues.push(Issue {
            severity: "error".into(),
            message: format!("non-finite value in attribute '{key}' at {path}: {val}"),
        });
    }
}

/// Check viewBox attribute on the root `<svg>` element.
fn check_viewbox_attrs<'a, I>(attrs: &mut I, has_viewbox: &mut bool, issues: &mut Vec<Issue>)
where
    I: Iterator<Item = quick_xml::events::attributes::Attribute<'a>>,
{
    for attr in attrs {
        let key = String::from_utf8_lossy(attr.key.as_ref());
        if key != "viewBox" {
            continue;
        }
        *has_viewbox = true;
        let val = String::from_utf8_lossy(&attr.value);
        let parts: Vec<&str> = val.split_whitespace().collect();
        if parts.len() != 4 {
            issues.push(Issue {
                severity: "error".into(),
                message: format!("viewBox must have exactly 4 numbers, got: {val}"),
            });
            return;
        }
        let nums: Vec<f64> = parts.iter().filter_map(|p| p.parse::<f64>().ok()).collect();
        if nums.len() != 4 {
            issues.push(Issue {
                severity: "error".into(),
                message: format!("viewBox contains non-numeric values: {val}"),
            });
            return;
        }
        if !nums.iter().all(|n| n.is_finite()) {
            issues.push(Issue {
                severity: "error".into(),
                message: format!("viewBox contains non-finite numbers: {val}"),
            });
            return;
        }
        // Width and height (parts[2] and parts[3]) must be positive
        if nums[2] <= 0.0 || nums[3] <= 0.0 {
            issues.push(Issue {
                severity: "error".into(),
                message: format!(
                    "viewBox width and height must be positive, got: {} {}",
                    nums[2], nums[3]
                ),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn fixture_dir() -> std::path::PathBuf {
        let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("tests");
        p.push("fixtures");
        p
    }

    fn write_fixture(name: &str, content: &str) -> std::path::PathBuf {
        let dir = fixture_dir();
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn inspect_valid_simple_logo() {
        let path = write_fixture(
            "valid_simple.svg",
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
  <rect x="10" y="10" width="80" height="80" fill="blue"/>
  <circle cx="50" cy="50" r="20" fill="red"/>
</svg>"#,
        );
        let result = inspect_svg(&path).unwrap();
        assert_eq!(result.verdict, Verdict::Inspected);
        assert_eq!(result.native_proof, "unavailable");
        assert_eq!(result.element_counts.get("svg"), Some(&1));
        assert_eq!(result.element_counts.get("rect"), Some(&1));
        assert_eq!(result.element_counts.get("circle"), Some(&1));
        assert!(result.unsupported.is_empty());
        assert!(result.issues.is_empty());
        assert_eq!(result.file_hash.len(), 64);
        fs::remove_file(&path).ok();
    }

    #[test]
    fn script_reported_unsupported() {
        let path = write_fixture(
            "script_unsupported.svg",
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
  <script>alert("x")</script>
  <rect x="0" y="0" width="100" height="100"/>
</svg>"#,
        );
        let result = inspect_svg(&path).unwrap();
        assert_eq!(result.verdict, Verdict::UnsupportedContent);
        assert!(!result.unsupported.is_empty());
        assert!(result.unsupported.iter().any(|u| u.name == "script"));
        fs::remove_file(&path).ok();
    }

    #[test]
    fn onclick_attr_reported() {
        let path = write_fixture(
            "onclick_attr.svg",
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
  <rect x="0" y="0" width="100" height="100" onclick="alert(1)"/>
</svg>"#,
        );
        let result = inspect_svg(&path).unwrap();
        assert_eq!(result.verdict, Verdict::UnsupportedContent);
        assert!(result
            .unsupported
            .iter()
            .any(|u| u.kind == "attribute" && u.name == "onclick"));
        fs::remove_file(&path).ok();
    }

    #[test]
    fn external_href_reported() {
        let path = write_fixture(
            "external_href.svg",
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
  <rect x="0" y="0" width="100" height="100" href="https://example.com/img.png"/>
</svg>"#,
        );
        let result = inspect_svg(&path).unwrap();
        assert_eq!(result.verdict, Verdict::Invalid);
        assert!(result
            .issues
            .iter()
            .any(|i| i.message.contains("external href")));
        fs::remove_file(&path).ok();
    }

    #[test]
    fn missing_viewbox_invalid() {
        let path = write_fixture(
            "missing_viewbox.svg",
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
  <rect x="0" y="0" width="100" height="100"/>
</svg>"#,
        );
        let result = inspect_svg(&path).unwrap();
        assert_eq!(result.verdict, Verdict::Invalid);
        assert!(result.issues.iter().any(|i| i.message.contains("viewBox")));
        fs::remove_file(&path).ok();
    }

    #[test]
    fn nonfinite_coord_invalid() {
        let path = write_fixture(
            "nonfinite_coord.svg",
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
  <rect x="NaN" y="0" width="100" height="100"/>
</svg>"#,
        );
        let result = inspect_svg(&path).unwrap();
        assert_eq!(result.verdict, Verdict::Invalid);
        assert!(result
            .issues
            .iter()
            .any(|i| i.message.contains("non-finite")));
        fs::remove_file(&path).ok();
    }

    #[test]
    fn oversize_input_rejected() {
        let dir = fixture_dir();
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("oversize.svg");
        // Write a file just over 2 MiB
        let mut content =
            String::from("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 1 1\">");
        // Pad with comments to exceed 2 MiB
        while content.len() < MAX_INPUT_BYTES + 1 {
            content.push_str("<!-- padding -->");
        }
        content.push_str("</svg>");
        fs::write(&path, &content).unwrap();
        let err = inspect_svg(&path).unwrap_err();
        assert!(matches!(err, InspectError::InputTooLarge { .. }));
        fs::remove_file(&path).ok();
    }

    #[test]
    fn depth_cap_enforced() {
        let dir = fixture_dir();
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("deep_nesting.svg");
        // Build SVG with 65 levels of <g> nesting (exceeds limit of 64)
        let mut content =
            String::from("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 100 100\">");
        for _ in 0..65 {
            content.push_str("<g>");
        }
        content.push_str("<rect x=\"0\" y=\"0\" width=\"1\" height=\"1\"/>");
        for _ in 0..65 {
            content.push_str("</g>");
        }
        content.push_str("</svg>");
        fs::write(&path, &content).unwrap();
        let err = inspect_svg(&path).unwrap_err();
        assert!(matches!(err, InspectError::DepthExceeded { .. }));
        fs::remove_file(&path).ok();
    }

    #[test]
    fn unsupported_not_stripped() {
        // Verify that the file bytes hash is unchanged — unsupported elements
        // are reported but never removed from the source.
        let svg_content = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
  <script>alert("x")</script>
  <rect x="0" y="0" width="100" height="100"/>
</svg>"#;
        let path = write_fixture("unsupported_not_stripped.svg", svg_content);
        let result = inspect_svg(&path).unwrap();
        // The file_hash must equal SHA-256 of the original bytes
        let expected_hash = distinct_core::hash::hash_bytes(svg_content.as_bytes());
        assert_eq!(result.file_hash, expected_hash.as_hex());
        // Unsupported items are reported
        assert!(!result.unsupported.is_empty());
        fs::remove_file(&path).ok();
    }
}
