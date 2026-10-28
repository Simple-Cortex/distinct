//! Command dispatch for `distinct-rs`.
//!
//! WO-02 adds the full command matrix with arg parsing, brand validation,
//! and the standard JSON envelope. Commands that need the store delegate
//! to distinct-core; commands that need the filesystem use workspace paths.

pub mod dispatch;

mod cmd_decision;
mod cmd_ingest;

use cmd_ingest::{cmd_evidence_forget, cmd_ingest};
use dispatch::{parse_args, write_envelope, write_error, CliArgs};
use distinct_core::{Exit, ENGINE_VERSION, PROTOCOL_VERSION};
use std::io::Write;

/// Runs the CLI with `args` (without the program name). Returns the exit status.
pub fn run<O: Write, E: Write>(args: &[String], out: &mut O, err: &mut E) -> i32 {
    let parsed = match parse_args(args) {
        Ok(p) => p,
        Err(msg) => {
            let _ = writeln!(err, "distinct-rs: {msg}");
            return Exit::Invalid.code();
        }
    };

    let code = dispatch_command(&parsed, out, err);
    code.code()
}

fn dispatch_command<O: Write, E: Write>(args: &CliArgs, out: &mut O, err: &mut E) -> Exit {
    match args.command.as_str() {
        "help" => write_text(out, help_text()),
        "version" => write_text(out, &format!("distinct-rs {ENGINE_VERSION}\n")),
        "protocol" => write_text(out, &protocol_envelope()),
        "capabilities" => cmd_capabilities(args, out),
        "status" => cmd_status(args, out, err),
        "init" => cmd_init(args, out, err),
        "query" => cmd_query(args, out, err),
        "gate" => cmd_gate(args, out, err),
        "verify" => cmd_verify(args, out, err),
        "record-import" | "record" => cmd_record_import(args, out, err),
        "evidence-import" => cmd_evidence_import(args, out, err),
        "evidence" => {
            // Route "evidence forget" to the forget handler, everything else to import.
            if args.rest.first().is_some_and(|s| s == "forget") {
                cmd_evidence_forget(args, out, err)
            } else {
                cmd_evidence_import(args, out, err)
            }
        }
        "packet" => cmd_packet(args, out, err),
        "export" => cmd_export(args, out, err),
        "check" => cmd_check(args, out, err),
        "review" => cmd_review_create(args, out, err),
        "decision" => cmd_decision::cmd_decision(args, out),
        "logo" => match args.rest.first().map(|s| s.as_str()) {
            Some("inspect") => cmd_logo_inspect(args, out, err),
            Some("proof") => cmd_logo_proof(args, out, err),
            Some("review-packet") => cmd_logo_review_packet(args, out, err),
            _ => write_error(
                out,
                "logo",
                "usage: logo inspect|proof|review-packet [options]",
                Exit::Invalid,
            ),
        },
        "logo-inspect" => cmd_logo_inspect(args, out, err),
        "logo-proof" => cmd_logo_proof(args, out, err),
        "logo-review-packet" => cmd_logo_review_packet(args, out, err),
        "color" => match args.rest.first().map(|s| s.as_str()) {
            Some("check") => cmd_color_check(args, out, err),
            _ => write_error(
                out,
                "color",
                "usage: color check --pairs <file>",
                Exit::Invalid,
            ),
        },
        "color-check" => cmd_color_check(args, out, err),
        "render-worker" => cmd_render_worker(out, err),
        "ingest" => cmd_ingest(args, out, err),
        "evidence-forget" => cmd_evidence_forget(args, out, err),
        "test" => cmd_test(args, out, err),
        "patch" => cmd_patch(args, out, err),
        "review-packet" => cmd_review_packet(args, out, err),
        other => {
            let _ = writeln!(err, "distinct-rs: unknown command '{other}'");
            Exit::Invalid
        }
    }
}

fn write_text<O: Write>(out: &mut O, text: &str) -> Exit {
    match out.write_all(text.as_bytes()).and_then(|()| out.flush()) {
        Ok(()) => Exit::Ok,
        Err(_) => Exit::Invalid,
    }
}

fn help_text() -> &'static str {
    dispatch::HELP
}

fn protocol_envelope() -> String {
    format!(
        "{{\"protocol_version\":\"{PROTOCOL_VERSION}\",\"engine_version\":\"{ENGINE_VERSION}\",\
\"execution\":{{\"engine\":\"native-rust\",\"command\":\"protocol\"}},\
\"result\":{{\"protocol\":\"{PROTOCOL_VERSION}\",\"version\":\"{ENGINE_VERSION}\"}},\
\"limitations\":[],\"changed\":[],\"error\":null}}\n"
    )
}

fn cmd_capabilities<O: Write>(args: &CliArgs, out: &mut O) -> Exit {
    let caps = serde_json::json!({
        "native": {
            "commands": dispatch::KNOWN_COMMANDS,
            "schema_version": distinct_core::record::SCHEMA_VERSION,
            "max_bytes_default": 16384,
            "max_bytes_min": 1024,
        },
        "host_guided": [
            "crawling", "document_search", "pdf_reading", "ocr",
            "interviewing", "strategy", "visual_inspection",
            "audience_testing", "expert_review", "optical_refinement",
        ],
        "not_first_pass": [
            "native_web_crawling", "pdf_parser", "arbitrary_svg_editing",
            "model_gateway", "universal_tokenizer", "trademark_clearance",
        ],
        "format": args.format,
    });
    write_envelope(out, "capabilities", caps, vec![], vec![])
}

fn cmd_status<O: Write, E: Write>(args: &CliArgs, out: &mut O, _err: &mut E) -> Exit {
    let brand = match dispatch::require_brand(args) {
        Ok(b) => b,
        Err(msg) => return write_error(out, "status", &msg, Exit::Invalid),
    };
    let workspace = &args.workspace;
    let state_path = format!("{workspace}/.distinct/{brand}/STATE.json");

    match std::fs::read_to_string(&state_path) {
        Ok(contents) => match serde_json::from_str::<serde_json::Value>(&contents) {
            Ok(state) => {
                let result = serde_json::json!({
                    "brand_id": brand,
                    "workspace": workspace,
                    "state": state,
                    "status": "initialized",
                });
                write_envelope(out, "status", result, vec![], vec![])
            }
            Err(e) => write_error(
                out,
                "status",
                &format!("corrupt STATE.json: {e}"),
                Exit::Invalid,
            ),
        },
        Err(_) => {
            let result = serde_json::json!({
                "brand_id": brand,
                "workspace": workspace,
                "status": "not_initialized",
            });
            write_envelope(out, "status", result, vec![], vec![])
        }
    }
}

fn cmd_init<O: Write, E: Write>(args: &CliArgs, out: &mut O, _err: &mut E) -> Exit {
    let brand = match dispatch::require_brand(args) {
        Ok(b) => b.to_string(),
        Err(msg) => return write_error(out, "init", &msg, Exit::Invalid),
    };

    let name = args
        .rest
        .iter()
        .position(|a| a == "--name")
        .and_then(|i| args.rest.get(i + 1))
        .map(|s| s.as_str())
        .unwrap_or(&brand);

    let workspace = &args.workspace;
    let brand_dir = format!("{workspace}/.distinct/{brand}");
    let state_path = format!("{brand_dir}/STATE.json");

    // Check if already initialized
    if std::path::Path::new(&state_path).exists() {
        let result = serde_json::json!({
            "brand_id": brand,
            "status": "already_initialized",
        });
        return write_envelope(out, "init", result, vec![], vec![]);
    }

    // Create directory structure
    let dirs = [
        &brand_dir,
        &format!("{brand_dir}/store/objects"),
        &format!("{brand_dir}/store/snapshots"),
        &format!("{brand_dir}/store/governance"),
        &format!("{brand_dir}/evidence/objects"),
        &format!("{brand_dir}/evidence/index"),
        &format!("{brand_dir}/packets"),
        &format!("{brand_dir}/reviews"),
        &format!("{brand_dir}/tests"),
        &format!("{brand_dir}/logos"),
        &format!("{brand_dir}/exports"),
        &format!("{brand_dir}/views"),
    ];
    for dir in &dirs {
        if let Err(e) = std::fs::create_dir_all(dir) {
            return write_error(
                out,
                "init",
                &format!("failed to create {dir}: {e}"),
                Exit::Invalid,
            );
        }
    }

    // Write initial STATE.json
    let state = serde_json::json!({
        "schema_version": distinct_core::record::SCHEMA_VERSION,
        "brand_id": brand,
        "content_snapshot_hash": null,
        "governance_index_hash": null,
        "generation": 0,
        "name": name,
        "created_at": "2026-10-26T00:00:00Z",
    });
    if let Err(e) = std::fs::write(&state_path, serde_json::to_string_pretty(&state).unwrap()) {
        return write_error(
            out,
            "init",
            &format!("failed to write STATE.json: {e}"),
            Exit::Invalid,
        );
    }

    let result = serde_json::json!({
        "brand_id": brand,
        "status": "initialized",
        "workspace": brand_dir,
    });
    write_envelope(out, "init", result, vec![], vec![state_path])
}

fn cmd_query<O: Write, E: Write>(args: &CliArgs, out: &mut O, _err: &mut E) -> Exit {
    let brand = match dispatch::require_brand(args) {
        Ok(b) => b,
        Err(msg) => return write_error(out, "query", &msg, Exit::Invalid),
    };

    let kind = args
        .rest
        .iter()
        .position(|a| a == "--kind")
        .and_then(|i| args.rest.get(i + 1))
        .map(|s| s.as_str());

    let limit: usize = args
        .rest
        .iter()
        .position(|a| a == "--limit")
        .and_then(|i| args.rest.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);

    // Scan the store for matching records
    let workspace = &args.workspace;
    let objects_dir = format!("{workspace}/.distinct/{brand}/store/objects");
    let mut found = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&objects_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json") {
                if let Ok(contents) = std::fs::read_to_string(&path) {
                    if let Ok(record) = serde_json::from_str::<serde_json::Value>(&contents) {
                        let matches = kind.is_none_or(|k| record["kind"] == k);
                        if matches {
                            found.push(serde_json::json!({
                                "hash": path.file_stem().unwrap().to_string_lossy(),
                                "kind": record["kind"],
                                "id": record["id"],
                                "privacy": record["privacy"],
                            }));
                            if found.len() >= limit {
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    let result = serde_json::json!({
        "brand_id": brand,
        "count": found.len(),
        "records": found,
        "truncated": found.len() >= limit,
    });
    write_envelope(out, "query", result, vec![], vec![])
}

fn cmd_gate<O: Write, E: Write>(args: &CliArgs, out: &mut O, _err: &mut E) -> Exit {
    use distinct_core::store::BrandWorkspace;

    let brand = match dispatch::require_brand(args) {
        Ok(b) => b,
        Err(msg) => return write_error(out, "gate", &msg, Exit::Invalid),
    };

    let scope = args
        .rest
        .iter()
        .position(|a| a == "--scope")
        .and_then(|i| args.rest.get(i + 1))
        .map(|s| s.as_str())
        .unwrap_or("default");

    let ws = BrandWorkspace::new(std::path::Path::new(&args.workspace), brand);
    if !ws.is_initialized() {
        let result = serde_json::json!({
            "brand_id": brand,
            "scope": scope,
            "ready": false,
            "blockers": ["brand not initialized"],
        });
        return write_envelope(out, "gate", result, vec![], vec![]);
    }

    match distinct_core::governance::reconcile_governance(&ws) {
        Ok(verdict) => {
            let objects = match ws.list_objects(None) {
                Ok(o) => o,
                Err(e) => return write_error(out, "gate", &e.to_string(), Exit::Invalid),
            };
            let approval = distinct_core::governance::owner_approval_blockers(&objects, scope);
            let ready = verdict.ready && approval.is_empty();
            let mut blockers: Vec<serde_json::Value> =
                verdict.blockers.into_iter().map(Into::into).collect();
            blockers.extend(approval);
            let result = serde_json::json!({
                "brand_id": brand,
                "scope": scope,
                "ready": ready,
                "blockers": blockers,
                "warnings": verdict.warnings,
                "stale": verdict.stale,
                "missing_reviews": verdict.missing_reviews,
                "missing_proofs": verdict.missing_proofs,
            });
            write_envelope(out, "gate", result, vec![], vec![])
        }
        Err(e) => write_error(out, "gate", &e, Exit::Invalid),
    }
}

fn cmd_verify<O: Write, E: Write>(args: &CliArgs, out: &mut O, _err: &mut E) -> Exit {
    use distinct_core::store::BrandWorkspace;

    let brand = match dispatch::require_brand(args) {
        Ok(b) => b,
        Err(msg) => return write_error(out, "verify", &msg, Exit::Invalid),
    };

    let ws = BrandWorkspace::new(std::path::Path::new(&args.workspace), brand);
    if !ws.is_initialized() {
        return write_error(out, "verify", "brand not initialized", Exit::Invalid);
    }

    let state = match ws.read_state() {
        Ok(s) => s,
        Err(e) => return write_error(out, "verify", &e.to_string(), Exit::Invalid),
    };

    // Reconcile governance (checks snapshot + governance hash consistency).
    let gov = match distinct_core::governance::reconcile_governance(&ws) {
        Ok(v) => v,
        Err(e) => return write_error(out, "verify", &e, Exit::Invalid),
    };

    // Also verify individual object integrity (detects file tampering).
    let integrity_blockers = match distinct_core::governance::verify_object_integrity(&ws) {
        Ok(b) => b,
        Err(e) => return write_error(out, "verify", &e, Exit::Invalid),
    };

    let mut all_blockers = gov.blockers;
    all_blockers.extend(integrity_blockers);

    let state_valid = all_blockers.is_empty();
    let result = serde_json::json!({
        "brand_id": brand,
        "state_valid": state_valid,
        "schema_version": state.schema_version,
        "generation": state.generation,
        "blockers": all_blockers,
        "stale": gov.stale,
    });
    // WO-15: exit 1 when state_valid is false.
    let _ = write_envelope(out, "verify", result, vec![], vec![]);
    if state_valid {
        Exit::Ok
    } else {
        Exit::CheckFailed
    }
}

fn cmd_record_import<O: Write, E: Write>(args: &CliArgs, out: &mut O, _err: &mut E) -> Exit {
    use distinct_core::store::BrandWorkspace;

    let brand = match dispatch::require_brand(args) {
        Ok(b) => b,
        Err(msg) => return write_error(out, "record-import", &msg, Exit::Invalid),
    };

    let file_path = args
        .rest
        .iter()
        .position(|a| a == "--file")
        .and_then(|i| args.rest.get(i + 1))
        .map(|s| s.as_str());

    let json_text = match file_path {
        Some(path) => match std::fs::read_to_string(path) {
            Ok(contents) => contents,
            Err(e) => {
                return write_error(
                    out,
                    "record-import",
                    &format!("cannot read {path}: {e}"),
                    Exit::Invalid,
                )
            }
        },
        None => {
            return write_error(
                out,
                "record-import",
                "--file <path> required",
                Exit::Invalid,
            )
        }
    };

    if let Err(e) = distinct_core::dup_keys::reject_duplicate_keys(&json_text) {
        return write_error(out, "record-import", &e.to_string(), Exit::Invalid);
    }

    let record: serde_json::Value = match serde_json::from_str(&json_text) {
        Ok(v) => v,
        Err(e) => {
            return write_error(
                out,
                "record-import",
                &format!("invalid JSON: {e}"),
                Exit::Invalid,
            )
        }
    };

    if record["brand_id"].as_str() != Some(brand) {
        return write_error(
            out,
            "record-import",
            &format!(
                "record brand_id '{}' does not match --brand '{}'",
                record["brand_id"].as_str().unwrap_or("?"),
                brand
            ),
            Exit::Invalid,
        );
    }

    let ws = BrandWorkspace::new(std::path::Path::new(&args.workspace), brand);
    if !ws.is_initialized() {
        return write_error(out, "record-import", "brand not initialized", Exit::Invalid);
    }

    // WO-15: reject governance kinds — use the dedicated command instead.
    if let Some(kind_str) = record["kind"].as_str() {
        if let Ok(k) =
            serde_json::from_str::<distinct_core::record::RecordKind>(&format!("\"{kind_str}\""))
        {
            if !k.is_content() {
                return write_error(
                    out,
                    "record-import",
                    &format!(
                        "kind '{kind_str}' is a governance kind; use the dedicated command instead"
                    ),
                    Exit::Invalid,
                );
            }
        }
    }

    match ws.store_object(&record) {
        Ok(hash) => {
            // WO-15: update STATE so verify stays clean after mutation.
            let mut state = match ws.read_state() {
                Ok(s) => s,
                Err(e) => {
                    return write_error(out, "record-import", &e.to_string(), Exit::Invalid);
                }
            };
            if let Err(e) = ws.update_snapshot(&mut state) {
                return write_error(
                    out,
                    "record-import",
                    &format!("update snapshot: {e}"),
                    Exit::Invalid,
                );
            }
            if let Err(e) = ws.update_governance_index(&mut state) {
                return write_error(
                    out,
                    "record-import",
                    &format!("update governance index: {e}"),
                    Exit::Invalid,
                );
            }
            if let Err(e) = ws.write_state(&state) {
                return write_error(
                    out,
                    "record-import",
                    &format!("write state: {e}"),
                    Exit::Invalid,
                );
            }

            let rid = distinct_core::hash::revision_id(&hash);
            let result = serde_json::json!({
                "brand_id": brand,
                "content_hash": hash.as_hex(),
                "revision_id": rid,
                "kind": record["kind"],
                "id": record["id"],
            });
            write_envelope(out, "record-import", result, vec![], vec![])
        }
        Err(e) => write_error(out, "record-import", &e.to_string(), Exit::Invalid),
    }
}

fn cmd_evidence_import<O: Write, E: Write>(args: &CliArgs, out: &mut O, _err: &mut E) -> Exit {
    use distinct_core::store::BrandWorkspace;

    let brand = match dispatch::require_brand(args) {
        Ok(b) => b,
        Err(msg) => return write_error(out, "evidence-import", &msg, Exit::Invalid),
    };

    let file_path = args
        .rest
        .iter()
        .position(|a| a == "--file")
        .and_then(|i| args.rest.get(i + 1));
    let source_id = args
        .rest
        .iter()
        .position(|a| a == "--source-id")
        .and_then(|i| args.rest.get(i + 1));

    let file_path = match file_path {
        Some(p) => p.as_str(),
        None => {
            return write_error(
                out,
                "evidence-import",
                "--file <path> required",
                Exit::Invalid,
            )
        }
    };
    let source_id = match source_id {
        Some(s) => s.as_str(),
        None => {
            return write_error(
                out,
                "evidence-import",
                "--source-id <id> required",
                Exit::Invalid,
            )
        }
    };

    let bytes = match std::fs::read(file_path) {
        Ok(b) => b,
        Err(e) => {
            return write_error(
                out,
                "evidence-import",
                &format!("cannot read {file_path}: {e}"),
                Exit::Invalid,
            )
        }
    };

    let ws = BrandWorkspace::new(std::path::Path::new(&args.workspace), brand);
    if !ws.is_initialized() {
        return write_error(
            out,
            "evidence-import",
            "brand not initialized",
            Exit::Invalid,
        );
    }

    match ws.store_evidence(&bytes, source_id) {
        Ok(hash) => {
            let result = serde_json::json!({
                "brand_id": brand,
                "source_id": source_id,
                "byte_hash": hash.as_hex(),
                "byte_count": bytes.len(),
            });
            write_envelope(out, "evidence-import", result, vec![], vec![])
        }
        Err(e) => write_error(out, "evidence-import", &e.to_string(), Exit::Invalid),
    }
}

fn cmd_packet<O: Write, E: Write>(args: &CliArgs, out: &mut O, _err: &mut E) -> Exit {
    use distinct_core::packet::{self, PacketOptions};
    use distinct_core::store::BrandWorkspace;

    let brand = match dispatch::require_brand(args) {
        Ok(b) => b,
        Err(msg) => return write_error(out, "packet", &msg, Exit::Invalid),
    };

    let task = args
        .rest
        .iter()
        .position(|a| a == "--task")
        .and_then(|i| args.rest.get(i + 1))
        .map(|s| s.as_str());
    let task = match task {
        Some(t) => t,
        None => return write_error(out, "packet", "--task <name> required", Exit::Invalid),
    };

    let scope_str = args
        .rest
        .iter()
        .position(|a| a == "--scope")
        .and_then(|i| args.rest.get(i + 1))
        .map(|s| s.as_str());
    let scope_str = match scope_str {
        Some(s) => s,
        None => return write_error(out, "packet", "--scope <fields> required", Exit::Invalid),
    };
    let scope: Vec<String> = scope_str.split(',').map(|s| s.trim().to_string()).collect();

    // WO-15: reject unknown scope field names.
    let unknown: Vec<&str> = scope
        .iter()
        .filter(|f| !distinct_core::packet::ANCHOR_KEYS.contains(&f.as_str()))
        .map(|s| s.as_str())
        .collect();
    if !unknown.is_empty() {
        return write_error(
            out,
            "packet",
            &format!("unknown scope fields: {}", unknown.join(", ")),
            Exit::Invalid,
        );
    }

    let destination = args
        .rest
        .iter()
        .position(|a| a == "--destination")
        .and_then(|i| args.rest.get(i + 1))
        .map(|s| s.as_str());

    let allow_str = args
        .rest
        .iter()
        .position(|a| a == "--allow")
        .and_then(|i| args.rest.get(i + 1))
        .map(|s| s.as_str());

    // Determine allowed privacy classes.
    let allowed_privacy = if destination.is_none() {
        vec![distinct_core::record::Privacy::Public]
    } else if let Some(a) = allow_str {
        packet::parse_privacy_list(a)
    } else {
        vec![
            distinct_core::record::Privacy::Public,
            distinct_core::record::Privacy::Internal,
        ]
    };

    // Determine max_bytes: the lesser of the global --max-bytes and any packet-level cap.
    let max_bytes = args.max_bytes;

    let ws = BrandWorkspace::new(std::path::Path::new(&args.workspace), brand);
    if !ws.is_initialized() {
        return write_error(out, "packet", "brand not initialized", Exit::Invalid);
    }

    let state = match ws.read_state() {
        Ok(s) => s,
        Err(e) => return write_error(out, "packet", &e.to_string(), Exit::Invalid),
    };

    let objects = match ws.list_objects(None) {
        Ok(o) => o,
        Err(e) => return write_error(out, "packet", &e.to_string(), Exit::Invalid),
    };

    let opts = PacketOptions {
        task,
        brand_id: brand,
        snapshot_hash: state.content_snapshot_hash.as_deref(),
        scope,
        destination,
        allowed_privacy,
        max_bytes,
        objects,
    };

    match packet::build_packet(&opts) {
        packet::PacketResult::Ok {
            envelope,
            packet_hash,
            ..
        } => {
            // Build the response result with the packet content inline.
            let mut result = envelope.clone();
            result["brand_id"] = serde_json::json!(brand);
            result["max_bytes"] = serde_json::json!(max_bytes);
            result["byte_length"] = serde_json::json!(0); // placeholder

            // Build the full response envelope.
            let mut response_env = serde_json::Map::new();
            response_env.insert(
                "protocol_version".into(),
                serde_json::json!(distinct_core::PROTOCOL_VERSION),
            );
            response_env.insert(
                "engine_version".into(),
                serde_json::json!(distinct_core::ENGINE_VERSION),
            );
            response_env.insert(
                "execution".into(),
                serde_json::json!({"engine":"native-rust","command":"packet"}),
            );
            response_env.insert("result".into(), result.clone());
            response_env.insert("limitations".into(), serde_json::json!([]));
            response_env.insert("changed".into(), serde_json::json!([]));
            response_env.insert("error".into(), serde_json::Value::Null);

            // Compute byte_length to a fixed point (the field value affects total size).
            let mut response_env = serde_json::Value::Object(response_env);
            let mut byte_length: usize = 0;
            for _ in 0..3 {
                let serialized = serde_json::to_string(&response_env).unwrap_or_default();
                let new_bl = serialized.len() + 1; // +1 for trailing newline
                response_env["result"]["byte_length"] = serde_json::json!(new_bl);
                byte_length = new_bl;
                let recheck = serde_json::to_string(&response_env).unwrap_or_default();
                if recheck.len() + 1 == byte_length {
                    break;
                }
            }

            if byte_length > max_bytes {
                let min_required = byte_length;
                let anchors = result["anchors"].as_object();
                let split = packet::build_split_proposal_public(
                    anchors.unwrap_or(&serde_json::Map::new()),
                    max_bytes,
                );
                let err_result = serde_json::json!({
                    "brand_id": brand,
                    "error": "budget_insufficient",
                    "min_required_bytes": min_required,
                    "split_proposal": split,
                });
                let mut err_env = serde_json::Map::new();
                err_env.insert(
                    "protocol_version".into(),
                    serde_json::json!(distinct_core::PROTOCOL_VERSION),
                );
                err_env.insert(
                    "engine_version".into(),
                    serde_json::json!(distinct_core::ENGINE_VERSION),
                );
                err_env.insert(
                    "execution".into(),
                    serde_json::json!({"engine":"native-rust","command":"packet"}),
                );
                err_env.insert("result".into(), err_result);
                err_env.insert("limitations".into(), serde_json::json!([]));
                err_env.insert("changed".into(), serde_json::json!([]));
                err_env.insert("error".into(), serde_json::json!("budget_insufficient"));
                let err_json = serde_json::to_string(&err_env).unwrap_or_default();
                let _ = writeln!(out, "{err_json}");
                return Exit::Unavailable;
            }

            // Write the response to stdout.
            let response_json = serde_json::to_string(&response_env).unwrap_or_default();
            let _ = writeln!(out, "{response_json}");

            // Persist the packet (content envelope without response-only fields).
            let packets_dir = ws.root.join("packets");
            let _ = std::fs::create_dir_all(&packets_dir);
            let packet_path = packets_dir.join(format!("{}.json", packet_hash.as_hex()));
            let packet_json = serde_json::to_string_pretty(&envelope).unwrap_or_default();
            let _ = std::fs::write(&packet_path, format!("{packet_json}\n"));

            Exit::Ok
        }
        packet::PacketResult::CapTooSmall => write_error(
            out,
            "packet",
            "max-bytes must be at least 1024",
            Exit::Invalid,
        ),
        packet::PacketResult::PrivacyDisallowed { blockers } => write_error(
            out,
            "packet",
            &format!("privacy_disallowed: {}", blockers.join("; ")),
            Exit::Invalid,
        ),
        packet::PacketResult::BudgetInsufficient {
            min_required_bytes,
            split_proposal,
        } => {
            let err_result = serde_json::json!({
                "brand_id": brand,
                "error": "budget_insufficient",
                "min_required_bytes": min_required_bytes,
                "split_proposal": split_proposal,
            });
            let mut err_env = serde_json::Map::new();
            err_env.insert(
                "protocol_version".into(),
                serde_json::json!(distinct_core::PROTOCOL_VERSION),
            );
            err_env.insert(
                "engine_version".into(),
                serde_json::json!(distinct_core::ENGINE_VERSION),
            );
            err_env.insert(
                "execution".into(),
                serde_json::json!({"engine":"native-rust","command":"packet"}),
            );
            err_env.insert("result".into(), err_result);
            err_env.insert("limitations".into(), serde_json::json!([]));
            err_env.insert("changed".into(), serde_json::json!([]));
            err_env.insert("error".into(), serde_json::json!("budget_insufficient"));
            let err_json = serde_json::to_string(&err_env).unwrap_or_default();
            let _ = writeln!(out, "{err_json}");
            Exit::Unavailable
        }
    }
}

fn cmd_export<O: Write, E: Write>(args: &CliArgs, out: &mut O, _err: &mut E) -> Exit {
    use distinct_core::export::{self, ExportOptions};
    use distinct_core::store::BrandWorkspace;

    let brand = match dispatch::require_brand(args) {
        Ok(b) => b,
        Err(msg) => return write_error(out, "export", &msg, Exit::Invalid),
    };

    let kind = args
        .rest
        .iter()
        .position(|a| a == "--kind")
        .and_then(|i| args.rest.get(i + 1))
        .map(|s| s.as_str());
    let kind = match kind {
        Some(k) => k,
        None => return write_error(out, "export", "--kind <type> required", Exit::Invalid),
    };

    let out_dir_str = args
        .rest
        .iter()
        .position(|a| a == "--out")
        .and_then(|i| args.rest.get(i + 1))
        .map(|s| s.as_str());
    let out_dir_str = match out_dir_str {
        Some(d) => d,
        None => return write_error(out, "export", "--out <dir> required", Exit::Invalid),
    };
    let out_dir = std::path::PathBuf::from(out_dir_str);

    let force = args.rest.iter().any(|a| a == "--force");

    // WO-13: --tokens <format>
    let tokens_format = args
        .rest
        .iter()
        .position(|a| a == "--tokens")
        .and_then(|i| args.rest.get(i + 1))
        .map(|s| s.as_str());

    let ws = BrandWorkspace::new(std::path::Path::new(&args.workspace), brand);
    if !ws.is_initialized() {
        return write_error(out, "export", "brand not initialized", Exit::Invalid);
    }

    let state = match ws.read_state() {
        Ok(s) => s,
        Err(e) => return write_error(out, "export", &e.to_string(), Exit::Invalid),
    };

    let objects = match ws.list_objects(None) {
        Ok(o) => o,
        Err(e) => return write_error(out, "export", &e.to_string(), Exit::Invalid),
    };

    let opts = ExportOptions {
        brand_id: brand,
        kind,
        out_dir: &out_dir,
        force,
        objects,
        snapshot_hash: state.content_snapshot_hash.as_deref(),
        tokens_format,
    };

    match export::run_export(&opts) {
        export::ExportResult::Ok {
            manifest,
            records_written,
            excluded,
            tokens_omitted,
        } => {
            let result = serde_json::json!({
                "brand_id": brand,
                "kind": kind,
                "out_dir": out_dir_str,
                "records_written": records_written,
                "excluded": excluded,
                "manifest_hash": export::manifest_hash(&manifest),
                "tokens_omitted": tokens_omitted,
            });
            write_envelope(out, "export", result, vec![], vec![out_dir_str.to_string()])
        }
        export::ExportResult::ProductionReadyRefused => write_error(
            out,
            "export",
            "production_ready export is not supported",
            Exit::Invalid,
        ),
        export::ExportResult::UnknownKind(k) => write_error(
            out,
            "export",
            &format!("unknown export kind: {k}"),
            Exit::Invalid,
        ),
        export::ExportResult::OutputExists => write_error(
            out,
            "export",
            "output directory exists and is not empty; use --force to overwrite",
            Exit::Invalid,
        ),
        export::ExportResult::IoError(e) => write_error(out, "export", &e, Exit::Invalid),
    }
}

fn cmd_check<O: Write, E: Write>(args: &CliArgs, out: &mut O, err: &mut E) -> Exit {
    // Thin wrappers over record-import with kind validation.
    // Subcommands: "plan create" and "result record".
    let sub1 = args.rest.first().map(|s| s.as_str());
    let sub2 = args.rest.get(1).map(|s| s.as_str());

    match (sub1, sub2) {
        (Some("plan"), Some("create")) => cmd_governance_import(out, err, args, "check_plan"),
        (Some("result"), Some("record")) => cmd_governance_import(out, err, args, "check_result"),
        _ => write_error(
            out,
            "check",
            "usage: check plan create --file <f> | check result record --file <f>",
            Exit::Invalid,
        ),
    }
}

fn cmd_review_create<O: Write, E: Write>(args: &CliArgs, out: &mut O, err: &mut E) -> Exit {
    cmd_governance_import(out, err, args, "review")
}

/// Shared governance-record import (check_plan, check_result, review).
fn cmd_governance_import<O: Write, E: Write>(
    out: &mut O,
    _err: &mut E,
    args: &CliArgs,
    expected_kind: &str,
) -> Exit {
    use distinct_core::store::BrandWorkspace;

    let brand = match dispatch::require_brand(args) {
        Ok(b) => b,
        Err(msg) => return write_error(out, expected_kind, &msg, Exit::Invalid),
    };

    let file_path = args
        .rest
        .iter()
        .position(|a| a == "--file")
        .and_then(|i| args.rest.get(i + 1))
        .map(|s| s.as_str());
    let file_path = match file_path {
        Some(p) => p,
        None => return write_error(out, expected_kind, "--file <path> required", Exit::Invalid),
    };

    let json_text = match std::fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(e) => {
            return write_error(
                out,
                expected_kind,
                &format!("cannot read {file_path}: {e}"),
                Exit::Invalid,
            )
        }
    };

    if let Err(e) = distinct_core::dup_keys::reject_duplicate_keys(&json_text) {
        return write_error(out, expected_kind, &e.to_string(), Exit::Invalid);
    }

    let record: serde_json::Value = match serde_json::from_str(&json_text) {
        Ok(v) => v,
        Err(e) => {
            return write_error(
                out,
                expected_kind,
                &format!("invalid JSON: {e}"),
                Exit::Invalid,
            )
        }
    };

    if record["kind"].as_str() != Some(expected_kind) {
        return write_error(
            out,
            expected_kind,
            &format!(
                "expected kind '{expected_kind}', got '{}'",
                record["kind"].as_str().unwrap_or("?")
            ),
            Exit::Invalid,
        );
    }

    if record["brand_id"].as_str() != Some(brand) {
        return write_error(
            out,
            expected_kind,
            &format!(
                "record brand_id '{}' does not match --brand '{}'",
                record["brand_id"].as_str().unwrap_or("?"),
                brand
            ),
            Exit::Invalid,
        );
    }

    let ws = BrandWorkspace::new(std::path::Path::new(&args.workspace), brand);
    if !ws.is_initialized() {
        return write_error(out, expected_kind, "brand not initialized", Exit::Invalid);
    }

    match ws.store_object(&record) {
        Ok(hash) => {
            // WO-09: update STATE governance index and content snapshot so verify stays clean.
            let mut state = match ws.read_state() {
                Ok(s) => s,
                Err(e) => {
                    return write_error(out, expected_kind, &e.to_string(), Exit::Invalid);
                }
            };
            if let Err(e) = ws.update_snapshot(&mut state) {
                return write_error(
                    out,
                    expected_kind,
                    &format!("update snapshot: {e}"),
                    Exit::Invalid,
                );
            }
            if let Err(e) = ws.update_governance_index(&mut state) {
                return write_error(
                    out,
                    expected_kind,
                    &format!("update governance index: {e}"),
                    Exit::Invalid,
                );
            }
            if let Err(e) = ws.write_state(&state) {
                return write_error(
                    out,
                    expected_kind,
                    &format!("write state: {e}"),
                    Exit::Invalid,
                );
            }

            let rid = distinct_core::hash::revision_id(&hash);
            let result = serde_json::json!({
                "brand_id": brand,
                "content_hash": hash.as_hex(),
                "revision_id": rid,
                "kind": expected_kind,
                "id": record["id"],
            });
            write_envelope(out, expected_kind, result, vec![], vec![])
        }
        Err(e) => write_error(out, expected_kind, &e.to_string(), Exit::Invalid),
    }
}

fn cmd_logo_inspect<O: Write, E: Write>(args: &CliArgs, out: &mut O, _err: &mut E) -> Exit {
    use distinct_assets::inspect;
    use distinct_core::store::BrandWorkspace;

    let brand = match dispatch::require_brand(args) {
        Ok(b) => b,
        Err(msg) => return write_error(out, "logo-inspect", &msg, Exit::Invalid),
    };

    let file_path = args
        .rest
        .iter()
        .position(|a| a == "--file")
        .and_then(|i| args.rest.get(i + 1))
        .map(|s| s.as_str());
    let file_path = match file_path {
        Some(p) => p,
        None => return write_error(out, "logo-inspect", "--file <path> required", Exit::Invalid),
    };

    let ws = BrandWorkspace::new(std::path::Path::new(&args.workspace), brand);
    if !ws.is_initialized() {
        return write_error(out, "logo-inspect", "brand not initialized", Exit::Invalid);
    }

    let path = std::path::Path::new(file_path);
    match inspect::inspect_svg(path) {
        Ok(result) => {
            // Build a logo_finding record from the inspection result
            let finding_id = format!("inspect-{}", &result.file_hash[..12]);
            let record = serde_json::json!({
                "schema_version": distinct_core::record::SCHEMA_VERSION,
                "kind": "logo_finding",
                "id": finding_id,
                "brand_id": brand,
                "created_at": distinct_core::store::chrono_now(),
                "created_by": {
                    "kind": "agent",
                    "id": "distinct-rs",
                },
                "privacy": "public",
                "dependencies": [],
                "limitations": [],
                "data": {
                    "verdict": serde_json::to_value(&result.verdict).unwrap_or_default(),
                    "file_hash": result.file_hash,
                    "element_counts": result.element_counts,
                    "unsupported": result.unsupported,
                    "issues": result.issues,
                    "native_proof": result.native_proof,
                    "element_count": result.element_count,
                    "max_depth_reached": result.max_depth_reached,
                }
            });

            // Store the finding record
            match ws.store_object(&record) {
                Ok(hash) => {
                    // WO-15: update STATE so verify stays clean after mutation.
                    let mut state = match ws.read_state() {
                        Ok(s) => s,
                        Err(e) => {
                            return write_error(out, "logo-inspect", &e.to_string(), Exit::Invalid);
                        }
                    };
                    if let Err(e) = ws.update_snapshot(&mut state) {
                        return write_error(
                            out,
                            "logo-inspect",
                            &format!("update snapshot: {e}"),
                            Exit::Invalid,
                        );
                    }
                    if let Err(e) = ws.update_governance_index(&mut state) {
                        return write_error(
                            out,
                            "logo-inspect",
                            &format!("update governance index: {e}"),
                            Exit::Invalid,
                        );
                    }
                    if let Err(e) = ws.write_state(&state) {
                        return write_error(
                            out,
                            "logo-inspect",
                            &format!("write state: {e}"),
                            Exit::Invalid,
                        );
                    }

                    let rid = distinct_core::hash::revision_id(&hash);
                    let response = serde_json::json!({
                        "brand_id": brand,
                        "content_hash": hash.as_hex(),
                        "revision_id": rid,
                        "finding_id": finding_id,
                        "verdict": serde_json::to_value(&result.verdict).unwrap_or_default(),
                        "file_hash": result.file_hash,
                        "element_count": result.element_count,
                        "unsupported_count": result.unsupported.len(),
                        "issue_count": result.issues.len(),
                        "native_proof": result.native_proof,
                    });
                    write_envelope(out, "logo-inspect", response, vec![], vec![])
                }
                Err(e) => write_error(out, "logo-inspect", &e.to_string(), Exit::Invalid),
            }
        }
        Err(inspect::InspectError::InputTooLarge { actual, max }) => write_error(
            out,
            "logo-inspect",
            &format!("input too large: {actual} bytes exceeds {max} byte cap"),
            Exit::Invalid,
        ),
        Err(inspect::InspectError::DepthExceeded { depth, max }) => write_error(
            out,
            "logo-inspect",
            &format!("nesting depth {depth} exceeds limit of {max}"),
            Exit::Invalid,
        ),
        Err(inspect::InspectError::ElementLimitExceeded { count, max }) => write_error(
            out,
            "logo-inspect",
            &format!("element count {count} exceeds limit of {max}"),
            Exit::Invalid,
        ),
        Err(e) => write_error(out, "logo-inspect", &e.to_string(), Exit::Invalid),
    }
}

fn cmd_render_worker<O: Write, E: Write>(out: &mut O, _err: &mut E) -> Exit {
    use distinct_assets::render;
    use std::io::Read;

    let mut input = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut input) {
        let _ = writeln!(out, "{{\"error\":\"stdin read: {e}\"}}");
        return Exit::Invalid;
    }

    let job: render::RenderJob = match serde_json::from_str(&input) {
        Ok(j) => j,
        Err(e) => {
            let _ = writeln!(out, "{{\"error\":\"invalid job JSON: {e}\"}}");
            return Exit::Invalid;
        }
    };

    match render::execute_job(&job) {
        Ok(manifest) => {
            let json = serde_json::to_string(&manifest).unwrap_or_default();
            let _ = writeln!(out, "{json}");
            Exit::Ok
        }
        Err(e) => {
            let _ = writeln!(out, "{{\"error\":\"{e}\"}}");
            Exit::Unavailable
        }
    }
}

fn dir_non_empty(path: &std::path::Path) -> bool {
    std::fs::read_dir(path)
        .map(|mut e| e.next().is_some())
        .unwrap_or(false)
}

/// Default proof renders when no `--profile` is given.
fn default_render_profile() -> Vec<distinct_assets::render::RenderSpec> {
    use distinct_assets::render::RenderSpec;
    [(128, "transparent"), (256, "transparent"), (512, "#ffffff")]
        .into_iter()
        .map(|(size_px, bg)| RenderSpec {
            size_px,
            background: bg.into(),
        })
        .collect()
}

/// Parses and validates a render profile: `{"renders":[{"size_px":N,"background":"..."}]}`.
fn load_render_profile(path: &str) -> Result<Vec<distinct_assets::render::RenderSpec>, String> {
    use distinct_assets::render::{RenderSpec, MAX_RENDERS, MAX_SIDE_PX};

    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Profile {
        renders: Vec<RenderSpec>,
    }

    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    let profile: Profile =
        serde_json::from_str(&text).map_err(|e| format!("invalid profile {path}: {e}"))?;
    let renders = profile.renders;
    if renders.is_empty() || renders.len() > MAX_RENDERS {
        return Err(format!(
            "profile must have 1..={MAX_RENDERS} renders, got {}",
            renders.len()
        ));
    }
    for r in &renders {
        if r.size_px < 1 || r.size_px > MAX_SIDE_PX {
            return Err(format!(
                "size_px {} out of range 1..={MAX_SIDE_PX}",
                r.size_px
            ));
        }
        if u64::from(r.size_px) * u64::from(r.size_px) > 16_000_000 {
            return Err(format!("size_px {} exceeds 16 megapixels", r.size_px));
        }
        let bg = r.background.as_str();
        let valid_hex = bg
            .strip_prefix('#')
            .is_some_and(|h| h.len() == 6 && h.bytes().all(|b| b.is_ascii_hexdigit()));
        if bg != "transparent" && !valid_hex {
            return Err(format!(
                "background '{bg}' must be 'transparent' or '#rrggbb'"
            ));
        }
    }
    Ok(renders)
}

/// Copies rendered PNGs from the proofs dir into the user-visible `--out` dir.
fn copy_renders(
    manifest: &distinct_assets::render::RenderManifest,
    proofs_dir: &str,
    out_dir: &std::path::Path,
) -> Result<(), String> {
    std::fs::create_dir_all(out_dir).map_err(|e| format!("create {}: {e}", out_dir.display()))?;
    for o in &manifest.outputs {
        // Filenames are generated by the renderer; refuse anything path-like.
        if o.file.contains(['/', '\\']) || o.file.contains("..") {
            return Err(format!("unsafe render filename: {}", o.file));
        }
        let from = std::path::Path::new(proofs_dir).join(&o.file);
        let to = out_dir.join(&o.file);
        std::fs::copy(&from, &to)
            .map_err(|e| format!("copy {} -> {}: {e}", from.display(), to.display()))?;
    }
    Ok(())
}

fn cmd_logo_proof<O: Write, E: Write>(args: &CliArgs, out: &mut O, err: &mut E) -> Exit {
    use distinct_core::store::BrandWorkspace;

    let brand = match dispatch::require_brand(args) {
        Ok(b) => b,
        Err(msg) => return write_error(out, "logo-proof", &msg, Exit::Invalid),
    };

    let file_path = args
        .rest
        .iter()
        .position(|a| a == "--file")
        .and_then(|i| args.rest.get(i + 1))
        .map(|s| s.as_str());
    let file_path = match file_path {
        Some(p) => p,
        None => return write_error(out, "logo-proof", "--file <path> required", Exit::Invalid),
    };

    let out_dir_str = args
        .rest
        .iter()
        .position(|a| a == "--out")
        .and_then(|i| args.rest.get(i + 1))
        .map(|s| s.as_str());
    let out_dir_str = match out_dir_str {
        Some(d) => d,
        None => return write_error(out, "logo-proof", "--out <dir> required", Exit::Invalid),
    };
    let out_copy_dir = std::path::PathBuf::from(out_dir_str);
    if out_copy_dir
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return write_error(
            out,
            "logo-proof",
            "--out must not contain '..' components",
            Exit::Invalid,
        );
    }
    let force = args.rest.iter().any(|a| a == "--force");
    if !force && out_copy_dir.is_dir() && dir_non_empty(&out_copy_dir) {
        return write_error(
            out,
            "logo-proof",
            "output directory exists and is not empty; use --force to overwrite",
            Exit::Invalid,
        );
    }

    let renders = match args
        .rest
        .iter()
        .position(|a| a == "--profile")
        .and_then(|i| args.rest.get(i + 1))
    {
        Some(path) => match load_render_profile(path) {
            Ok(r) => r,
            Err(msg) => return write_error(out, "logo-proof", &msg, Exit::Invalid),
        },
        None if args.rest.iter().any(|a| a == "--profile") => {
            return write_error(
                out,
                "logo-proof",
                "--profile requires a value",
                Exit::Invalid,
            )
        }
        None => default_render_profile(),
    };

    let ws = BrandWorkspace::new(std::path::Path::new(&args.workspace), brand);
    if !ws.is_initialized() {
        return write_error(out, "logo-proof", "brand not initialized", Exit::Invalid);
    }

    // Check that the latest inspection verdict is `inspected`
    let svg_data = match std::fs::read(file_path) {
        Ok(d) => d,
        Err(e) => {
            return write_error(
                out,
                "logo-proof",
                &format!("cannot read {file_path}: {e}"),
                Exit::Invalid,
            )
        }
    };
    let svg_hash = distinct_core::hash::hash_bytes(&svg_data);

    // Find the latest logo_finding for this SVG hash
    let objects = match ws.list_objects(Some(distinct_core::record::RecordKind::LogoFinding)) {
        Ok(o) => o,
        Err(e) => return write_error(out, "logo-proof", &e.to_string(), Exit::Invalid),
    };

    let latest_finding = objects
        .iter()
        .find(|obj| obj["data"]["file_hash"].as_str() == Some(svg_hash.as_hex()));

    let verdict = latest_finding
        .and_then(|o| o["data"]["verdict"].as_str())
        .unwrap_or("none");

    if verdict != "inspected" {
        return write_error(
            out,
            "logo-proof",
            &format!("latest inspection verdict is '{verdict}', must be 'inspected' before proof"),
            Exit::Unavailable,
        );
    }

    let proofs_dir = format!(
        "{}/.distinct/{}/proofs/{}",
        args.workspace,
        brand,
        &svg_hash.as_hex()[..12]
    );

    use distinct_assets::render;
    let job = render::RenderJob {
        svg_path: file_path.to_string(),
        out_dir: proofs_dir.clone(),
        renders,
    };

    match spawn_render_worker_with_timeout(&job) {
        WorkerOutcome::Ok(manifest, os_sandbox) => {
            if let Err(msg) = copy_renders(&manifest, &proofs_dir, &out_copy_dir) {
                return write_error(out, "logo-proof", &msg, Exit::Invalid);
            }
            write_proof_record(
                out,
                brand,
                &ws,
                &svg_hash,
                &manifest,
                &proofs_dir,
                os_sandbox,
            )
        }
        WorkerOutcome::SandboxFailed(detail) => {
            let _ = writeln!(err, "os_sandbox_failed: {}", detail.trim());
            write_error(out, "logo-proof", "os_sandbox_failed", Exit::Unavailable)
        }
        WorkerOutcome::Timeout => {
            write_error(out, "logo-proof", "render_timeout", Exit::Unavailable)
        }
        WorkerOutcome::MemoryLimit => write_error(
            out,
            "logo-proof",
            "render_memory_limit_exceeded",
            Exit::Unavailable,
        ),
        WorkerOutcome::Failed(msg) => write_error(
            out,
            "logo-proof",
            &format!("render-worker failed: {msg}"),
            Exit::Unavailable,
        ),
        WorkerOutcome::SpawnFailed(msg) => write_error(
            out,
            "logo-proof",
            &format!("spawn render-worker: {msg}"),
            Exit::Unavailable,
        ),
    }
}

#[derive(Debug)]
enum WorkerOutcome {
    /// Manifest plus the OS sandbox actually applied (`seatbelt`, `host_inherited` or `none`).
    Ok(distinct_assets::render::RenderManifest, &'static str),
    Timeout,
    MemoryLimit,
    /// The OS sandbox was requested but could not be applied (fail closed).
    SandboxFailed(String),
    Failed(String),
    SpawnFailed(String),
}

fn spawn_render_worker_with_timeout(job: &distinct_assets::render::RenderJob) -> WorkerOutcome {
    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(e) => return WorkerOutcome::SpawnFailed(e.to_string()),
    };
    let job_json = match serde_json::to_string(job) {
        Ok(j) => j,
        Err(e) => return WorkerOutcome::SpawnFailed(e.to_string()),
    };

    // The sandbox allows writes only to this job's canonical output directory.
    let out_dir = std::path::Path::new(&job.out_dir);
    if let Err(e) = std::fs::create_dir_all(out_dir) {
        return WorkerOutcome::SpawnFailed(format!("create {}: {e}", job.out_dir));
    }
    let canonical_out = match out_dir.canonicalize() {
        Ok(p) => p,
        Err(e) => return WorkerOutcome::SpawnFailed(format!("canonicalize {}: {e}", job.out_dir)),
    };

    // D-N: decide the mode BEFORE the untrusted job runs, from a trusted probe
    // (`sandbox-exec` on /usr/bin/true). Nothing the worker prints can select
    // the unsandboxed path, and the job is never re-run after a sandboxed attempt.
    let host_inherited = seatbelt_nesting_refused(&canonical_out);
    run_worker_attempt(&exe, &job_json, &canonical_out, host_inherited)
}

/// True only when a trivial, input-free `sandbox-exec` run fails with the exact
/// nesting refusal, i.e. this process is already confined by a host sandbox.
#[cfg(target_os = "macos")]
fn seatbelt_nesting_refused(proofs_dir: &std::path::Path) -> bool {
    let mut define = std::ffi::OsString::from("PROOFS_DIR=");
    define.push(proofs_dir);
    let probe = std::process::Command::new("/usr/bin/sandbox-exec")
        .arg("-D")
        .arg(define)
        .arg("-p")
        .arg(SEATBELT_PROFILE)
        .arg("/usr/bin/true")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output();
    match probe {
        Ok(o) => {
            classify_seatbelt_attempt(o.status.success(), &String::from_utf8_lossy(&o.stderr))
                == SeatbeltAttempt::HostInherited
        }
        Err(_) => false,
    }
}

#[cfg(not(target_os = "macos"))]
fn seatbelt_nesting_refused(_proofs_dir: &std::path::Path) -> bool {
    false
}

/// How a `sandbox-exec` attempt ended (D-N).
#[cfg(any(target_os = "macos", test))]
#[derive(Debug, PartialEq, Eq)]
enum SeatbeltAttempt {
    /// The worker ran under the seatbelt profile.
    Applied,
    /// The host already sandboxes this process and refused to nest.
    HostInherited,
    /// Any other failure: fail closed.
    Failed,
}

/// Classifies a seatbelt-wrapped worker run. Host inheritance requires a
/// non-success status AND the exact nesting-refusal text from `sandbox-exec`.
#[cfg(any(target_os = "macos", test))]
fn classify_seatbelt_attempt(success: bool, stderr: &str) -> SeatbeltAttempt {
    if success {
        SeatbeltAttempt::Applied
    } else if stderr.starts_with("sandbox-exec:")
        && stderr.contains("sandbox_apply: Operation not permitted")
    {
        SeatbeltAttempt::HostInherited
    } else {
        SeatbeltAttempt::Failed
    }
}

/// One worker run. `host_inherited` skips `sandbox-exec` (D-N retry path).
fn run_worker_attempt(
    exe: &std::path::Path,
    job_json: &str,
    canonical_out: &std::path::Path,
    host_inherited: bool,
) -> WorkerOutcome {
    use std::io::Read;
    use std::time::Instant;

    let WorkerCommand {
        mut cmd,
        os_sandbox,
    } = if host_inherited {
        let mut cmd = std::process::Command::new(exe);
        cmd.arg("render-worker");
        WorkerCommand {
            cmd,
            os_sandbox: "host_inherited",
        }
    } else {
        build_worker_command(exe, canonical_out)
    };
    let mut child = match cmd
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) if os_sandbox == "seatbelt" => return WorkerOutcome::SandboxFailed(e.to_string()),
        Err(e) => return WorkerOutcome::SpawnFailed(e.to_string()),
    };

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        let _ = stdin.write_all(job_json.as_bytes());
    }

    let stdout_buf = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let stderr_buf = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));

    let stdout_child = stdout_buf.clone();
    let stderr_child = stderr_buf.clone();
    let mut child_stdout = child.stdout.take().unwrap();
    let mut child_stderr = child.stderr.take().unwrap();

    let t_out = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = child_stdout.read_to_end(&mut buf);
        *stdout_child.lock().unwrap() = buf;
    });
    let t_err = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = child_stderr.read_to_end(&mut buf);
        *stderr_child.lock().unwrap() = buf;
    });

    let deadline = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let _ = t_out.join();
                let _ = t_err.join();
                let stdout = std::sync::Arc::try_unwrap(stdout_buf)
                    .unwrap()
                    .into_inner()
                    .unwrap();
                let stderr = std::sync::Arc::try_unwrap(stderr_buf)
                    .unwrap()
                    .into_inner()
                    .unwrap();

                return if status.success() {
                    match serde_json::from_slice(&stdout) {
                        Ok(m) => WorkerOutcome::Ok(m, os_sandbox),
                        Err(e) => WorkerOutcome::Failed(format!("parse manifest: {e}")),
                    }
                } else if status.code() == Some(101) || killed_by_signal(&status) {
                    // Allocation failure aborts the worker (SIGABRT); a panic exits 101.
                    WorkerOutcome::MemoryLimit
                } else {
                    let err = String::from_utf8_lossy(&stderr).to_string();
                    if os_sandbox == "seatbelt" && err.starts_with("sandbox-exec:") {
                        WorkerOutcome::SandboxFailed(err)
                    } else {
                        WorkerOutcome::Failed(err)
                    }
                };
            }
            Ok(None) => {
                if deadline.elapsed().as_secs() >= 10 {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = t_out.join();
                    let _ = t_err.join();
                    return WorkerOutcome::Timeout;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = t_out.join();
                let _ = t_err.join();
                return WorkerOutcome::Failed(e.to_string());
            }
        }
    }
}

/// True when the worker was terminated by a signal (unix only).
fn killed_by_signal(status: &std::process::ExitStatus) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        status.signal().is_some()
    }
    #[cfg(not(unix))]
    {
        let _ = status;
        false
    }
}

/// A worker command plus the OS sandbox that was actually wrapped around it.
struct WorkerCommand {
    cmd: std::process::Command,
    /// `"seatbelt"` only when the command goes through `sandbox-exec`.
    os_sandbox: &'static str,
}

/// Seatbelt profile: no network, no file writes except the `PROOFS_DIR`
/// parameter (and `/dev/null`). The path is passed with `-D`, never
/// interpolated, so it needs no escaping.
#[cfg(any(target_os = "macos", test))]
const SEATBELT_PROFILE: &str = concat!(
    "(version 1)(allow default)(deny network*)(deny file-write*)",
    "(allow file-write* (subpath (param \"PROOFS_DIR\")))",
    "(allow file-write* (literal \"/dev/null\"))"
);

#[cfg(any(target_os = "macos", test))]
fn seatbelt_args(exe: &std::path::Path, proofs_dir: &std::path::Path) -> Vec<std::ffi::OsString> {
    let mut define = std::ffi::OsString::from("PROOFS_DIR=");
    define.push(proofs_dir);
    vec![
        "-D".into(),
        define,
        "-p".into(),
        SEATBELT_PROFILE.into(),
        exe.into(),
        "render-worker".into(),
    ]
}

#[cfg(target_os = "linux")]
fn build_worker_command(exe: &std::path::Path, _proofs_dir: &std::path::Path) -> WorkerCommand {
    let mem_kb = 512 * 1024;
    let shell_cmd = format!("ulimit -v {mem_kb}; exec \"$0\" render-worker");
    let mut cmd = std::process::Command::new("/bin/sh");
    cmd.arg("-c").arg(&shell_cmd).arg(exe);
    WorkerCommand {
        cmd,
        os_sandbox: "none",
    }
}

#[cfg(target_os = "macos")]
fn build_worker_command(exe: &std::path::Path, proofs_dir: &std::path::Path) -> WorkerCommand {
    const SANDBOX_EXEC: &str = "/usr/bin/sandbox-exec";
    if std::path::Path::new(SANDBOX_EXEC).exists() {
        let mut cmd = std::process::Command::new(SANDBOX_EXEC);
        cmd.args(seatbelt_args(exe, proofs_dir));
        return WorkerCommand {
            cmd,
            os_sandbox: "seatbelt",
        };
    }
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("render-worker");
    WorkerCommand {
        cmd,
        os_sandbox: "none",
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn build_worker_command(exe: &std::path::Path, _proofs_dir: &std::path::Path) -> WorkerCommand {
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("render-worker");
    WorkerCommand {
        cmd,
        os_sandbox: "none",
    }
}

fn write_proof_record<O: Write>(
    out: &mut O,
    brand: &str,
    ws: &distinct_core::store::BrandWorkspace,
    svg_hash: &distinct_core::hash::HexHash,
    manifest: &distinct_assets::render::RenderManifest,
    proofs_dir: &str,
    os_sandbox: &str,
) -> Exit {
    let proof_id = format!("proof-{}", &svg_hash.as_hex()[..12]);
    let advisory: Vec<serde_json::Value> = manifest
        .outputs
        .iter()
        .filter_map(|o| {
            o.advisory.as_ref().map(|a| {
                serde_json::json!({
                    "file": o.file,
                    "min_feature_px": a.min_feature_px,
                    "contrast_ratio": a.contrast_ratio,
                })
            })
        })
        .collect();
    let memory_enforcement = if cfg!(target_os = "linux") {
        "in_process_allocator_cap+ulimit"
    } else {
        "in_process_allocator_cap"
    };
    let limitations: Vec<String> = match os_sandbox {
        "none" => vec!["os_sandbox_unavailable".to_string()],
        "host_inherited" => vec!["os_sandbox_host_inherited".to_string()],
        _ => vec![],
    };
    let record = serde_json::json!({
        "schema_version": distinct_core::record::SCHEMA_VERSION,
        "kind": "logo_proof",
        "id": proof_id,
        "brand_id": brand,
        "created_at": distinct_core::store::chrono_now(),
        "created_by": { "kind": "agent", "id": "distinct-rs" },
        "privacy": "public",
        "dependencies": [],
        "limitations": limitations,
        "data": {
            "svg_hash": svg_hash.as_hex(),
            "renderer": manifest.renderer,
            "renders": manifest.outputs,
            "proofs_dir": proofs_dir,
            "advisory": advisory,
            "memory_enforcement": memory_enforcement,
            "os_sandbox": os_sandbox,
        }
    });

    match ws.store_object(&record) {
        Ok(hash) => {
            // WO-15: update STATE so verify stays clean after mutation.
            let mut state = match ws.read_state() {
                Ok(s) => s,
                Err(e) => {
                    return write_error(out, "logo-proof", &e.to_string(), Exit::Invalid);
                }
            };
            if let Err(e) = ws.update_snapshot(&mut state) {
                return write_error(
                    out,
                    "logo-proof",
                    &format!("update snapshot: {e}"),
                    Exit::Invalid,
                );
            }
            if let Err(e) = ws.update_governance_index(&mut state) {
                return write_error(
                    out,
                    "logo-proof",
                    &format!("update governance index: {e}"),
                    Exit::Invalid,
                );
            }
            if let Err(e) = ws.write_state(&state) {
                return write_error(
                    out,
                    "logo-proof",
                    &format!("write state: {e}"),
                    Exit::Invalid,
                );
            }

            let rid = distinct_core::hash::revision_id(&hash);
            let result = serde_json::json!({
                "brand_id": brand,
                "content_hash": hash.as_hex(),
                "revision_id": rid,
                "proof_id": proof_id,
                "renderer": manifest.renderer,
                "render_count": manifest.outputs.len(),
                "svg_hash": svg_hash.as_hex(),
            });
            write_envelope(
                out,
                "logo-proof",
                result,
                limitations,
                vec![proofs_dir.to_string()],
            )
        }
        Err(e) => write_error(out, "logo-proof", &e.to_string(), Exit::Invalid),
    }
}

fn cmd_color_check<O: Write, E: Write>(args: &CliArgs, out: &mut O, _err: &mut E) -> Exit {
    use distinct_assets::colour;
    use distinct_core::store::BrandWorkspace;

    let brand = match dispatch::require_brand(args) {
        Ok(b) => b,
        Err(msg) => return write_error(out, "color-check", &msg, Exit::Invalid),
    };

    let pairs_file = args
        .rest
        .iter()
        .position(|a| a == "--pairs")
        .and_then(|i| args.rest.get(i + 1))
        .map(|s| s.as_str());
    let pairs_file = match pairs_file {
        Some(p) => p,
        None => return write_error(out, "color-check", "--pairs <file> required", Exit::Invalid),
    };

    let ws = BrandWorkspace::new(std::path::Path::new(&args.workspace), brand);
    if !ws.is_initialized() {
        return write_error(out, "color-check", "brand not initialized", Exit::Invalid);
    }

    let pairs_text = match std::fs::read_to_string(pairs_file) {
        Ok(t) => t,
        Err(e) => {
            return write_error(
                out,
                "color-check",
                &format!("cannot read {pairs_file}: {e}"),
                Exit::Invalid,
            )
        }
    };

    // Parse pairs: each line is "fg_hex bg_hex". Lines starting with // are comments.
    // Hex tokens may have a # prefix (#rrggbb or #rgb) or be bare.
    let mut results = Vec::new();
    for (line_no, line) in pairs_text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() < 2 {
            return write_error(
                out,
                "color-check",
                &format!(
                    "line {}: expected 'fg_hex bg_hex', got: {trimmed}",
                    line_no + 1
                ),
                Exit::Invalid,
            );
        }
        let fg = match colour::Colour::from_hex(parts[0]) {
            Ok(c) => c,
            Err(e) => {
                return write_error(
                    out,
                    "color-check",
                    &format!("line {}: {e}", line_no + 1),
                    Exit::Invalid,
                )
            }
        };
        let bg = match colour::Colour::from_hex(parts[1]) {
            Ok(c) => c,
            Err(e) => {
                return write_error(
                    out,
                    "color-check",
                    &format!("line {}: {e}", line_no + 1),
                    Exit::Invalid,
                )
            }
        };
        results.push(colour::check_pair(fg, bg));
    }

    // WO-15: zero valid pairs is an error.
    if results.is_empty() {
        return write_error(
            out,
            "color-check",
            "no valid colour pairs found in input",
            Exit::Invalid,
        );
    }

    // Store a color_check record
    let check_id = format!(
        "color-{}",
        &distinct_core::hash::hash_bytes(pairs_text.as_bytes()).as_hex()[..12]
    );
    let record = serde_json::json!({
        "schema_version": distinct_core::record::SCHEMA_VERSION,
        "kind": "color_check",
        "id": check_id,
        "brand_id": brand,
        "created_at": distinct_core::store::chrono_now(),
        "created_by": { "kind": "agent", "id": "distinct-rs" },
        "privacy": "public",
        "dependencies": [],
        "limitations": ["apca_advisory", "simulation advisory"],
        "data": {
            "pairs_file": pairs_file,
            "results": results,
        }
    });

    match ws.store_object(&record) {
        Ok(hash) => {
            // WO-15: update STATE so verify stays clean after mutation.
            let mut state = match ws.read_state() {
                Ok(s) => s,
                Err(e) => {
                    return write_error(out, "color-check", &e.to_string(), Exit::Invalid);
                }
            };
            if let Err(e) = ws.update_snapshot(&mut state) {
                return write_error(
                    out,
                    "color-check",
                    &format!("update snapshot: {e}"),
                    Exit::Invalid,
                );
            }
            if let Err(e) = ws.update_governance_index(&mut state) {
                return write_error(
                    out,
                    "color-check",
                    &format!("update governance index: {e}"),
                    Exit::Invalid,
                );
            }
            if let Err(e) = ws.write_state(&state) {
                return write_error(
                    out,
                    "color-check",
                    &format!("write state: {e}"),
                    Exit::Invalid,
                );
            }

            let rid = distinct_core::hash::revision_id(&hash);
            let result = serde_json::json!({
                "brand_id": brand,
                "content_hash": hash.as_hex(),
                "revision_id": rid,
                "check_id": check_id,
                "pair_count": results.len(),
            });
            write_envelope(
                out,
                "color-check",
                result,
                vec!["apca_advisory".into(), "simulation advisory".into()],
                vec![],
            )
        }
        Err(e) => write_error(out, "color-check", &e.to_string(), Exit::Invalid),
    }
}

fn cmd_logo_review_packet<O: Write, E: Write>(args: &CliArgs, out: &mut O, _err: &mut E) -> Exit {
    use distinct_core::store::BrandWorkspace;

    let brand = match dispatch::require_brand(args) {
        Ok(b) => b,
        Err(msg) => return write_error(out, "logo-review-packet", &msg, Exit::Invalid),
    };

    let ws = BrandWorkspace::new(std::path::Path::new(&args.workspace), brand);
    if !ws.is_initialized() {
        return write_error(
            out,
            "logo-review-packet",
            "brand not initialized",
            Exit::Invalid,
        );
    }

    // Collect all logo_finding and logo_proof records
    let findings = match ws.list_objects(Some(distinct_core::record::RecordKind::LogoFinding)) {
        Ok(o) => o,
        Err(e) => return write_error(out, "logo-review-packet", &e.to_string(), Exit::Invalid),
    };
    let proofs = match ws.list_objects(Some(distinct_core::record::RecordKind::LogoProof)) {
        Ok(o) => o,
        Err(e) => return write_error(out, "logo-review-packet", &e.to_string(), Exit::Invalid),
    };

    // Extract frozen data: SVG hashes, inspection findings, proof renders
    // No verdict is included — this is a review packet, not a decision
    let svg_hashes: Vec<serde_json::Value> = findings
        .iter()
        .filter_map(|f| f["data"]["file_hash"].as_str())
        .map(|h| serde_json::json!(h))
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let frozen_findings: Vec<serde_json::Value> = findings
        .iter()
        .map(|f| {
            serde_json::json!({
                "id": f["id"],
                "verdict": f["data"]["verdict"],
                "file_hash": f["data"]["file_hash"],
                "element_count": f["data"]["element_count"],
                "unsupported_count": f["data"]["unsupported"].as_array().map(|a| a.len()).unwrap_or(0),
                "issue_count": f["data"]["issues"].as_array().map(|a| a.len()).unwrap_or(0),
                "native_proof": f["data"]["native_proof"],
            })
        })
        .collect();

    let frozen_proofs: Vec<serde_json::Value> = proofs
        .iter()
        .map(|p| {
            serde_json::json!({
                "id": p["id"],
                "svg_hash": p["data"]["svg_hash"],
                "renderer": p["data"]["renderer"],
                "render_count": p["data"]["renders"].as_array().map(|a| a.len()).unwrap_or(0),
            })
        })
        .collect();

    let result = serde_json::json!({
        "brand_id": brand,
        "svg_hashes": svg_hashes,
        "findings": frozen_findings,
        "proofs": frozen_proofs,
        "finding_count": findings.len(),
        "proof_count": proofs.len(),
    });

    write_envelope(
        out,
        "logo-review-packet",
        result,
        vec!["no verdict included; review packet only".into()],
        vec![],
    )
}

fn cmd_test<O: Write, E: Write>(args: &CliArgs, out: &mut O, _err: &mut E) -> Exit {
    use distinct_core::store::BrandWorkspace;

    let brand = match dispatch::require_brand(args) {
        Ok(b) => b,
        Err(msg) => return write_error(out, "test", &msg, Exit::Invalid),
    };

    let sub = args.rest.first().map(|s| s.as_str());
    let sub = match sub {
        Some(s) => s,
        None => {
            return write_error(
                out,
                "test",
                "usage: test plan|packet|import|summarize ...",
                Exit::Invalid,
            )
        }
    };

    let ws = BrandWorkspace::new(std::path::Path::new(&args.workspace), brand);
    if !ws.is_initialized() {
        return write_error(out, "test", "brand not initialized", Exit::Invalid);
    }

    match sub {
        "plan" => cmd_test_plan(args, out, brand, &ws),
        "packet" => cmd_test_packet(args, out, brand, &ws),
        "import" => cmd_test_import(args, out, brand, &ws),
        "summarize" => cmd_test_summarize(args, out, brand, &ws),
        other => write_error(
            out,
            "test",
            &format!("unknown test subcommand: {other}"),
            Exit::Invalid,
        ),
    }
}

fn cmd_test_plan<O: Write>(
    args: &CliArgs,
    out: &mut O,
    brand: &str,
    ws: &distinct_core::store::BrandWorkspace,
) -> Exit {
    use distinct_core::test_bench;

    let file_path = args
        .rest
        .iter()
        .position(|a| a == "--request")
        .and_then(|i| args.rest.get(i + 1))
        .map(|s| s.as_str());
    let file_path = match file_path {
        Some(p) => p,
        None => return write_error(out, "test-plan", "--request <path> required", Exit::Invalid),
    };

    let json_text = match std::fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(e) => {
            return write_error(
                out,
                "test-plan",
                &format!("cannot read {file_path}: {e}"),
                Exit::Invalid,
            )
        }
    };

    if let Err(e) = distinct_core::dup_keys::reject_duplicate_keys(&json_text) {
        return write_error(out, "test-plan", &e.to_string(), Exit::Invalid);
    }

    let request: serde_json::Value = match serde_json::from_str(&json_text) {
        Ok(v) => v,
        Err(e) => {
            return write_error(
                out,
                "test-plan",
                &format!("invalid JSON: {e}"),
                Exit::Invalid,
            )
        }
    };

    let data = match test_bench::validate_test_plan(&request) {
        Ok(d) => d,
        Err(e) => return write_error(out, "test-plan", &e, Exit::Invalid),
    };

    let plan_id = request["id"]
        .as_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            let q = data["question"].as_str().unwrap_or("plan");
            format!("plan-{}", &q[..q.len().min(20)].replace(' ', "-"))
        });

    let record = test_bench::build_test_plan_record(
        brand,
        &plan_id,
        data,
        &distinct_core::store::chrono_now(),
    );

    match ws.store_object(&record) {
        Ok(hash) => {
            // WO-11b: update STATE so verify stays clean after mutation.
            let mut state = match ws.read_state() {
                Ok(s) => s,
                Err(e) => return write_error(out, "test-plan", &e.to_string(), Exit::Invalid),
            };
            if let Err(e) = ws.update_snapshot(&mut state) {
                return write_error(
                    out,
                    "test-plan",
                    &format!("update snapshot: {e}"),
                    Exit::Invalid,
                );
            }
            if let Err(e) = ws.update_governance_index(&mut state) {
                return write_error(
                    out,
                    "test-plan",
                    &format!("update governance index: {e}"),
                    Exit::Invalid,
                );
            }
            if let Err(e) = ws.write_state(&state) {
                return write_error(
                    out,
                    "test-plan",
                    &format!("write state: {e}"),
                    Exit::Invalid,
                );
            }

            let rid = distinct_core::hash::revision_id(&hash);
            let result = serde_json::json!({
                "brand_id": brand,
                "plan_id": plan_id,
                "content_hash": hash.as_hex(),
                "revision_id": rid,
                "lifecycle": "frozen",
            });
            write_envelope(out, "test-plan", result, vec![], vec![])
        }
        Err(e) => write_error(out, "test-plan", &e.to_string(), Exit::Invalid),
    }
}

fn cmd_test_packet<O: Write>(
    args: &CliArgs,
    out: &mut O,
    brand: &str,
    ws: &distinct_core::store::BrandWorkspace,
) -> Exit {
    use distinct_core::test_bench;

    let test_id = args
        .rest
        .iter()
        .position(|a| a == "--test")
        .and_then(|i| args.rest.get(i + 1))
        .map(|s| s.as_str());
    let test_id = match test_id {
        Some(t) => t,
        None => return write_error(out, "test-packet", "--test <id> required", Exit::Invalid),
    };

    match test_bench::build_test_packet(ws, test_id, args.max_bytes) {
        Ok(packet) => {
            let result = serde_json::json!({
                "brand_id": brand,
                "test_id": test_id,
                "packet_hash": packet["packet_hash"],
                "byte_length": packet["byte_length"],
                "body": packet["body"],
            });
            write_envelope(out, "test-packet", result, vec![], vec![])
        }
        Err(e) => write_error(out, "test-packet", &e.to_string(), Exit::Invalid),
    }
}

fn cmd_test_import<O: Write>(
    args: &CliArgs,
    out: &mut O,
    brand: &str,
    ws: &distinct_core::store::BrandWorkspace,
) -> Exit {
    use distinct_core::test_bench;

    let file_path = args
        .rest
        .iter()
        .position(|a| a == "--results")
        .and_then(|i| args.rest.get(i + 1))
        .map(|s| s.as_str());
    let file_path = match file_path {
        Some(p) => p,
        None => {
            return write_error(
                out,
                "test-import",
                "--results <path> required",
                Exit::Invalid,
            )
        }
    };

    let json_text = match std::fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(e) => {
            return write_error(
                out,
                "test-import",
                &format!("cannot read {file_path}: {e}"),
                Exit::Invalid,
            )
        }
    };

    if let Err(e) = distinct_core::dup_keys::reject_duplicate_keys(&json_text) {
        return write_error(out, "test-import", &e.to_string(), Exit::Invalid);
    }

    let input: serde_json::Value = match serde_json::from_str(&json_text) {
        Ok(v) => v,
        Err(e) => {
            return write_error(
                out,
                "test-import",
                &format!("invalid JSON: {e}"),
                Exit::Invalid,
            )
        }
    };

    // Validate the result data.
    let data = input["data"].clone();
    if data.is_null() {
        return write_error(out, "test-import", "missing 'data' field", Exit::Invalid);
    }
    if let Err(e) = test_bench::validate_test_result(&data) {
        return write_error(out, "test-import", &e, Exit::Invalid);
    }

    // WO-15: validate frozen_plan_ref against existing test_plan records.
    let plan_ref = data["frozen_plan_ref"].as_str().unwrap_or("");
    if !plan_ref.is_empty() {
        let plans = match ws.list_objects(Some(distinct_core::record::RecordKind::TestPlan)) {
            Ok(p) => p,
            Err(e) => {
                return write_error(out, "test-import", &e.to_string(), Exit::Invalid);
            }
        };
        let found = plans.iter().any(|p| p["id"].as_str() == Some(plan_ref));
        if !found {
            return write_error(
                out,
                "test-import",
                &format!("frozen_plan_ref '{plan_ref}' does not match any stored test_plan"),
                Exit::Invalid,
            );
        }
    }

    let result_id = input["id"]
        .as_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            let plan_ref = data["frozen_plan_ref"].as_str().unwrap_or("unknown");
            format!("result-{}", &plan_ref[..plan_ref.len().min(12)])
        });

    let record = test_bench::build_test_result_record(
        brand,
        &result_id,
        data,
        &distinct_core::store::chrono_now(),
    );

    match ws.store_object(&record) {
        Ok(hash) => {
            // WO-11b: update STATE so verify stays clean after mutation.
            let mut state = match ws.read_state() {
                Ok(s) => s,
                Err(e) => return write_error(out, "test-import", &e.to_string(), Exit::Invalid),
            };
            if let Err(e) = ws.update_snapshot(&mut state) {
                return write_error(
                    out,
                    "test-import",
                    &format!("update snapshot: {e}"),
                    Exit::Invalid,
                );
            }
            if let Err(e) = ws.update_governance_index(&mut state) {
                return write_error(
                    out,
                    "test-import",
                    &format!("update governance index: {e}"),
                    Exit::Invalid,
                );
            }
            if let Err(e) = ws.write_state(&state) {
                return write_error(
                    out,
                    "test-import",
                    &format!("write state: {e}"),
                    Exit::Invalid,
                );
            }

            let rid = distinct_core::hash::revision_id(&hash);
            let result = serde_json::json!({
                "brand_id": brand,
                "result_id": result_id,
                "content_hash": hash.as_hex(),
                "revision_id": rid,
                "execution": input["data"]["execution"],
            });
            write_envelope(out, "test-import", result, vec![], vec![])
        }
        Err(e) => write_error(out, "test-import", &e.to_string(), Exit::Invalid),
    }
}

fn cmd_test_summarize<O: Write>(
    args: &CliArgs,
    out: &mut O,
    brand: &str,
    ws: &distinct_core::store::BrandWorkspace,
) -> Exit {
    use distinct_core::test_bench;

    let run_id = args
        .rest
        .iter()
        .position(|a| a == "--run")
        .and_then(|i| args.rest.get(i + 1))
        .map(|s| s.as_str());
    let run_id = match run_id {
        Some(r) => r,
        None => return write_error(out, "test-summarize", "--run <id> required", Exit::Invalid),
    };

    match test_bench::summarize_run(ws, run_id) {
        Ok(summary) => {
            let mut result = summary;
            result["brand_id"] = serde_json::json!(brand);
            write_envelope(out, "test-summarize", result, vec![], vec![])
        }
        Err(e) => write_error(out, "test-summarize", &e.to_string(), Exit::Invalid),
    }
}

fn cmd_patch<O: Write, E: Write>(args: &CliArgs, out: &mut O, _err: &mut E) -> Exit {
    use distinct_core::patch;
    use distinct_core::store::BrandWorkspace;

    let brand = match dispatch::require_brand(args) {
        Ok(b) => b,
        Err(msg) => return write_error(out, "patch", &msg, Exit::Invalid),
    };

    let plan_path = args
        .rest
        .iter()
        .position(|a| a == "--plan")
        .and_then(|i| args.rest.get(i + 1))
        .map(|s| s.as_str());
    let plan_path = match plan_path {
        Some(p) => p,
        None => return write_error(out, "patch", "--plan <path> required", Exit::Invalid),
    };

    let dry_run = args.rest.iter().any(|a| a == "--dry-run");
    let apply = args.rest.iter().any(|a| a == "--apply");

    if !dry_run && !apply {
        return write_error(out, "patch", "specify --dry-run or --apply", Exit::Invalid);
    }
    if dry_run && apply {
        return write_error(
            out,
            "patch",
            "--dry-run and --apply are mutually exclusive",
            Exit::Invalid,
        );
    }

    let json_text = match std::fs::read_to_string(plan_path) {
        Ok(c) => c,
        Err(e) => {
            return write_error(
                out,
                "patch",
                &format!("cannot read {plan_path}: {e}"),
                Exit::Invalid,
            )
        }
    };

    if let Err(e) = distinct_core::dup_keys::reject_duplicate_keys(&json_text) {
        return write_error(out, "patch", &e.to_string(), Exit::Invalid);
    }

    let plan_json: serde_json::Value = match serde_json::from_str(&json_text) {
        Ok(v) => v,
        Err(e) => return write_error(out, "patch", &format!("invalid JSON: {e}"), Exit::Invalid),
    };

    let plan = match patch::parse_patch_plan(&plan_json) {
        Ok(p) => p,
        Err(e) => return write_error(out, "patch", &e, Exit::Invalid),
    };

    if plan.brand_id != brand {
        return write_error(
            out,
            "patch",
            &format!(
                "plan brand_id '{}' does not match --brand '{}'",
                plan.brand_id, brand
            ),
            Exit::Invalid,
        );
    }

    let ws = BrandWorkspace::new(std::path::Path::new(&args.workspace), brand);
    if !ws.is_initialized() {
        return write_error(out, "patch", "brand not initialized", Exit::Invalid);
    }

    if dry_run {
        match patch::preview_patch(&plan, &ws) {
            Ok(preview) => {
                let result = serde_json::json!({
                    "brand_id": brand,
                    "mode": "dry-run",
                    "operation_count": preview.operation_count,
                    "adds": preview.adds,
                    "replaces": preview.replaces,
                    "removes": preview.removes,
                    "affected_kinds": preview.affected_kinds,
                    "affected_ids": preview.affected_ids,
                    "warnings": preview.warnings,
                });
                write_envelope(out, "patch", result, vec![], vec![])
            }
            Err(e) => write_error(out, "patch", &e.to_string(), Exit::Invalid),
        }
    } else {
        // --apply: never approves anything, only stores records.

        // WO-11b: --expected-state CAS guard.
        let expected_state = args
            .rest
            .iter()
            .position(|a| a == "--expected-state")
            .and_then(|i| args.rest.get(i + 1))
            .map(|s| s.to_string());
        if let Some(ref expected) = expected_state {
            let state = match ws.read_state() {
                Ok(s) => s,
                Err(e) => return write_error(out, "patch", &e.to_string(), Exit::Invalid),
            };
            let current = state.content_snapshot_hash.as_deref().unwrap_or("");
            if current != expected {
                return write_error(
                    out,
                    "patch",
                    &format!(
                        "--expected-state mismatch: current={}, expected={}",
                        if current.is_empty() {
                            "<none>"
                        } else {
                            current
                        },
                        expected
                    ),
                    Exit::Invalid,
                );
            }
        }

        match patch::apply_patch(&plan, &ws, &distinct_core::store::chrono_now()) {
            Ok(applied) => {
                // WO-11b: update STATE so verify stays clean after mutation.
                let mut state = match ws.read_state() {
                    Ok(s) => s,
                    Err(e) => return write_error(out, "patch", &e.to_string(), Exit::Invalid),
                };
                if let Err(e) = ws.update_snapshot(&mut state) {
                    return write_error(
                        out,
                        "patch",
                        &format!("update snapshot: {e}"),
                        Exit::Invalid,
                    );
                }
                if let Err(e) = ws.update_governance_index(&mut state) {
                    return write_error(
                        out,
                        "patch",
                        &format!("update governance index: {e}"),
                        Exit::Invalid,
                    );
                }
                if let Err(e) = ws.write_state(&state) {
                    return write_error(out, "patch", &format!("write state: {e}"), Exit::Invalid);
                }

                let result = serde_json::json!({
                    "brand_id": brand,
                    "mode": "apply",
                    "applied": applied.applied,
                    "hashes": applied.hashes,
                    "warnings": applied.warnings,
                    "approval_required": true,
                });
                write_envelope(
                    out,
                    "patch",
                    result,
                    vec![
                        "patch --apply does not approve; approval is a separate governance step"
                            .to_string(),
                    ],
                    vec![],
                )
            }
            Err(e) => write_error(out, "patch", &e.to_string(), Exit::Invalid),
        }
    }
}

fn cmd_review_packet<O: Write, E: Write>(args: &CliArgs, out: &mut O, _err: &mut E) -> Exit {
    use distinct_core::record::Privacy;
    use distinct_core::review_packet;
    use distinct_core::store::BrandWorkspace;

    let brand = match dispatch::require_brand(args) {
        Ok(b) => b,
        Err(msg) => return write_error(out, "review-packet", &msg, Exit::Invalid),
    };

    let scope = args
        .rest
        .iter()
        .position(|a| a == "--scope")
        .and_then(|i| args.rest.get(i + 1))
        .map(|s| s.as_str());
    let scope = match scope {
        Some(s) => s,
        None => return write_error(out, "review-packet", "--scope <id> required", Exit::Invalid),
    };

    let out_path = args
        .rest
        .iter()
        .position(|a| a == "--out")
        .and_then(|i| args.rest.get(i + 1))
        .map(|s| s.as_str());
    let out_path = match out_path {
        Some(p) => p,
        None => return write_error(out, "review-packet", "--out <path> required", Exit::Invalid),
    };

    // Determine allowed privacy from --allow or default to public+internal.
    let allow_str = args
        .rest
        .iter()
        .position(|a| a == "--allow")
        .and_then(|i| args.rest.get(i + 1))
        .map(|s| s.as_str());
    let allowed_privacy = match allow_str {
        Some(a) => review_packet::parse_privacy_list(a),
        None => vec![Privacy::Public, Privacy::Internal],
    };

    let ws = BrandWorkspace::new(std::path::Path::new(&args.workspace), brand);
    if !ws.is_initialized() {
        return write_error(out, "review-packet", "brand not initialized", Exit::Invalid);
    }

    match review_packet::build_review_packet(&ws, scope, out_path, &allowed_privacy, args.max_bytes)
    {
        Ok(result) => {
            let response = serde_json::json!({
                "brand_id": brand,
                "scope": scope,
                "packet_hash": result.packet_hash.as_hex(),
                "record_count": result.record_count,
                "out_path": result.out_path,
                "byte_length": result.byte_length,
            });
            write_envelope(
                out,
                "review-packet",
                response,
                vec!["builder rationale fields excluded".to_string()],
                vec![out_path.to_string()],
            )
        }
        Err(e) => write_error(out, "review-packet", &e.to_string(), Exit::Invalid),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn version_line() {
        let (code, out, _) = run_args(&["--version"]);
        assert_eq!(code, 0);
        assert_eq!(out, "distinct-rs 0.2.0\n");
    }

    #[test]
    fn help_works() {
        let (code, out, _) = run_args(&["--help"]);
        assert_eq!(code, 0);
        assert!(out.contains("distinct-rs"));
    }

    #[test]
    fn protocol_probe() {
        let (code, out, _) = run_args(&["protocol"]);
        assert_eq!(code, 0);
        let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
        assert_eq!(val["protocol_version"], "distinct/1.0");
    }

    #[test]
    fn unknown_command_rejected() {
        let (code, _, err) = run_args(&["bogus"]);
        assert_eq!(code, 3);
        assert!(err.contains("unknown command 'bogus'"));
    }

    #[test]
    fn capabilities_json() {
        let (code, out, _) = run_args(&["capabilities"]);
        assert_eq!(code, 0);
        let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
        assert_eq!(val["protocol_version"], "distinct/1.0");
        assert!(val["result"]["native"]["commands"].is_array());
    }

    #[test]
    fn status_requires_brand() {
        let (code, out, _err) = run_args(&["status"]);
        assert_eq!(code, 3);
        // Error goes to JSON envelope on stdout, not stderr
        let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
        assert!(val["error"].as_str().unwrap().contains("--brand"));
    }

    #[test]
    fn status_without_init() {
        let (code, out, _) = run_args(&[
            "status",
            "--brand",
            "acme",
            "--workspace",
            "/tmp/distinct-test-nonexistent",
        ]);
        assert_eq!(code, 0);
        let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
        assert_eq!(val["result"]["status"], "not_initialized");
    }

    #[test]
    fn init_creates_workspace() {
        let dir = std::env::temp_dir().join("distinct-test-init");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let (code, out, _) = run_args(&[
            "init",
            "--brand",
            "testbrand",
            "--workspace",
            &dir.to_string_lossy(),
            "--name",
            "Test Brand",
        ]);
        assert_eq!(code, 0);
        let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
        assert_eq!(val["result"]["status"], "initialized");

        // STATE.json should exist
        let state_path = dir.join(".distinct/testbrand/STATE.json");
        assert!(state_path.exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn init_idempotent() {
        let dir = std::env::temp_dir().join("distinct-test-init-idem");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        run_args(&[
            "init",
            "--brand",
            "testbrand",
            "--workspace",
            &dir.to_string_lossy(),
        ]);
        let (code, out, _) = run_args(&[
            "init",
            "--brand",
            "testbrand",
            "--workspace",
            &dir.to_string_lossy(),
        ]);
        assert_eq!(code, 0);
        let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
        assert_eq!(val["result"]["status"], "already_initialized");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn invalid_brand_rejected() {
        let (code, _, _) = run_args(&["status", "--brand", "INVALID"]);
        assert_eq!(code, 3);
    }

    #[test]
    fn cli_logo_inspect_stores_finding() {
        let dir = std::env::temp_dir().join(format!(
            "distinct-test-logo-inspect-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // Initialize brand
        let (code, _, _) = run_args(&[
            "init",
            "--brand",
            "acme",
            "--workspace",
            &dir.to_string_lossy(),
        ]);
        assert_eq!(code, 0);

        // Write a test SVG fixture
        let svg_path = dir.join("test_logo.svg");
        std::fs::write(
            &svg_path,
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
  <rect x="10" y="10" width="80" height="80" fill="blue"/>
</svg>"#,
        )
        .unwrap();

        // Run logo-inspect
        let (code, out, _) = run_args(&[
            "logo-inspect",
            "--brand",
            "acme",
            "--workspace",
            &dir.to_string_lossy(),
            "--file",
            &svg_path.to_string_lossy(),
        ]);
        assert_eq!(code, 0, "logo-inspect should succeed");

        let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
        assert_eq!(val["result"]["brand_id"], "acme");
        assert_eq!(val["result"]["verdict"], "inspected");
        assert_eq!(val["result"]["native_proof"], "unavailable");
        assert!(val["result"]["content_hash"].as_str().is_some());
        assert!(val["result"]["finding_id"].as_str().is_some());

        // Verify the record was stored
        let objects_dir = dir.join(".distinct/acme/store/objects");
        let entries: Vec<_> = std::fs::read_dir(&objects_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert!(!entries.is_empty(), "at least one object should be stored");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cli_logo_proof_refuses_without_inspection() {
        let dir = std::env::temp_dir().join(format!(
            "distinct-test-proof-refuse-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let (code, _, _) = run_args(&[
            "init",
            "--brand",
            "acme",
            "--workspace",
            &dir.to_string_lossy(),
        ]);
        assert_eq!(code, 0);

        let svg_path = dir.join("test_logo.svg");
        std::fs::write(
            &svg_path,
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
  <rect x="10" y="10" width="80" height="80" fill="blue"/>
</svg>"#,
        )
        .unwrap();

        let (code, out, _) = run_args(&[
            "logo-proof",
            "--brand",
            "acme",
            "--workspace",
            &dir.to_string_lossy(),
            "--file",
            &svg_path.to_string_lossy(),
            "--out",
            &dir.join("proofs").to_string_lossy(),
        ]);
        assert_eq!(code, 2, "logo-proof should refuse without inspection");
        let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
        assert!(
            val["error"].as_str().unwrap().contains("inspected"),
            "error should mention inspected verdict requirement"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cli_color_check_stores_record() {
        let dir = std::env::temp_dir().join(format!(
            "distinct-test-color-check-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let (code, _, _) = run_args(&[
            "init",
            "--brand",
            "acme",
            "--workspace",
            &dir.to_string_lossy(),
        ]);
        assert_eq!(code, 0);

        let pairs_path = dir.join("pairs.txt");
        std::fs::write(&pairs_path, "000000 ffffff\n777777 ffffff\n").unwrap();

        let (code, out, _) = run_args(&[
            "color-check",
            "--brand",
            "acme",
            "--workspace",
            &dir.to_string_lossy(),
            "--pairs",
            &pairs_path.to_string_lossy(),
        ]);
        assert_eq!(code, 0, "color-check should succeed");
        let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
        assert_eq!(val["result"]["pair_count"], 2);
        assert!(val["result"]["check_id"].as_str().is_some());

        let objects_dir = dir.join(".distinct/acme/store/objects");
        let entries: Vec<_> = std::fs::read_dir(&objects_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert!(!entries.is_empty(), "color_check record should be stored");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cli_logo_review_packet_has_no_top_level_verdict() {
        let dir = std::env::temp_dir().join(format!(
            "distinct-test-review-packet-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let (code, _, _) = run_args(&[
            "init",
            "--brand",
            "acme",
            "--workspace",
            &dir.to_string_lossy(),
        ]);
        assert_eq!(code, 0);

        let svg_path = dir.join("test_logo.svg");
        std::fs::write(
            &svg_path,
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
  <rect x="10" y="10" width="80" height="80" fill="blue"/>
</svg>"#,
        )
        .unwrap();

        run_args(&[
            "logo-inspect",
            "--brand",
            "acme",
            "--workspace",
            &dir.to_string_lossy(),
            "--file",
            &svg_path.to_string_lossy(),
        ]);

        let (code, out, _) = run_args(&[
            "logo-review-packet",
            "--brand",
            "acme",
            "--workspace",
            &dir.to_string_lossy(),
        ]);
        assert_eq!(code, 0, "logo-review-packet should succeed");
        let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
        assert!(
            val["result"].get("verdict").is_none(),
            "review-packet must not have a top-level verdict field"
        );
        assert!(val["result"]["findings"].is_array());
        assert!(val["result"]["proofs"].is_array());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn seatbelt_args_pass_proofs_dir_as_parameter() {
        let exe = std::path::Path::new("/opt/distinct-rs");
        let proofs = std::path::Path::new("/work/a \"b\") (allow file-write*)/proofs");
        let args: Vec<String> = seatbelt_args(exe, proofs)
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args[0], "-D");
        assert_eq!(
            args[1],
            "PROOFS_DIR=/work/a \"b\") (allow file-write*)/proofs"
        );
        assert_eq!(args[2], "-p");
        let profile = &args[3];
        assert!(profile.contains("(deny file-write*)"));
        assert!(profile.contains("(subpath (param \"PROOFS_DIR\"))"));
        assert!(!profile.contains("/work"), "path must not be interpolated");
        assert!(!profile.contains("/opt"), "exe must not be interpolated");
        assert_eq!(&args[4..], ["/opt/distinct-rs", "render-worker"]);
    }

    #[test]
    fn classify_seatbelt_attempt_distinguishes_host_inheritance() {
        let refusal = "sandbox-exec: sandbox_apply: Operation not permitted\n";
        assert_eq!(
            classify_seatbelt_attempt(false, refusal),
            SeatbeltAttempt::HostInherited
        );
        assert_eq!(
            classify_seatbelt_attempt(false, "sandbox-exec: bad profile\n"),
            SeatbeltAttempt::Failed
        );
        // Refusal text alone is not enough without the sandbox-exec prefix.
        assert_eq!(
            classify_seatbelt_attempt(false, "worker: sandbox_apply: Operation not permitted"),
            SeatbeltAttempt::Failed
        );
        assert_eq!(
            classify_seatbelt_attempt(false, ""),
            SeatbeltAttempt::Failed
        );
        assert_eq!(
            classify_seatbelt_attempt(true, refusal),
            SeatbeltAttempt::Applied
        );
    }

    #[test]
    fn spawn_timeout_kills_slow_child() {
        use std::time::Instant;
        let start = Instant::now();
        let outcome = spawn_render_worker_with_timeout(&distinct_assets::render::RenderJob {
            svg_path: "/dev/null".into(),
            out_dir: "/tmp".into(),
            renders: vec![],
        });
        let elapsed = start.elapsed();
        match outcome {
            WorkerOutcome::Timeout
            | WorkerOutcome::Failed(_)
            | WorkerOutcome::SandboxFailed(_)
            | WorkerOutcome::SpawnFailed(_) => {}
            other => panic!("expected timeout/fail, got {other:?}"),
        }
        assert!(
            elapsed.as_secs() < 20,
            "must return well before 20s, took {:.1}s",
            elapsed.as_secs_f64()
        );
    }
}
