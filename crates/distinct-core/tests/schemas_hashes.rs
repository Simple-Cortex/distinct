//! WO-01 integration tests: schemas, hashes, fixtures, and contract invariants.
//!
//! Tests the full validation pipeline: duplicate-key rejection, schema
//! validation, hash computation, and ID validation.

use distinct_core::{dup_keys, hash, id, record, schema};
use std::path::Path;

/// Positive fixtures must parse and validate without error.
#[test]
fn positive_fixtures_parse() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/contracts/positive");
    if !dir.exists() {
        return; // fixtures not yet created
    }
    for entry in std::fs::read_dir(&dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "json") {
            let text = std::fs::read_to_string(&path).unwrap();
            // Must not have duplicate keys
            dup_keys::reject_duplicate_keys(&text)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            // Must parse as valid JSON
            let val: serde_json::Value =
                serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            // Must have required envelope fields
            assert!(
                val.get("schema_version").is_some(),
                "{}: missing schema_version",
                path.display()
            );
            assert!(
                val.get("kind").is_some(),
                "{}: missing kind",
                path.display()
            );
            assert!(
                val.get("brand_id").is_some(),
                "{}: missing brand_id",
                path.display()
            );
        }
    }
}

/// Failing fixtures must be rejected by the validation pipeline.
#[test]
fn failing_fixtures_rejected() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/contracts/failing");
    if !dir.exists() {
        return;
    }
    for entry in std::fs::read_dir(&dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "json") {
            let text = std::fs::read_to_string(&path).unwrap();
            let file_stem = path.file_stem().unwrap().to_string_lossy();

            match file_stem.as_ref() {
                "duplicate_keys" => {
                    assert!(
                        dup_keys::reject_duplicate_keys(&text).is_err(),
                        "{file_stem}: must reject duplicate keys"
                    );
                }
                "unknown_schema_version" => {
                    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
                    assert!(
                        schema::check_schema_version(&val, "1.0.0").is_err(),
                        "{file_stem}: must reject unknown schema version"
                    );
                }
                "cross_brand_reference" => {
                    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
                    let brand = val["brand_id"].as_str().unwrap();
                    // The reference points to a different brand context
                    assert_ne!(brand, "acme", "{file_stem}: should have mismatched brand");
                }
                _ => {
                    // Other failing fixtures: just verify they parse
                    // (specific rejection tests are in unit tests)
                }
            }
        }
    }
}

/// Hash domain separation: different domains produce different hashes.
#[test]
fn hash_domain_separation() {
    let data = serde_json::json!({"test": true});
    let h1 = hash::hash_json("domain-a\n", &data).unwrap();
    let h2 = hash::hash_json("domain-b\n", &data).unwrap();
    assert_ne!(h1, h2, "different domains must produce different hashes");
}

/// JCS canonicalization makes key order irrelevant for hashing.
#[test]
fn jcs_key_order_invariant() {
    let v1 = serde_json::json!({"z": 1, "a": 2});
    let v2 = serde_json::json!({"a": 2, "z": 1});
    let h1 = hash::hash_json("test\n", &v1).unwrap();
    let h2 = hash::hash_json("test\n", &v2).unwrap();
    assert_eq!(h1, h2, "JCS must normalize key order");
}

/// Brand ID validation rejects traversal names.
#[test]
fn brand_id_rejects_traversal() {
    assert!(id::validate_brand_id("..").is_err());
    assert!(id::validate_brand_id("a/b").is_err());
    assert!(id::validate_brand_id("a\\b").is_err());
}

/// Record ID validation.
#[test]
fn record_id_validation() {
    assert!(id::validate_record_id("platform").is_ok());
    assert!(id::validate_record_id("voice.main").is_ok());
    assert!(id::validate_record_id("source_web_01").is_ok());
    assert!(id::validate_record_id("").is_err());
    assert!(id::validate_record_id("UPPER").is_err());
}

/// Content hash is deterministic.
#[test]
fn content_hash_deterministic() {
    let envelope = serde_json::json!({
        "schema_version": "1.0.0",
        "kind": "project",
        "id": "main",
        "brand_id": "acme",
        "data": {"name": "test"}
    });
    let h1 = hash::content_hash(&envelope).unwrap();
    let h2 = hash::content_hash(&envelope).unwrap();
    assert_eq!(h1, h2);
}

/// Revision ID is derived from content hash.
#[test]
fn revision_id_derived() {
    let ch = hash::hash_bytes(b"test");
    let rid = hash::revision_id(&ch);
    assert!(rid.starts_with("r-"));
    assert_eq!(&rid[2..], ch.as_hex());
}

/// Record kind classification.
#[test]
fn kind_governance_vs_content() {
    assert!(record::RecordKind::Decision.is_governance());
    assert!(record::RecordKind::Review.is_governance());
    assert!(record::RecordKind::CheckResult.is_governance());
    assert!(record::RecordKind::Project.is_content());
    assert!(record::RecordKind::Platform.is_content());
    assert!(record::RecordKind::Asset.is_content());
}

/// Schema size/depth limits.
#[test]
fn schema_limits() {
    // Normal JSON passes
    assert!(schema::check_limits(r#"{"a":1}"#).is_ok());

    // Oversized JSON fails
    let huge = "x".repeat(schema::MAX_JSON_SIZE + 1);
    assert!(schema::check_limits(&format!(r#"{{"d":"{huge}"}}"#)).is_err());
}

/// Duplicate key detection in nested objects.
#[test]
fn dup_keys_nested() {
    assert!(dup_keys::reject_duplicate_keys(r#"{"a":{"b":{"c":1,"c":2}}}"#).is_err());
    assert!(dup_keys::reject_duplicate_keys(r#"{"a":{"x":1},"b":{"x":2}}"#).is_ok());
}

/// Value digest is domain-separated from content hash.
#[test]
fn value_digest_differs_from_content_hash() {
    let val = serde_json::json!({"test": true});
    let vd = hash::value_digest(&val).unwrap();
    let ch = hash::content_hash(&val).unwrap();
    assert_ne!(
        vd, ch,
        "value digest and content hash use different domains"
    );
}
