//! WO-00 dependency spike: functional verification that each pinned crate
//! actually works on Rust 1.85.0 offline. These are NOT feature tests — they
//! prove the dependency can perform the specific capability Distinct needs.

use sha2::{Digest, Sha256};

/// jcs-canonicalize produces RFC 8785 canonical JSON (keys sorted, &str input).
#[test]
fn jcs_canonicalizes_known_object() {
    let input = r#"{"b":2,"a":1}"#;
    let canonical = jcs_canonicalize::canonicalize(input).expect("valid JSON");
    assert_eq!(canonical, r#"{"a":1,"b":2}"#);
}

/// jcs-canonicalize silently ACCEPTS duplicate keys (keeps last value).
/// Distinct MUST reject duplicates separately (jcs-admit or custom pre-parser).
/// This test records the finding — it is a negative capability check.
#[test]
fn jcs_does_not_reject_duplicate_keys() {
    let input = r#"{"a":1,"a":2}"#;
    let result = jcs_canonicalize::canonicalize(input);
    // jcs-canonicalize does NOT reject duplicates; it accepts them.
    // This is expected behavior from this crate — Distinct must add its own
    // duplicate-key rejection layer (recorded for WO-01).
    assert!(
        result.is_ok(),
        "jcs-canonicalize accepts duplicates (known limitation)"
    );
}

/// SHA-256 produces correct 64-hex digest for empty input.
#[test]
fn sha256_known_vector() {
    let mut hasher = Sha256::new();
    hasher.update(b"");
    let hex = hex::encode(hasher.finalize());
    assert_eq!(
        hex,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

/// jsonschema validates against draft 2020-12 with local-only references.
#[test]
fn jsonschema_validates_draft202012() {
    let schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "age": { "type": "integer", "minimum": 0 }
        },
        "required": ["name"],
        "additionalProperties": false
    });
    let compiled = jsonschema::validator_for(&schema).expect("valid schema");

    let valid = serde_json::json!({"name": "test", "age": 5});
    assert!(compiled.is_valid(&valid));

    let invalid = serde_json::json!({"age": -1});
    assert!(!compiled.is_valid(&invalid));

    let extra_field = serde_json::json!({"name": "test", "extra": true});
    assert!(!compiled.is_valid(&extra_field));
}

/// quick-xml parses SVG XML safely.
#[test]
fn quick_xml_parses_svg_element() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><rect width="50" height="50"/></svg>"#;
    let mut reader = quick_xml::Reader::from_str(svg);
    reader.config_mut().trim_text(true);
    let mut found_svg = false;
    let mut found_rect = false;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(quick_xml::events::Event::Start(ref e))
            | Ok(quick_xml::events::Event::Empty(ref e)) => match e.name().as_ref() {
                b"svg" => found_svg = true,
                b"rect" => found_rect = true,
                _ => {}
            },
            Ok(quick_xml::events::Event::Eof) => break,
            Err(e) => panic!("XML parse error: {e}"),
            _ => {}
        }
        buf.clear();
    }
    assert!(found_svg, "must find <svg>");
    assert!(found_rect, "must find <rect>");
}

/// fs4 provides process-safe file locking via the FileExt trait.
/// fs4 1.1.0: exclusive lock is `lock()`, shared is `lock_shared()`.
#[test]
fn fs4_exclusive_lock_works() {
    use fs4::FileExt;
    use std::io::{Read, Seek, SeekFrom, Write};
    let dir = std::env::temp_dir().join("distinct-dep-spike-fs4");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("lock-test");
    let mut f = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&path)
        .expect("open lock file");
    FileExt::lock(&f).expect("acquire exclusive lock");
    f.write_all(b"locked").expect("write while locked");
    FileExt::unlock(&f).expect("release lock");
    f.seek(SeekFrom::Start(0)).expect("seek to start");
    let mut contents = String::new();
    f.read_to_string(&mut contents).expect("read back");
    assert_eq!(contents, "locked");
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_dir(&dir);
}

/// serde_json with preserve_order keeps insertion order (for JCS re-sort).
#[test]
fn serde_json_preserve_order_roundtrip() {
    let input = r#"{"z":1,"a":2,"m":3}"#;
    let val: serde_json::Value = serde_json::from_str(input).unwrap();
    let output = serde_json::to_string(&val).unwrap();
    assert_eq!(output, input, "preserve_order must keep insertion order");
}
