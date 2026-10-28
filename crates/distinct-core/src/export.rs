//! Draft export kit builder (WO-07, design decision D-C).
//!
//! Exports brand records to a directory structure with a manifest,
//! excluding restricted/confidential records and quotes without consent.

use crate::hash;
use crate::id::validate_record_id;
use crate::record::{Privacy, RecordKind};
use serde_json::{Map, Value};
use std::path::Path;

/// Valid export kinds.
pub const EXPORT_KINDS: &[&str] = &["draft", "concept_only", "specification_only"];

/// Options for an export operation.
pub struct ExportOptions<'a> {
    pub brand_id: &'a str,
    pub kind: &'a str,
    pub out_dir: &'a Path,
    pub force: bool,
    pub objects: Vec<Value>,
    pub snapshot_hash: Option<&'a str>,
    /// WO-13: token export format (e.g. "dtcg"). None = no token export.
    pub tokens_format: Option<&'a str>,
}

/// Outcome of an export attempt.
#[derive(Debug)]
pub enum ExportResult {
    /// Export completed successfully.
    Ok {
        manifest: Value,
        records_written: usize,
        excluded: usize,
        /// WO-13: omitted token fields `{field, record_id, reason}` (empty when tokens not requested).
        tokens_omitted: Vec<Value>,
    },
    /// `production_ready` was requested — refused.
    ProductionReadyRefused,
    /// Unknown export kind.
    UnknownKind(String),
    /// Output directory exists and is non-empty, and `--force` was not set.
    OutputExists,
    /// I/O error during export.
    IoError(String),
}

/// Execute an export operation.
pub fn run_export(opts: &ExportOptions<'_>) -> ExportResult {
    // Refuse production_ready.
    if opts.kind == "production_ready" {
        return ExportResult::ProductionReadyRefused;
    }

    // Validate kind.
    if !EXPORT_KINDS.contains(&opts.kind) {
        return ExportResult::UnknownKind(opts.kind.to_string());
    }

    // Check output directory.
    if opts.out_dir.exists() && is_dir_non_empty(opts.out_dir) && !opts.force {
        return ExportResult::OutputExists;
    }

    // Create output directory.
    if let Err(e) = std::fs::create_dir_all(opts.out_dir) {
        return ExportResult::IoError(format!("create output dir: {e}"));
    }

    // Canonicalize output directory for path-traversal defense.
    let canon_out = match opts.out_dir.canonicalize() {
        Ok(p) => p,
        Err(e) => return ExportResult::IoError(format!("canonicalize output dir: {e}")),
    };

    // D-H: kinds excluded from specification_only (binary/proof payloads).
    let spec_excluded_kinds = ["asset", "logo_finding", "proof", "logo_intent"];
    // D-H: visual record kinds that get "concept": true in concept_only.
    let visual_kinds = ["logo_intent", "logo_finding", "direction", "asset", "voice"];

    // Filter records: exclude restricted, confidential, and quotes without consent.
    let mut included = Vec::new();
    let mut excluded_count = 0usize;

    for obj in &opts.objects {
        // Missing or non-string privacy is treated as restricted (fail-closed).
        let privacy = match obj["privacy"].as_str() {
            Some("public") => Privacy::Public,
            Some("internal") => Privacy::Internal,
            Some("confidential") => Privacy::Confidential,
            Some("restricted") => Privacy::Restricted,
            _ => {
                excluded_count += 1;
                continue;
            }
        };

        if privacy == Privacy::Restricted || privacy == Privacy::Confidential {
            excluded_count += 1;
            continue;
        }

        // Exclude quotes without consent (shared helper with packet builder).
        if crate::record::is_quote_without_consent(obj) {
            excluded_count += 1;
            continue;
        }

        // D-H: specification_only excludes binary/proof payloads.
        if opts.kind == "specification_only" {
            let kind_str = obj["kind"].as_str().unwrap_or("");
            if spec_excluded_kinds.contains(&kind_str) {
                excluded_count += 1;
                continue;
            }
        }

        let mut record = obj.clone();

        // D-H: concept_only marks visual records with "concept": true.
        if opts.kind == "concept_only" {
            let kind_str = obj["kind"].as_str().unwrap_or("");
            if visual_kinds.contains(&kind_str) {
                if let Some(data) = record.get_mut("data") {
                    if let Some(map) = data.as_object_mut() {
                        map.insert("concept".to_string(), serde_json::json!(true));
                    }
                }
            }
        }

        included.push(record);
    }

    // Write each record as <kind>/<id>.json.
    let mut manifest_records = Vec::new();
    let mut records_written = 0usize;

    for obj in &included {
        let id = match obj["id"].as_str() {
            Some(id) => id,
            None => {
                excluded_count += 1;
                continue;
            }
        };
        if validate_record_id(id).is_err() {
            excluded_count += 1;
            continue;
        }

        let kind = match obj["kind"].as_str() {
            Some(k) => k,
            None => {
                excluded_count += 1;
                continue;
            }
        };
        if serde_json::from_str::<RecordKind>(&format!("\"{kind}\"")).is_err() {
            excluded_count += 1;
            continue;
        }

        let content_hash = obj["content_hash"].as_str().unwrap_or("");

        let kind_dir = opts.out_dir.join(opts.kind).join(kind);
        if let Err(e) = std::fs::create_dir_all(&kind_dir) {
            return ExportResult::IoError(format!("create kind dir: {e}"));
        }

        let record_path = kind_dir.join(format!("{id}.json"));

        // Canonicalize and verify the record path stays within out_dir.
        let canon_record = match record_path.canonicalize() {
            Ok(p) => p,
            Err(_) => {
                // Path does not exist yet; canonicalize the parent directory instead.
                let parent = record_path.parent().unwrap_or(Path::new(""));
                match parent.canonicalize() {
                    Ok(canon_parent) => canon_parent.join(record_path.file_name().unwrap()),
                    Err(_) => {
                        excluded_count += 1;
                        continue;
                    }
                }
            }
        };
        if !canon_record.starts_with(&canon_out) {
            excluded_count += 1;
            continue;
        }

        let json = match serde_json::to_string_pretty(obj) {
            Ok(j) => j,
            Err(e) => return ExportResult::IoError(format!("serialize record: {e}")),
        };
        if let Err(e) = std::fs::write(&record_path, &json) {
            return ExportResult::IoError(format!("write record: {e}"));
        }

        manifest_records.push(serde_json::json!({
            "kind": kind,
            "id": id,
            "hash": content_hash,
            "path": format!("{}/{}/{}.json", opts.kind, kind, id),
        }));
        records_written += 1;
    }

    // Build manifest.
    let mut manifest = Map::new();
    manifest.insert(
        "schema_version".into(),
        Value::String(crate::record::SCHEMA_VERSION.to_string()),
    );
    manifest.insert("brand_id".into(), Value::String(opts.brand_id.to_string()));
    manifest.insert("kind".into(), Value::String(opts.kind.to_string()));
    manifest.insert(
        "snapshot_hash".into(),
        opts.snapshot_hash
            .map(|s| Value::String(s.to_string()))
            .unwrap_or(Value::Null),
    );
    manifest.insert("records".into(), Value::Array(manifest_records));
    manifest.insert(
        "coverage".into(),
        serde_json::json!({
            "included": records_written,
            "excluded": excluded_count,
        }),
    );
    manifest.insert("missing_proofs".into(), Value::Array(Vec::new()));
    manifest.insert("missing_reviews".into(), Value::Array(Vec::new()));
    manifest.insert(
        "rights".into(),
        serde_json::json!({
            "license": "UNLICENSED — internal draft",
            "redistribution": false,
        }),
    );

    // D-H: export-kind-specific manifest fields.
    match opts.kind {
        "concept_only" => {
            manifest.insert(
                "label".into(),
                Value::String("concept_only: not for production use".to_string()),
            );
        }
        "specification_only" => {
            let excluded_kinds = ["asset", "logo_finding", "proof", "logo_intent"];
            manifest.insert(
                "assets_excluded".into(),
                Value::Array(
                    excluded_kinds
                        .iter()
                        .map(|k| Value::String(k.to_string()))
                        .collect(),
                ),
            );
        }
        _ => {}
    }

    // WO-13: token export (DTCG) and GUIDELINES.md.
    let mut tokens_omitted: Vec<Value> = Vec::new();
    if let Some(fmt) = opts.tokens_format {
        if fmt != "dtcg" {
            return ExportResult::IoError(format!("unsupported tokens format: {fmt}"));
        }
        let token_export = crate::tokens::extract_tokens(&opts.objects);
        tokens_omitted = token_export.omitted_json();

        // Add tokens_omitted to manifest.
        manifest.insert(
            "tokens_omitted".into(),
            Value::Array(tokens_omitted.clone()),
        );

        // Write tokens.json.
        let tokens_json = serde_json::to_string_pretty(&token_export.tokens).unwrap_or_default();
        let tokens_path = opts.out_dir.join("tokens.json");
        if let Err(e) = std::fs::write(&tokens_path, &tokens_json) {
            return ExportResult::IoError(format!("write tokens.json: {e}"));
        }

        // Write GUIDELINES.md (deterministic).
        let guidelines = crate::tokens::generate_guidelines(
            opts.brand_id,
            opts.kind,
            &opts.objects,
            &token_export,
            &[], // missing_proofs populated by caller if needed
            &[], // missing_reviews populated by caller if needed
        );
        let guidelines_path = opts.out_dir.join("GUIDELINES.md");
        if let Err(e) = std::fs::write(&guidelines_path, &guidelines) {
            return ExportResult::IoError(format!("write GUIDELINES.md: {e}"));
        }
    }

    let manifest_value = Value::Object(manifest);
    let manifest_json = serde_json::to_string_pretty(&manifest_value).unwrap_or_default();
    let manifest_path = opts.out_dir.join("manifest.json");
    if let Err(e) = std::fs::write(&manifest_path, &manifest_json) {
        return ExportResult::IoError(format!("write manifest: {e}"));
    }

    ExportResult::Ok {
        manifest: manifest_value,
        records_written,
        excluded: excluded_count,
        tokens_omitted,
    }
}

// ── helpers ──────────────────────────────────────────────────────────

fn is_dir_non_empty(path: &Path) -> bool {
    match std::fs::read_dir(path) {
        Ok(mut entries) => entries.next().is_some(),
        Err(_) => false,
    }
}

/// Compute a manifest hash from the manifest JSON.
pub fn manifest_hash(manifest: &Value) -> Option<String> {
    hash::hash_json("distinct.export.manifest.v1\n", manifest)
        .ok()
        .map(|h| h.as_hex().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_ready_refused() {
        let dir = std::env::temp_dir().join("distinct-export-test-prod");
        let _ = std::fs::remove_dir_all(&dir);
        let opts = ExportOptions {
            brand_id: "acme",
            kind: "production_ready",
            out_dir: &dir,
            force: false,
            objects: vec![],
            snapshot_hash: None,
            tokens_format: None,
        };
        assert!(matches!(
            run_export(&opts),
            ExportResult::ProductionReadyRefused
        ));
    }

    #[test]
    fn unknown_kind_rejected() {
        let dir = std::env::temp_dir().join("distinct-export-test-unknown");
        let _ = std::fs::remove_dir_all(&dir);
        let opts = ExportOptions {
            brand_id: "acme",
            kind: "bogus",
            out_dir: &dir,
            force: false,
            objects: vec![],
            snapshot_hash: None,
            tokens_format: None,
        };
        assert!(matches!(run_export(&opts), ExportResult::UnknownKind(_)));
    }

    #[test]
    fn export_rejects_traversal_id() {
        let dir = std::env::temp_dir().join("distinct-export-test-traversal");
        let _ = std::fs::remove_dir_all(&dir);
        let objects = vec![
            serde_json::json!({
                "schema_version": "1.0.0",
                "kind": "project",
                "id": "main",
                "brand_id": "acme",
                "privacy": "public",
                "content_hash": "abc123",
                "data": {"name": "Acme"}
            }),
            serde_json::json!({
                "schema_version": "1.0.0",
                "kind": "platform",
                "id": "../../evil",
                "brand_id": "acme",
                "privacy": "public",
                "content_hash": "def456",
                "data": {"name": "Evil"}
            }),
        ];
        let opts = ExportOptions {
            brand_id: "acme",
            kind: "draft",
            out_dir: &dir,
            force: true,
            objects,
            snapshot_hash: None,
            tokens_format: None,
        };
        match run_export(&opts) {
            ExportResult::Ok {
                records_written,
                excluded,
                ..
            } => {
                assert_eq!(
                    records_written, 1,
                    "only the valid record should be written"
                );
                assert_eq!(excluded, 1, "traversal-id record must be excluded");
            }
            other => panic!("expected Ok, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
