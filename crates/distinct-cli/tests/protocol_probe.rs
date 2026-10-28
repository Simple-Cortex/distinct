//! Integration tests that execute the real `distinct-rs` binary.

use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_distinct-rs"))
        .args(args)
        .output()
        .expect("spawn distinct-rs")
}

fn stdout(o: &Output) -> String {
    String::from_utf8(o.stdout.clone()).expect("utf-8 stdout")
}

#[test]
fn protocol_probe_real_binary() {
    let o = run(&["protocol"]);
    assert_eq!(o.status.code(), Some(0));
    assert!(o.stderr.is_empty(), "stderr must be empty");
    let out = stdout(&o);
    assert_eq!(out.lines().count(), 1, "one JSON envelope on stdout");
    for needle in [
        "\"protocol_version\":\"distinct/1.0\"",
        "\"engine_version\":\"0.2.0\"",
        "\"result\":{\"protocol\":\"distinct/1.0\",\"version\":\"0.2.0\"}",
        "\"error\":null",
    ] {
        assert!(out.contains(needle), "missing {needle} in {out}");
    }
}

#[test]
fn version_real_binary() {
    let o = run(&["--version"]);
    assert_eq!(o.status.code(), Some(0));
    assert_eq!(stdout(&o), "distinct-rs 0.2.0\n");
}

#[test]
fn help_real_binary() {
    let o = run(&["--help"]);
    assert_eq!(o.status.code(), Some(0));
    assert!(stdout(&o).contains("Usage: distinct-rs"));
}

#[test]
fn unknown_command_exits_nonzero_with_message() {
    let o = run(&["frobnicate"]);
    assert_eq!(o.status.code(), Some(3));
    assert!(o.stdout.is_empty());
    let err = String::from_utf8(o.stderr).expect("utf-8 stderr");
    assert!(err.contains("unknown command 'frobnicate'"), "{err}");
}
