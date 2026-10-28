//! Domain-separated hashing (contract section 4).
//!
//! All hashes are SHA-256, output as lowercase 64-hex.
//! Structured hashes use RFC 8785 JCS canonicalization with domain prefixes.

use sha2::{Digest, Sha256};

/// A SHA-256 digest as lowercase 64-hex.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct HexHash(String);

impl HexHash {
    /// Create from a pre-validated 64-char lowercase hex string.
    pub fn from_hex(hex: impl Into<String>) -> Result<Self, HashError> {
        let s = hex.into();
        if s.len() != 64 {
            return Err(HashError::InvalidLength(s.len()));
        }
        if !s
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        {
            return Err(HashError::InvalidHex);
        }
        Ok(Self(s))
    }

    /// The hex string.
    #[must_use]
    pub fn as_hex(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for HexHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HexHash({})", &self.0[..12])
    }
}

impl std::fmt::Display for HexHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Hash errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HashError {
    /// Hex string is not 64 characters.
    InvalidLength(usize),
    /// Hex string contains invalid characters.
    InvalidHex,
    /// JSON canonicalization failed.
    Canonicalization(String),
}

impl std::fmt::Display for HashError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidLength(n) => write!(f, "hash must be 64 hex chars, got {n}"),
            Self::InvalidHex => write!(f, "hash contains non-lowercase-hex characters"),
            Self::Canonicalization(e) => write!(f, "JCS canonicalization failed: {e}"),
        }
    }
}

impl std::error::Error for HashError {}

/// Hash raw bytes with a domain prefix.
///
/// `result = SHA256(UTF8(domain) || bytes)`
#[must_use]
pub fn hash_raw(domain: &str, bytes: &[u8]) -> HexHash {
    let mut hasher = Sha256::new();
    hasher.update(domain.as_bytes());
    hasher.update(bytes);
    HexHash(hex::encode(hasher.finalize()))
}

/// Hash a JSON value with a domain prefix using JCS canonicalization.
///
/// `result = SHA256(UTF8(domain) || JCS(value))`
pub fn hash_json(domain: &str, json: &serde_json::Value) -> Result<HexHash, HashError> {
    let canonical = jcs_canonicalize::canonicalize(
        &serde_json::to_string(json).map_err(|e| HashError::Canonicalization(e.to_string()))?,
    )
    .map_err(|e| HashError::Canonicalization(e.to_string()))?;
    Ok(hash_raw(domain, canonical.as_bytes()))
}

/// Domain prefix for content record hashes.
pub const DOMAIN_RECORD: &str = "distinct.record.v1\n";

/// Domain prefix for value digest hashes.
pub const DOMAIN_VALUE: &str = "distinct.value.v1\n";

/// Domain prefix for snapshot hashes.
pub const DOMAIN_SNAPSHOT: &str = "distinct.snapshot.v1\n";

/// Domain prefix for governance index hashes.
pub const DOMAIN_GOVERNANCE: &str = "distinct.governance.v1\n";

/// Domain prefix for STATE hashes.
pub const DOMAIN_STATE: &str = "distinct.state.v1\n";

/// Domain prefix for packet hashes.
pub const DOMAIN_PACKET: &str = "distinct.packet.v1\n";

/// Domain prefix for scope digests.
pub const DOMAIN_SCOPE: &str = "distinct.scope.v1\n";

/// Compute the content_hash for a record envelope.
///
/// `content_hash = SHA256(UTF8("distinct.record.v1\n") || JCS(hash_body))`
/// where `hash_body` is the envelope with `content_hash` and `revision_id` omitted.
pub fn content_hash(envelope_without_derived: &serde_json::Value) -> Result<HexHash, HashError> {
    hash_json(DOMAIN_RECORD, envelope_without_derived)
}

/// Compute the revision_id from a content_hash: `"r-" + content_hash`.
#[must_use]
pub fn revision_id(content_hash: &HexHash) -> String {
    format!("r-{}", content_hash.as_hex())
}

/// Compute a value digest: `SHA256(UTF8("distinct.value.v1\n") || JCS(selected_value))`.
pub fn value_digest(value: &serde_json::Value) -> Result<HexHash, HashError> {
    hash_json(DOMAIN_VALUE, value)
}

/// Hash raw bytes (e.g. evidence payload) without structured canonicalization.
///
/// `result = SHA256(bytes)` — no domain prefix for raw file hashes.
#[must_use]
pub fn hash_bytes(bytes: &[u8]) -> HexHash {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    HexHash(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_hash_valid() {
        let hex = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        let h = HexHash::from_hex(hex).unwrap();
        assert_eq!(h.as_hex(), hex);
    }

    #[test]
    fn hex_hash_rejects_uppercase() {
        let hex = "E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855";
        assert_eq!(HexHash::from_hex(hex), Err(HashError::InvalidHex));
    }

    #[test]
    fn hex_hash_rejects_wrong_length() {
        assert!(matches!(
            HexHash::from_hex("abc"),
            Err(HashError::InvalidLength(3))
        ));
    }

    #[test]
    fn hash_bytes_empty_sha256() {
        let h = hash_bytes(b"");
        assert_eq!(
            h.as_hex(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn hash_raw_domain_separation() {
        let h1 = hash_raw("domain-a\n", b"data");
        let h2 = hash_raw("domain-b\n", b"data");
        assert_ne!(h1, h2, "different domains must produce different hashes");
    }

    #[test]
    fn hash_json_deterministic() {
        let val = serde_json::json!({"b": 2, "a": 1});
        let h1 = hash_json("test\n", &val).unwrap();
        let h2 = hash_json("test\n", &val).unwrap();
        assert_eq!(h1, h2, "same input must produce same hash");
    }

    #[test]
    fn hash_json_key_order_invariant() {
        let v1 = serde_json::json!({"b": 2, "a": 1});
        let v2 = serde_json::json!({"a": 1, "b": 2});
        let h1 = hash_json("test\n", &v1).unwrap();
        let h2 = hash_json("test\n", &v2).unwrap();
        assert_eq!(h1, h2, "JCS must normalize key order");
    }

    #[test]
    fn revision_id_format() {
        let ch = hash_bytes(b"test");
        let rid = revision_id(&ch);
        assert!(rid.starts_with("r-"));
        assert_eq!(rid.len(), 2 + 64);
    }

    #[test]
    fn content_hash_excludes_self() {
        let body = serde_json::json!({
            "schema_version": "1.0.0",
            "kind": "project",
            "id": "main",
            "brand_id": "acme",
            "data": {"name": "test"}
        });
        let h = content_hash(&body).unwrap();
        assert_eq!(h.as_hex().len(), 64);
    }
}
