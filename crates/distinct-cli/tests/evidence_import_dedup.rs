//! Evidence import deduplication: reusing a source id must not silently orphan
//! the earlier file's bytes. Cases 1-4 from the contract.

use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::SystemTime;

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_distinct-rs"))
        .args(args)
        .output()
        .expect("spawn distinct-rs")
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn exit_code(o: &Output) -> i32 {
    o.status.code().expect("exit code")
}

fn json_out(o: &Output) -> Value {
    serde_json::from_str(stdout(o).trim()).expect("valid json")
}

fn temp_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target")
        .join(format!("test-evid-dedup-{name}-{nanos}"));
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
    assert_eq!(exit_code(&o), 0, "{}", stdout(&o));
    dir
}

fn write_file(dir: &Path, name: &str, content: &[u8]) -> PathBuf {
    let p = dir.join(name);
    fs::write(&p, content).unwrap();
    p
}

fn import(dir: &Path, file: &Path, source_id: &str) -> Output {
    run(&[
        "evidence-import",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
        "--file",
        &file.to_string_lossy(),
        "--source-id",
        source_id,
    ])
}

fn import_replace(dir: &Path, file: &Path, source_id: &str) -> Output {
    run(&[
        "evidence-import",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
        "--file",
        &file.to_string_lossy(),
        "--source-id",
        source_id,
        "--replace",
    ])
}

use std::path::Path;

#[test]
fn case1_fresh_import_succeeds() {
    let dir = setup("case1");
    let f = write_file(&dir, "a.bin", b"first bytes");
    let o = import(&dir, &f, "src-001");
    assert_eq!(exit_code(&o), 0, "{}", stdout(&o));
    let j = json_out(&o);
    assert_eq!(j["result"]["source_id"], "src-001");
    assert!(j["result"]["byte_hash"].as_str().is_some());
    assert!(j["result"]["unchanged"].is_null());
    assert!(j["result"]["replaced"].is_null());
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn case2_identical_bytes_are_idempotent() {
    let dir = setup("case2");
    let f = write_file(&dir, "a.bin", b"same content");
    let o1 = import(&dir, &f, "src-002");
    assert_eq!(exit_code(&o1), 0, "{}", stdout(&o1));
    let hash1 = json_out(&o1)["result"]["byte_hash"]
        .as_str()
        .unwrap()
        .to_owned();

    let o2 = import(&dir, &f, "src-002");
    assert_eq!(exit_code(&o2), 0, "{}", stdout(&o2));
    let j2 = json_out(&o2);
    assert_eq!(j2["result"]["byte_hash"].as_str().unwrap(), hash1);
    assert_eq!(j2["result"]["unchanged"], true);
    assert!(j2["result"]["replaced"].is_null());
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn case3_different_bytes_exit_3_without_replace() {
    let dir = setup("case3");
    let f1 = write_file(&dir, "a.bin", b"original bytes");
    let o1 = import(&dir, &f1, "src-003");
    assert_eq!(exit_code(&o1), 0, "{}", stdout(&o1));
    let orig_hash = json_out(&o1)["result"]["byte_hash"]
        .as_str()
        .unwrap()
        .to_owned();

    let f2 = write_file(&dir, "b.bin", b"different bytes");
    let o2 = import(&dir, &f2, "src-003");
    assert_eq!(exit_code(&o2), 3, "expected exit 3, got: {}", stdout(&o2));
    let j2 = json_out(&o2);
    let err = j2["error"].as_str().unwrap();
    assert!(
        err.contains("src-003") && err.contains("already holds different evidence"),
        "unexpected error text: {err}"
    );
    assert!(
        err.contains("--replace"),
        "error should mention --replace: {err}"
    );

    // Original index entry is intact.
    let idx_path = dir.join(".distinct/acme/evidence/index/src-003.json");
    let idx: Value = serde_json::from_str(&fs::read_to_string(&idx_path).unwrap()).unwrap();
    assert_eq!(idx["byte_hash"].as_str().unwrap(), orig_hash);
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn case4_replace_flag_overwrites_and_reports_previous_hash() {
    let dir = setup("case4");
    let f1 = write_file(&dir, "a.bin", b"original bytes");
    let o1 = import(&dir, &f1, "src-004");
    assert_eq!(exit_code(&o1), 0, "{}", stdout(&o1));
    let orig_hash = json_out(&o1)["result"]["byte_hash"]
        .as_str()
        .unwrap()
        .to_owned();

    let f2 = write_file(&dir, "b.bin", b"replacement bytes");
    let o2 = import_replace(&dir, &f2, "src-004");
    assert_eq!(exit_code(&o2), 0, "{}", stdout(&o2));
    let j2 = json_out(&o2);
    assert_eq!(j2["result"]["replaced"], true);
    assert_eq!(
        j2["result"]["previous_byte_hash"].as_str().unwrap(),
        orig_hash
    );
    let new_hash = j2["result"]["byte_hash"].as_str().unwrap();
    assert_ne!(new_hash, orig_hash);

    // Index entry now holds the new hash.
    let idx_path = dir.join(".distinct/acme/evidence/index/src-004.json");
    let idx: Value = serde_json::from_str(&fs::read_to_string(&idx_path).unwrap()).unwrap();
    assert_eq!(idx["byte_hash"].as_str().unwrap(), new_hash);
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn traversal_source_id_is_rejected_before_any_read() {
    let dir = setup("dedup-traversal");
    // A JSON file outside the evidence index that a `../` id would reach.
    fs::write(
        dir.join(".distinct/acme/outside.json"),
        br#"{"byte_hash":"x"}"#,
    )
    .unwrap();
    let file = write_file(&dir, "a.txt", b"payload");
    let o = import(&dir, &file, "../../outside");
    assert_eq!(exit_code(&o), 3, "{}", stdout(&o));
    assert!(stdout(&o).contains("invalid source_id"), "{}", stdout(&o));
    fs::remove_dir_all(&dir).ok();
}
