//! Duplicate-key detection for JSON input (contract section 4).
//!
//! `serde_json` silently keeps the last value for duplicate keys. This module
//! provides a pre-parse scan that rejects any JSON text containing duplicate
//! object keys at any nesting depth, before `serde_json::from_str` is called.

/// Error from duplicate-key detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateKeyError {
    /// The duplicated key.
    pub key: String,
    /// Byte offset where the duplicate was detected.
    pub offset: usize,
}

impl std::fmt::Display for DuplicateKeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "duplicate key '{}' at byte offset {}",
            self.key, self.offset
        )
    }
}

impl std::error::Error for DuplicateKeyError {}

/// Scan raw JSON text for duplicate object keys at any depth.
///
/// Returns `Ok(())` if no duplicates are found, or `Err(DuplicateKeyError)`
/// with the first duplicate encountered.
///
/// This is a lightweight scanner — it does not validate JSON syntax beyond
/// what is needed to track object key sets. Malformed JSON may pass this scan
/// but will be caught by `serde_json` later.
pub fn reject_duplicate_keys(json: &str) -> Result<(), DuplicateKeyError> {
    let bytes = json.as_bytes();
    let len = bytes.len();
    let mut pos = 0;
    // Stack of key sets, one per nested object.
    let mut key_stack: Vec<std::collections::HashSet<String>> = Vec::new();

    while pos < len {
        match bytes[pos] {
            b'{' => {
                key_stack.push(std::collections::HashSet::new());
                pos += 1;
            }
            b'}' => {
                key_stack.pop();
                pos += 1;
            }
            b'"' if !key_stack.is_empty() => {
                // Only parse keys when inside an object.
                // We need to determine if this string is a key (followed by ':')
                // or a value.
                let start = pos;
                let (s, end) = parse_json_string(bytes, pos)?;
                // Skip whitespace after string
                let mut p = end;
                while p < len && bytes[p].is_ascii_whitespace() {
                    p += 1;
                }
                if p < len && bytes[p] == b':' {
                    // This is a key
                    if !key_stack.last_mut().unwrap().insert(s.clone()) {
                        return Err(DuplicateKeyError {
                            key: s,
                            offset: start,
                        });
                    }
                }
                pos = end;
            }
            b'"' => {
                // String outside any object context — skip it
                let (_, end) = parse_json_string(bytes, pos)?;
                pos = end;
            }
            b'\\' => {
                pos += 2; // skip escape
            }
            _ => {
                pos += 1;
            }
        }
    }
    Ok(())
}

/// Parse a JSON string starting at `pos` (which must point to the opening `"`).
/// Returns `(unescaped_string, position_after_closing_quote)`.
fn parse_json_string(bytes: &[u8], pos: usize) -> Result<(String, usize), DuplicateKeyError> {
    debug_assert_eq!(bytes[pos], b'"');
    let mut i = pos + 1;
    let mut result = String::new();
    while i < bytes.len() {
        match bytes[i] {
            b'"' => return Ok((result, i + 1)),
            b'\\' => {
                if i + 1 >= bytes.len() {
                    break;
                }
                match bytes[i + 1] {
                    b'"' | b'\\' | b'/' => {
                        result.push(bytes[i + 1] as char);
                        i += 2;
                    }
                    b'n' => {
                        result.push('\n');
                        i += 2;
                    }
                    b'r' => {
                        result.push('\r');
                        i += 2;
                    }
                    b't' => {
                        result.push('\t');
                        i += 2;
                    }
                    b'b' => {
                        result.push('\u{0008}');
                        i += 2;
                    }
                    b'f' => {
                        result.push('\u{000C}');
                        i += 2;
                    }
                    b'u' => {
                        // Unicode escape: \uXXXX
                        if i + 5 < bytes.len() {
                            let hex = std::str::from_utf8(&bytes[i + 2..i + 6]).unwrap_or("0000");
                            if let Ok(cp) = u32::from_str_radix(hex, 16) {
                                if let Some(c) = char::from_u32(cp) {
                                    result.push(c);
                                }
                            }
                            i += 6;
                        } else {
                            i += 2;
                        }
                    }
                    _ => {
                        result.push(bytes[i + 1] as char);
                        i += 2;
                    }
                }
            }
            b => {
                result.push(b as char);
                i += 1;
            }
        }
    }
    // Unterminated string — return what we have; serde_json will catch it.
    Ok((result, i))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_duplicates_passes() {
        assert!(reject_duplicate_keys(r#"{"a":1,"b":2}"#).is_ok());
    }

    #[test]
    fn top_level_duplicate_rejected() {
        let err = reject_duplicate_keys(r#"{"a":1,"a":2}"#).unwrap_err();
        assert_eq!(err.key, "a");
    }

    #[test]
    fn nested_duplicate_rejected() {
        let err = reject_duplicate_keys(r#"{"outer":{"x":1,"x":2}}"#).unwrap_err();
        assert_eq!(err.key, "x");
    }

    #[test]
    fn same_key_different_objects_passes() {
        assert!(reject_duplicate_keys(r#"{"a":{"x":1},"b":{"x":2}}"#).is_ok());
    }

    #[test]
    fn empty_object_passes() {
        assert!(reject_duplicate_keys(r#"{}"#).is_ok());
    }

    #[test]
    fn array_with_objects_passes() {
        assert!(reject_duplicate_keys(r#"[{"a":1},{"a":2}]"#).is_ok());
    }

    #[test]
    fn string_value_not_confused_for_key() {
        assert!(reject_duplicate_keys(r#"{"a":"a","b":"c"}"#).is_ok());
    }

    #[test]
    fn deeply_nested_duplicate() {
        let json = r#"{"a":{"b":{"c":{"d":1,"d":2}}}}"#;
        let err = reject_duplicate_keys(json).unwrap_err();
        assert_eq!(err.key, "d");
    }
}
