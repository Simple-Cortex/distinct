//! Ingest commands and evidence forget (WO-10, WO-10b, D-I).
//!
//! - `ingest plan` — emit/freeze bounded host-acquisition plan, no crawling
//! - `ingest import` — validate adapter results with provenance
//! - `ingest diff` — report changed/new/unchanged/unavailable/removed sources
//! - `ingest extract` — turn host-saved files into evidence + extracted text
//! - `evidence forget` — tombstone, delete payload + derivatives, mark dependents stale

use distinct_core::store::BrandWorkspace;
use distinct_core::Exit;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::dispatch::{self, write_envelope, write_error, CliArgs};

/// Dispatch ingest subcommands.
pub fn cmd_ingest<O: Write, E: Write>(args: &CliArgs, out: &mut O, err: &mut E) -> Exit {
    let sub = args.rest.first().map(|s| s.as_str());
    match sub {
        Some("plan") => cmd_ingest_plan(args, out, err),
        Some("import") => cmd_ingest_import(args, out, err),
        Some("diff") => cmd_ingest_diff(args, out, err),
        Some("extract") => cmd_ingest_extract(args, out, err),
        Some(other) => write_error(
            out,
            "ingest",
            &format!("unknown ingest subcommand: {other}"),
            Exit::Invalid,
        ),
        None => write_error(
            out,
            "ingest",
            "usage: ingest plan|import|diff|extract [options]",
            Exit::Invalid,
        ),
    }
}

/// Helper: update STATE after a mutating command (snapshot + governance index + write).
fn update_state<O: Write>(ws: &BrandWorkspace, out: &mut O, command: &str) -> Result<(), Exit> {
    let mut state = ws.read_state().map_err(|e| {
        let _ = write_error(out, command, &e.to_string(), Exit::Invalid);
        Exit::Invalid
    })?;
    ws.update_snapshot(&mut state).map_err(|e| {
        let _ = write_error(
            out,
            command,
            &format!("update snapshot: {e}"),
            Exit::Invalid,
        );
        Exit::Invalid
    })?;
    ws.update_governance_index(&mut state).map_err(|e| {
        let _ = write_error(
            out,
            command,
            &format!("update governance index: {e}"),
            Exit::Invalid,
        );
        Exit::Invalid
    })?;
    ws.write_state(&state).map_err(|e| {
        let _ = write_error(out, command, &format!("write state: {e}"), Exit::Invalid);
        Exit::Invalid
    })?;
    Ok(())
}

/// Valid privacy classes.
const VALID_PRIVACY: &[&str] = &["public", "internal", "confidential", "restricted"];

/// Valid coverage values for ingest import.
const VALID_COVERAGE: &[&str] = &["planned", "fetched", "extracted", "failed", "unavailable"];

/// `ingest plan --sources <file>` — emit a bounded host-acquisition plan.
fn cmd_ingest_plan<O: Write, E: Write>(args: &CliArgs, out: &mut O, _err: &mut E) -> Exit {
    let brand = match dispatch::require_brand(args) {
        Ok(b) => b,
        Err(msg) => return write_error(out, "ingest-plan", &msg, Exit::Invalid),
    };

    let file_path = args
        .rest
        .iter()
        .position(|a| a == "--sources")
        .and_then(|i| args.rest.get(i + 1))
        .map(|s| s.as_str());
    let file_path = match file_path {
        Some(p) => p,
        None => {
            return write_error(
                out,
                "ingest-plan",
                "--sources <file> required",
                Exit::Invalid,
            )
        }
    };

    let json_text = match std::fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(e) => {
            return write_error(
                out,
                "ingest-plan",
                &format!("cannot read {file_path}: {e}"),
                Exit::Invalid,
            )
        }
    };

    if let Err(e) = distinct_core::dup_keys::reject_duplicate_keys(&json_text) {
        return write_error(out, "ingest-plan", &e.to_string(), Exit::Invalid);
    }

    let sources: serde_json::Value = match serde_json::from_str(&json_text) {
        Ok(v) => v,
        Err(e) => {
            return write_error(
                out,
                "ingest-plan",
                &format!("invalid JSON: {e}"),
                Exit::Invalid,
            )
        }
    };

    let sources_list = sources.as_array().unwrap_or(&vec![]).clone();

    let ws = BrandWorkspace::new(std::path::Path::new(&args.workspace), brand);
    if !ws.is_initialized() {
        return write_error(out, "ingest-plan", "brand not initialized", Exit::Invalid);
    }

    // Build a frozen plan record.
    let plan_id = format!(
        "ingest-plan-{}",
        &distinct_core::hash::hash_raw("ingest_plan", json_text.as_bytes()).as_hex()[..12]
    );
    let record = serde_json::json!({
        "schema_version": distinct_core::record::SCHEMA_VERSION,
        "kind": "source",
        "id": plan_id,
        "brand_id": brand,
        "created_at": distinct_core::store::chrono_now(),
        "created_by": { "kind": "agent", "id": "distinct-rs" },
        "privacy": "internal",
        "dependencies": [],
        "limitations": ["no_network_crawling"],
        "data": {
            "type": "ingest_plan",
            "sources": sources_list,
            "source_count": sources_list.len(),
        }
    });

    match ws.store_object(&record) {
        Ok(hash) => {
            // WO-10b: update STATE so verify stays clean.
            if update_state(&ws, out, "ingest-plan").is_err() {
                return Exit::Invalid;
            }

            let result = serde_json::json!({
                "brand_id": brand,
                "plan_id": plan_id,
                "content_hash": hash.as_hex(),
                "source_count": sources_list.len(),
                "limitations": ["no_network_crawling"],
            });
            write_envelope(
                out,
                "ingest-plan",
                result,
                vec!["no_network_crawling".to_string()],
                vec![],
            )
        }
        Err(e) => write_error(out, "ingest-plan", &e.to_string(), Exit::Invalid),
    }
}

/// Validate that a payload path is safe: relative, no traversal, no symlink escape.
fn validate_payload_path(payload_rel: &str, source_root: &Path) -> Result<PathBuf, String> {
    // Reject absolute paths.
    let payload_path = Path::new(payload_rel);
    if payload_path.is_absolute() {
        return Err(format!("payload path must be relative: {payload_rel}"));
    }

    // Reject path traversal components.
    for component in payload_path.components() {
        match component {
            std::path::Component::ParentDir => {
                return Err(format!("payload path contains '..': {payload_rel}"));
            }
            std::path::Component::Prefix(_) => {
                return Err(format!("payload path has prefix component: {payload_rel}"));
            }
            _ => {}
        }
    }

    let full_path = source_root.join(payload_rel);

    // Canonicalize source_root (must exist).
    let canon_root = source_root
        .canonicalize()
        .map_err(|e| format!("cannot canonicalize source-root {source_root:?}: {e}"))?;

    // Canonicalize the full path (file must exist).
    let canon_full = full_path
        .canonicalize()
        .map_err(|e| format!("cannot canonicalize payload {full_path:?}: {e}"))?;

    // Check the canonical path is under the canonical root.
    if !canon_full.starts_with(&canon_root) {
        return Err(format!("payload path escapes source-root: {payload_rel}"));
    }

    // Check it's a file, not a symlink to outside.
    if !canon_full.is_file() {
        return Err(format!("payload is not a regular file: {payload_rel}"));
    }

    // Return the checked canonical path so the later read cannot follow a swapped symlink.
    Ok(canon_full)
}

/// `ingest import --manifest <file> --source-root <dir>` — validate adapter results.
fn cmd_ingest_import<O: Write, E: Write>(args: &CliArgs, out: &mut O, _err: &mut E) -> Exit {
    let brand = match dispatch::require_brand(args) {
        Ok(b) => b,
        Err(msg) => return write_error(out, "ingest-import", &msg, Exit::Invalid),
    };

    let manifest_path = args
        .rest
        .iter()
        .position(|a| a == "--manifest")
        .and_then(|i| args.rest.get(i + 1))
        .map(|s| s.as_str());
    let manifest_path = match manifest_path {
        Some(p) => p,
        None => {
            return write_error(
                out,
                "ingest-import",
                "--manifest <file> required",
                Exit::Invalid,
            )
        }
    };

    let source_root = args
        .rest
        .iter()
        .position(|a| a == "--source-root")
        .and_then(|i| args.rest.get(i + 1))
        .map(|s| s.as_str());
    let source_root = match source_root {
        Some(p) => p,
        None => {
            return write_error(
                out,
                "ingest-import",
                "--source-root <dir> required",
                Exit::Invalid,
            )
        }
    };
    let source_root_path = Path::new(source_root);

    let json_text = match std::fs::read_to_string(manifest_path) {
        Ok(c) => c,
        Err(e) => {
            return write_error(
                out,
                "ingest-import",
                &format!("cannot read {manifest_path}: {e}"),
                Exit::Invalid,
            )
        }
    };

    if let Err(e) = distinct_core::dup_keys::reject_duplicate_keys(&json_text) {
        return write_error(out, "ingest-import", &e.to_string(), Exit::Invalid);
    }

    let manifest: serde_json::Value = match serde_json::from_str(&json_text) {
        Ok(v) => v,
        Err(e) => {
            return write_error(
                out,
                "ingest-import",
                &format!("invalid JSON: {e}"),
                Exit::Invalid,
            )
        }
    };

    let ws = BrandWorkspace::new(std::path::Path::new(&args.workspace), brand);
    if !ws.is_initialized() {
        return write_error(out, "ingest-import", "brand not initialized", Exit::Invalid);
    }

    let items = manifest["items"].as_array().cloned().unwrap_or_default();
    let mut imported = 0u64;
    let mut failures = Vec::new();

    for item in &items {
        let source_id = match item["source_id"].as_str() {
            Some(s) if distinct_core::id::validate_record_id(s).is_ok() => s.to_string(),
            _ => {
                failures.push(serde_json::json!({
                    "source_id": "unknown",
                    "error": "missing or invalid source_id"
                }));
                continue;
            }
        };

        // Validate privacy (required, no default).
        let privacy = match item["privacy"].as_str() {
            Some(p) if VALID_PRIVACY.contains(&p) => p.to_string(),
            Some(p) => {
                failures.push(serde_json::json!({
                    "source_id": source_id,
                    "error": format!("invalid privacy class: {p}")
                }));
                continue;
            }
            None => {
                failures.push(serde_json::json!({
                    "source_id": source_id,
                    "error": "privacy is required"
                }));
                continue;
            }
        };

        // Validate consent (required).
        let consent = match item["consent"].as_str() {
            Some(c) => c.to_string(),
            None => {
                failures.push(serde_json::json!({
                    "source_id": source_id,
                    "error": "consent is required"
                }));
                continue;
            }
        };

        // Validate retrieved_at (required).
        let retrieved_at = match item["retrieved_at"].as_str() {
            Some(r) => r.to_string(),
            None => {
                failures.push(serde_json::json!({
                    "source_id": source_id,
                    "error": "retrieved_at is required"
                }));
                continue;
            }
        };

        // Validate original_ref (required).
        let original_ref = match item["original_ref"].as_str() {
            Some(r) => r.to_string(),
            None => {
                failures.push(serde_json::json!({
                    "source_id": source_id,
                    "error": "original_ref is required"
                }));
                continue;
            }
        };

        // Validate coverage.
        let coverage = match item["coverage"].as_str() {
            Some(c) if VALID_COVERAGE.contains(&c) => c.to_string(),
            Some(c) => {
                failures.push(serde_json::json!({
                    "source_id": source_id,
                    "error": format!("invalid coverage: {c}")
                }));
                continue;
            }
            None => {
                failures.push(serde_json::json!({
                    "source_id": source_id,
                    "error": "coverage is required"
                }));
                continue;
            }
        };

        // Validate removed flag (optional, defaults to false).
        let removed = item["removed"].as_bool().unwrap_or(false);

        // For items with a payload, validate and copy it.
        let mut byte_hash = None;
        let mut byte_count = 0u64;
        if let Some(payload_rel) = item["payload"].as_str() {
            match validate_payload_path(payload_rel, source_root_path) {
                Ok(full_path) => match std::fs::read(&full_path) {
                    Ok(bytes) => {
                        let hash = distinct_core::hash::hash_bytes(&bytes);
                        if let Err(e) = ws.store_evidence(&bytes, &source_id) {
                            failures.push(serde_json::json!({
                                "source_id": source_id,
                                "error": format!("store evidence: {e}")
                            }));
                            continue;
                        }
                        byte_hash = Some(hash.as_hex().to_string());
                        byte_count = bytes.len() as u64;
                    }
                    Err(e) => {
                        failures.push(serde_json::json!({
                            "source_id": source_id,
                            "error": format!("cannot read payload: {e}")
                        }));
                        continue;
                    }
                },
                Err(e) => {
                    failures.push(serde_json::json!({
                        "source_id": source_id,
                        "error": e
                    }));
                    continue;
                }
            }
        }

        // Build the source record with provenance.
        let record_id = format!("ingest-{source_id}");
        let mut data = serde_json::json!({
            "type": "ingest_result",
            "source_id": source_id,
            "coverage": coverage,
            "original_ref": original_ref,
            "retrieved_at": retrieved_at,
            "consent": consent,
            "removed": removed,
        });

        if let Some(ref bh) = byte_hash {
            data["byte_hash"] = serde_json::json!(bh);
            data["byte_count"] = serde_json::json!(byte_count);
            // Derivative provenance: the evidence bytes are a derivative of the original.
            data["derivative_of"] = serde_json::json!({
                "original_ref": original_ref,
                "derivative_hash": bh,
                "derivative_kind": "evidence_bytes",
            });
        }

        let record = serde_json::json!({
            "schema_version": distinct_core::record::SCHEMA_VERSION,
            "kind": "source",
            "id": record_id,
            "brand_id": brand,
            "created_at": distinct_core::store::chrono_now(),
            "created_by": { "kind": "agent", "id": "distinct-rs" },
            "privacy": privacy,
            "dependencies": [],
            "limitations": [],
            "data": data,
        });

        match ws.store_object(&record) {
            Ok(_) => imported += 1,
            Err(e) => {
                failures.push(serde_json::json!({
                    "source_id": source_id,
                    "error": format!("store object: {e}")
                }));
            }
        }
    }

    // WO-10b: update STATE so verify stays clean.
    if imported > 0 && update_state(&ws, out, "ingest-import").is_err() {
        return Exit::Invalid;
    }

    let result = serde_json::json!({
        "brand_id": brand,
        "imported": imported,
        "failed": failures.len(),
        "total": items.len(),
        "failures": failures,
    });
    write_envelope(out, "ingest-import", result, vec![], vec![])
}

/// `ingest diff --against <snapshot>` — report source changes.
fn cmd_ingest_diff<O: Write, E: Write>(args: &CliArgs, out: &mut O, _err: &mut E) -> Exit {
    let brand = match dispatch::require_brand(args) {
        Ok(b) => b,
        Err(msg) => return write_error(out, "ingest-diff", &msg, Exit::Invalid),
    };

    let snapshot_id = args
        .rest
        .iter()
        .position(|a| a == "--against")
        .and_then(|i| args.rest.get(i + 1))
        .map(|s| s.as_str());
    let snapshot_id = match snapshot_id {
        Some(s) => s,
        None => {
            return write_error(
                out,
                "ingest-diff",
                "--against <snapshot> required",
                Exit::Invalid,
            )
        }
    };

    let ws = BrandWorkspace::new(std::path::Path::new(&args.workspace), brand);
    if !ws.is_initialized() {
        return write_error(out, "ingest-diff", "brand not initialized", Exit::Invalid);
    }

    // Look for the snapshot file.
    let snap_path = ws.snapshots_dir().join(format!("{snapshot_id}.json"));
    let old_records = if snap_path.exists() {
        match std::fs::read_to_string(&snap_path) {
            Ok(text) => serde_json::from_str::<serde_json::Value>(&text)
                .ok()
                .and_then(|v| v["records"].as_object().cloned())
                .unwrap_or_default(),
            Err(_) => serde_json::Map::new(),
        }
    } else {
        serde_json::Map::new()
    };

    // Current objects.
    let current = ws.list_objects(None).unwrap_or_default();
    let mut current_ids = std::collections::BTreeMap::new();
    // Track source records for removed/unavailable detection.
    let mut source_records = Vec::new();
    for obj in &current {
        let kind = obj["kind"].as_str().unwrap_or("unknown").to_string();
        let id = obj["id"].as_str().unwrap_or("unknown").to_string();
        let hash = obj["content_hash"].as_str().unwrap_or("").to_string();
        current_ids
            .entry(kind.clone())
            .or_insert_with(std::collections::BTreeMap::new)
            .insert(id.clone(), hash);
        if kind == "source" {
            source_records.push(obj.clone());
        }
    }

    let mut changed = Vec::new();
    let mut new_entries = Vec::new();
    let mut unchanged = 0u64;

    for (kind, ids) in &current_ids {
        for (id, hash) in ids {
            let old_hash = old_records
                .get(kind)
                .and_then(|m| m.as_object())
                .and_then(|m| m.get(id))
                .and_then(|v| v.as_str());
            match old_hash {
                Some(oh) if oh == hash => unchanged += 1,
                Some(_) => changed.push(serde_json::json!({"kind": kind, "id": id})),
                None => new_entries.push(serde_json::json!({"kind": kind, "id": id})),
            }
        }
    }

    // WO-10b: report explicitly_removed sources (removed: true) and
    // unavailable sources (coverage unavailable or failed).
    // Never infer deletion from absence.
    let mut explicitly_removed = Vec::new();
    let mut unavailable = Vec::new();
    for obj in &source_records {
        let id = obj["id"].as_str().unwrap_or("unknown");
        let data = &obj["data"];
        let coverage = data["coverage"].as_str().unwrap_or("");
        let removed = data["removed"].as_bool().unwrap_or(false);

        if removed {
            explicitly_removed.push(serde_json::json!({
                "source_id": data["source_id"],
                "id": id,
            }));
        }
        if coverage == "unavailable" || coverage == "failed" {
            unavailable.push(serde_json::json!({
                "source_id": data["source_id"],
                "id": id,
                "coverage": coverage,
            }));
        }
    }

    let result = serde_json::json!({
        "brand_id": brand,
        "against": snapshot_id,
        "changed": changed,
        "new": new_entries,
        "unchanged": unchanged,
        "unavailable": unavailable,
        "explicitly_removed": explicitly_removed,
    });
    write_envelope(out, "ingest-diff", result, vec![], vec![])
}

/// `ingest extract --brand B --file <path> --privacy <class>
///   [--consent quote_ok|no_quotes] [--source-url U]`
fn cmd_ingest_extract<O: Write, E: Write>(args: &CliArgs, out: &mut O, _err: &mut E) -> Exit {
    use distinct_core::extract;

    let brand = match dispatch::require_brand(args) {
        Ok(b) => b,
        Err(msg) => return write_error(out, "ingest-extract", &msg, Exit::Invalid),
    };

    let file_path = args
        .rest
        .iter()
        .position(|a| a == "--file")
        .and_then(|i| args.rest.get(i + 1))
        .map(|s| s.as_str());
    let file_path = match file_path {
        Some(p) => p,
        None => {
            return write_error(
                out,
                "ingest-extract",
                "--file <path> required",
                Exit::Invalid,
            )
        }
    };

    let privacy = args
        .rest
        .iter()
        .position(|a| a == "--privacy")
        .and_then(|i| args.rest.get(i + 1))
        .map(|s| s.as_str());
    let privacy = match privacy {
        Some(p) if VALID_PRIVACY.contains(&p) => p,
        Some(p) => {
            return write_error(
                out,
                "ingest-extract",
                &format!("invalid privacy class: {p}"),
                Exit::Invalid,
            )
        }
        None => {
            return write_error(
                out,
                "ingest-extract",
                "--privacy <class> required",
                Exit::Invalid,
            )
        }
    };

    let consent = args
        .rest
        .iter()
        .position(|a| a == "--consent")
        .and_then(|i| args.rest.get(i + 1))
        .map(|s| s.as_str());

    let source_url = args
        .rest
        .iter()
        .position(|a| a == "--source-url")
        .and_then(|i| args.rest.get(i + 1))
        .map(|s| s.as_str());

    let ws = BrandWorkspace::new(std::path::Path::new(&args.workspace), brand);
    if !ws.is_initialized() {
        return write_error(
            out,
            "ingest-extract",
            "brand not initialized",
            Exit::Invalid,
        );
    }

    // Read the file.
    let data = match std::fs::read(file_path) {
        Ok(d) => d,
        Err(e) => {
            return write_error(
                out,
                "ingest-extract",
                &format!("cannot read {file_path}: {e}"),
                Exit::Invalid,
            )
        }
    };

    let filename = std::path::Path::new(file_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();

    // Extract text.
    match extract::extract(&filename, &data) {
        Ok(result) => {
            let byte_hash = distinct_core::hash::hash_bytes(&data);
            let text_hash = distinct_core::hash::hash_raw("extracted_text", result.text.as_bytes());

            // Store evidence bytes.
            let source_id = format!("extract-{}", &byte_hash.as_hex()[..16]);
            if let Err(e) = ws.store_evidence(&data, &source_id) {
                return write_error(out, "ingest-extract", &e.to_string(), Exit::Invalid);
            }

            // Store extracted text as a source record.
            let record = serde_json::json!({
                "schema_version": distinct_core::record::SCHEMA_VERSION,
                "kind": "source",
                "id": source_id,
                "brand_id": brand,
                "created_at": distinct_core::store::chrono_now(),
                "created_by": { "kind": "agent", "id": "distinct-rs" },
                "privacy": privacy,
                "dependencies": [],
                "source_refs": [{
                    "evidence_id": source_id,
                    "evidence_hash": byte_hash.as_hex(),
                }],
                "limitations": if result.truncated {
                    vec!["text_truncated_to_1mib".to_string()]
                } else {
                    vec![]
                },
                "data": {
                    "type": "extracted_text",
                    "filename": filename,
                    "format": result.format,
                    "byte_hash": byte_hash.as_hex(),
                    "byte_count": data.len(),
                    "text_hash": text_hash.as_hex(),
                    "text_byte_count": result.text.len(),
                    "truncated": result.truncated,
                    "consent": consent == Some("quote_ok"),
                    "source_url": source_url,
                }
            });

            match ws.store_object(&record) {
                Ok(hash) => {
                    // WO-10b: update STATE so verify stays clean.
                    if update_state(&ws, out, "ingest-extract").is_err() {
                        return Exit::Invalid;
                    }

                    let mut limitations = Vec::new();
                    if result.truncated {
                        limitations.push("text_truncated_to_1mib".to_string());
                    }
                    let result_json = serde_json::json!({
                        "brand_id": brand,
                        "source_id": source_id,
                        "content_hash": hash.as_hex(),
                        "byte_hash": byte_hash.as_hex(),
                        "format": result.format,
                        "byte_count": data.len(),
                        "text_byte_count": result.text.len(),
                        "truncated": result.truncated,
                    });
                    write_envelope(out, "ingest-extract", result_json, limitations, vec![])
                }
                Err(e) => write_error(out, "ingest-extract", &e.to_string(), Exit::Invalid),
            }
        }
        Err(extract::ExtractError::PdfHostGuided) => {
            let result = serde_json::json!({
                "brand_id": brand,
                "filename": filename,
                "error": "pdf extraction is host-guided",
            });
            let mut env = serde_json::Map::new();
            env.insert(
                "protocol_version".into(),
                serde_json::json!(distinct_core::PROTOCOL_VERSION),
            );
            env.insert(
                "engine_version".into(),
                serde_json::json!(distinct_core::ENGINE_VERSION),
            );
            env.insert(
                "execution".into(),
                serde_json::json!({"engine": "native-rust", "command": "ingest-extract"}),
            );
            env.insert("result".into(), result);
            env.insert("limitations".into(), serde_json::json!(["host_guided_pdf"]));
            env.insert("changed".into(), serde_json::json!([]));
            env.insert(
                "error".into(),
                serde_json::json!("pdf extraction is host-guided"),
            );
            let _ = writeln!(
                out,
                "{}",
                serde_json::to_string(&serde_json::Value::Object(env)).unwrap_or_default()
            );
            Exit::Unavailable
        }
        Err(e) => write_error(out, "ingest-extract", &e.to_string(), Exit::Invalid),
    }
}

/// `evidence forget --id <id> --reason <text> [--expected-state <hash>]`
pub fn cmd_evidence_forget<O: Write, E: Write>(args: &CliArgs, out: &mut O, _err: &mut E) -> Exit {
    let brand = match dispatch::require_brand(args) {
        Ok(b) => b,
        Err(msg) => return write_error(out, "evidence-forget", &msg, Exit::Invalid),
    };

    // Skip the "forget" subcommand token if present in rest.
    let rest: Vec<&String> = if args.rest.first().is_some_and(|s| s == "forget") {
        args.rest.iter().skip(1).collect()
    } else {
        args.rest.iter().collect()
    };

    let evidence_id = rest
        .iter()
        .position(|a| a.as_str() == "--id")
        .and_then(|i| rest.get(i + 1))
        .map(|s| s.as_str());
    let evidence_id = match evidence_id {
        Some(id) => id,
        None => return write_error(out, "evidence-forget", "--id <id> required", Exit::Invalid),
    };

    let reason = rest
        .iter()
        .position(|a| a.as_str() == "--reason")
        .and_then(|i| rest.get(i + 1))
        .map(|s| s.as_str());
    let reason = match reason {
        Some(r) => r,
        None => {
            return write_error(
                out,
                "evidence-forget",
                "--reason <text> required",
                Exit::Invalid,
            )
        }
    };

    let expected_state = rest
        .iter()
        .position(|a| a.as_str() == "--expected-state")
        .and_then(|i| rest.get(i + 1))
        .map(|s| s.as_str());

    let ws = BrandWorkspace::new(std::path::Path::new(&args.workspace), brand);
    if !ws.is_initialized() {
        return write_error(
            out,
            "evidence-forget",
            "brand not initialized",
            Exit::Invalid,
        );
    }

    // WO-10b: honour --expected-state if given.
    let state = match ws.read_state() {
        Ok(s) => s,
        Err(e) => return write_error(out, "evidence-forget", &e.to_string(), Exit::Invalid),
    };
    if let Some(expected) = expected_state {
        let current_hash = state.content_snapshot_hash.as_deref().unwrap_or("");
        if current_hash != expected {
            return write_error(
                out,
                "evidence-forget",
                &format!("state mismatch: expected {expected}, current {current_hash}"),
                Exit::Invalid,
            );
        }
    }

    // Find the evidence index entry.
    let index_path = ws.evidence_index_dir().join(format!("{evidence_id}.json"));
    let index_entry = if index_path.exists() {
        match std::fs::read_to_string(&index_path) {
            Ok(text) => serde_json::from_str::<serde_json::Value>(&text).ok(),
            Err(_) => None,
        }
    } else {
        None
    };

    let byte_hash = index_entry
        .as_ref()
        .and_then(|v| v["byte_hash"].as_str())
        .unwrap_or("")
        .to_string();

    // Delete the evidence payload.
    let mut deleted_payload = false;
    if !byte_hash.is_empty() {
        let payload_path = ws.evidence_objects_dir().join(&byte_hash);
        if payload_path.exists() && std::fs::remove_file(&payload_path).is_ok() {
            deleted_payload = true;
        }
    }

    // Delete the index entry.
    let mut deleted_index = false;
    if index_path.exists() && std::fs::remove_file(&index_path).is_ok() {
        deleted_index = true;
    }

    // Delete pure derivatives: extracted-text records that cite this evidence.
    let mut deleted_derivatives = Vec::new();
    let mut dependent_ids = Vec::new();
    let objects = ws.list_objects(None).unwrap_or_default();
    for obj in &objects {
        let data_hash = obj["data"]["byte_hash"].as_str().unwrap_or("");
        let obj_id = obj["id"].as_str().unwrap_or("");
        let obj_kind = obj["kind"].as_str().unwrap_or("");

        // Check if this is an extracted-text record for this evidence.
        if data_hash == byte_hash && !byte_hash.is_empty() {
            let obj_path = ws.objects_dir().join(format!(
                "{}.json",
                obj["content_hash"].as_str().unwrap_or("")
            ));
            if obj_path.exists() {
                let _ = std::fs::remove_file(&obj_path);
                deleted_derivatives.push(obj_id.to_string());
            }
        }

        // Check if this record references the evidence in dependencies or source_refs.
        let has_source_ref = obj["source_refs"].as_array().is_some_and(|refs| {
            refs.iter().any(|r| {
                r["evidence_id"].as_str() == Some(evidence_id)
                    || r["evidence_hash"].as_str() == Some(byte_hash.as_str())
            })
        });

        let has_dep = obj["dependencies"].as_array().is_some_and(|deps| {
            deps.iter().any(|d| {
                d["target"]["id"].as_str() == Some(evidence_id)
                    || d["target"]["kind"].as_str() == Some("evidence")
                        && d["target"]["id"].as_str() == Some(evidence_id)
            })
        });

        if (has_source_ref || has_dep) && !deleted_derivatives.contains(&obj_id.to_string()) {
            dependent_ids.push(format!("{obj_kind}/{obj_id}"));
        }
    }

    // Delete packets that cite the evidence id or hash.
    let mut deleted_packets = 0u64;
    let packets_dir = ws.packets_dir();
    if packets_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&packets_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "json") {
                    if let Ok(contents) = std::fs::read_to_string(&path) {
                        if contents.contains(evidence_id)
                            || (!byte_hash.is_empty() && contents.contains(&byte_hash))
                        {
                            let _ = std::fs::remove_file(&path);
                            deleted_packets += 1;
                        }
                    }
                }
            }
        }
    }

    // Write tombstone record (no plaintext of the evidence content).
    let tombstone_id = format!("forget-{evidence_id}");
    let tombstone = serde_json::json!({
        "schema_version": distinct_core::record::SCHEMA_VERSION,
        "kind": "source",
        "id": tombstone_id,
        "brand_id": brand,
        "created_at": distinct_core::store::chrono_now(),
        "created_by": { "kind": "agent", "id": "distinct-rs" },
        "privacy": "restricted",
        "dependencies": [],
        "limitations": ["external_copies_not_covered"],
        "data": {
            "type": "tombstone",
            "original_id": evidence_id,
            "byte_hash": byte_hash,
            "reason_hash": distinct_core::hash::hash_raw("forget_reason", reason.as_bytes()).as_hex(),
            "deleted_payload": deleted_payload,
            "deleted_index": deleted_index,
            "deleted_derivatives": deleted_derivatives.len(),
            "deleted_packets": deleted_packets,
            "dependents_marked_stale": dependent_ids.len(),
            "forgotten_evidence_refs": [evidence_id],
        }
    });

    match ws.store_object(&tombstone) {
        Ok(hash) => {
            // WO-10b: update STATE so verify stays clean.
            if update_state(&ws, out, "evidence-forget").is_err() {
                return Exit::Invalid;
            }

            let result = serde_json::json!({
                "brand_id": brand,
                "evidence_id": evidence_id,
                "tombstone_id": tombstone_id,
                "content_hash": hash.as_hex(),
                "deleted_payload": deleted_payload,
                "deleted_index": deleted_index,
                "deleted_derivatives": deleted_derivatives.len(),
                "deleted_packets": deleted_packets,
                "dependents_marked_stale": dependent_ids.len(),
                "limitations": ["external_copies_not_covered"],
            });
            write_envelope(
                out,
                "evidence-forget",
                result,
                vec!["external_copies_not_covered".to_string()],
                vec![],
            )
        }
        Err(e) => write_error(out, "evidence-forget", &e.to_string(), Exit::Invalid),
    }
}

#[cfg(test)]
mod tests {
    use crate::run;

    fn run_args(args: &[&str]) -> (i32, String, String) {
        let args: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run(&args, &mut out, &mut err);
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    fn setup_brand(dir: &std::path::Path) {
        let (code, _, _) = run_args(&[
            "init",
            "--brand",
            "acme",
            "--workspace",
            &dir.to_string_lossy(),
        ]);
        assert_eq!(code, 0);
    }

    #[test]
    fn ingest_plan_requires_sources() {
        let (code, out, _) = run_args(&[
            "ingest",
            "plan",
            "--brand",
            "acme",
            "--workspace",
            "/tmp/nonexistent",
        ]);
        assert_eq!(code, 3);
        let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
        assert!(val["error"].as_str().unwrap().contains("--sources"));
    }

    #[test]
    fn ingest_plan_updates_state() {
        let dir = std::env::temp_dir().join(format!(
            "distinct-test-ingest-plan-state-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        setup_brand(&dir);

        let sources_path = dir.join("sources.json");
        std::fs::write(&sources_path, r#"[{"url":"https://example.com"}]"#).unwrap();

        let (code, _out, _) = run_args(&[
            "ingest",
            "plan",
            "--brand",
            "acme",
            "--workspace",
            &dir.to_string_lossy(),
            "--sources",
            &sources_path.to_string_lossy(),
        ]);
        assert_eq!(code, 0, "ingest plan must succeed");

        // Now verify must exit 0.
        let (vcode, vout, _) = run_args(&[
            "verify",
            "--brand",
            "acme",
            "--workspace",
            &dir.to_string_lossy(),
        ]);
        assert_eq!(vcode, 0, "verify must pass after ingest plan");
        let vval: serde_json::Value = serde_json::from_str(vout.trim()).unwrap();
        assert!(
            vval["result"]["state_valid"].as_bool().unwrap(),
            "state must be valid after ingest plan"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ingest_import_requires_source_root() {
        let dir = std::env::temp_dir().join(format!(
            "distinct-test-ingest-import-no-sr-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        setup_brand(&dir);

        let manifest_path = dir.join("manifest.json");
        std::fs::write(&manifest_path, r#"{"items":[]}"#).unwrap();

        let (code, out, _) = run_args(&[
            "ingest",
            "import",
            "--brand",
            "acme",
            "--workspace",
            &dir.to_string_lossy(),
            "--manifest",
            &manifest_path.to_string_lossy(),
        ]);
        assert_eq!(code, 3, "must require --source-root");
        let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
        assert!(val["error"].as_str().unwrap().contains("--source-root"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ingest_import_validates_privacy() {
        let dir = std::env::temp_dir().join(format!(
            "distinct-test-ingest-import-priv-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        setup_brand(&dir);

        let source_root = dir.join("sources");
        std::fs::create_dir_all(&source_root).unwrap();

        let manifest_path = dir.join("manifest.json");
        std::fs::write(
            &manifest_path,
            r#"{
                "items": [{
                    "source_id": "src-1",
                    "coverage": "fetched",
                    "consent": "quote_ok",
                    "retrieved_at": "2026-10-27T00:00:00Z",
                    "original_ref": "https://example.com/page"
                }]
            }"#,
        )
        .unwrap();

        let (code, out, _) = run_args(&[
            "ingest",
            "import",
            "--brand",
            "acme",
            "--workspace",
            &dir.to_string_lossy(),
            "--manifest",
            &manifest_path.to_string_lossy(),
            "--source-root",
            &source_root.to_string_lossy(),
        ]);
        assert_eq!(code, 0);
        let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
        // Item should fail because privacy is missing.
        assert_eq!(val["result"]["imported"], 0);
        assert_eq!(val["result"]["failed"], 1);
        let failure = &val["result"]["failures"][0];
        assert!(failure["error"].as_str().unwrap().contains("privacy"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ingest_import_rejects_path_traversal() {
        let dir = std::env::temp_dir().join(format!(
            "distinct-test-ingest-import-trav-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        setup_brand(&dir);

        let source_root = dir.join("sources");
        std::fs::create_dir_all(&source_root).unwrap();

        let manifest_path = dir.join("manifest.json");
        std::fs::write(
            &manifest_path,
            r#"{
                "items": [{
                    "source_id": "src-1",
                    "privacy": "internal",
                    "coverage": "fetched",
                    "consent": "quote_ok",
                    "retrieved_at": "2026-10-27T00:00:00Z",
                    "original_ref": "https://example.com/page",
                    "payload": "../../etc/passwd"
                }]
            }"#,
        )
        .unwrap();

        let (code, out, _) = run_args(&[
            "ingest",
            "import",
            "--brand",
            "acme",
            "--workspace",
            &dir.to_string_lossy(),
            "--manifest",
            &manifest_path.to_string_lossy(),
            "--source-root",
            &source_root.to_string_lossy(),
        ]);
        assert_eq!(code, 0);
        let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
        assert_eq!(val["result"]["imported"], 0);
        assert!(val["result"]["failed"].as_u64().unwrap() >= 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ingest_import_rejects_traversal_source_id() {
        let dir = std::env::temp_dir().join(format!(
            "distinct-test-ingest-import-sid-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        setup_brand(&dir);

        let source_root = dir.join("sources");
        std::fs::create_dir_all(&source_root).unwrap();
        std::fs::write(source_root.join("page.txt"), "fictional page").unwrap();

        let manifest_path = dir.join("manifest.json");
        std::fs::write(
            &manifest_path,
            r#"{
                "items": [{
                    "source_id": "../../../escaped",
                    "privacy": "public",
                    "coverage": "fetched",
                    "consent": "quote_ok",
                    "retrieved_at": "2026-10-27T00:00:00Z",
                    "original_ref": "https://example.com/page",
                    "payload": "page.txt"
                }]
            }"#,
        )
        .unwrap();

        let (code, out, _) = run_args(&[
            "ingest",
            "import",
            "--brand",
            "acme",
            "--workspace",
            &dir.to_string_lossy(),
            "--manifest",
            &manifest_path.to_string_lossy(),
            "--source-root",
            &source_root.to_string_lossy(),
        ]);
        assert_eq!(code, 0);
        let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
        assert_eq!(val["result"]["imported"], 0);
        assert!(!dir.join("escaped.json").exists());
        assert!(!dir.parent().unwrap().join("escaped.json").exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ingest_import_valid_item_succeeds() {
        let dir = std::env::temp_dir().join(format!(
            "distinct-test-ingest-import-ok-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        setup_brand(&dir);

        let source_root = dir.join("sources");
        std::fs::create_dir_all(&source_root).unwrap();
        std::fs::write(source_root.join("page.html"), b"<p>Hello</p>").unwrap();

        let manifest_path = dir.join("manifest.json");
        std::fs::write(
            &manifest_path,
            r#"{
                "items": [{
                    "source_id": "src-valid",
                    "privacy": "public",
                    "coverage": "fetched",
                    "consent": "quote_ok",
                    "retrieved_at": "2026-10-27T00:00:00Z",
                    "original_ref": "https://example.com/page",
                    "payload": "page.html"
                }]
            }"#,
        )
        .unwrap();

        let (code, out, _) = run_args(&[
            "ingest",
            "import",
            "--brand",
            "acme",
            "--workspace",
            &dir.to_string_lossy(),
            "--manifest",
            &manifest_path.to_string_lossy(),
            "--source-root",
            &source_root.to_string_lossy(),
        ]);
        assert_eq!(code, 0, "valid import must succeed");
        let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
        assert_eq!(val["result"]["imported"], 1);
        assert_eq!(val["result"]["failed"], 0);

        // Verify must pass.
        let (vcode, vout, _) = run_args(&[
            "verify",
            "--brand",
            "acme",
            "--workspace",
            &dir.to_string_lossy(),
        ]);
        assert_eq!(vcode, 0, "verify must pass after ingest import");
        let vval: serde_json::Value = serde_json::from_str(vout.trim()).unwrap();
        assert!(vval["result"]["state_valid"].as_bool().unwrap());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ingest_diff_reports_removed_and_unavailable() {
        let dir = std::env::temp_dir().join(format!(
            "distinct-test-ingest-diff-rem-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        setup_brand(&dir);

        let source_root = dir.join("sources");
        std::fs::create_dir_all(&source_root).unwrap();

        // Import a removed source and an unavailable source.
        let manifest_path = dir.join("manifest.json");
        std::fs::write(
            &manifest_path,
            r#"{
                "items": [
                    {
                        "source_id": "src-removed",
                        "privacy": "internal",
                        "coverage": "fetched",
                        "consent": "quote_ok",
                        "retrieved_at": "2026-10-27T00:00:00Z",
                        "original_ref": "https://example.com/old",
                        "removed": true
                    },
                    {
                        "source_id": "src-unavail",
                        "privacy": "internal",
                        "coverage": "unavailable",
                        "consent": "quote_ok",
                        "retrieved_at": "2026-10-27T00:00:00Z",
                        "original_ref": "https://example.com/gone"
                    }
                ]
            }"#,
        )
        .unwrap();

        let (code, _dout, _) = run_args(&[
            "ingest",
            "import",
            "--brand",
            "acme",
            "--workspace",
            &dir.to_string_lossy(),
            "--manifest",
            &manifest_path.to_string_lossy(),
            "--source-root",
            &source_root.to_string_lossy(),
        ]);
        assert_eq!(code, 0);

        // Run diff against a nonexistent snapshot (all are "new").
        let (dcode, dout, _) = run_args(&[
            "ingest",
            "diff",
            "--brand",
            "acme",
            "--workspace",
            &dir.to_string_lossy(),
            "--against",
            "nonexistent-snapshot",
        ]);
        assert_eq!(dcode, 0);
        let dval: serde_json::Value = serde_json::from_str(dout.trim()).unwrap();
        let removed = dval["result"]["explicitly_removed"].as_array().unwrap();
        let unavail = dval["result"]["unavailable"].as_array().unwrap();
        assert_eq!(removed.len(), 1, "must report explicitly_removed");
        assert_eq!(unavail.len(), 1, "must report unavailable");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ingest_extract_updates_state() {
        let dir = std::env::temp_dir().join(format!(
            "distinct-test-ingest-extract-state-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        setup_brand(&dir);

        let html_path = dir.join("page.html");
        std::fs::write(&html_path, b"<html><body><p>Hello world</p></body></html>").unwrap();

        let (code, out, _) = run_args(&[
            "ingest",
            "extract",
            "--brand",
            "acme",
            "--workspace",
            &dir.to_string_lossy(),
            "--file",
            &html_path.to_string_lossy(),
            "--privacy",
            "internal",
        ]);
        assert_eq!(code, 0);
        let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
        assert_eq!(val["result"]["brand_id"], "acme");

        // Verify must pass.
        let (vcode, vout, _) = run_args(&[
            "verify",
            "--brand",
            "acme",
            "--workspace",
            &dir.to_string_lossy(),
        ]);
        assert_eq!(vcode, 0, "verify must pass after ingest extract");
        let vval: serde_json::Value = serde_json::from_str(vout.trim()).unwrap();
        assert!(vval["result"]["state_valid"].as_bool().unwrap());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn evidence_forget_requires_id() {
        let (code, out, _) = run_args(&[
            "evidence",
            "forget",
            "--brand",
            "acme",
            "--workspace",
            "/tmp/nonexistent",
        ]);
        assert_eq!(code, 3);
        let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
        assert!(val["error"].as_str().unwrap().contains("--id"));
    }

    #[test]
    fn evidence_forget_tombstones_and_updates_state() {
        let dir = std::env::temp_dir().join(format!(
            "distinct-test-forget-state-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        setup_brand(&dir);

        // Import some evidence first.
        let evidence_path = dir.join("evidence.txt");
        std::fs::write(&evidence_path, b"some evidence data").unwrap();

        let (code, out, _) = run_args(&[
            "evidence-import",
            "--brand",
            "acme",
            "--workspace",
            &dir.to_string_lossy(),
            "--file",
            &evidence_path.to_string_lossy(),
            "--source-id",
            "src-001",
        ]);
        assert_eq!(code, 0);
        let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
        assert_eq!(val["result"]["source_id"], "src-001");

        // Now forget it.
        let (code, out, _) = run_args(&[
            "evidence",
            "forget",
            "--brand",
            "acme",
            "--workspace",
            &dir.to_string_lossy(),
            "--id",
            "src-001",
            "--reason",
            "owner requested deletion",
        ]);
        assert_eq!(code, 0);
        let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
        assert_eq!(val["result"]["evidence_id"], "src-001");
        assert!(val["result"]["deleted_payload"].as_bool().unwrap());
        assert!(val["result"]["deleted_index"].as_bool().unwrap());

        // Verify must pass after forget.
        let (vcode, vout, _) = run_args(&[
            "verify",
            "--brand",
            "acme",
            "--workspace",
            &dir.to_string_lossy(),
        ]);
        assert_eq!(vcode, 0, "verify must pass after evidence forget");
        let vval: serde_json::Value = serde_json::from_str(vout.trim()).unwrap();
        assert!(vval["result"]["state_valid"].as_bool().unwrap());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn evidence_forget_expected_state_mismatch() {
        let dir = std::env::temp_dir().join(format!(
            "distinct-test-forget-expected-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        setup_brand(&dir);

        let (code, out, _) = run_args(&[
            "evidence",
            "forget",
            "--brand",
            "acme",
            "--workspace",
            &dir.to_string_lossy(),
            "--id",
            "nonexistent",
            "--reason",
            "test",
            "--expected-state",
            "wrong-hash",
        ]);
        assert_eq!(code, 3, "expected-state mismatch must exit 3");
        let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
        assert!(val["error"].as_str().unwrap().contains("state mismatch"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn evidence_forget_keeps_dependents_stale() {
        let dir = std::env::temp_dir().join(format!(
            "distinct-test-forget-deps-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        setup_brand(&dir);

        // Import evidence.
        let evidence_path = dir.join("evidence.txt");
        std::fs::write(&evidence_path, b"evidence for dep test").unwrap();

        let (code, _eout, _) = run_args(&[
            "evidence-import",
            "--brand",
            "acme",
            "--workspace",
            &dir.to_string_lossy(),
            "--file",
            &evidence_path.to_string_lossy(),
            "--source-id",
            "src-dep",
        ]);
        assert_eq!(code, 0);

        // Create a record that references this evidence in source_refs.
        let dep_record = serde_json::json!({
            "schema_version": "1.0.0",
            "kind": "source",
            "id": "claim-dep",
            "brand_id": "acme",
            "privacy": "internal",
            "dependencies": [],
            "source_refs": [{
                "evidence_id": "src-dep",
            }],
            "data": {
                "type": "claim",
                "text": "depends on evidence"
            }
        });
        let dep_path = dir.join("dep-record.json");
        std::fs::write(&dep_path, serde_json::to_string(&dep_record).unwrap()).unwrap();

        let (code, _rout, _) = run_args(&[
            "record-import",
            "--brand",
            "acme",
            "--workspace",
            &dir.to_string_lossy(),
            "--file",
            &dep_path.to_string_lossy(),
        ]);
        assert_eq!(code, 0);

        // Forget the evidence.
        let (code, out, _) = run_args(&[
            "evidence",
            "forget",
            "--brand",
            "acme",
            "--workspace",
            &dir.to_string_lossy(),
            "--id",
            "src-dep",
            "--reason",
            "test deletion",
        ]);
        assert_eq!(code, 0);
        let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
        assert!(
            val["result"]["dependents_marked_stale"].as_u64().unwrap() >= 1,
            "must report dependents marked stale"
        );

        // Gate should report stale entries.
        let (gcode, gout, _) = run_args(&[
            "gate",
            "--brand",
            "acme",
            "--workspace",
            &dir.to_string_lossy(),
            "--scope",
            "default",
        ]);
        assert_eq!(gcode, 0);
        let gval: serde_json::Value = serde_json::from_str(gout.trim()).unwrap();
        let stale = gval["result"]["stale"].as_array().unwrap();
        assert!(
            stale
                .iter()
                .any(|s| s.as_str().unwrap_or("").contains("src-dep")),
            "gate must report stale evidence refs, got: {stale:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ingest_extract_pdf_returns_exit_2() {
        let dir = std::env::temp_dir().join(format!(
            "distinct-test-ingest-pdf-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        setup_brand(&dir);

        let pdf_path = dir.join("test.pdf");
        std::fs::write(&pdf_path, b"%PDF-1.4 fake content").unwrap();

        let (code, out, _) = run_args(&[
            "ingest",
            "extract",
            "--brand",
            "acme",
            "--workspace",
            &dir.to_string_lossy(),
            "--file",
            &pdf_path.to_string_lossy(),
            "--privacy",
            "internal",
        ]);
        assert_eq!(code, 2, "PDF extract must return exit 2");
        let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
        assert_eq!(
            val["error"].as_str().unwrap(),
            "pdf extraction is host-guided"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
