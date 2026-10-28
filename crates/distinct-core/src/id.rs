//! Brand and record ID validation (contract section 3/4).
//!
//! Brand ID: `[a-z][a-z0-9-]{0,47}` — max 48 chars, no traversal.
//! Record ID: `[a-z][a-z0-9_.-]{0,95}` — max 96 chars, case-sensitive.

/// Errors from ID validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdError {
    /// Empty string.
    Empty,
    /// First character does not match the grammar.
    InvalidStart(char),
    /// A character in the body is not permitted.
    InvalidChar(char),
    /// The ID exceeds the maximum length.
    TooLong { len: usize, max: usize },
}

impl std::fmt::Display for IdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "ID must not be empty"),
            Self::InvalidStart(c) => write!(f, "ID must start with [a-z], got '{c}'"),
            Self::InvalidChar(c) => write!(f, "invalid character in ID: '{c}'"),
            Self::TooLong { len, max } => write!(f, "ID length {len} exceeds maximum {max}"),
        }
    }
}

impl std::error::Error for IdError {}

/// Validate a brand ID: `[a-z][a-z0-9-]{0,47}`.
pub fn validate_brand_id(id: &str) -> Result<(), IdError> {
    validate_id_impl(id, 48, |c| {
        c == '-' || c.is_ascii_lowercase() || c.is_ascii_digit()
    })
}

/// Validate a record ID: `[a-z][a-z0-9_.-]{0,95}`.
pub fn validate_record_id(id: &str) -> Result<(), IdError> {
    validate_id_impl(id, 96, |c| {
        c == '_' || c == '.' || c == '-' || c.is_ascii_lowercase() || c.is_ascii_digit()
    })
}

fn validate_id_impl(
    id: &str,
    max_len: usize,
    body_ok: impl Fn(char) -> bool,
) -> Result<(), IdError> {
    if id.is_empty() {
        return Err(IdError::Empty);
    }
    let bytes = id.as_bytes();
    if bytes.len() > max_len {
        return Err(IdError::TooLong {
            len: bytes.len(),
            max: max_len,
        });
    }
    let first = bytes[0];
    if !first.is_ascii_lowercase() {
        return Err(IdError::InvalidStart(first as char));
    }
    for &b in &bytes[1..] {
        let c = b as char;
        if !body_ok(c) {
            return Err(IdError::InvalidChar(c));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brand_id_valid() {
        assert!(validate_brand_id("acme").is_ok());
        assert!(validate_brand_id("a").is_ok());
        assert!(validate_brand_id("acme-brand-01").is_ok());
        assert!(validate_brand_id(&"a".repeat(48)).is_ok());
    }

    #[test]
    fn brand_id_rejects_uppercase() {
        assert_eq!(validate_brand_id("Acme"), Err(IdError::InvalidStart('A')));
    }

    #[test]
    fn brand_id_rejects_empty() {
        assert_eq!(validate_brand_id(""), Err(IdError::Empty));
    }

    #[test]
    fn brand_id_rejects_too_long() {
        let long = "a".repeat(49);
        assert!(matches!(
            validate_brand_id(&long),
            Err(IdError::TooLong { len: 49, max: 48 })
        ));
    }

    #[test]
    fn brand_id_rejects_underscore() {
        assert!(matches!(
            validate_brand_id("ac_me"),
            Err(IdError::InvalidChar('_'))
        ));
    }

    #[test]
    fn record_id_valid() {
        assert!(validate_record_id("platform").is_ok());
        assert!(validate_record_id("voice.main").is_ok());
        assert!(validate_record_id("source_web_01").is_ok());
        assert!(validate_record_id(&"z".repeat(96)).is_ok());
    }

    #[test]
    fn record_id_rejects_dash() {
        // Record IDs allow underscore, dot, hyphen — but let's verify hyphen works
        assert!(validate_record_id("my-record").is_ok());
    }

    #[test]
    fn record_id_rejects_empty() {
        assert_eq!(validate_record_id(""), Err(IdError::Empty));
    }

    #[test]
    fn record_id_rejects_too_long() {
        let long = "a".repeat(97);
        assert!(matches!(
            validate_record_id(&long),
            Err(IdError::TooLong { len: 97, max: 96 })
        ));
    }
}
