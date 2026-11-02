//! Integration tests for `name check` (WO-25). Runs the real `distinct-rs` binary.

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

fn temp_file(name: &str, body: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target")
        .join(format!("test-namecheck-{name}-{nanos}"));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("candidates.json");
    fs::write(&path, body).unwrap();
    path
}

fn check(name: &str, body: &str) -> (i32, serde_json::Value) {
    let path = temp_file(name, body);
    let o = run(&[
        "name",
        "check",
        "--candidates",
        &path.to_string_lossy(),
        "--workspace",
        ".",
        "--brand",
        "acme",
    ]);
    let v = serde_json::from_slice(&o.stdout).expect("json stdout");
    (o.status.code().expect("exit code"), v)
}

fn candidates(v: &serde_json::Value) -> &Vec<serde_json::Value> {
    v["result"]["candidates"].as_array().expect("candidates")
}

#[test]
fn japanese_and_korean_are_single_script() {
    // Han+Hiragana+Katakana (Jpan) and Hangul+Han (Kore) are UTS #39
    // augmented single scripts, not mixed-script red flags.
    let (code, v) = check(
        "cjk",
        "{\"candidates\":[\"\u{6771}\u{4EAC}\u{3066}\u{3043}\u{30FC}\",\"\u{D55C}\u{570B}\u{C5B4}\"]}",
    );
    assert_eq!(code, 0);
    let c = candidates(&v);
    assert_eq!(c[0]["mixed_script"], false, "{v}");
    assert_eq!(c[1]["mixed_script"], false, "{v}");
}

#[test]
fn cyrillic_lookalike_shares_skeleton_and_is_mixed_script() {
    let (code, v) = check(
        "lookalike",
        "{\"candidates\":[\"Larkspur\",\"L\u{0430}rkspur\"]}",
    );
    assert_eq!(code, 0);
    let c = candidates(&v);
    assert_eq!(c[0]["confusable_skeleton"], c[1]["confusable_skeleton"]);
    assert_eq!(c[0]["confusable_with"][0], "L\u{0430}rkspur");
    assert_eq!(c[1]["confusable_with"][0], "Larkspur");
    assert_eq!(c[0]["mixed_script"], false);
    assert_eq!(c[1]["mixed_script"], true);
    assert_eq!(c[1]["non_ascii"], true);
    assert_eq!(c[0]["non_ascii"], false);
    assert_eq!(c[0]["length_chars"], 8);
}

#[test]
fn nfkc_change_detected_for_fullwidth_letters() {
    let (code, v) = check(
        "nfkc",
        "{\"candidates\":[\"\u{ff2c}arkspur\",\"Larkspur\"]}",
    );
    assert_eq!(code, 0);
    let c = candidates(&v);
    assert_eq!(c[0]["changed_by_nfkc"], true);
    assert_eq!(c[0]["normalized"], "Larkspur");
    assert_eq!(c[1]["changed_by_nfkc"], false);
}

#[test]
fn flag_hits_exact_and_substring() {
    let (code, v) = check(
        "flags",
        r#"{"candidates":["Fraudly","ASS","Bassline","Larkspur","AnalCo","Grape Analytics","Spice Tycoon"],"flag_lists":["default"]}"#,
    );
    assert_eq!(code, 0);
    let c = candidates(&v);
    let hit = &c[0]["flag_hits"][0];
    assert_eq!(hit["list"], "default");
    assert_eq!(hit["term"], "fraud");
    assert_eq!(hit["match"], "substring");
    let hit = &c[1]["flag_hits"][0];
    assert_eq!(hit["term"], "ass");
    assert_eq!(hit["match"], "exact");
    // "=" terms match whole words only, never inside a longer word.
    assert!(c[2]["flag_hits"].as_array().unwrap().is_empty());
    assert!(c[3]["flag_hits"].as_array().unwrap().is_empty());
    let hit = &c[4]["flag_hits"][0];
    assert_eq!(hit["term"], "anal");
    assert_eq!(hit["match"], "word");
    assert!(c[5]["flag_hits"].as_array().unwrap().is_empty(), "{}", c[5]);
    assert!(c[6]["flag_hits"].as_array().unwrap().is_empty(), "{}", c[6]);
}

#[test]
fn limitations_always_present_and_no_verdict() {
    let (code, v) = check("limits", r#"{"candidates":["Larkspur"]}"#);
    assert_eq!(code, 0);
    let want = [
        "offline_heuristic_only",
        "not_trademark_clearance",
        "not_linguistic_screening",
        "flag_list_incomplete",
    ];
    for key in [&v["limitations"], &v["result"]["limitations"]] {
        let got: Vec<&str> = key
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap())
            .collect();
        assert_eq!(got, want);
    }
    let c = &candidates(&v)[0];
    for banned in ["verdict", "score", "pass", "safe"] {
        assert!(c.get(banned).is_none(), "unexpected field {banned}");
    }
}

#[test]
fn bad_input_exits_3() {
    assert_eq!(check("badjson", "{not json").0, 3);
    assert_eq!(check("empty", r#"{"candidates":[]}"#).0, 3);
    assert_eq!(check("blank", r#"{"candidates":["  "]}"#).0, 3);
    assert_eq!(check("unknown-field", r#"{"candidates":["a"],"x":1}"#).0, 3);
    assert_eq!(
        check(
            "unknown-list",
            r#"{"candidates":["a"],"flag_lists":["nope"]}"#
        )
        .0,
        3
    );
    let o = run(&["name", "check", "--brand", "acme"]);
    assert_eq!(o.status.code(), Some(3));
    let o = run(&["name", "check", "--candidates", "/nonexistent.json"]);
    assert_eq!(o.status.code(), Some(3));
}

#[test]
fn command_module_makes_no_network_calls() {
    let src = include_str!("../src/cmd_name.rs");
    for banned in ["std::net", "reqwest", "TcpStream", "UdpSocket", "hyper"] {
        // The needle list lives in this test file, not in the module under test.
        assert!(!src.contains(banned), "cmd_name.rs mentions {banned}");
    }
    let manifest = include_str!("../Cargo.toml");
    for banned in ["reqwest", "ureq", "hyper", "curl", "isahc"] {
        assert!(!manifest.contains(banned), "Cargo.toml mentions {banned}");
    }
}
