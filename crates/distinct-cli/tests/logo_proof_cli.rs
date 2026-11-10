//! Integration tests for `logo` / `color` subcommand routing and `logo proof`
//! profile + `--out` handling. Runs the real `distinct-rs` binary.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::SystemTime;

const SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><rect x="10" y="10" width="80" height="80" fill="blue"/></svg>"#;

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_distinct-rs"))
        .args(args)
        .output()
        .expect("spawn distinct-rs")
}

fn code(o: &Output) -> i32 {
    o.status.code().expect("exit code")
}

fn json(o: &Output) -> serde_json::Value {
    serde_json::from_slice(&o.stdout).expect("json stdout")
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
    // Canonical so paths passed to `--out` carry no `..` components.
    dir.canonicalize().unwrap()
}

/// Initialises a brand and writes the SVG; returns (workspace, svg path).
fn setup(name: &str) -> (PathBuf, PathBuf) {
    let dir = temp_dir(name);
    let o = run(&[
        "init",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
    ]);
    assert_eq!(code(&o), 0);
    let svg = dir.join("logo.svg");
    fs::write(&svg, SVG).unwrap();
    (dir, svg)
}

fn inspect_space(dir: &Path, svg: &Path) -> Output {
    run(&[
        "logo",
        "inspect",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
        "--file",
        &svg.to_string_lossy(),
    ])
}

fn png_dims(path: &Path) -> (u32, u32) {
    let b = fs::read(path).unwrap();
    assert_eq!(&b[1..4], b"PNG");
    let w = u32::from_be_bytes(b[16..20].try_into().unwrap());
    let h = u32::from_be_bytes(b[20..24].try_into().unwrap());
    (w, h)
}

fn proof(dir: &Path, svg: &Path, out: &Path, extra: &[&str]) -> Output {
    let (d, s, o) = (
        dir.to_string_lossy(),
        svg.to_string_lossy(),
        out.to_string_lossy(),
    );
    let mut args = vec![
        "logo",
        "proof",
        "--brand",
        "acme",
        "--workspace",
        &d,
        "--file",
        &s,
        "--out",
        &o,
    ];
    args.extend_from_slice(extra);
    run(&args)
}

#[test]
fn logo_inspect_space_form_works() {
    let (dir, svg) = setup("logo-inspect-space");
    let o = inspect_space(&dir, &svg);
    assert_eq!(code(&o), 0, "{}", String::from_utf8_lossy(&o.stdout));
    assert_eq!(json(&o)["execution"]["command"], "logo-inspect");
}

#[test]
fn logo_proof_space_form_renders_profile_sizes() {
    let (dir, svg) = setup("logo-proof-profile");
    assert_eq!(code(&inspect_space(&dir, &svg)), 0);
    let profile = dir.join("profile.json");
    fs::write(
        &profile,
        r##"{"renders":[{"size_px":16,"background":"transparent"},{"size_px":64,"background":"#ffffff"}]}"##,
    )
    .unwrap();
    let out = dir.join("out");
    let o = proof(&dir, &svg, &out, &["--profile", &profile.to_string_lossy()]);
    assert_eq!(code(&o), 0, "{}", String::from_utf8_lossy(&o.stdout));
    assert_eq!(json(&o)["result"]["render_count"], 2);

    let mut dims: Vec<(u32, u32)> = fs::read_dir(&out)
        .unwrap()
        .map(|e| png_dims(&e.unwrap().path()))
        .collect();
    dims.sort_unstable();
    assert_eq!(dims, vec![(16, 16), (64, 64)]);

    // Non-empty --out is refused without --force, accepted with it.
    let o = proof(&dir, &svg, &out, &["--profile", &profile.to_string_lossy()]);
    assert_eq!(code(&o), 3);
    let o = proof(
        &dir,
        &svg,
        &out,
        &["--profile", &profile.to_string_lossy(), "--force"],
    );
    assert_eq!(code(&o), 0);
}

#[test]
fn logo_proof_default_profile_without_flag() {
    let (dir, svg) = setup("logo-proof-default");
    assert_eq!(code(&inspect_space(&dir, &svg)), 0);
    let out = dir.join("out");
    let o = proof(&dir, &svg, &out, &[]);
    assert_eq!(code(&o), 0);
    assert_eq!(fs::read_dir(&out).unwrap().count(), 3);
}

#[test]
fn logo_proof_rejects_invalid_profile_and_traversal() {
    let (dir, svg) = setup("logo-proof-invalid");
    assert_eq!(code(&inspect_space(&dir, &svg)), 0);
    let out = dir.join("out");
    for (i, bad) in [
        r#"{"renders":[{"size_px":5000,"background":"transparent"}]}"#,
        r#"{"renders":[]}"#,
        r#"{"renders":[{"size_px":0,"background":"transparent"}]}"#,
        r#"{"renders":[{"size_px":32,"background":"red"}]}"#,
        "not json",
    ]
    .iter()
    .enumerate()
    {
        let profile = dir.join(format!("bad{i}.json"));
        fs::write(&profile, bad).unwrap();
        let o = proof(&dir, &svg, &out, &["--profile", &profile.to_string_lossy()]);
        assert_eq!(code(&o), 3, "profile {bad}");
    }
    assert!(!out.exists(), "no output written for invalid profiles");

    let trav = dir.join("out").join("..").join("escape");
    let o = proof(&dir, &svg, &trav, &[]);
    assert_eq!(code(&o), 3);
}

#[test]
fn color_check_space_form_stores_record() {
    let (dir, _svg) = setup("color-space");
    let pairs = dir.join("pairs.txt");
    fs::write(&pairs, "000000 ffffff\n777777 ffffff\n").unwrap();
    let o = run(&[
        "color",
        "check",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
        "--pairs",
        &pairs.to_string_lossy(),
    ]);
    assert_eq!(code(&o), 0, "{}", String::from_utf8_lossy(&o.stdout));
    let v = json(&o);
    assert_eq!(v["result"]["pair_count"], 2);
    assert!(v["result"]["check_id"].is_string());
}

#[test]
fn bare_logo_and_color_are_usage_errors() {
    assert_eq!(code(&run(&["logo"])), 3);
    assert_eq!(code(&run(&["color"])), 3);
}

/// Finds the stored `logo_proof` record under a workspace.
fn find_proof_record(dir: &Path) -> serde_json::Value {
    fn walk(dir: &Path, found: &mut Option<serde_json::Value>) {
        for e in fs::read_dir(dir).unwrap() {
            let path = e.unwrap().path();
            if path.is_dir() {
                walk(&path, found);
            } else if let Ok(text) = fs::read_to_string(&path) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                    if v["kind"] == "logo_proof" {
                        *found = Some(v);
                    }
                }
            }
        }
    }
    let mut found = None;
    walk(dir, &mut found);
    found.expect("logo_proof record in workspace")
}

#[cfg(target_os = "macos")]
#[test]
fn logo_proof_succeeds_under_seatbelt_and_records_it() {
    let (dir, svg) = setup("logo-proof-seatbelt");
    assert_eq!(code(&inspect_space(&dir, &svg)), 0);
    let out = dir.join("out");
    let o = proof(&dir, &svg, &out, &[]);
    assert_eq!(code(&o), 0, "{}", String::from_utf8_lossy(&o.stdout));
    assert_eq!(fs::read_dir(&out).unwrap().count(), 3);
    assert_eq!(json(&o)["limitations"], serde_json::json!([]));
    let record = find_proof_record(&dir);
    assert_eq!(record["data"]["os_sandbox"], "seatbelt");
}

/// D-N: under an outer seatbelt the inner `sandbox-exec` refuses to nest, so
/// a trusted probe detects the nesting refusal up front, the proof runs once without
/// sandbox-exec and records `host_inherited`.
#[cfg(target_os = "macos")]
#[test]
fn logo_proof_records_host_inherited_under_outer_seatbelt() {
    const OUTER: &str = "/usr/bin/sandbox-exec";
    if !Path::new(OUTER).exists() {
        return;
    }
    let (dir, svg) = setup("logo-proof-host-inherited");
    assert_eq!(code(&inspect_space(&dir, &svg)), 0);
    let out = dir.join("out");
    let (d, s, o) = (
        dir.to_string_lossy(),
        svg.to_string_lossy(),
        out.to_string_lossy(),
    );
    let Ok(res) = Command::new(OUTER)
        .args(["-p", "(version 1)(allow default)"])
        .arg(env!("CARGO_BIN_EXE_distinct-rs"))
        .args([
            "logo",
            "proof",
            "--brand",
            "acme",
            "--workspace",
            &d,
            "--file",
            &s,
            "--out",
            &o,
        ])
        .output()
    else {
        return;
    };
    let stderr = String::from_utf8_lossy(&res.stderr);
    if stderr.contains("sandbox-exec: sandbox_apply") && res.stdout.is_empty() {
        // The outer sandbox itself could not be applied; nothing to test.
        return;
    }
    assert_eq!(code(&res), 0, "{}", String::from_utf8_lossy(&res.stdout));
    assert_eq!(
        json(&res)["limitations"],
        serde_json::json!(["os_sandbox_host_inherited"])
    );
    assert_eq!(fs::read_dir(&out).unwrap().count(), 3);
    let record = find_proof_record(&dir);
    assert_eq!(record["data"]["os_sandbox"], "host_inherited");
    assert_eq!(
        record["limitations"],
        serde_json::json!(["os_sandbox_host_inherited"])
    );
}

#[cfg(not(target_os = "macos"))]
#[test]
fn logo_proof_reports_no_os_sandbox_off_macos() {
    let (dir, svg) = setup("logo-proof-nosandbox");
    assert_eq!(code(&inspect_space(&dir, &svg)), 0);
    let o = proof(&dir, &svg, &dir.join("out"), &[]);
    assert_eq!(code(&o), 0, "{}", String::from_utf8_lossy(&o.stdout));
    assert_eq!(
        json(&o)["limitations"],
        serde_json::json!(["os_sandbox_unavailable"])
    );
    let record = find_proof_record(&dir);
    assert_eq!(record["data"]["os_sandbox"], "none");
    assert_eq!(
        record["limitations"],
        serde_json::json!(["os_sandbox_unavailable"])
    );
}

/// The render-worker honours `DISTINCT_TEST_RENDER_LIMIT_BYTES` in debug
/// builds only; a tiny cap must surface as `render_memory_limit_exceeded`.
#[cfg(debug_assertions)]
#[test]
fn logo_proof_reports_render_memory_limit_exceeded() {
    let (dir, svg) = setup("logo-proof-memcap");
    assert_eq!(code(&inspect_space(&dir, &svg)), 0);
    let out = dir.join("out");
    let (d, s, o) = (
        dir.to_string_lossy(),
        svg.to_string_lossy(),
        out.to_string_lossy(),
    );
    let o = Command::new(env!("CARGO_BIN_EXE_distinct-rs"))
        .args([
            "logo",
            "proof",
            "--brand",
            "acme",
            "--workspace",
            &d,
            "--file",
            &s,
            "--out",
            &o,
        ])
        .env("DISTINCT_TEST_RENDER_LIMIT_BYTES", "4096")
        .output()
        .expect("spawn distinct-rs");
    assert_eq!(code(&o), 2, "{}", String::from_utf8_lossy(&o.stdout));
    assert_eq!(json(&o)["error"], "render_memory_limit_exceeded");
    assert!(!out.exists(), "no output copied on failure");
}

// ---------------------------------------------------------------------------
// Issue #3: proof precondition and engine-only kinds
// ---------------------------------------------------------------------------

#[test]
fn record_import_rejects_engine_written_kinds() {
    let (dir, svg) = setup("forged-finding");
    let hash = distinct_core::hash::hash_bytes(&fs::read(&svg).unwrap());
    for kind in ["logo_finding", "logo_proof", "color_check"] {
        let rec = serde_json::json!({
            "schema_version": distinct_core::record::SCHEMA_VERSION,
            "kind": kind,
            "id": format!("inspect-{}", &hash.as_hex()[..12]),
            "brand_id": "acme",
            "created_at": "2026-01-01T00:00:00Z",
            "created_by": {"kind": "agent", "id": "forger"},
            "privacy": "public",
            "dependencies": [],
            "limitations": [],
            "data": {"verdict": "inspected", "file_hash": hash.as_hex()}
        });
        let f = dir.join(format!("forged-{kind}.json"));
        fs::write(&f, rec.to_string()).unwrap();
        let o = run(&[
            "record-import",
            "--brand",
            "acme",
            "--workspace",
            &dir.to_string_lossy(),
            "--file",
            &f.to_string_lossy(),
        ]);
        assert_eq!(
            code(&o),
            3,
            "{kind}: {}",
            String::from_utf8_lossy(&o.stdout)
        );
        assert!(
            String::from_utf8_lossy(&o.stdout).contains("written only by the engine"),
            "{kind}: {}",
            String::from_utf8_lossy(&o.stdout)
        );
    }
    // Proof is still refused: no genuine inspection exists.
    let out = dir.join("proof-out");
    let o = proof(&dir, &svg, &out, &[]);
    assert_ne!(code(&o), 0);
}

#[test]
fn logo_proof_uses_inspect_record_for_exact_hash() {
    let (dir, good) = setup("proof-exact-hash");
    let bad = dir.join("bad.svg");
    fs::write(
        &bad,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><script>x</script></svg>"#,
    )
    .unwrap();
    assert_eq!(code(&inspect_space(&dir, &good)), 0);
    let o = inspect_space(&dir, &bad);
    assert_eq!(json(&o)["result"]["verdict"], "unsupported_content");

    // The unsupported SVG must not ride on the other file's finding.
    let o = proof(&dir, &bad, &dir.join("out-bad"), &[]);
    assert_ne!(code(&o), 0);
    assert!(String::from_utf8_lossy(&o.stdout).contains("unsupported_content"));

    let o = proof(&dir, &good, &dir.join("out-good"), &[]);
    assert_eq!(code(&o), 0, "{}", String::from_utf8_lossy(&o.stdout));
}

#[test]
fn logo_proof_uses_latest_finding_for_same_hash() {
    let (dir, svg) = setup("proof-latest-finding");
    let hash = distinct_core::hash::hash_bytes(&fs::read(&svg).unwrap());
    let ws = distinct_core::store::BrandWorkspace::new(&dir, "acme");
    let mk = |created_at: &str, verdict: &str| {
        serde_json::json!({
            "schema_version": distinct_core::record::SCHEMA_VERSION,
            "kind": "logo_finding",
            "id": format!("inspect-{}", &hash.as_hex()[..12]),
            "brand_id": "acme",
            "created_at": created_at,
            "created_by": {"kind": "agent", "id": "distinct-rs"},
            "privacy": "public",
            "dependencies": [],
            "limitations": [],
            "data": {"verdict": verdict, "file_hash": hash.as_hex()}
        })
    };
    ws.store_object(&mk("2026-01-01T00:00:00Z", "inspected"))
        .unwrap();
    ws.store_object(&mk("2026-02-01T00:00:00Z", "unsupported_content"))
        .unwrap();
    for i in 0..3 {
        let o = proof(&dir, &svg, &dir.join(format!("out-{i}")), &[]);
        assert_eq!(code(&o), 2, "{}", String::from_utf8_lossy(&o.stdout));
        assert!(String::from_utf8_lossy(&o.stdout).contains("unsupported_content"));
    }
}

#[test]
fn patch_refuses_engine_only_kinds() {
    let (dir, _svg) = setup("patch-engine-only");
    let ws = dir.to_string_lossy().to_string();
    for kind in ["logo_finding", "logo_proof", "color_check"] {
        for op in ["add", "replace", "remove"] {
            let plan = serde_json::json!({
                "brand_id": "acme",
                "operations": [{
                    "op": op, "kind": kind, "id": "inspect-aaaaaaaaaaaa",
                    "parent_hash": "a".repeat(64),
                    "data": {"verdict": "inspected"}
                }]
            });
            let f = dir.join("plan.json");
            fs::write(&f, plan.to_string()).unwrap();
            let o = run(&[
                "patch",
                "--brand",
                "acme",
                "--workspace",
                &ws,
                "--plan",
                &f.to_string_lossy(),
                "--apply",
            ]);
            let out = String::from_utf8_lossy(&o.stdout);
            assert_eq!(code(&o), 3, "{op} {kind}: {out}");
            assert!(
                out.contains("written only by the engine"),
                "{op} {kind}: {out}"
            );
        }
    }
    // Content kinds still patch fine.
    let plan = serde_json::json!({
        "brand_id": "acme",
        "operations": [{"op": "add", "kind": "claim", "id": "claim-1", "data": {"text": "x"}}]
    });
    let f = dir.join("plan-ok.json");
    fs::write(&f, plan.to_string()).unwrap();
    let o = run(&[
        "patch",
        "--brand",
        "acme",
        "--workspace",
        &ws,
        "--plan",
        &f.to_string_lossy(),
        "--apply",
    ]);
    assert_eq!(code(&o), 0, "{}", String::from_utf8_lossy(&o.stdout));
}

// ---------------------------------------------------------------------------
// Issue #2: exports never contain absolute local paths
// ---------------------------------------------------------------------------

fn collect_files(dir: &Path, acc: &mut Vec<PathBuf>) {
    for e in fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            collect_files(&p, acc);
        } else {
            acc.push(p);
        }
    }
}

fn has_drive_path(s: &str) -> bool {
    let b = s.as_bytes();
    (0..b.len().saturating_sub(2)).any(|i| {
        b[i].is_ascii_alphabetic()
            && (i == 0 || !b[i - 1].is_ascii_alphanumeric())
            && b[i + 1] == b':'
            && (b[i + 2] == b'\\' || (b[i + 2] == b'/' && b.get(i + 3) != Some(&b'/')))
    })
}

#[test]
fn drive_path_detector_ignores_urls() {
    assert!(has_drive_path("C:\\Users\\x"));
    assert!(has_drive_path("path D:/work"));
    assert!(!has_drive_path("see https://example.com/a"));
    assert!(!has_drive_path("file://host"));
}

#[test]
fn export_kits_contain_no_absolute_local_paths() {
    let (dir, svg) = setup("export-no-paths");
    assert_eq!(code(&inspect_space(&dir, &svg)), 0);
    let o = proof(&dir, &svg, &dir.join("proof-out"), &[]);
    assert_eq!(code(&o), 0, "{}", String::from_utf8_lossy(&o.stdout));
    let pairs = dir.join("pairs.txt");
    fs::write(&pairs, "000000 ffffff\n").unwrap();
    let o = run(&[
        "color",
        "check",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
        "--pairs",
        &pairs.to_string_lossy(),
    ]);
    assert_eq!(code(&o), 0);

    let ws = dir.to_string_lossy().to_string();
    for kind in ["draft", "concept_only", "specification_only"] {
        let out = dir.join(format!("kit-{kind}"));
        let o = run(&[
            "export",
            "--brand",
            "acme",
            "--workspace",
            &ws,
            "--kind",
            kind,
            "--out",
            &out.to_string_lossy(),
            "--tokens",
            "dtcg",
        ]);
        assert_eq!(
            code(&o),
            0,
            "{kind}: {}",
            String::from_utf8_lossy(&o.stdout)
        );
        let mut files = Vec::new();
        collect_files(&out, &mut files);
        assert!(!files.is_empty());
        assert!(
            out.join("tokens.json").exists() && out.join("GUIDELINES.md").exists(),
            "{kind}: tokens.json/GUIDELINES.md must be exported and scanned"
        );
        for f in &files {
            let text = fs::read_to_string(f).unwrap();
            assert!(
                !text.contains(&ws),
                "{kind}: {} leaks workspace path",
                f.display()
            );
            assert!(
                !text.contains("/Users/"),
                "{kind}: {} leaks /Users/",
                f.display()
            );
            assert!(
                !has_drive_path(&text),
                "{kind}: {} leaks drive path",
                f.display()
            );
            assert!(
                !text.contains("proofs_dir"),
                "{kind}: {} has proofs_dir",
                f.display()
            );
        }
        let has_kind = |k: &str| {
            files
                .iter()
                .any(|f| f.to_string_lossy().contains(&format!("/{k}/")))
        };
        if kind == "specification_only" {
            assert!(!has_kind("logo_proof") && !has_kind("color_check"));
        } else {
            assert!(
                has_kind("logo_proof"),
                "{kind} should keep scrubbed logo_proof"
            );
        }
    }
}
