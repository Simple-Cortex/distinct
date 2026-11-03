//! Landscape, agency-process and logo-breakdown evidence gates.

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

fn setup(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target")
        .join(format!("test-research-{name}-{nanos}"));
    fs::create_dir_all(&dir).unwrap();
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

fn forget(dir: &Path, id: &str) {
    let o = run(&[
        "evidence",
        "forget",
        "--brand",
        "acme",
        "--workspace",
        &dir.to_string_lossy(),
        "--id",
        id,
        "--reason",
        "test",
    ]);
    assert_eq!(code_of(&o), 0, "{}", out_s(&o));
}

fn import(dir: &Path, kind: &str, id: &str, data: &Value) {
    let rec = json!({
        "schema_version": "1.0.0", "kind": kind, "id": id,
        "brand_id": "acme", "privacy": "internal",
        "created_at": "2026-10-27T00:00:00Z",
        "created_by": {"kind": "agent", "id": "distinct-rs"},
        "dependencies": [], "limitations": [], "data": data
    });
    let p = dir.join(format!("{kind}-{id}.json"));
    fs::write(&p, rec.to_string()).unwrap();
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

/// Blocker codes starting with one of `prefixes`.
fn codes(result: &Value, prefixes: &[&str]) -> Vec<String> {
    result["blockers"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|b| b["code"].as_str().map(String::from))
        .filter(|c| prefixes.iter().any(|p| c.starts_with(p)))
        .collect()
}

fn warnings(result: &Value) -> Vec<String> {
    result["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|w| w.as_str().map(String::from))
        .collect()
}

fn competitor(id: &str, evidence: &[&str]) -> Value {
    json!({"competitor_id": id, "name": "Rival", "url": "https://example.com",
        "depth": "deep", "status": "fetched", "evidence_ids": evidence,
        "observations": ["navy and gold, serif wordmark"], "unreachable_reason": null})
}

fn landscape(competitors: &Value, reason: Value) -> Value {
    json!({"type": "landscape", "not_applicable_reason": reason,
        "competitors": competitors,
        "codes_to_keep": [], "codes_to_break": [], "whitespace": []})
}

fn import_landscape(dir: &Path, competitors: &Value) {
    import(
        dir,
        "research_item",
        "landscape",
        &landscape(competitors, Value::Null),
    );
}

fn fetched_ref(ref_id: &str, kind: &str, evidence: &[&str], sources: Value) -> Value {
    json!({"ref_id": ref_id, "url": "https://example.com", "label": "x",
        "kind": kind, "stance": "like", "status": "fetched",
        "evidence_ids": evidence,
        "observations": ["condensed grotesk, all caps, heavy rules"],
        "user_reaction": null, "unreachable_reason": null,
        "process_sources": sources})
}

fn process_source(evidence: &[&str], takeaways: &[&str]) -> Value {
    json!({"url": "https://example.com/case", "title": "Case study",
        "source_type": "case_study", "evidence_ids": evidence, "takeaways": takeaways})
}

fn import_board(dir: &Path, references: &Value, extra: &Value) {
    let mut data = json!({"type": "inspiration_board", "origin": "user_supplied",
        "references": references});
    if let Some(m) = extra.as_object() {
        for (k, v) in m {
            data[k] = v.clone();
        }
    }
    import(dir, "source", "inspiration-board", &data);
}

fn breakdown(evidence: &[&str], facets: &Value) -> Value {
    json!({"logo_id": "l1", "ref_id": "r1", "label": "mark", "evidence_ids": evidence,
        "user_reaction": null, "facets": facets,
        "transferable_principles": ["build the mark from the typeface"],
        "do_not_copy": ["the exact notch in the counter"]})
}

fn four_facets() -> Value {
    json!({"idea": "a verbal pun", "construction": "one stroke", "typography": "",
        "negative_space": "", "colour": "single ink", "small_size": "holds at 16px",
        "distinctiveness": ""})
}

// ── A. landscape ─────────────────────────────────────────────────────

#[test]
fn landscape_missing_blocks_direction_scopes() {
    let dir = setup("land-missing");
    for scope in ["direction", "deliver", "default"] {
        let r = gate(&dir, scope);
        assert!(
            codes(&r, &["landscape_"]).contains(&"landscape_missing".to_string()),
            "scope {scope}: {r}"
        );
        assert_eq!(r["ready"], false);
    }
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn landscape_with_only_unreachable_is_unseen() {
    let dir = setup("land-unseen");
    let mut c = competitor("c1", &[]);
    c["status"] = json!("unreachable");
    c["observations"] = json!([]);
    c["unreachable_reason"] = json!("bot wall");
    import_landscape(&dir, &json!([c]));
    let r = gate(&dir, "direction");
    assert_eq!(codes(&r, &["landscape_"]), vec!["landscape_unseen"]);
    assert!(
        warnings(&r)
            .iter()
            .any(|w| w.starts_with("landscape_unreachable") && w.contains("c1")),
        "{r}"
    );
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn landscape_forgotten_evidence_is_dangling() {
    let dir = setup("land-dangling");
    add_evidence(&dir, "land-c1-1");
    add_evidence(&dir, "land-c2-1");
    import_landscape(
        &dir,
        &json!([
            competitor("c1", &["land-c1-1"]),
            competitor("c2", &["land-c2-1"])
        ]),
    );
    assert!(codes(&gate(&dir, "direction"), &["landscape_"]).is_empty());
    forget(&dir, "land-c2-1");
    let r = gate(&dir, "direction");
    let dangling: Vec<&Value> = r["blockers"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|b| b["code"] == "landscape_evidence_dangling")
        .collect();
    assert_eq!(dangling.len(), 1, "{r}");
    assert_eq!(dangling[0]["competitor_id"], "c2");
    assert!(!codes(&r, &["landscape_"]).contains(&"landscape_unseen".to_string()));
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn landscape_not_applicable_passes_with_advisory() {
    let dir = setup("land-na");
    import(
        &dir,
        "research_item",
        "landscape",
        &landscape(&json!([]), json!("internal sub-brand, no market")),
    );
    let r = gate(&dir, "direction");
    assert!(codes(&r, &["landscape_"]).is_empty(), "{r}");
    assert!(
        warnings(&r)
            .iter()
            .any(|w| w == "landscape_not_applicable: internal sub-brand, no market"),
        "{r}"
    );
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn landscape_thin_is_advisory_only() {
    let dir = setup("land-thin");
    add_evidence(&dir, "land-c1-1");
    import_landscape(&dir, &json!([competitor("c1", &["land-c1-1"])]));
    let r = gate(&dir, "direction");
    assert!(codes(&r, &["landscape_"]).is_empty(), "{r}");
    assert!(
        warnings(&r).iter().any(|w| w.starts_with("landscape_thin")),
        "{r}"
    );
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn landscape_valid_with_three_competitors_has_no_landscape_findings() {
    let dir = setup("land-valid");
    let mut cs = Vec::new();
    for n in 1..=3 {
        let id = format!("land-c{n}-1");
        add_evidence(&dir, &id);
        cs.push(competitor(&format!("c{n}"), &[&id]));
    }
    import_landscape(&dir, &json!(cs));
    for scope in ["direction", "deliver", "default"] {
        let r = gate(&dir, scope);
        assert!(codes(&r, &["landscape_"]).is_empty(), "scope {scope}: {r}");
        assert!(
            !warnings(&r).iter().any(|w| w.starts_with("landscape_")),
            "scope {scope}: {r}"
        );
    }
    fs::remove_dir_all(&dir).ok();
}

// ── B. agency process ────────────────────────────────────────────────

#[test]
fn agency_without_process_sources_is_unread() {
    let dir = setup("agency-unread");
    add_evidence(&dir, "insp-r1");
    import_board(
        &dir,
        &json!([fetched_ref("r1", "agency", &["insp-r1"], json!([]))]),
        &json!({}),
    );
    let r = gate(&dir, "direction");
    let unread: Vec<&Value> = r["blockers"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|b| b["code"] == "agency_process_unread")
        .collect();
    assert_eq!(unread.len(), 1, "{r}");
    assert_eq!(unread[0]["ref_id"], "r1");
    // The reference itself is seen; only the process study is missing.
    assert!(codes(&r, &["inspiration_"]).is_empty(), "{r}");
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn agency_with_blank_takeaways_is_still_unread() {
    let dir = setup("agency-blank");
    add_evidence(&dir, "insp-r1");
    add_evidence(&dir, "insp-r1-p1");
    let src = process_source(&["insp-r1-p1"], &["  "]);
    import_board(
        &dir,
        &json!([fetched_ref("r1", "agency", &["insp-r1"], json!([src]))]),
        &json!({}),
    );
    assert_eq!(
        codes(&gate(&dir, "direction"), &["agency_"]),
        vec!["agency_process_unread"]
    );
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn agency_with_process_sources_passes() {
    let dir = setup("agency-ok");
    add_evidence(&dir, "insp-r1");
    add_evidence(&dir, "insp-r1-p1");
    let src = process_source(&["insp-r1-p1"], &["starts from one verbal idea"]);
    import_board(
        &dir,
        &json!([fetched_ref("r1", "agency", &["insp-r1"], json!([src]))]),
        &json!({}),
    );
    let r = gate(&dir, "direction");
    assert!(codes(&r, &["agency_", "inspiration_"]).is_empty(), "{r}");
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn agency_process_dangling_evidence_blocks() {
    let dir = setup("agency-dangling");
    add_evidence(&dir, "insp-r1");
    add_evidence(&dir, "insp-r1-p1");
    let src = process_source(&["insp-r1-p1"], &["starts from one verbal idea"]);
    import_board(
        &dir,
        &json!([fetched_ref("r1", "agency", &["insp-r1"], json!([src]))]),
        &json!({}),
    );
    forget(&dir, "insp-r1-p1");
    let r = gate(&dir, "direction");
    let codes = codes(&r, &["agency_", "inspiration_"]);
    assert!(codes.contains(&"inspiration_evidence_dangling".to_string()));
    assert!(codes.contains(&"agency_process_unread".to_string()));
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn non_agency_reference_needs_no_process_sources() {
    let dir = setup("agency-other");
    add_evidence(&dir, "insp-r1");
    import_board(
        &dir,
        &json!([fetched_ref("r1", "brand", &["insp-r1"], json!([]))]),
        &json!({}),
    );
    let r = gate(&dir, "direction");
    assert!(codes(&r, &["agency_", "inspiration_"]).is_empty(), "{r}");
    fs::remove_dir_all(&dir).ok();
}

// ── C. logo breakdown ────────────────────────────────────────────────

#[test]
fn logo_scope_without_board_is_inspiration_missing() {
    let dir = setup("logo-noboard");
    let r = gate(&dir, "logo_intent");
    assert_eq!(codes(&r, &["inspiration_"]), vec!["inspiration_missing"]);
    assert!(!codes(&r, &["logo_"]).contains(&"logo_breakdown_missing".to_string()));
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn logo_scope_without_breakdown_blocks() {
    let dir = setup("logo-none");
    add_evidence(&dir, "insp-r1");
    import_board(
        &dir,
        &json!([fetched_ref("r1", "agency", &["insp-r1"], json!([]))]),
        &json!({}),
    );
    let r = gate(&dir, "logo_intent");
    assert_eq!(codes(&r, &["logo_"]), vec!["logo_breakdown_missing"]);
    assert_eq!(r["ready"], false);
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn logo_breakdown_with_three_facets_blocks() {
    let dir = setup("logo-3facets");
    add_evidence(&dir, "insp-l1-1");
    add_evidence(&dir, "insp-r1");
    let three = json!({"idea": "a verbal pun", "construction": "one stroke",
        "colour": "single ink", "typography": " "});
    import_board(
        &dir,
        &json!([fetched_ref("r1", "agency", &["insp-r1"], json!([]))]),
        &json!({"logo_breakdowns": [breakdown(&["insp-l1-1"], &three)]}),
    );
    assert_eq!(
        codes(&gate(&dir, "logo_intent"), &["logo_"]),
        vec!["logo_breakdown_missing"]
    );
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn logo_breakdown_needs_principles_and_do_not_copy() {
    let dir = setup("logo-empty-lists");
    add_evidence(&dir, "insp-l1-1");
    add_evidence(&dir, "insp-r1");
    let mut b = breakdown(&["insp-l1-1"], &four_facets());
    b["do_not_copy"] = json!([]);
    import_board(
        &dir,
        &json!([fetched_ref("r1", "agency", &["insp-r1"], json!([]))]),
        &json!({"logo_breakdowns": [b]}),
    );
    assert_eq!(
        codes(&gate(&dir, "logo_intent"), &["logo_"]),
        vec!["logo_breakdown_missing"]
    );
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn logo_breakdown_dangling_evidence_blocks() {
    let dir = setup("logo-dangling");
    add_evidence(&dir, "insp-l1-1");
    add_evidence(&dir, "insp-r1");
    import_board(
        &dir,
        &json!([fetched_ref("r1", "agency", &["insp-r1"], json!([]))]),
        &json!({"logo_breakdowns": [breakdown(&["insp-l1-1"], &four_facets())]}),
    );
    forget(&dir, "insp-l1-1");
    let r = gate(&dir, "logo_intent");
    let dangling: Vec<&Value> = r["blockers"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|b| b["code"] == "inspiration_evidence_dangling")
        .collect();
    assert_eq!(dangling.len(), 1, "{r}");
    assert_eq!(dangling[0]["logo_id"], "l1");
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn valid_logo_breakdown_passes() {
    let dir = setup("logo-valid");
    add_evidence(&dir, "insp-l1-1");
    add_evidence(&dir, "insp-r1");
    import_board(
        &dir,
        &json!([fetched_ref("r1", "agency", &["insp-r1"], json!([]))]),
        &json!({"logo_breakdowns": [breakdown(&["insp-l1-1"], &four_facets())]}),
    );
    let r = gate(&dir, "logo_intent");
    assert!(codes(&r, &["logo_", "inspiration_"]).is_empty(), "{r}");
    assert_eq!(r["ready"], true, "{r}");
    fs::remove_dir_all(&dir).ok();
}

// ── scope isolation ──────────────────────────────────────────────────

#[test]
fn voice_scope_is_unaffected() {
    let dir = setup("voice");
    let r = gate(&dir, "voice");
    assert!(
        codes(&r, &["landscape_", "agency_", "logo_", "inspiration_"]).is_empty(),
        "{r}"
    );
    assert_eq!(r["ready"], true);
    assert!(r["warnings"].as_array().unwrap().is_empty());
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn direction_scope_ignores_logo_breakdowns() {
    let dir = setup("dir-no-logo");
    add_evidence(&dir, "insp-r1");
    import_board(
        &dir,
        &json!([fetched_ref("r1", "brand", &["insp-r1"], json!([]))]),
        &json!({}),
    );
    let r = gate(&dir, "direction");
    assert!(codes(&r, &["logo_"]).is_empty(), "{r}");
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn extra_not_applicable_record_does_not_bypass_a_real_landscape() {
    let dir = setup("land-na-bypass");
    // A real landscape whose only competitor was never fetched...
    import(
        &dir,
        "research_item",
        "landscape",
        &landscape(
            &json!([{"competitor_id": "c1", "name": "Rival", "url": "https://example.com",
                "depth": "deep", "status": "unreachable", "evidence_ids": [],
                "observations": [], "unreachable_reason": "blocked"}]),
            Value::Null,
        ),
    );
    // ...plus a second record claiming not-applicable must not switch the rule off.
    import(
        &dir,
        "research_item",
        "landscape-na",
        &landscape(&json!([]), json!("trying to skip")),
    );
    let r = gate(&dir, "direction");
    assert_eq!(codes(&r, &["landscape_"]), vec!["landscape_unseen"], "{r}");
    assert!(
        !warnings(&r)
            .iter()
            .any(|w| w.starts_with("landscape_not_applicable")),
        "{r}"
    );
    fs::remove_dir_all(&dir).ok();
}
