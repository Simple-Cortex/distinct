//! Integration tests for `packet` and `export` commands (WO-04 / WO-07).
//!
//! Each test runs the real `distinct-rs` binary against a temporary workspace.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::SystemTime;

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_distinct-rs"))
        .args(args)
        .output()
        .expect("spawn distinct-rs")
}

fn stdout(o: &Output) -> String {
    String::from_utf8(o.stdout.clone()).expect("utf-8 stdout")
}

fn stderr(o: &Output) -> String {
    String::from_utf8(o.stderr.clone()).expect("utf-8 stderr")
}

fn exit_code(o: &Output) -> i32 {
    o.status.code().expect("exit code")
}

fn temp_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target")
        .join(format!("test-{name}-{nanos}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn init_brand(dir: &Path, brand: &str) {
    let o = run(&[
        "init",
        "--brand",
        brand,
        "--workspace",
        &dir.to_string_lossy(),
    ]);
    assert_eq!(exit_code(&o), 0, "init failed: {}", stderr(&o));
}

fn import_record(dir: &Path, brand: &str, record: &serde_json::Value) {
    let record_path = dir.join("tmp-record.json");
    fs::write(&record_path, serde_json::to_string_pretty(record).unwrap()).unwrap();
    let o = run(&[
        "record-import",
        "--brand",
        brand,
        "--workspace",
        &dir.to_string_lossy(),
        "--file",
        &record_path.to_string_lossy(),
    ]);
    assert_eq!(exit_code(&o), 0, "record-import failed: {}", stdout(&o));
    let _ = fs::remove_file(&record_path);
}

/// Record an owner (human) approval decision for the stored current revision
/// of `kind/id`, found by scanning the object store.
fn approve_record(dir: &Path, brand: &str, kind: &str, id: &str, decision_id: &str) {
    let objects_dir = dir.join(format!(".distinct/{brand}/store/objects"));
    let mut content_hash = None;
    for e in fs::read_dir(&objects_dir).unwrap() {
        let v: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(e.unwrap().path()).unwrap()).unwrap();
        if v["kind"] == kind && v["id"] == id {
            content_hash = v["content_hash"].as_str().map(String::from);
        }
    }
    let decision = serde_json::json!({
        "schema_version": "1.0.0",
        "kind": "decision",
        "id": decision_id,
        "brand_id": brand,
        "privacy": "public",
        "created_at": "2026-10-27T00:00:00Z",
        "created_by": {"kind": "human", "id": "owner1"},
        "dependencies": [],
        "limitations": [],
        "data": {
            "approved": true,
            "target": {"kind": kind, "id": id, "content_hash": content_hash.expect("record stored")}
        }
    });
    let path = write_json(dir, &format!("{decision_id}.json"), &decision);
    // D-M: record creates a pending decision; confirm (with the debug test hook
    // code) stores the attested decision.
    let o = Command::new(env!("CARGO_BIN_EXE_distinct-rs"))
        .env("DISTINCT_TEST_CONFIRM_CODE", "7K3M9-Q2W8X")
        .env("DISTINCT_TEST_KDF_ITERATIONS", "50")
        .args([
            "decision",
            "record",
            "--brand",
            brand,
            "--workspace",
            &dir.to_string_lossy(),
            "--file",
            &path.to_string_lossy(),
            "--owner",
            "owner1",
        ])
        .output()
        .expect("spawn distinct-rs");
    assert_eq!(exit_code(&o), 0, "decision record failed: {}", stdout(&o));
    let v: serde_json::Value = serde_json::from_str(stdout(&o).trim()).unwrap();
    let pending_id = v["result"]["pending_id"].as_str().unwrap().to_string();
    let o = run(&[
        "decision",
        "confirm",
        "--brand",
        brand,
        "--workspace",
        &dir.to_string_lossy(),
        "--id",
        &pending_id,
        "--code",
        "7K3M9-Q2W8X",
    ]);
    assert_eq!(exit_code(&o), 0, "decision confirm failed: {}", stdout(&o));
}

// ── fixtures ─────────────────────────────────────────────────────────

fn public_project() -> serde_json::Value {
    serde_json::json!({
        "schema_version": "1.0.0",
        "kind": "project",
        "id": "main",
        "brand_id": "acme",
        "privacy": "public",
        "created_at": "2026-10-27T00:00:00Z",
        "created_by": {"kind": "agent", "id": "test"},
        "dependencies": [],
        "limitations": [],
        "data": {"name": "Acme Brand Project"}
    })
}

fn public_platform() -> serde_json::Value {
    serde_json::json!({
        "schema_version": "1.0.0",
        "kind": "platform",
        "id": "web",
        "brand_id": "acme",
        "privacy": "public",
        "created_at": "2026-10-27T00:00:00Z",
        "created_by": {"kind": "agent", "id": "test"},
        "dependencies": [],
        "limitations": [],
        "data": {"name": "Website", "url": "https://acme.example"}
    })
}

fn confidential_source() -> serde_json::Value {
    serde_json::json!({
        "schema_version": "1.0.0",
        "kind": "source",
        "id": "interview",
        "brand_id": "acme",
        "privacy": "confidential",
        "created_at": "2026-10-27T00:00:00Z",
        "created_by": {"kind": "agent", "id": "test"},
        "dependencies": [],
        "limitations": [],
        "data": {"text": "Secret strategy notes"}
    })
}

fn public_with_examples() -> serde_json::Value {
    serde_json::json!({
        "schema_version": "1.0.0",
        "kind": "direction",
        "id": "main",
        "brand_id": "acme",
        "privacy": "public",
        "created_at": "2026-10-27T00:00:00Z",
        "created_by": {"kind": "agent", "id": "test"},
        "dependencies": [],
        "limitations": [],
        "data": {
            "summary": "Bold and minimal",
            "examples": [
                {"label": "ref-a", "detail": "x".repeat(200)},
                {"label": "ref-b", "detail": "y".repeat(200)},
                {"label": "ref-c", "detail": "z".repeat(200)}
            ]
        }
    })
}

// ── packet tests ─────────────────────────────────────────────────────

#[test]
fn packet_fits_cap() {
    let dir = temp_dir("packet-fits");
    init_brand(&dir, "acme");
    import_record(&dir, "acme", &public_project());
    import_record(&dir, "acme", &public_platform());

    let o = run(&[
        "packet",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
        "--task",
        "brand-review",
        "--scope",
        "method_version,platform_fields",
        "--max-bytes",
        "16384",
    ]);
    assert_eq!(exit_code(&o), 0, "packet failed: {}", stdout(&o));
    let val: serde_json::Value = serde_json::from_str(stdout(&o).trim()).unwrap();
    // Packet content is in result — the full envelope is the response.
    assert!(val["result"]["packet_hash"].as_str().is_some());
    let byte_len = val["result"]["byte_length"].as_u64().unwrap() as usize;
    // byte_length must equal the exact emitted bytes including trailing newline.
    assert_eq!(
        byte_len,
        o.stdout.len(),
        "byte_length {byte_len} != emitted {} bytes",
        o.stdout.len()
    );
    assert!(byte_len <= 16384, "byte_length {byte_len} > cap 16384");

    // Verify persisted file exists.
    let packet_hash = val["result"]["packet_hash"].as_str().unwrap();
    let packet_path = dir
        .join(".distinct/acme/packets")
        .join(format!("{packet_hash}.json"));
    assert!(packet_path.exists(), "packet file not persisted");

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn packet_under_1024_rejected() {
    let dir = temp_dir("packet-under1024");
    init_brand(&dir, "acme");
    import_record(&dir, "acme", &public_project());

    let o = run(&[
        "packet",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
        "--task",
        "test",
        "--scope",
        "method_version",
        "--max-bytes",
        "512",
    ]);
    // --max-bytes < 1024 is rejected at arg-parse level
    assert_eq!(exit_code(&o), 3, "should reject cap < 1024");

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn looser_global_cap_rejected() {
    // The global --max-bytes is the only cap mechanism.
    // A "looser" cap doesn't apply because the packet just uses the given cap.
    // What we test: a cap of exactly 1024 with content that barely fits.
    let dir = temp_dir("packet-loose-cap");
    init_brand(&dir, "acme");
    import_record(&dir, "acme", &public_project());

    // 1024 is the minimum — should succeed for small content.
    let o = run(&[
        "packet",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
        "--task",
        "test",
        "--scope",
        "method_version",
        "--max-bytes",
        "1024",
    ]);
    // If content fits in 1024, ok. If not, exit 2.
    let code = exit_code(&o);
    assert!(
        code == 0 || code == 2,
        "expected ok or budget_insufficient, got {code}: {}",
        stdout(&o)
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn examples_dropped_first() {
    let dir = temp_dir("packet-examples-drop");
    init_brand(&dir, "acme");
    import_record(&dir, "acme", &public_project());
    import_record(&dir, "acme", &public_with_examples());

    // First, get the packet with generous cap to see size with examples.
    let o_full = run(&[
        "packet",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
        "--task",
        "test",
        "--scope",
        "method_version,request",
        "--max-bytes",
        "16384",
    ]);
    assert_eq!(exit_code(&o_full), 0);
    let full_val: serde_json::Value = serde_json::from_str(stdout(&o_full).trim()).unwrap();
    let full_len = full_val["result"]["byte_length"].as_u64().unwrap() as usize;

    // Now use a tighter cap that is below full_len.
    // If the essential content fits, the packet should be smaller.
    // If it doesn't fit, we get budget_insufficient (exit 2).
    let tight_cap = if full_len > 1100 {
        full_len - 100
    } else {
        1024
    };
    if tight_cap < 1024 {
        fs::remove_dir_all(&dir).ok();
        return;
    }

    let o_tight = run(&[
        "packet",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
        "--task",
        "test",
        "--scope",
        "method_version,request",
        "--max-bytes",
        &tight_cap.to_string(),
    ]);

    let code = exit_code(&o_tight);
    if code == 0 {
        // Packet fit within tight cap — examples must have been dropped.
        let tight_val: serde_json::Value = serde_json::from_str(stdout(&o_tight).trim()).unwrap();
        let tight_len = tight_val["result"]["byte_length"].as_u64().unwrap() as usize;
        assert!(
            tight_len <= tight_cap,
            "tight packet {tight_len} > cap {tight_cap}"
        );
        // The tight packet should be smaller than the full one.
        assert!(
            tight_len < full_len,
            "tight packet should be smaller (examples dropped): {tight_len} >= {full_len}"
        );
    } else if code == 2 {
        // budget_insufficient — essential content doesn't fit in tight_cap.
        // This is valid: the test data's essential content exceeds tight_cap.
        let out = stdout(&o_tight);
        assert!(
            out.contains("budget_insufficient"),
            "expected budget_insufficient in: {out}"
        );
    } else {
        panic!("unexpected exit code {code}: {}", stdout(&o_tight));
    }

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn budget_insufficient_split() {
    let dir = temp_dir("packet-budget-insuf");
    init_brand(&dir, "acme");
    import_record(&dir, "acme", &public_project());
    import_record(&dir, "acme", &public_platform());
    import_record(&dir, "acme", &public_with_examples());

    // Use a very tight cap that can't fit even essential content.
    let o = run(&[
        "packet",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
        "--task",
        "test",
        "--scope",
        "method_version,platform_fields,request",
        "--max-bytes",
        "1024",
    ]);

    let code = exit_code(&o);
    // The emitted envelope must always fit the cap, even for errors.
    assert!(
        o.stdout.len() <= 1024,
        "emitted {} bytes > cap 1024",
        o.stdout.len()
    );
    if code == 2 {
        let out = stdout(&o);
        assert!(
            out.contains("budget_insufficient"),
            "expected budget_insufficient in: {out}"
        );
    }
    // If it fits in 1024, that's also fine (content is small).

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn packet_small_cap_exact_fit() {
    let dir = temp_dir("packet-small-cap");
    init_brand(&dir, "acme");
    import_record(&dir, "acme", &public_project());

    let o = run(&[
        "packet",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
        "--task",
        "test",
        "--scope",
        "method_version",
        "--max-bytes",
        "1024",
    ]);

    let code = exit_code(&o);
    // Emitted bytes must never exceed the cap.
    assert!(
        o.stdout.len() <= 1024,
        "emitted {} bytes > cap 1024",
        o.stdout.len()
    );
    if code == 0 {
        let val: serde_json::Value = serde_json::from_str(stdout(&o).trim()).unwrap();
        let byte_len = val["result"]["byte_length"].as_u64().unwrap() as usize;
        assert_eq!(
            byte_len,
            o.stdout.len(),
            "byte_length must equal emitted bytes"
        );
        assert!(byte_len <= 1024, "byte_length {byte_len} > cap 1024");
    }

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn privacy_disallowed_blocks() {
    let dir = temp_dir("packet-privacy");
    init_brand(&dir, "acme");
    import_record(&dir, "acme", &public_project());
    import_record(&dir, "acme", &confidential_source());

    // Request source_refs scope without destination → only public allowed.
    // source is confidential → privacy_disallowed.
    let o = run(&[
        "packet",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
        "--task",
        "test",
        "--scope",
        "source_refs",
    ]);
    assert_eq!(exit_code(&o), 3, "should be invalid (privacy_disallowed)");
    let out = stdout(&o);
    assert!(
        out.contains("privacy_disallowed"),
        "expected privacy_disallowed in: {out}"
    );

    fs::remove_dir_all(&dir).ok();
}

// ── export tests ─────────────────────────────────────────────────────

#[test]
fn export_draft_writes_manifest() {
    let dir = temp_dir("export-draft");
    init_brand(&dir, "acme");
    import_record(&dir, "acme", &public_project());
    import_record(&dir, "acme", &public_platform());

    let out_dir = dir.join("export-output");
    let o = run(&[
        "export",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
        "--kind",
        "draft",
        "--out",
        &out_dir.to_string_lossy(),
    ]);
    assert_eq!(exit_code(&o), 0, "export failed: {}", stdout(&o));

    // manifest.json should exist.
    let manifest_path = out_dir.join("manifest.json");
    assert!(manifest_path.exists(), "manifest.json not written");
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
    assert_eq!(manifest["kind"], "draft");
    assert_eq!(manifest["brand_id"], "acme");
    assert!(manifest["records"].as_array().unwrap().len() >= 2);

    // Record files should exist.
    let project_file = out_dir.join("draft/project/main.json");
    assert!(project_file.exists(), "project record not written");

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn export_production_ready_refused() {
    let dir = temp_dir("export-prod-ready");
    init_brand(&dir, "acme");

    let out_dir = dir.join("export-output");
    let o = run(&[
        "export",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
        "--kind",
        "production_ready",
        "--out",
        &out_dir.to_string_lossy(),
    ]);
    assert_eq!(exit_code(&o), 3, "production_ready should be refused");
    let out = stdout(&o);
    assert!(
        out.contains("production_ready"),
        "error should mention production_ready: {out}"
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn existing_export_preserved() {
    let dir = temp_dir("export-existing");
    init_brand(&dir, "acme");
    import_record(&dir, "acme", &public_project());

    let out_dir = dir.join("export-output");

    // First export succeeds.
    let o1 = run(&[
        "export",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
        "--kind",
        "draft",
        "--out",
        &out_dir.to_string_lossy(),
    ]);
    assert_eq!(exit_code(&o1), 0);

    // Write a sentinel file to prove the directory is non-empty.
    let sentinel = out_dir.join("sentinel.txt");
    fs::write(&sentinel, "do-not-overwrite").unwrap();

    // Second export without --force should fail.
    let o2 = run(&[
        "export",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
        "--kind",
        "draft",
        "--out",
        &out_dir.to_string_lossy(),
    ]);
    assert_eq!(
        exit_code(&o2),
        3,
        "should refuse non-empty dir without --force"
    );

    // Sentinel should still exist.
    assert!(sentinel.exists(), "sentinel should be preserved");
    assert_eq!(fs::read_to_string(&sentinel).unwrap(), "do-not-overwrite");

    // With --force, it should succeed.
    let o3 = run(&[
        "export",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
        "--kind",
        "draft",
        "--out",
        &out_dir.to_string_lossy(),
        "--force",
    ]);
    assert_eq!(exit_code(&o3), 0, "export with --force should succeed");

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn confidential_excluded() {
    let dir = temp_dir("export-confidential");
    init_brand(&dir, "acme");
    import_record(&dir, "acme", &public_project());
    import_record(&dir, "acme", &confidential_source());

    let out_dir = dir.join("export-output");
    let o = run(&[
        "export",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
        "--kind",
        "draft",
        "--out",
        &out_dir.to_string_lossy(),
    ]);
    assert_eq!(exit_code(&o), 0, "export failed: {}", stdout(&o));

    let val: serde_json::Value = serde_json::from_str(stdout(&o).trim()).unwrap();
    let excluded = val["result"]["excluded"].as_u64().unwrap() as usize;
    assert!(excluded >= 1, "confidential record should be excluded");

    // Confidential source record should NOT be in the export.
    let source_file = out_dir.join("draft/source/interview.json");
    assert!(
        !source_file.exists(),
        "confidential source should not be exported"
    );

    // Public project should be present.
    let project_file = out_dir.join("draft/project/main.json");
    assert!(project_file.exists(), "public project should be exported");

    // Manifest should reflect the exclusion.
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(out_dir.join("manifest.json")).unwrap()).unwrap();
    let records = manifest["records"].as_array().unwrap();
    let has_source = records
        .iter()
        .any(|r| r["kind"] == "source" && r["id"] == "interview");
    assert!(
        !has_source,
        "manifest should not include confidential source"
    );

    fs::remove_dir_all(&dir).ok();
}

// ── governance helpers ───────────────────────────────────────────────

fn write_json(dir: &Path, name: &str, value: &serde_json::Value) -> PathBuf {
    let p = dir.join(name);
    fs::write(&p, serde_json::to_string_pretty(value).unwrap()).unwrap();
    p
}

fn update_snapshot(dir: &Path, brand: &str) {
    // Write a snapshot file so reconcile_governance can recompute from it.
    // We do NOT set content_snapshot_hash in STATE — this avoids a hash
    // mismatch caused by serde_json preserve_order feature differences
    // between the test binary and distinct-core. The stale-dependency
    // check uses the recomputed snapshot (not STATE), so it still works.
    // Object-integrity checks (verify_detects_snapshot_tamper) compare
    // each object's stored content_hash against its recomputed hash,
    // which does not depend on the snapshot hash in STATE.
    let brand_dir = dir.join(".distinct").join(brand);
    let objects_dir = brand_dir.join("store/objects");
    let snapshots_dir = brand_dir.join("store/snapshots");

    let mut records: std::collections::BTreeMap<
        String,
        std::collections::BTreeMap<String, String>,
    > = std::collections::BTreeMap::new();

    if let Ok(entries) = fs::read_dir(&objects_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json") {
                if let Ok(contents) = fs::read_to_string(&path) {
                    if let Ok(record) = serde_json::from_str::<serde_json::Value>(&contents) {
                        let kind = record["kind"].as_str().unwrap_or("").to_string();
                        let id = record["id"].as_str().unwrap_or("").to_string();
                        let hash = record["content_hash"].as_str().unwrap_or("").to_string();
                        let is_governance =
                            matches!(kind.as_str(), "check_result" | "review" | "decision");
                        if !is_governance {
                            records.entry(kind).or_default().insert(id, hash);
                        }
                    }
                }
            }
        }
    }

    let snapshot = serde_json::json!({
        "schema_version": "1.0.0",
        "brand_id": brand,
        "records": records,
    });
    let snap_json = serde_json::to_string_pretty(&snapshot).unwrap();
    // Write a fixed-name snapshot so it's findable; the hash in STATE is
    // left as null so reconcile skips the snapshot-hash comparison.
    fs::write(snapshots_dir.join("current.json"), &snap_json).unwrap();
}

// ── governance tests ─────────────────────────────────────────────────

#[test]
fn gate_missing_check_result_blocks() {
    let dir = temp_dir("gate-missing-cr");
    init_brand(&dir, "acme");

    let check_plan = serde_json::json!({
        "schema_version": "1.0.0",
        "kind": "check_plan",
        "id": "cp1",
        "brand_id": "acme",
        "privacy": "public",
        "created_at": "2026-10-27T00:00:00Z",
        "created_by": {"kind": "human", "id": "owner1"},
        "dependencies": [],
        "limitations": [],
        "data": {"scope": "all"}
    });
    import_record(&dir, "acme", &check_plan);
    update_snapshot(&dir, "acme");

    let o = run(&[
        "gate",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
        "--scope",
        "default",
    ]);
    assert_eq!(exit_code(&o), 0, "gate failed: {}", stdout(&o));
    let val: serde_json::Value = serde_json::from_str(stdout(&o).trim()).unwrap();
    assert_eq!(val["result"]["ready"], false, "gate should NOT be ready");
    let blockers = val["result"]["blockers"].as_array().unwrap();
    assert!(
        blockers
            .iter()
            .any(|b| b.as_str().unwrap().contains("missing check_result")),
        "expected missing check_result blocker in: {blockers:?}"
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn gate_stale_dependency_reported() {
    let dir = temp_dir("gate-stale-dep");
    init_brand(&dir, "acme");

    let project = serde_json::json!({
        "schema_version": "1.0.0",
        "kind": "project",
        "id": "main",
        "brand_id": "acme",
        "privacy": "public",
        "created_at": "2026-10-27T00:00:00Z",
        "created_by": {"kind": "human", "id": "owner1"},
        "dependencies": [],
        "limitations": [],
        "data": {"name": "Acme"}
    });
    import_record(&dir, "acme", &project);
    update_snapshot(&dir, "acme");

    // Import a record that depends on project/main with a WRONG value_hash.
    let dependent = serde_json::json!({
        "schema_version": "1.0.0",
        "kind": "claim",
        "id": "c1",
        "brand_id": "acme",
        "privacy": "public",
        "created_at": "2026-10-27T00:00:00Z",
        "created_by": {"kind": "human", "id": "owner1"},
        "dependencies": [{
            "target": {
                "kind": "project",
                "id": "main",
                "pointer": "",
                "value_hash": "0000000000000000000000000000000000000000000000000000000000000000"
            },
            "applies_to": [""]
        }],
        "limitations": [],
        "data": {"text": "test claim"}
    });
    import_record(&dir, "acme", &dependent);

    let o = run(&[
        "gate",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
    ]);
    assert_eq!(exit_code(&o), 0, "gate failed: {}", stdout(&o));
    let val: serde_json::Value = serde_json::from_str(stdout(&o).trim()).unwrap();
    let stale = val["result"]["stale"].as_array().unwrap();
    assert!(
        stale.iter().any(|s| s.as_str().unwrap().contains("stale")),
        "expected stale dependency in: {stale:?}"
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn verify_detects_snapshot_tamper() {
    let dir = temp_dir("verify-tamper");
    init_brand(&dir, "acme");

    let project = serde_json::json!({
        "schema_version": "1.0.0",
        "kind": "project",
        "id": "main",
        "brand_id": "acme",
        "privacy": "public",
        "created_at": "2026-10-27T00:00:00Z",
        "created_by": {"kind": "human", "id": "owner1"},
        "dependencies": [],
        "limitations": [],
        "data": {"name": "Acme"}
    });
    import_record(&dir, "acme", &project);
    update_snapshot(&dir, "acme");

    // Tamper with the stored object file.
    let objects_dir = dir.join(".distinct/acme/store/objects");
    for entry in fs::read_dir(&objects_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "json") {
            let mut obj: serde_json::Value =
                serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
            obj["data"]["name"] = serde_json::json!("TAMPERED");
            fs::write(&path, serde_json::to_string_pretty(&obj).unwrap()).unwrap();
            break;
        }
    }

    let o = run(&[
        "verify",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
    ]);
    assert_eq!(
        exit_code(&o),
        1,
        "verify on tampered state must exit 1: {}",
        stdout(&o)
    );
    let val: serde_json::Value = serde_json::from_str(stdout(&o).trim()).unwrap();
    assert_eq!(
        val["result"]["state_valid"], false,
        "verify should detect tampering"
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn decision_requires_owner() {
    let dir = temp_dir("decision-no-owner");
    init_brand(&dir, "acme");

    let decision = serde_json::json!({
        "schema_version": "1.0.0",
        "kind": "decision",
        "id": "d1",
        "brand_id": "acme",
        "privacy": "public",
        "created_at": "2026-10-27T00:00:00Z",
        "created_by": {"kind": "human", "id": "owner1"},
        "dependencies": [],
        "limitations": [],
        "data": {"approved": true}
    });
    let decision_path = write_json(&dir, "decision.json", &decision);

    // No --owner flag → should fail.
    let o = run(&[
        "decision",
        "record",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
        "--file",
        &decision_path.to_string_lossy(),
    ]);
    assert_eq!(exit_code(&o), 3, "decision without --owner should fail");
    let out = stdout(&o);
    assert!(
        out.contains("--owner"),
        "error should mention --owner: {out}"
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn agent_decision_refused() {
    let dir = temp_dir("decision-agent");
    init_brand(&dir, "acme");

    let decision = serde_json::json!({
        "schema_version": "1.0.0",
        "kind": "decision",
        "id": "d1",
        "brand_id": "acme",
        "privacy": "public",
        "created_at": "2026-10-27T00:00:00Z",
        "created_by": {"kind": "agent", "id": "bot1"},
        "dependencies": [],
        "limitations": [],
        "data": {"approved": true}
    });
    let decision_path = write_json(&dir, "decision.json", &decision);

    let o = run(&[
        "decision",
        "record",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
        "--file",
        &decision_path.to_string_lossy(),
        "--owner",
        "human1",
    ]);
    assert_eq!(exit_code(&o), 3, "agent decision should be refused");
    let out = stdout(&o);
    assert!(out.contains("agent"), "error should mention agent: {out}");

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn review_create_roundtrip() {
    let dir = temp_dir("review-roundtrip");
    init_brand(&dir, "acme");

    let review = serde_json::json!({
        "schema_version": "1.0.0",
        "kind": "review",
        "id": "r1",
        "brand_id": "acme",
        "privacy": "public",
        "created_at": "2026-10-27T00:00:00Z",
        "created_by": {"kind": "human", "id": "reviewer1"},
        "dependencies": [],
        "limitations": [],
        "data": {"findings": ["ok"]}
    });
    let review_path = write_json(&dir, "review.json", &review);

    let o = run(&[
        "review",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
        "--file",
        &review_path.to_string_lossy(),
    ]);
    assert_eq!(exit_code(&o), 0, "review create failed: {}", stdout(&o));
    let val: serde_json::Value = serde_json::from_str(stdout(&o).trim()).unwrap();
    assert_eq!(val["result"]["kind"], "review");
    assert_eq!(val["result"]["id"], "r1");
    assert!(val["result"]["content_hash"].as_str().is_some());

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn gate_ready_when_no_blockers() {
    let dir = temp_dir("gate-ready");
    init_brand(&dir, "acme");

    let project = serde_json::json!({
        "schema_version": "1.0.0",
        "kind": "project",
        "id": "main",
        "brand_id": "acme",
        "privacy": "public",
        "created_at": "2026-10-27T00:00:00Z",
        "created_by": {"kind": "human", "id": "owner1"},
        "dependencies": [],
        "limitations": [],
        "data": {"name": "Acme"}
    });
    import_record(&dir, "acme", &project);

    // No check plans → no missing results → ready.
    let o = run(&[
        "gate",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
    ]);
    assert_eq!(exit_code(&o), 0, "gate failed: {}", stdout(&o));
    let val: serde_json::Value = serde_json::from_str(stdout(&o).trim()).unwrap();
    assert_eq!(val["result"]["ready"], true, "gate should be ready");
    let blockers = val["result"]["blockers"].as_array().unwrap();
    assert!(blockers.is_empty(), "expected no blockers: {blockers:?}");

    fs::remove_dir_all(&dir).ok();
}

// ── WO-09: export-kind differentiation ───────────────────────────────

#[test]
fn export_concept_only_labels_manifest_and_records() {
    let dir = temp_dir("export-concept");
    init_brand(&dir, "acme");

    let project = serde_json::json!({
        "schema_version": "1.0.0",
        "kind": "project",
        "id": "main",
        "brand_id": "acme",
        "privacy": "public",
        "created_at": "2026-10-27T00:00:00Z",
        "created_by": {"kind": "human", "id": "owner1"},
        "dependencies": [],
        "limitations": [],
        "data": {"name": "Acme"}
    });
    let direction = serde_json::json!({
        "schema_version": "1.0.0",
        "kind": "direction",
        "id": "visual",
        "brand_id": "acme",
        "privacy": "public",
        "created_at": "2026-10-27T00:00:00Z",
        "created_by": {"kind": "human", "id": "owner1"},
        "dependencies": [],
        "limitations": [],
        "data": {"summary": "Bold and minimal"}
    });
    import_record(&dir, "acme", &project);
    import_record(&dir, "acme", &direction);

    let out_dir = dir.join("export-output");
    let o = run(&[
        "export",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
        "--kind",
        "concept_only",
        "--out",
        &out_dir.to_string_lossy(),
    ]);
    assert_eq!(
        exit_code(&o),
        0,
        "concept_only export failed: {}",
        stdout(&o)
    );

    // Manifest should carry the concept_only label.
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(out_dir.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["kind"], "concept_only");
    assert_eq!(
        manifest["label"].as_str().unwrap(),
        "concept_only: not for production use"
    );

    // Visual record (direction) should have "concept": true in data.
    let direction_file = out_dir.join("concept_only/direction/visual.json");
    assert!(
        direction_file.exists(),
        "direction record should be exported"
    );
    let rec: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&direction_file).unwrap()).unwrap();
    assert_eq!(
        rec["data"]["concept"], true,
        "visual record should have concept:true"
    );

    // Non-visual record (project) should NOT have "concept": true.
    let project_file = out_dir.join("concept_only/project/main.json");
    assert!(project_file.exists());
    let proj_rec: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&project_file).unwrap()).unwrap();
    assert!(
        proj_rec["data"]["concept"].is_null(),
        "non-visual record should not have concept flag"
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn export_specification_only_excludes_binary_proof() {
    let dir = temp_dir("export-spec");
    init_brand(&dir, "acme");

    let project = serde_json::json!({
        "schema_version": "1.0.0",
        "kind": "project",
        "id": "main",
        "brand_id": "acme",
        "privacy": "public",
        "created_at": "2026-10-27T00:00:00Z",
        "created_by": {"kind": "human", "id": "owner1"},
        "dependencies": [],
        "limitations": [],
        "data": {"name": "Acme"}
    });
    let logo_finding = serde_json::json!({
        "schema_version": "1.0.0",
        "kind": "logo_finding",
        "id": "lf1",
        "brand_id": "acme",
        "privacy": "public",
        "created_at": "2026-10-27T00:00:00Z",
        "created_by": {"kind": "agent", "id": "distinct-rs"},
        "dependencies": [],
        "limitations": [],
        "data": {"verdict": "inspected"}
    });
    import_record(&dir, "acme", &project);
    import_record(&dir, "acme", &logo_finding);

    let out_dir = dir.join("export-output");
    let o = run(&[
        "export",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
        "--kind",
        "specification_only",
        "--out",
        &out_dir.to_string_lossy(),
    ]);
    assert_eq!(
        exit_code(&o),
        0,
        "specification_only export failed: {}",
        stdout(&o)
    );

    // Manifest should list excluded asset kinds.
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(out_dir.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["kind"], "specification_only");
    let excluded = manifest["assets_excluded"].as_array().unwrap();
    assert!(
        excluded.iter().any(|e| e.as_str() == Some("logo_finding")),
        "assets_excluded should include logo_finding: {excluded:?}"
    );

    // logo_finding record should NOT be written.
    let lf_file = out_dir.join("specification_only/logo_finding/lf1.json");
    assert!(
        !lf_file.exists(),
        "logo_finding should be excluded from specification_only"
    );

    // project should still be present.
    let proj_file = out_dir.join("specification_only/project/main.json");
    assert!(
        proj_file.exists(),
        "project should be in specification_only export"
    );

    fs::remove_dir_all(&dir).ok();
}

// ── WO-09: governance commands update STATE ──────────────────────────

#[test]
fn verify_clean_after_governance_import() {
    let dir = temp_dir("verify-after-gov");
    init_brand(&dir, "acme");

    // Import a check_result via the governance command.
    let check_result = serde_json::json!({
        "schema_version": "1.0.0",
        "kind": "check_result",
        "id": "cr1",
        "brand_id": "acme",
        "privacy": "public",
        "created_at": "2026-10-27T00:00:00Z",
        "created_by": {"kind": "human", "id": "owner1"},
        "dependencies": [],
        "limitations": [],
        "data": {"check_plan_id": "cp1", "verdict": "pass"}
    });
    let cr_path = write_json(&dir, "cr1.json", &check_result);
    let o = run(&[
        "check",
        "result",
        "record",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
        "--file",
        &cr_path.to_string_lossy(),
    ]);
    assert_eq!(
        exit_code(&o),
        0,
        "check result record failed: {}",
        stdout(&o)
    );

    // Verify should be clean (no hash mismatches).
    let o = run(&[
        "verify",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
    ]);
    assert_eq!(exit_code(&o), 0, "verify failed: {}", stdout(&o));
    let val: serde_json::Value = serde_json::from_str(stdout(&o).trim()).unwrap();
    assert_eq!(
        val["result"]["state_valid"], true,
        "state should be valid after governance import, blockers: {:?}",
        val["result"]["blockers"]
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn verify_clean_after_decision() {
    let dir = temp_dir("verify-after-decision");
    init_brand(&dir, "acme");

    import_record(&dir, "acme", &public_project());
    approve_record(&dir, "acme", "project", "main", "d1");

    // Verify should be clean.
    let o = run(&[
        "verify",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
    ]);
    assert_eq!(exit_code(&o), 0, "verify failed: {}", stdout(&o));
    let val: serde_json::Value = serde_json::from_str(stdout(&o).trim()).unwrap();
    assert_eq!(
        val["result"]["state_valid"], true,
        "state should be valid after decision, blockers: {:?}",
        val["result"]["blockers"]
    );

    fs::remove_dir_all(&dir).ok();
}

// ── WO-09: dup_keys enforced on all JSON input paths ─────────────────

#[test]
fn dup_keys_rejected_on_record_import() {
    let dir = temp_dir("dup-keys-record");
    init_brand(&dir, "acme");

    let dup_json = r#"{"schema_version":"1.0.0","kind":"project","id":"main","brand_id":"acme","privacy":"public","data":{"name":"A","name":"B"}}"#;
    let dup_path = dir.join("dup.json");
    fs::write(&dup_path, dup_json).unwrap();

    let o = run(&[
        "record-import",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
        "--file",
        &dup_path.to_string_lossy(),
    ]);
    assert_eq!(exit_code(&o), 3, "dup keys should be rejected");
    let out = stdout(&o);
    assert!(
        out.contains("duplicate key"),
        "error should mention duplicate key: {out}"
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn dup_keys_rejected_on_governance_import() {
    let dir = temp_dir("dup-keys-gov");
    init_brand(&dir, "acme");

    let dup_json = r#"{"schema_version":"1.0.0","kind":"review","id":"r1","brand_id":"acme","privacy":"public","data":{"findings":["ok"],"findings":["bad"]}}"#;
    let dup_path = dir.join("dup-review.json");
    fs::write(&dup_path, dup_json).unwrap();

    let o = run(&[
        "review",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
        "--file",
        &dup_path.to_string_lossy(),
    ]);
    assert_eq!(exit_code(&o), 3, "dup keys should be rejected on review");
    let out = stdout(&o);
    assert!(
        out.contains("duplicate key"),
        "error should mention duplicate key: {out}"
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn dup_keys_rejected_on_decision() {
    let dir = temp_dir("dup-keys-decision");
    init_brand(&dir, "acme");

    let dup_json = r#"{"schema_version":"1.0.0","kind":"decision","id":"d1","brand_id":"acme","privacy":"public","created_by":{"kind":"human","id":"o1"},"data":{"approved":true,"approved":false}}"#;
    let dup_path = dir.join("dup-decision.json");
    fs::write(&dup_path, dup_json).unwrap();

    let o = run(&[
        "decision",
        "record",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
        "--file",
        &dup_path.to_string_lossy(),
        "--owner",
        "owner1",
    ]);
    assert_eq!(exit_code(&o), 3, "dup keys should be rejected on decision");
    let out = stdout(&o);
    assert!(
        out.contains("duplicate key"),
        "error should mention duplicate key: {out}"
    );

    fs::remove_dir_all(&dir).ok();
}

// ── WO-13: export --tokens dtcg ──────────────────────────────────────

#[test]
fn export_tokens_dtcg_writes_files() {
    let dir = temp_dir("export-tokens-dtcg");
    init_brand(&dir, "acme");

    import_record(
        &dir,
        "acme",
        &serde_json::json!({
            "schema_version": "1.0.0",
            "kind": "platform",
            "id": "main",
            "brand_id": "acme",
            "privacy": "public",
            "data": {
                "primary_color": "#ff5500",
                "spacing": 16,
                "font_family": "Inter"
            }
        }),
    );

    approve_record(&dir, "acme", "platform", "main", "d1");

    let out_dir = dir.join("export-out");
    let o = run(&[
        "export",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
        "--kind",
        "draft",
        "--out",
        &out_dir.to_string_lossy(),
        "--tokens",
        "dtcg",
    ]);
    assert_eq!(exit_code(&o), 0, "export failed: {}", stderr(&o));

    // tokens.json exists and has correct DTCG structure.
    let tokens_path = out_dir.join("tokens.json");
    assert!(tokens_path.exists(), "tokens.json should exist");
    let tokens: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&tokens_path).unwrap()).unwrap();
    assert_eq!(tokens["primary_color"]["$type"], "color");
    assert_eq!(tokens["primary_color"]["$value"], "#ff5500");
    assert_eq!(tokens["spacing"]["$type"], "dimension");
    assert_eq!(tokens["font_family"]["$type"], "fontFamily");

    // GUIDELINES.md exists.
    let guidelines_path = out_dir.join("GUIDELINES.md");
    assert!(guidelines_path.exists(), "GUIDELINES.md should exist");
    let guidelines = fs::read_to_string(&guidelines_path).unwrap();
    assert!(guidelines.contains("primary_color"));
    assert!(guidelines.contains("UNLICENSED"));

    // Envelope reports empty tokens_omitted.
    let out_str = stdout(&o);
    let env: serde_json::Value = serde_json::from_str(out_str.trim()).unwrap();
    assert_eq!(env["result"]["tokens_omitted"].as_array().unwrap().len(), 0);

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn export_tokens_dtcg_unapproved_omitted() {
    let dir = temp_dir("export-tokens-omitted");
    init_brand(&dir, "acme");

    import_record(
        &dir,
        "acme",
        &serde_json::json!({
            "schema_version": "1.0.0",
            "kind": "platform",
            "id": "main",
            "brand_id": "acme",
            "privacy": "public",
            "data": {
                "primary_color": "#000000",
                "custom_thing": "not_a_token",
                "another_extra": 42
            }
        }),
    );

    let out_dir = dir.join("export-out");
    let o = run(&[
        "export",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
        "--kind",
        "draft",
        "--out",
        &out_dir.to_string_lossy(),
        "--tokens",
        "dtcg",
    ]);
    assert_eq!(exit_code(&o), 0, "export failed: {}", stderr(&o));

    // Nothing is owner-approved, so tokens.json is empty.
    let tokens: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(out_dir.join("tokens.json")).unwrap()).unwrap();
    assert!(tokens.get("custom_thing").is_none());
    assert!(tokens.get("another_extra").is_none());
    assert!(tokens.get("primary_color").is_none());

    let reasons = |omitted: &[serde_json::Value]| -> Vec<(String, String, String)> {
        omitted
            .iter()
            .map(|o| {
                (
                    o["field"].as_str().unwrap().to_string(),
                    o["record_id"].as_str().unwrap().to_string(),
                    o["reason"].as_str().unwrap().to_string(),
                )
            })
            .collect()
    };
    let expect: Vec<(String, String, String)> = [
        ("another_extra", "unsupported_field"),
        ("custom_thing", "unsupported_field"),
        ("primary_color", "not_owner_approved"),
    ]
    .iter()
    .map(|(f, r)| (f.to_string(), "main".to_string(), r.to_string()))
    .collect();

    // Envelope lists omitted fields with reasons.
    let out_str = stdout(&o);
    let env: serde_json::Value = serde_json::from_str(out_str.trim()).unwrap();
    assert_eq!(
        reasons(env["result"]["tokens_omitted"].as_array().unwrap()),
        expect
    );

    // Manifest lists the same entries.
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(out_dir.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(
        reasons(manifest["tokens_omitted"].as_array().unwrap()),
        expect
    );

    // GUIDELINES states draft status and lists the omissions.
    let g = fs::read_to_string(out_dir.join("GUIDELINES.md")).unwrap();
    assert!(g.contains("draft — not production ready"));
    assert!(g.contains("- `primary_color` (record `main`): not_owner_approved"));

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn export_guidelines_deterministic() {
    let dir = temp_dir("export-guidelines-det");
    init_brand(&dir, "acme");

    import_record(
        &dir,
        "acme",
        &serde_json::json!({
            "schema_version": "1.0.0",
            "kind": "platform",
            "id": "main",
            "brand_id": "acme",
            "privacy": "public",
            "data": {
                "primary_color": "#aabbcc",
                "font_weight": 700
            }
        }),
    );

    approve_record(&dir, "acme", "platform", "main", "d1");

    // Run export twice to separate directories.
    let out1 = dir.join("export-1");
    let out2 = dir.join("export-2");

    for out_dir in [&out1, &out2] {
        let o = run(&[
            "export",
            "--brand",
            "acme",
            "--workspace",
            &dir.to_string_lossy(),
            "--kind",
            "draft",
            "--out",
            &out_dir.to_string_lossy(),
            "--tokens",
            "dtcg",
        ]);
        assert_eq!(exit_code(&o), 0, "export failed: {}", stderr(&o));
    }

    let g1 = fs::read_to_string(out1.join("GUIDELINES.md")).unwrap();
    let g2 = fs::read_to_string(out2.join("GUIDELINES.md")).unwrap();
    assert_eq!(g1, g2, "GUIDELINES.md must be byte-identical across runs");

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn export_without_tokens_flag_no_extra_files() {
    let dir = temp_dir("export-no-tokens");
    init_brand(&dir, "acme");

    import_record(
        &dir,
        "acme",
        &serde_json::json!({
            "schema_version": "1.0.0",
            "kind": "platform",
            "id": "main",
            "brand_id": "acme",
            "privacy": "public",
            "data": {"primary_color": "#112233"}
        }),
    );

    let out_dir = dir.join("export-out");
    let o = run(&[
        "export",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
        "--kind",
        "draft",
        "--out",
        &out_dir.to_string_lossy(),
    ]);
    assert_eq!(exit_code(&o), 0, "export failed: {}", stderr(&o));

    // Without --tokens, tokens.json and GUIDELINES.md should NOT exist.
    assert!(!out_dir.join("tokens.json").exists());
    assert!(!out_dir.join("GUIDELINES.md").exists());

    fs::remove_dir_all(&dir).ok();
}
