//! WO-21: the gate refuses `direction`-covering scopes until an inspiration
//! board holds at least one fetched, observed reference with live evidence.

use serde_json::{json, Value};
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

fn out_s(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn code_of(o: &Output) -> i32 {
    o.status.code().expect("exit code")
}

fn temp_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target")
        .join(format!("test-insp-{name}-{nanos}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn setup(name: &str) -> PathBuf {
    let dir = temp_dir(name);
    let o = run(&[
        "init",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
    ]);
    assert_eq!(code_of(&o), 0, "{}", out_s(&o));
    dir
}

fn add_evidence(dir: &Path, id: &str) {
    let f = dir.join(format!("{id}.png"));
    fs::write(&f, format!("bytes for {id}")).unwrap();
    let o = run(&[
        "evidence-import",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
        "--file",
        &f.to_string_lossy(),
        "--source-id",
        id,
    ]);
    assert_eq!(code_of(&o), 0, "{}", out_s(&o));
}

fn import_board(dir: &Path, references: &Value) {
    let board = json!({
        "schema_version": "1.0.0", "kind": "source", "id": "inspiration-board",
        "brand_id": "acme", "privacy": "public",
        "created_at": "2026-10-27T00:00:00Z",
        "created_by": {"kind": "agent", "id": "distinct-rs"},
        "dependencies": [], "limitations": [],
        "data": {"type": "inspiration_board", "origin": "user_supplied",
            "references": references}
    });
    let p = dir.join("board.json");
    fs::write(&p, board.to_string()).unwrap();
    let o = run(&[
        "record-import",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
        "--file",
        &p.to_string_lossy(),
    ]);
    assert_eq!(code_of(&o), 0, "{}", out_s(&o));
}

fn fetched(ref_id: &str, evidence: &[&str]) -> Value {
    json!({"ref_id": ref_id, "url": "https://example.com", "label": "x",
        "stance": "like", "status": "fetched", "evidence_ids": evidence,
        "observations": ["condensed grotesk, all caps, heavy rules"],
        "user_reaction": null, "unreachable_reason": null})
}

fn unreachable_ref(ref_id: &str) -> Value {
    json!({"ref_id": ref_id, "url": "https://blocked.example", "label": "y",
        "stance": "dislike", "status": "unreachable", "evidence_ids": [],
        "observations": [], "user_reaction": null,
        "unreachable_reason": "blocked by bot wall"})
}

fn gate(dir: &Path, scope: &str) -> Value {
    let o = run(&[
        "gate",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
        "--scope",
        scope,
    ]);
    assert_eq!(code_of(&o), 0, "{}", out_s(&o));
    serde_json::from_str::<Value>(out_s(&o).trim()).unwrap()["result"].clone()
}

fn codes(result: &Value) -> Vec<String> {
    result["blockers"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|b| b["code"].as_str().map(String::from))
        .filter(|c| c.starts_with("inspiration_"))
        .collect()
}

#[test]
fn no_board_is_inspiration_missing() {
    let dir = setup("missing");
    for scope in ["direction", "deliver", "default"] {
        let r = gate(&dir, scope);
        assert_eq!(codes(&r), vec!["inspiration_missing"], "scope {scope}");
        assert_eq!(r["ready"], false);
    }
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn only_unreachable_is_unseen_with_advisory() {
    let dir = setup("unseen");
    import_board(&dir, &json!([unreachable_ref("r1")]));
    let r = gate(&dir, "direction");
    assert_eq!(codes(&r), vec!["inspiration_unseen"]);
    assert_eq!(r["ready"], false);
    let warnings = r["warnings"].as_array().unwrap();
    assert!(
        warnings.iter().any(
            |w| w.as_str().unwrap().starts_with("inspiration_unreachable")
                && w.as_str().unwrap().contains("r1")
        ),
        "{warnings:?}"
    );
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn forgotten_evidence_is_dangling() {
    let dir = setup("dangling");
    add_evidence(&dir, "insp-r1");
    add_evidence(&dir, "insp-r2");
    import_board(
        &dir,
        &json!([fetched("r1", &["insp-r1"]), fetched("r2", &["insp-r2"])]),
    );
    assert!(codes(&gate(&dir, "direction")).is_empty());

    let o = run(&[
        "evidence",
        "forget",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
        "--id",
        "insp-r2",
        "--reason",
        "test",
    ]);
    assert_eq!(code_of(&o), 0, "{}", out_s(&o));

    let r = gate(&dir, "direction");
    let dangling: Vec<&Value> = r["blockers"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|b| b["code"] == "inspiration_evidence_dangling")
        .collect();
    assert_eq!(dangling.len(), 1, "{r}");
    assert_eq!(dangling[0]["ref_id"], "r2");
    // r1 is still seen, so the board is not "unseen".
    assert!(!codes(&r).contains(&"inspiration_unseen".to_string()));
    assert_eq!(r["ready"], false);
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn fetched_without_evidence_or_observations_is_unseen() {
    let dir = setup("empty-fetched");
    let mut r1 = fetched("r1", &[]);
    r1["observations"] = json!(["  "]);
    import_board(&dir, &json!([r1]));
    assert_eq!(codes(&gate(&dir, "direction")), vec!["inspiration_unseen"]);
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn valid_board_has_no_inspiration_blockers() {
    let dir = setup("valid");
    add_evidence(&dir, "insp-r1");
    import_board(
        &dir,
        &json!([fetched("r1", &["insp-r1"]), unreachable_ref("r2")]),
    );
    for scope in ["direction", "deliver", "default"] {
        let r = gate(&dir, scope);
        assert!(codes(&r).is_empty(), "scope {scope}: {r}");
        assert_eq!(r["ready"], true, "scope {scope}: {r}");
    }
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn voice_scope_is_unaffected() {
    let dir = setup("voice");
    let r = gate(&dir, "voice");
    assert!(codes(&r).is_empty());
    assert_eq!(r["ready"], true);
    assert!(r["warnings"].as_array().unwrap().is_empty());
    fs::remove_dir_all(&dir).ok();
}
