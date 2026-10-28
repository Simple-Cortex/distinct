//! WO-11 integration tests: test bench, patch, review-packet.
//!
//! These tests run the real binary against a temporary workspace.

use std::path::{Path, PathBuf};
use std::process::Command;

fn binary() -> PathBuf {
    let mut p = std::env::current_exe().unwrap();
    p.pop(); // remove test binary name
    p.pop(); // remove deps/
    p.push("distinct-rs");
    p
}

/// Unique per test: the clock alone collides when tests run in parallel
/// (macOS reports microseconds), and a collision lets one test delete another's workspace.
fn rand_suffix() -> String {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::SystemTime;
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let d = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    format!(
        "{}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
        d.as_nanos()
    )
}

fn init_workspace(label: &str) -> (PathBuf, PathBuf) {
    let ws = std::env::temp_dir().join(format!("distinct-wo11-{label}-{}", rand_suffix()));
    let _ = std::fs::remove_dir_all(&ws);
    std::fs::create_dir_all(&ws).unwrap();
    let status = Command::new(binary())
        .args([
            "init",
            "--brand",
            label,
            "--workspace",
            ws.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(status.status.success(), "init failed");
    (ws.clone(), ws)
}

fn run_cmd_with_brand(ws: &Path, brand: &str, args: &[&str]) -> (i32, String) {
    let mut full_args = vec!["--workspace", ws.to_str().unwrap(), "--brand", brand];
    full_args.extend_from_slice(args);
    let output = Command::new(binary()).args(&full_args).output().unwrap();
    let code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    (code, stdout)
}

// --- test plan ---

#[test]
fn test_plan_stores_frozen_record() {
    let (ws, _clean) = init_workspace("acme");

    let plan_file = ws.join("plan-request.json");
    std::fs::write(
        &plan_file,
        serde_json::json!({
            "id": "comprehension-1",
            "question": "Do unfamiliar readers understand the offer?",
            "method": "unaided_comprehension",
            "scope": "voice",
            "stimuli_refs": [{"kind": "asset", "id": "homepage-v1"}],
            "stop_condition": "three consistent misunderstandings",
            "facilitator": "host",
        })
        .to_string(),
    )
    .unwrap();

    let (code, out) = run_cmd_with_brand(
        &ws,
        "acme",
        &["test", "plan", "--request", plan_file.to_str().unwrap()],
    );
    assert_eq!(code, 0, "test plan should succeed: {out}");

    let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(val["result"]["plan_id"], "comprehension-1");
    assert_eq!(val["result"]["lifecycle"], "frozen");
    assert!(val["result"]["content_hash"].as_str().is_some());

    let _ = std::fs::remove_dir_all(&ws);
}

#[test]
fn test_plan_rejects_missing_question() {
    let (ws, _clean) = init_workspace("acme");

    let plan_file = ws.join("bad-plan.json");
    std::fs::write(
        &plan_file,
        serde_json::json!({
            "method": "comprehension",
            "scope": "voice",
            "stimuli_refs": [],
            "stop_condition": "stop",
            "facilitator": "host",
        })
        .to_string(),
    )
    .unwrap();

    let (code, out) = run_cmd_with_brand(
        &ws,
        "acme",
        &["test", "plan", "--request", plan_file.to_str().unwrap()],
    );
    assert_eq!(code, 3, "should reject missing question");
    let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert!(val["error"].as_str().unwrap().contains("question"));

    let _ = std::fs::remove_dir_all(&ws);
}

// --- test import ---

#[test]
fn test_import_stores_result() {
    let (ws, _clean) = init_workspace("acme");

    // First store a plan.
    let plan_file = ws.join("plan.json");
    std::fs::write(
        &plan_file,
        serde_json::json!({
            "id": "plan-1",
            "question": "test?",
            "method": "m",
            "scope": "s",
            "stimuli_refs": [],
            "stop_condition": "stop",
            "facilitator": "f",
        })
        .to_string(),
    )
    .unwrap();
    let (code, _) = run_cmd_with_brand(
        &ws,
        "acme",
        &["test", "plan", "--request", plan_file.to_str().unwrap()],
    );
    assert_eq!(code, 0);

    // Now import a result.
    let result_file = ws.join("result.json");
    std::fs::write(
        &result_file,
        serde_json::json!({
            "id": "result-1",
            "data": {
                "frozen_plan_ref": "plan-1",
                "execution": "completed",
                "observations": [{"text": "participant confused offer with competitor"}],
                "interpretation": "challenges",
            }
        })
        .to_string(),
    )
    .unwrap();

    let (code, out) = run_cmd_with_brand(
        &ws,
        "acme",
        &["test", "import", "--results", result_file.to_str().unwrap()],
    );
    assert_eq!(code, 0, "test import should succeed: {out}");

    let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(val["result"]["result_id"], "result-1");
    assert_eq!(val["result"]["execution"], "completed");

    let _ = std::fs::remove_dir_all(&ws);
}

#[test]
fn test_import_rejects_unknown_execution() {
    let (ws, _clean) = init_workspace("acme");

    let result_file = ws.join("bad-result.json");
    std::fs::write(
        &result_file,
        serde_json::json!({
            "id": "result-bad",
            "data": {
                "frozen_plan_ref": "plan-1",
                "execution": "passed",
                "observations": [{"text": "ok"}],
            }
        })
        .to_string(),
    )
    .unwrap();

    let (code, out) = run_cmd_with_brand(
        &ws,
        "acme",
        &["test", "import", "--results", result_file.to_str().unwrap()],
    );
    assert_eq!(code, 3, "should reject unknown execution status");
    let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert!(val["error"].as_str().unwrap().contains("execution"));

    let _ = std::fs::remove_dir_all(&ws);
}

// --- test summarize ---

#[test]
fn test_summarize_counts_results() {
    let (ws, _clean) = init_workspace("acme");

    // Store a plan.
    let plan_file = ws.join("plan.json");
    std::fs::write(
        &plan_file,
        serde_json::json!({
            "id": "plan-sum",
            "question": "test?",
            "method": "m",
            "scope": "s",
            "stimuli_refs": [{"kind": "asset", "id": "a1"}, {"kind": "asset", "id": "a2"}],
            "stop_condition": "stop",
            "facilitator": "f",
        })
        .to_string(),
    )
    .unwrap();
    run_cmd_with_brand(
        &ws,
        "acme",
        &["test", "plan", "--request", plan_file.to_str().unwrap()],
    );

    // Store two results.
    for (i, exec) in ["completed", "blocked"].iter().enumerate() {
        let rf = ws.join(format!("result-{i}.json"));
        let obs = if *exec == "completed" {
            serde_json::json!([{"text": "obs"}])
        } else {
            serde_json::json!([])
        };
        std::fs::write(
            &rf,
            serde_json::json!({
                "id": format!("result-{i}"),
                "data": {
                    "frozen_plan_ref": "plan-sum",
                    "execution": exec,
                    "observations": obs,
                }
            })
            .to_string(),
        )
        .unwrap();
        run_cmd_with_brand(
            &ws,
            "acme",
            &["test", "import", "--results", rf.to_str().unwrap()],
        );
    }

    // Summarize.
    let (code, out) = run_cmd_with_brand(&ws, "acme", &["test", "summarize", "--run", "plan-sum"]);
    assert_eq!(code, 0, "summarize should succeed: {out}");

    let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(val["result"]["total_results"], 2);
    assert_eq!(val["result"]["execution_counts"]["completed"], 1);
    assert_eq!(val["result"]["execution_counts"]["blocked"], 1);

    let _ = std::fs::remove_dir_all(&ws);
}

// --- test packet ---

#[test]
fn test_packet_returns_neutral_stimuli() {
    let (ws, _clean) = init_workspace("acme");

    let plan_file = ws.join("plan.json");
    std::fs::write(
        &plan_file,
        serde_json::json!({
            "id": "plan-pkt",
            "question": "test?",
            "method": "m",
            "scope": "s",
            "stimuli_refs": [{"kind": "asset", "id": "a1"}],
            "stop_condition": "stop",
            "facilitator": "f",
        })
        .to_string(),
    )
    .unwrap();
    run_cmd_with_brand(
        &ws,
        "acme",
        &["test", "plan", "--request", plan_file.to_str().unwrap()],
    );

    let (code, out) = run_cmd_with_brand(&ws, "acme", &["test", "packet", "--test", "plan-pkt"]);
    assert_eq!(code, 0, "test packet should succeed: {out}");

    let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(val["result"]["test_id"], "plan-pkt");
    assert!(val["result"]["packet_hash"].as_str().is_some());
    assert!(val["result"]["body"]["stimuli_refs"].is_array());

    let _ = std::fs::remove_dir_all(&ws);
}

// --- patch ---

#[test]
fn patch_dry_run_shows_preview() {
    let (ws, _clean) = init_workspace("acme");

    let patch_file = ws.join("patch.json");
    std::fs::write(
        &patch_file,
        serde_json::json!({
            "brand_id": "acme",
            "operations": [
                {"op": "add", "kind": "claim", "id": "claim-new", "data": {"text": "fastest"}},
            ],
        })
        .to_string(),
    )
    .unwrap();

    let (code, out) = run_cmd_with_brand(
        &ws,
        "acme",
        &["patch", "--plan", patch_file.to_str().unwrap(), "--dry-run"],
    );
    assert_eq!(code, 0, "patch dry-run should succeed: {out}");

    let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(val["result"]["mode"], "dry-run");
    assert_eq!(val["result"]["adds"], 1);
    assert_eq!(val["result"]["operation_count"], 1);

    let _ = std::fs::remove_dir_all(&ws);
}

#[test]
fn patch_apply_stores_records() {
    let (ws, _clean) = init_workspace("acme");

    let patch_file = ws.join("patch.json");
    std::fs::write(
        &patch_file,
        serde_json::json!({
            "brand_id": "acme",
            "operations": [
                {"op": "add", "kind": "claim", "id": "claim-applied", "data": {"text": "fastest"}},
            ],
        })
        .to_string(),
    )
    .unwrap();

    let (code, out) = run_cmd_with_brand(
        &ws,
        "acme",
        &["patch", "--plan", patch_file.to_str().unwrap(), "--apply"],
    );
    assert_eq!(code, 0, "patch apply should succeed: {out}");

    let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(val["result"]["mode"], "apply");
    assert_eq!(val["result"]["applied"], 1);
    assert_eq!(val["result"]["approval_required"], true);
    assert!(val["limitations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|l| l.as_str().unwrap().contains("does not approve")));

    let _ = std::fs::remove_dir_all(&ws);
}

#[test]
fn patch_apply_never_approves() {
    let (ws, _clean) = init_workspace("acme");

    let patch_file = ws.join("patch.json");
    std::fs::write(
        &patch_file,
        serde_json::json!({
            "brand_id": "acme",
            "operations": [
                {"op": "add", "kind": "claim", "id": "c1", "data": {"text": "test"}},
            ],
        })
        .to_string(),
    )
    .unwrap();

    let (code, out) = run_cmd_with_brand(
        &ws,
        "acme",
        &["patch", "--plan", patch_file.to_str().unwrap(), "--apply"],
    );
    assert_eq!(code, 0);

    let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(val["result"]["approval_required"], true);

    let _ = std::fs::remove_dir_all(&ws);
}

#[test]
fn patch_rejects_both_dry_run_and_apply() {
    let (ws, _clean) = init_workspace("acme");

    let patch_file = ws.join("patch.json");
    std::fs::write(
        &patch_file,
        serde_json::json!({
            "brand_id": "acme",
            "operations": [
                {"op": "add", "kind": "claim", "id": "c1", "data": {"text": "test"}},
            ],
        })
        .to_string(),
    )
    .unwrap();

    let (code, _) = run_cmd_with_brand(
        &ws,
        "acme",
        &[
            "patch",
            "--plan",
            patch_file.to_str().unwrap(),
            "--dry-run",
            "--apply",
        ],
    );
    assert_eq!(code, 3, "should reject both flags");

    let _ = std::fs::remove_dir_all(&ws);
}

#[test]
fn patch_rejects_brand_mismatch() {
    let (ws, _clean) = init_workspace("acme");

    let patch_file = ws.join("patch.json");
    std::fs::write(
        &patch_file,
        serde_json::json!({
            "brand_id": "other",
            "operations": [
                {"op": "add", "kind": "claim", "id": "c1", "data": {"text": "test"}},
            ],
        })
        .to_string(),
    )
    .unwrap();

    let (code, out) = run_cmd_with_brand(
        &ws,
        "acme",
        &["patch", "--plan", patch_file.to_str().unwrap(), "--dry-run"],
    );
    assert_eq!(code, 3, "should reject brand mismatch");
    let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert!(val["error"].as_str().unwrap().contains("brand_id"));

    let _ = std::fs::remove_dir_all(&ws);
}

// --- review-packet ---

#[test]
fn review_packet_writes_file_and_strips_rationale() {
    let (ws, _clean) = init_workspace("acme");

    // Store a record with builder rationale that should be stripped.
    let record_file = ws.join("record.json");
    std::fs::write(
        &record_file,
        serde_json::json!({
            "schema_version": "1.0.0",
            "kind": "claim",
            "id": "claim-1",
            "brand_id": "acme",
            "created_at": "2026-10-27T00:00:00Z",
            "created_by": {"kind": "agent", "id": "distinct-rs"},
            "privacy": "public",
            "dependencies": [],
            "limitations": [],
            "data": {
                "text": "fastest delivery",
                "builder_rationale": "we think this works because...",
            }
        })
        .to_string(),
    )
    .unwrap();
    run_cmd_with_brand(
        &ws,
        "acme",
        &["record-import", "--file", record_file.to_str().unwrap()],
    );

    let out_path = ws.join("review-packet.json");
    let (code, out) = run_cmd_with_brand(
        &ws,
        "acme",
        &[
            "review-packet",
            "--scope",
            "all",
            "--out",
            out_path.to_str().unwrap(),
        ],
    );
    assert_eq!(code, 0, "review-packet should succeed: {out}");
    assert!(out_path.exists(), "review packet file should exist");

    // Verify builder rationale was stripped from the file.
    let packet_content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&out_path).unwrap()).unwrap();
    let records = packet_content["records"].as_array().unwrap();
    for record in records {
        assert!(record.get("builder_rationale").is_none());
        if let Some(data) = record.get("data") {
            assert!(data.get("builder_rationale").is_none());
        }
    }

    let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert!(val["result"]["packet_hash"].as_str().is_some());

    let _ = std::fs::remove_dir_all(&ws);
}

#[test]
fn review_packet_excludes_confidential() {
    let (ws, _clean) = init_workspace("acme");

    // Store a public record and a confidential record.
    let public_file = ws.join("pub-record.json");
    std::fs::write(
        &public_file,
        serde_json::json!({
            "schema_version": "1.0.0",
            "kind": "claim",
            "id": "pub-1",
            "brand_id": "acme",
            "created_at": "2026-10-27T00:00:00Z",
            "created_by": {"kind": "agent", "id": "distinct-rs"},
            "privacy": "public",
            "dependencies": [],
            "limitations": [],
            "data": {"text": "public claim"}
        })
        .to_string(),
    )
    .unwrap();
    let (code, _) = run_cmd_with_brand(
        &ws,
        "acme",
        &["record-import", "--file", public_file.to_str().unwrap()],
    );
    assert_eq!(code, 0, "public record import should succeed");

    let conf_file = ws.join("conf-record.json");
    std::fs::write(
        &conf_file,
        serde_json::json!({
            "schema_version": "1.0.0",
            "kind": "claim",
            "id": "conf-1",
            "brand_id": "acme",
            "created_at": "2026-10-27T00:00:00Z",
            "created_by": {"kind": "agent", "id": "distinct-rs"},
            "privacy": "confidential",
            "dependencies": [],
            "limitations": [],
            "data": {"text": "secret claim"}
        })
        .to_string(),
    )
    .unwrap();
    let (code, _) = run_cmd_with_brand(
        &ws,
        "acme",
        &["record-import", "--file", conf_file.to_str().unwrap()],
    );
    assert_eq!(code, 0, "confidential record import should succeed");

    let out_path = ws.join("review-packet.json");
    let (code, out) = run_cmd_with_brand(
        &ws,
        "acme",
        &[
            "review-packet",
            "--scope",
            "all",
            "--out",
            out_path.to_str().unwrap(),
        ],
    );
    assert_eq!(code, 0, "review-packet should succeed: {out}");

    // Default allow is public+internal, so confidential should be excluded.
    // Only the public record should appear.
    let packet_content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&out_path).unwrap()).unwrap();
    let records = packet_content["records"].as_array().unwrap();
    assert_eq!(records.len(), 1, "only public record should be included");
    assert_eq!(records[0]["id"], "pub-1");
    // Confidential record must not appear.
    assert!(
        records.iter().all(|r| r["id"] != "conf-1"),
        "confidential record must be excluded"
    );

    let _ = std::fs::remove_dir_all(&ws);
}

// --- dup_keys enforcement on new JSON input paths ---

#[test]
fn test_plan_rejects_duplicate_keys() {
    let (ws, _clean) = init_workspace("acme");

    let plan_file = ws.join("dup-plan.json");
    std::fs::write(
        &plan_file,
        r#"{"question":"test?","question":"duplicate","method":"m","scope":"s","stimuli_refs":[],"stop_condition":"stop","facilitator":"f"}"#,
    )
    .unwrap();

    let (code, out) = run_cmd_with_brand(
        &ws,
        "acme",
        &["test", "plan", "--request", plan_file.to_str().unwrap()],
    );
    assert_eq!(code, 3, "should reject duplicate keys");
    let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert!(val["error"].as_str().unwrap().contains("duplicate"));

    let _ = std::fs::remove_dir_all(&ws);
}

#[test]
fn patch_rejects_duplicate_keys() {
    let (ws, _clean) = init_workspace("acme");

    let patch_file = ws.join("dup-patch.json");
    std::fs::write(
        &patch_file,
        r#"{"brand_id":"acme","brand_id":"acme","operations":[{"op":"add","kind":"claim","id":"c1","data":{"text":"t"}}]}"#,
    )
    .unwrap();

    let (code, out) = run_cmd_with_brand(
        &ws,
        "acme",
        &["patch", "--plan", patch_file.to_str().unwrap(), "--dry-run"],
    );
    assert_eq!(code, 3, "should reject duplicate keys");
    let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert!(val["error"].as_str().unwrap().contains("duplicate"));

    let _ = std::fs::remove_dir_all(&ws);
}

// --- WO-11b: STATE consistency after mutating commands ---

#[test]
fn test_plan_verify_clean() {
    let (ws, _clean) = init_workspace("acme");

    let plan_file = ws.join("plan.json");
    std::fs::write(
        &plan_file,
        serde_json::json!({
            "id": "verify-plan-1",
            "question": "test?",
            "method": "m",
            "scope": "s",
            "stimuli_refs": [],
            "stop_condition": "stop",
            "facilitator": "f",
        })
        .to_string(),
    )
    .unwrap();

    let (code, _) = run_cmd_with_brand(
        &ws,
        "acme",
        &["test", "plan", "--request", plan_file.to_str().unwrap()],
    );
    assert_eq!(code, 0, "test plan should succeed");

    // verify must exit 0 after test plan.
    let (code, out) = run_cmd_with_brand(&ws, "acme", &["verify"]);
    assert_eq!(code, 0, "verify should pass after test plan: {out}");
    let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert!(val["result"]["state_valid"].as_bool().unwrap());

    let _ = std::fs::remove_dir_all(&ws);
}

#[test]
fn test_import_verify_clean() {
    let (ws, _clean) = init_workspace("acme");

    // Store a plan first.
    let plan_file = ws.join("plan.json");
    std::fs::write(
        &plan_file,
        serde_json::json!({
            "id": "verify-plan-2",
            "question": "test?",
            "method": "m",
            "scope": "s",
            "stimuli_refs": [],
            "stop_condition": "stop",
            "facilitator": "f",
        })
        .to_string(),
    )
    .unwrap();
    let (code, _) = run_cmd_with_brand(
        &ws,
        "acme",
        &["test", "plan", "--request", plan_file.to_str().unwrap()],
    );
    assert_eq!(code, 0);

    // Import a result.
    let result_file = ws.join("result.json");
    std::fs::write(
        &result_file,
        serde_json::json!({
            "id": "verify-result-1",
            "data": {
                "frozen_plan_ref": "verify-plan-2",
                "execution": "completed",
                "observations": [{"text": "ok"}],
                "interpretation": "challenges",
            }
        })
        .to_string(),
    )
    .unwrap();
    let (code, _) = run_cmd_with_brand(
        &ws,
        "acme",
        &["test", "import", "--results", result_file.to_str().unwrap()],
    );
    assert_eq!(code, 0, "test import should succeed");

    // verify must exit 0 after test import.
    let (code, out) = run_cmd_with_brand(&ws, "acme", &["verify"]);
    assert_eq!(code, 0, "verify should pass after test import: {out}");
    let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert!(val["result"]["state_valid"].as_bool().unwrap());

    let _ = std::fs::remove_dir_all(&ws);
}

#[test]
fn patch_apply_verify_clean() {
    let (ws, _clean) = init_workspace("acme");

    let patch_file = ws.join("patch.json");
    std::fs::write(
        &patch_file,
        serde_json::json!({
            "brand_id": "acme",
            "operations": [
                {"op": "add", "kind": "claim", "id": "claim-v", "data": {"text": "fastest"}},
            ],
        })
        .to_string(),
    )
    .unwrap();

    let (code, _) = run_cmd_with_brand(
        &ws,
        "acme",
        &["patch", "--plan", patch_file.to_str().unwrap(), "--apply"],
    );
    assert_eq!(code, 0, "patch apply should succeed");

    // verify must exit 0 after patch --apply.
    let (code, out) = run_cmd_with_brand(&ws, "acme", &["verify"]);
    assert_eq!(code, 0, "verify should pass after patch --apply: {out}");
    let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert!(val["result"]["state_valid"].as_bool().unwrap());

    let _ = std::fs::remove_dir_all(&ws);
}

#[test]
fn patch_dry_run_leaves_state_unchanged() {
    let (ws, _clean) = init_workspace("acme");

    let state_path = ws.join(".distinct/acme/STATE.json");
    let state_before = std::fs::read_to_string(&state_path).unwrap();

    let patch_file = ws.join("patch.json");
    std::fs::write(
        &patch_file,
        serde_json::json!({
            "brand_id": "acme",
            "operations": [
                {"op": "add", "kind": "claim", "id": "c1", "data": {"text": "test"}},
            ],
        })
        .to_string(),
    )
    .unwrap();

    let (code, _) = run_cmd_with_brand(
        &ws,
        "acme",
        &["patch", "--plan", patch_file.to_str().unwrap(), "--dry-run"],
    );
    assert_eq!(code, 0, "patch dry-run should succeed");

    // STATE must be byte-identical after dry-run.
    let state_after = std::fs::read_to_string(&state_path).unwrap();
    assert_eq!(state_before, state_after, "dry-run must not modify STATE");

    let _ = std::fs::remove_dir_all(&ws);
}

#[test]
fn patch_apply_expected_state_mismatch() {
    let (ws, _clean) = init_workspace("acme");

    let patch_file = ws.join("patch.json");
    std::fs::write(
        &patch_file,
        serde_json::json!({
            "brand_id": "acme",
            "operations": [
                {"op": "add", "kind": "claim", "id": "c1", "data": {"text": "test"}},
            ],
        })
        .to_string(),
    )
    .unwrap();

    let (code, out) = run_cmd_with_brand(
        &ws,
        "acme",
        &[
            "patch",
            "--plan",
            patch_file.to_str().unwrap(),
            "--apply",
            "--expected-state",
            "0000000000000000000000000000000000000000000000000000000000000000",
        ],
    );
    assert_eq!(code, 3, "expected-state mismatch should exit 3: {out}");
    let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert!(val["error"]
        .as_str()
        .unwrap()
        .contains("expected-state mismatch"));

    let _ = std::fs::remove_dir_all(&ws);
}

// --- WO-11b: security — governance kinds rejected by patch ---

#[test]
fn patch_rejects_governance_kind_add_decision() {
    let (ws, _clean) = init_workspace("acme");

    let patch_file = ws.join("patch.json");
    std::fs::write(
        &patch_file,
        serde_json::json!({
            "brand_id": "acme",
            "operations": [
                {"op": "add", "kind": "decision", "id": "d1", "data": {"text": "test"}},
            ],
        })
        .to_string(),
    )
    .unwrap();

    let (code, out) = run_cmd_with_brand(
        &ws,
        "acme",
        &["patch", "--plan", patch_file.to_str().unwrap(), "--apply"],
    );
    assert_eq!(code, 3, "should reject governance kind in add: {out}");
    let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert!(val["error"].as_str().unwrap().contains("governance"));

    // Verify STATE unchanged — no objects written.
    let (code, vout) = run_cmd_with_brand(&ws, "acme", &["verify"]);
    assert_eq!(code, 0, "verify should pass: {vout}");
    let vval: serde_json::Value = serde_json::from_str(vout.trim()).unwrap();
    assert!(vval["result"]["state_valid"].as_bool().unwrap());

    let _ = std::fs::remove_dir_all(&ws);
}

#[test]
fn patch_rejects_governance_kind_replace_review() {
    let (ws, _clean) = init_workspace("acme");

    let patch_file = ws.join("patch.json");
    std::fs::write(
        &patch_file,
        serde_json::json!({
            "brand_id": "acme",
            "operations": [
                {"op": "replace", "kind": "review", "id": "r1", "parent_hash": "abc", "data": {"text": "test"}},
            ],
        })
        .to_string(),
    )
    .unwrap();

    let (code, out) = run_cmd_with_brand(
        &ws,
        "acme",
        &["patch", "--plan", patch_file.to_str().unwrap(), "--dry-run"],
    );
    assert_eq!(code, 3, "should reject governance kind in replace: {out}");
    let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert!(val["error"].as_str().unwrap().contains("governance"));

    let _ = std::fs::remove_dir_all(&ws);
}

#[test]
fn patch_rejects_governance_kind_remove_check_result() {
    let (ws, _clean) = init_workspace("acme");

    let patch_file = ws.join("patch.json");
    std::fs::write(
        &patch_file,
        serde_json::json!({
            "brand_id": "acme",
            "operations": [
                {"op": "remove", "kind": "check_result", "id": "cr1", "parent_hash": "abc"},
            ],
        })
        .to_string(),
    )
    .unwrap();

    let (code, out) = run_cmd_with_brand(
        &ws,
        "acme",
        &["patch", "--plan", patch_file.to_str().unwrap(), "--apply"],
    );
    assert_eq!(code, 3, "should reject governance kind in remove: {out}");
    let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert!(val["error"].as_str().unwrap().contains("governance"));

    // Verify STATE unchanged.
    let (code, vout) = run_cmd_with_brand(&ws, "acme", &["verify"]);
    assert_eq!(code, 0, "verify should pass: {vout}");

    let _ = std::fs::remove_dir_all(&ws);
}

/// Content hashes of every stored revision of `kind`/`id`, read from the object store.
fn revision_hashes(ws: &Path, brand: &str, kind: &str, id: &str) -> Vec<String> {
    let dir = ws
        .join(".distinct")
        .join(brand)
        .join("store")
        .join("objects");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).unwrap() {
        let text = std::fs::read_to_string(entry.unwrap().path()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        if v["kind"] == kind && v["id"] == id {
            out.push(v["content_hash"].as_str().unwrap().to_string());
        }
    }
    out
}

#[test]
fn patch_rejects_stale_parent_revision() {
    let (ws, _clean) = init_workspace("acme");
    let plan = |ops: serde_json::Value| {
        let f = ws.join(format!("plan-{}.json", rand_suffix()));
        std::fs::write(
            &f,
            serde_json::json!({"brand_id": "acme", "operations": ops}).to_string(),
        )
        .unwrap();
        f
    };

    let add = plan(serde_json::json!([
        {"op": "add", "kind": "claim", "id": "claim-s", "data": {"text": "first"}}
    ]));
    let (code, _) = run_cmd_with_brand(
        &ws,
        "acme",
        &["patch", "--plan", add.to_str().unwrap(), "--apply"],
    );
    assert_eq!(code, 0);
    let first = revision_hashes(&ws, "acme", "claim", "claim-s");
    assert_eq!(first.len(), 1);

    let replace = plan(serde_json::json!([
        {"op": "replace", "kind": "claim", "id": "claim-s", "parent_hash": first[0], "data": {"text": "second"}}
    ]));
    let (code, out) = run_cmd_with_brand(
        &ws,
        "acme",
        &["patch", "--plan", replace.to_str().unwrap(), "--apply"],
    );
    assert_eq!(code, 0, "replace from head should apply: {out}");
    assert_eq!(revision_hashes(&ws, "acme", "claim", "claim-s").len(), 2);

    // The first revision is now stale: replacing from it must conflict and write nothing.
    let stale = plan(serde_json::json!([
        {"op": "replace", "kind": "claim", "id": "claim-s", "parent_hash": first[0], "data": {"text": "third"}}
    ]));
    let (code, out) = run_cmd_with_brand(
        &ws,
        "acme",
        &["patch", "--plan", stale.to_str().unwrap(), "--apply"],
    );
    assert_eq!(code, 3, "stale parent must be rejected: {out}");
    assert_eq!(revision_hashes(&ws, "acme", "claim", "claim-s").len(), 2);

    let _ = std::fs::remove_dir_all(&ws);
}
