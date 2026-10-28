//! D-M owner confirmation: `decision record` -> pending, `decision confirm` -> attested.
//!
//! Runs the real debug binary. Tests never use the real terminal:
//! `DISTINCT_TEST_TTY_PATH` (debug only) redirects the terminal device and
//! `DISTINCT_TEST_CONFIRM_CODE` (debug only) fixes the code.

use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::SystemTime;

const CODE: &str = "7K3M9-Q2W8X";

fn run_env(args: &[&str], env: &[(&str, &str)]) -> Output {
    let mut c = Command::new(env!("CARGO_BIN_EXE_distinct-rs"));
    c.env("DISTINCT_TEST_KDF_ITERATIONS", "50")
        .env_remove("DISTINCT_TEST_CONFIRM_CODE")
        .env_remove("DISTINCT_TEST_TTY_PATH");
    for (k, v) in env {
        c.env(k, v);
    }
    c.args(args).output().expect("spawn distinct-rs")
}

fn out_s(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}
fn err_s(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
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
        .join(format!("test-dc-{name}-{nanos}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

struct Fixture {
    dir: PathBuf,
    ws: String,
    decision_file: String,
}

fn setup(name: &str) -> Fixture {
    let dir = temp_dir(name);
    let ws = dir.to_string_lossy().into_owned();
    let o = run_env(&["init", "--brand", "acme", "--workspace", &ws], &[]);
    assert_eq!(code_of(&o), 0, "{}", err_s(&o));
    let rec = json!({
        "schema_version": "1.0.0", "kind": "platform", "id": "main",
        "brand_id": "acme", "privacy": "public",
        "created_at": "2026-10-27T00:00:00Z",
        "created_by": {"kind": "agent", "id": "distinct-rs"},
        "dependencies": [], "limitations": [],
        "data": {"primary_color": "#ff5500"}
    });
    let rp = dir.join("rec.json");
    fs::write(&rp, rec.to_string()).unwrap();
    let o = run_env(
        &[
            "record-import",
            "--brand",
            "acme",
            "--workspace",
            &ws,
            "--file",
            &rp.to_string_lossy(),
        ],
        &[],
    );
    assert_eq!(code_of(&o), 0, "{}", out_s(&o));
    let hash = stored_hash(&dir, "platform", "main");
    let dec = json!({
        "schema_version": "1.0.0", "kind": "decision", "id": "d1",
        "brand_id": "acme", "privacy": "public",
        "created_at": "2026-10-27T00:00:00Z",
        "created_by": {"kind": "human", "id": "owner1"},
        "dependencies": [], "limitations": [],
        "data": {"approved": true,
            "target": {"kind": "platform", "id": "main", "content_hash": hash}}
    });
    let dp = dir.join("dec.json");
    fs::write(&dp, dec.to_string()).unwrap();
    Fixture {
        decision_file: dp.to_string_lossy().into_owned(),
        dir,
        ws,
    }
}

fn stored_hash(dir: &Path, kind: &str, id: &str) -> String {
    for e in fs::read_dir(dir.join(".distinct/acme/store/objects")).unwrap() {
        let v: Value =
            serde_json::from_str(&fs::read_to_string(e.unwrap().path()).unwrap()).unwrap();
        if v["kind"] == kind && v["id"] == id {
            return v["content_hash"].as_str().unwrap().to_string();
        }
    }
    panic!("not stored");
}

fn record_hooked(f: &Fixture) -> (Output, Option<String>) {
    let o = run_env(
        &[
            "decision",
            "record",
            "--brand",
            "acme",
            "--workspace",
            &f.ws,
            "--file",
            &f.decision_file,
            "--owner",
            "owner1",
        ],
        &[("DISTINCT_TEST_CONFIRM_CODE", CODE)],
    );
    let id = serde_json::from_str::<Value>(out_s(&o).trim())
        .ok()
        .and_then(|v| v["result"]["pending_id"].as_str().map(String::from));
    (o, id)
}

fn confirm(f: &Fixture, id: &str, code: &str) -> Output {
    run_env(
        &[
            "decision",
            "confirm",
            "--brand",
            "acme",
            "--workspace",
            &f.ws,
            "--id",
            id,
            "--code",
            code,
        ],
        &[],
    )
}

fn pending_file(f: &Fixture, id: &str) -> PathBuf {
    f.dir.join(format!(".distinct/acme/pending/{id}.json"))
}

fn decisions_stored(f: &Fixture) -> usize {
    fs::read_dir(f.dir.join(".distinct/acme/store/objects"))
        .unwrap()
        .filter(|e| {
            fs::read_to_string(e.as_ref().unwrap().path())
                .unwrap()
                .contains("\"kind\": \"decision\"")
        })
        .count()
}

#[test]
fn no_terminal_exits_2_and_stores_nothing() {
    let f = setup("noterm");
    let o = run_env(
        &[
            "decision",
            "record",
            "--brand",
            "acme",
            "--workspace",
            &f.ws,
            "--file",
            &f.decision_file,
            "--owner",
            "owner1",
        ],
        &[("DISTINCT_TEST_TTY_PATH", "/nonexistent-dir/tty")],
    );
    assert_eq!(code_of(&o), 2, "{}", out_s(&o));
    assert!(out_s(&o).contains("owner_terminal_unavailable"));
    assert!(
        !f.dir.join(".distinct/acme/pending").exists()
            || fs::read_dir(f.dir.join(".distinct/acme/pending"))
                .unwrap()
                .count()
                == 0
    );
    assert_eq!(decisions_stored(&f), 0);
    fs::remove_dir_all(&f.dir).ok();
}

#[test]
fn real_code_goes_to_terminal_only() {
    let f = setup("tty");
    let tty = f.dir.join("fake-tty");
    fs::write(&tty, "").unwrap();
    let o = run_env(
        &[
            "decision",
            "record",
            "--brand",
            "acme",
            "--workspace",
            &f.ws,
            "--file",
            &f.decision_file,
            "--owner",
            "owner1",
        ],
        &[("DISTINCT_TEST_TTY_PATH", &tty.to_string_lossy())],
    );
    assert_eq!(code_of(&o), 0, "{}", out_s(&o));
    let shown = fs::read_to_string(&tty).unwrap();
    let code = shown.split("One-time code: ").nth(1).unwrap()[..11].to_string();
    assert_eq!(code.as_bytes()[5], b'-');
    let bare = code.replace('-', "");
    assert!(!out_s(&o).contains(&bare) && !err_s(&o).contains(&bare));
    let id = serde_json::from_str::<Value>(out_s(&o).trim()).unwrap()["result"]["pending_id"]
        .as_str()
        .unwrap()
        .to_string();
    let pending = fs::read_to_string(pending_file(&f, &id)).unwrap();
    assert!(
        !pending.contains(&bare) && !pending.contains(&code),
        "pending must hold only a salted verifier"
    );
    let pv: Value = serde_json::from_str(&pending).unwrap();
    assert!(pv["salt"].is_string() && pv["verifier"].is_string() && pv["iterations"].is_u64());
    // Lowercase, no hyphen are accepted.
    let c = confirm(&f, &id, &bare.to_lowercase());
    assert_eq!(code_of(&c), 0, "{}", out_s(&c));
    fs::remove_dir_all(&f.dir).ok();
}

#[test]
fn hook_record_then_right_code_stores_attested_decision() {
    let f = setup("right");
    let (o, id) = record_hooked(&f);
    assert_eq!(code_of(&o), 0, "{}", out_s(&o));
    let id = id.unwrap();
    assert!(pending_file(&f, &id).exists());
    assert_eq!(
        decisions_stored(&f),
        0,
        "record must not create an approval"
    );
    assert!(!out_s(&o).contains(CODE) && !err_s(&o).contains(CODE));

    let c = confirm(&f, &id, CODE);
    assert_eq!(code_of(&c), 0, "{}", out_s(&c));
    assert!(!out_s(&c).contains(CODE) && !err_s(&c).contains(CODE));
    assert!(!pending_file(&f, &id).exists());
    assert_eq!(decisions_stored(&f), 1);

    let dec_text = fs::read_dir(f.dir.join(".distinct/acme/store/objects"))
        .unwrap()
        .map(|e| fs::read_to_string(e.unwrap().path()).unwrap())
        .find(|t| t.contains("\"kind\": \"decision\""))
        .unwrap();
    let dec: Value = serde_json::from_str(&dec_text).unwrap();
    assert_eq!(dec["attestation"]["method"], "terminal_code");
    assert!(dec["attestation"]["confirmed_at"].is_string());

    let v = run_env(&["verify", "--brand", "acme", "--workspace", &f.ws], &[]);
    assert_eq!(code_of(&v), 0, "{}", out_s(&v));
    let g = run_env(&["gate", "--brand", "acme", "--workspace", &f.ws], &[]);
    let gv: Value = serde_json::from_str(out_s(&g).trim()).unwrap();
    assert!(gv["result"]["warnings"].as_array().unwrap().is_empty());
    fs::remove_dir_all(&f.dir).ok();
}

#[test]
fn wrong_code_exits_3_and_counts_attempts() {
    let f = setup("wrong");
    let id = record_hooked(&f).1.unwrap();
    let c = confirm(&f, &id, "00000-00000");
    assert_eq!(code_of(&c), 3, "{}", out_s(&c));
    let p: Value =
        serde_json::from_str(&fs::read_to_string(pending_file(&f, &id)).unwrap()).unwrap();
    assert_eq!(p["attempts"], 1);
    assert_eq!(decisions_stored(&f), 0);
    fs::remove_dir_all(&f.dir).ok();
}

#[test]
fn expired_pending_exits_2() {
    let f = setup("expired");
    let id = record_hooked(&f).1.unwrap();
    let path = pending_file(&f, &id);
    let mut p: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    p["expires_at"] = json!(1);
    fs::write(&path, p.to_string()).unwrap();
    let c = confirm(&f, &id, CODE);
    assert_eq!(code_of(&c), 2, "{}", out_s(&c));
    assert_eq!(decisions_stored(&f), 0);
    fs::remove_dir_all(&f.dir).ok();
}

#[test]
fn sixth_attempt_exits_2_even_with_right_code() {
    let f = setup("attempts");
    let id = record_hooked(&f).1.unwrap();
    for _ in 0..5 {
        assert_eq!(code_of(&confirm(&f, &id, "00000-00000")), 3);
    }
    let c = confirm(&f, &id, CODE);
    assert_eq!(code_of(&c), 2, "{}", out_s(&c));
    assert_eq!(decisions_stored(&f), 0);
    fs::remove_dir_all(&f.dir).ok();
}

#[test]
fn bad_pending_id_and_agent_attestation_rejected() {
    let f = setup("badid");
    let c = confirm(&f, "../../etc/passwd", CODE);
    assert_eq!(code_of(&c), 3);
    // An agent-supplied attestation in the input file is refused.
    let mut d: Value =
        serde_json::from_str(&fs::read_to_string(&f.decision_file).unwrap()).unwrap();
    d["attestation"] = json!({"method": "terminal_code"});
    fs::write(&f.decision_file, d.to_string()).unwrap();
    let (o, _) = record_hooked(&f);
    assert_eq!(code_of(&o), 3, "{}", out_s(&o));
    fs::remove_dir_all(&f.dir).ok();
}

// ── gate: owner approval blocks readiness ────────────────────────────

fn gate_blockers(f: &Fixture, scope: &str) -> (bool, Vec<Value>) {
    let g = run_env(
        &[
            "gate",
            "--brand",
            "acme",
            "--workspace",
            &f.ws,
            "--scope",
            scope,
        ],
        &[],
    );
    assert_eq!(code_of(&g), 0, "{}", out_s(&g));
    let v: Value = serde_json::from_str(out_s(&g).trim()).unwrap();
    (
        v["result"]["ready"].as_bool().unwrap(),
        v["result"]["blockers"].as_array().unwrap().clone(),
    )
}

fn approval_codes(blockers: &[Value]) -> Vec<(String, String, String)> {
    blockers
        .iter()
        .filter_map(|b| {
            Some((
                b["code"].as_str()?.to_string(),
                b["kind"].as_str()?.to_string(),
                b["id"].as_str()?.to_string(),
            ))
        })
        .collect()
}

#[test]
fn gate_unapproved_platform_is_blocked() {
    let f = setup("gate-missing");
    let (ready, blockers) = gate_blockers(&f, "platform");
    assert!(!ready);
    assert_eq!(
        approval_codes(&blockers),
        vec![(
            "missing_owner_approval".into(),
            "platform".into(),
            "main".into()
        )]
    );
    // Pending (unconfirmed) decisions do not count either.
    record_hooked(&f);
    assert!(!gate_blockers(&f, "platform").0);
    // Unrelated scopes add no approval requirement.
    assert!(gate_blockers(&f, "voice").0);
    fs::remove_dir_all(&f.dir).ok();
}

#[test]
fn gate_attested_approval_is_ready() {
    let f = setup("gate-approved");
    let id = record_hooked(&f).1.unwrap();
    assert_eq!(code_of(&confirm(&f, &id, CODE)), 0);
    for scope in ["platform", "deliver", "default"] {
        let (ready, blockers) = gate_blockers(&f, scope);
        assert!(ready, "scope {scope}: {blockers:?}");
        assert!(blockers.is_empty());
    }
    fs::remove_dir_all(&f.dir).ok();
}

#[test]
fn gate_approval_of_older_revision_is_stale() {
    let f = setup("gate-stale");
    let id = record_hooked(&f).1.unwrap();
    assert_eq!(code_of(&confirm(&f, &id, CODE)), 0);
    let first = stored_hash(&f.dir, "platform", "main");

    let rec = json!({
        "schema_version": "1.0.0", "kind": "platform", "id": "main",
        "brand_id": "acme", "privacy": "public",
        "created_at": "2026-10-28T00:00:00Z",
        "created_by": {"kind": "agent", "id": "distinct-rs"},
        "supersedes": first,
        "dependencies": [], "limitations": [],
        "data": {"primary_color": "#00aa55"}
    });
    let rp = f.dir.join("rec2.json");
    fs::write(&rp, rec.to_string()).unwrap();
    let o = run_env(
        &[
            "record-import",
            "--brand",
            "acme",
            "--workspace",
            &f.ws,
            "--file",
            &rp.to_string_lossy(),
        ],
        &[],
    );
    assert_eq!(code_of(&o), 0, "{}", out_s(&o));

    let (ready, blockers) = gate_blockers(&f, "platform");
    assert!(!ready);
    assert_eq!(
        approval_codes(&blockers),
        vec![(
            "stale_owner_approval".into(),
            "platform".into(),
            "main".into()
        )]
    );
    fs::remove_dir_all(&f.dir).ok();
}
