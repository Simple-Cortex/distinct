//! Governance reconciliation (contract §9, WO-05).
//!
//! `reconcile_governance()` recomputes content and governance hashes from
//! stored objects and compares them with STATE.json. It also checks for
//! missing check results, stale dependencies, and other readiness blockers.

use crate::hash::{self, DOMAIN_GOVERNANCE, DOMAIN_SNAPSHOT};
use crate::record::RecordKind;
use crate::store::BrandWorkspace;
use serde_json::Value;
use std::collections::BTreeMap;

/// Readiness verdict produced by governance reconciliation.
#[derive(Debug)]
pub struct ReadinessVerdict {
    pub ready: bool,
    pub blockers: Vec<String>,
    pub warnings: Vec<String>,
    pub stale: Vec<String>,
    pub missing_reviews: Vec<String>,
    pub missing_proofs: Vec<String>,
}

/// Reconcile governance state: recompute hashes and check readiness.
pub fn reconcile_governance(ws: &BrandWorkspace) -> Result<ReadinessVerdict, String> {
    let state = ws.read_state().map_err(|e| e.to_string())?;
    let mut blockers = Vec::new();
    let mut warnings = Vec::new();
    let mut stale = Vec::new();
    let missing_reviews = Vec::new();
    let missing_proofs = Vec::new();

    let all_objects = ws.list_objects(None).map_err(|e| e.to_string())?;

    // Recompute content snapshot hash.
    let content_snapshot = recompute_content_snapshot(ws, &all_objects)?;
    let content_json =
        serde_json::to_string_pretty(&content_snapshot).map_err(|e| format!("serialize: {e}"))?;
    let computed_content_hash = hash::hash_raw(DOMAIN_SNAPSHOT, content_json.as_bytes());

    // Compare with STATE.
    if let Some(ref stored) = state.content_snapshot_hash {
        if stored != computed_content_hash.as_hex() {
            blockers.push("content_snapshot_hash mismatch".to_string());
        }
    }

    // Recompute governance index hash.
    let gov_index = recompute_governance_index(ws, &all_objects)?;
    let gov_json =
        serde_json::to_string_pretty(&gov_index).map_err(|e| format!("serialize: {e}"))?;
    let computed_gov_hash = hash::hash_raw(DOMAIN_GOVERNANCE, gov_json.as_bytes());

    if let Some(ref stored) = state.governance_index_hash {
        if stored != computed_gov_hash.as_hex() {
            blockers.push("governance_index_hash mismatch".to_string());
        }
    }

    // Decisions without a terminal-code attestation never count as approvals.
    for d in all_objects
        .iter()
        .filter(|o| o["kind"].as_str() == Some("decision"))
    {
        if !is_attested(d) {
            warnings.push(format!(
                "unattested_decision: decision/{} has no terminal_code attestation and is ignored",
                d["id"].as_str().unwrap_or("?")
            ));
        }
    }

    // Check for missing check results (a planned check without a result is a blocker).
    let content_objects = content_records(&all_objects);
    let gov_objects = governance_records(&all_objects);
    check_missing_results(&content_objects, &gov_objects, &mut blockers);

    // Check for stale dependencies.
    check_stale_dependencies(&content_objects, &content_snapshot, &mut stale);

    // WO-10b: check for stale evidence references (forgotten evidence).
    check_stale_evidence(&content_objects, &all_objects, &mut stale);

    let ready = blockers.is_empty();
    Ok(ReadinessVerdict {
        ready,
        blockers,
        warnings,
        stale,
        missing_reviews,
        missing_proofs,
    })
}

/// Verify that stored object files match their recorded hashes in the snapshot.
pub fn verify_object_integrity(ws: &BrandWorkspace) -> Result<Vec<String>, String> {
    let mut blockers = Vec::new();
    let state = ws.read_state().map_err(|e| e.to_string())?;
    let all_objects = ws.list_objects(None).map_err(|e| e.to_string())?;

    for obj in &all_objects {
        let mut hash_body = obj.clone();
        if let Some(o) = hash_body.as_object_mut() {
            o.remove("content_hash");
            o.remove("revision_id");
        }
        let computed = hash::content_hash(&hash_body).map_err(|e| format!("hash error: {e}"))?;
        if let Some(stored) = obj["content_hash"].as_str() {
            if stored != computed.as_hex() {
                blockers.push(format!(
                    "object {}:{} hash mismatch",
                    obj["kind"].as_str().unwrap_or("?"),
                    obj["id"].as_str().unwrap_or("?")
                ));
            }
        }
    }

    // Also check snapshot hash consistency.
    let snapshot = recompute_content_snapshot(ws, &all_objects)?;
    let snap_json =
        serde_json::to_string_pretty(&snapshot).map_err(|e| format!("serialize: {e}"))?;
    let computed_hash = hash::hash_raw(DOMAIN_SNAPSHOT, snap_json.as_bytes());
    if let Some(ref stored) = state.content_snapshot_hash {
        if stored != computed_hash.as_hex() {
            blockers.push("content_snapshot_hash mismatch".to_string());
        }
    }

    Ok(blockers)
}

// ── owner approvals ──────────────────────────────────────────────────

/// Approval state of one record's current revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalStatus {
    /// A valid owner decision approves the current revision.
    Approved { decision_id: String },
    /// Valid owner decisions exist, but only for older revisions.
    Stale,
    /// No valid owner decision approves this record.
    NotApproved,
}

/// Approvals derived from stored decision records.
///
/// A decision counts only if it is `approved: true`, made by a non-agent
/// actor (`human` or `owner`), not labelled a model recommendation or
/// fixture-only, names a target `{kind, id, revision_id | content_hash}` (`data.target`),
/// carries a `terminal_code` attestation (`decision confirm`), and is not superseded by another decision (`data.supersedes`, string or
/// array of decision ids). Approval is derived, never read from the
/// approved record itself.
#[derive(Debug, Default)]
pub struct ApprovalIndex {
    /// (kind, id) -> content hash -> lowest approving decision id.
    approvals: BTreeMap<(String, String), BTreeMap<String, String>>,
}

impl ApprovalIndex {
    /// Build the index from all stored objects.
    pub fn build(objects: &[Value]) -> Self {
        let decisions: Vec<&Value> = objects
            .iter()
            .filter(|o| o["kind"].as_str() == Some("decision"))
            .collect();

        let mut superseded: std::collections::BTreeSet<String> = Default::default();
        for d in &decisions {
            if !is_owner_actor(d) || !is_attested(d) {
                continue;
            }
            match &d["data"]["supersedes"] {
                Value::String(s) => {
                    superseded.insert(s.clone());
                }
                Value::Array(a) => {
                    superseded.extend(a.iter().filter_map(|v| v.as_str().map(String::from)));
                }
                _ => {}
            }
        }

        let mut approvals: BTreeMap<(String, String), BTreeMap<String, String>> = BTreeMap::new();
        for d in decisions {
            let Some(did) = d["id"].as_str() else {
                continue;
            };
            if !is_owner_actor(d)
                || !is_attested(d)
                || d["data"]["approved"].as_bool() != Some(true)
                || is_non_authoritative_source(&d["data"])
                || superseded.contains(did)
            {
                continue;
            }
            // `data.target` is what `decision record` stores.
            let subject = &d["data"]["target"];
            let (Some(kind), Some(id)) = (subject["kind"].as_str(), subject["id"].as_str()) else {
                continue;
            };
            let hash = subject["content_hash"]
                .as_str()
                .or_else(|| {
                    subject["revision_id"]
                        .as_str()
                        .map(|r| r.strip_prefix("r-").unwrap_or(r))
                })
                .unwrap_or("");
            if hash.is_empty() {
                continue;
            }
            let entry = approvals
                .entry((kind.to_string(), id.to_string()))
                .or_default()
                .entry(hash.to_string())
                .or_insert_with(|| did.to_string());
            if did < entry.as_str() {
                *entry = did.to_string();
            }
        }
        Self { approvals }
    }

    /// Status of a record's revision (`record["content_hash"]`).
    pub fn status(&self, record: &Value) -> ApprovalStatus {
        let key = (
            record["kind"].as_str().unwrap_or("").to_string(),
            record["id"].as_str().unwrap_or("").to_string(),
        );
        let Some(by_hash) = self.approvals.get(&key) else {
            return ApprovalStatus::NotApproved;
        };
        let current = record["content_hash"].as_str().unwrap_or("");
        match by_hash.get(current) {
            Some(did) if !current.is_empty() => ApprovalStatus::Approved {
                decision_id: did.clone(),
            },
            _ => ApprovalStatus::Stale,
        }
    }
}

/// True when the decision was confirmed by the owner's one-time terminal code.
pub fn is_attested(decision: &Value) -> bool {
    decision["attestation"]["method"].as_str() == Some("terminal_code")
}

fn is_owner_actor(decision: &Value) -> bool {
    let actor = &decision["created_by"];
    matches!(actor["kind"].as_str(), Some("human" | "owner"))
        && actor["id"].as_str().is_some_and(|s| !s.is_empty())
}

fn is_non_authoritative_source(data: &Value) -> bool {
    data["fixture_only"].as_bool() == Some(true)
        || ["authorization_source", "source"].iter().any(|k| {
            matches!(
                data[*k].as_str(),
                Some("model_recommendation" | "fixture_only")
            )
        })
}

/// Content kinds whose current revisions need an owner approval for `scope`.
///
/// Minimal, explicit mapping: a content-kind scope (`platform`, `voice`, ...)
/// checks that kind only; `deliver` and the unscoped default check the
/// approvable strategy kinds below. Any other scope name (for example a
/// check-plan id) adds no approval requirement. No upstream-prerequisite
/// mapping exists in the contract, so none is inferred.
#[must_use]
pub fn approval_scope_kinds(scope: &str) -> Vec<&'static str> {
    const DELIVER: [&str; 4] = ["platform", "voice", "direction", "system"];
    if scope == "deliver" || scope == "default" {
        return DELIVER.to_vec();
    }
    match serde_json::from_value::<RecordKind>(Value::String(scope.to_string())) {
        Ok(k) if k.is_content() => vec![k.as_str()],
        _ => Vec::new(),
    }
}

/// Owner-approval blockers for the gated scope: one
/// `{code, kind, id}` object per current record lacking a valid attested
/// owner approval (`missing_owner_approval`) or approved only at an older
/// revision (`stale_owner_approval`).
#[must_use]
pub fn owner_approval_blockers(objects: &[Value], scope: &str) -> Vec<Value> {
    let kinds = approval_scope_kinds(scope);
    if kinds.is_empty() {
        return Vec::new();
    }
    let index = ApprovalIndex::build(objects);
    current_revisions(objects)
        .into_iter()
        .filter(|r| r["kind"].as_str().is_some_and(|k| kinds.contains(&k)))
        .filter_map(|r| {
            let code = match index.status(r) {
                ApprovalStatus::Approved { .. } => return None,
                ApprovalStatus::Stale => "stale_owner_approval",
                ApprovalStatus::NotApproved => "missing_owner_approval",
            };
            Some(serde_json::json!({"code": code, "kind": r["kind"], "id": r["id"]}))
        })
        .collect()
}

/// Select the current revision of each `(kind, id)` from all stored
/// revisions: greatest `created_at`, ties broken by `content_hash`.
/// Returned in sorted `(kind, id)` order.
pub fn current_revisions(objects: &[Value]) -> Vec<&Value> {
    // A revision named in another revision's top-level `supersedes` (its parent
    // content hash) is never current. `created_at` has one-second resolution, so
    // it only breaks ties between revisions that have no lineage between them.
    let superseded: std::collections::BTreeSet<&str> = objects
        .iter()
        .filter_map(|o| o["supersedes"].as_str())
        .collect();
    let is_head = |o: &Value| {
        !o["content_hash"]
            .as_str()
            .is_some_and(|h| superseded.contains(h))
    };
    let rank = |v: &Value| {
        (
            is_head(v),
            v["created_at"].as_str().unwrap_or("").to_string(),
            v["content_hash"].as_str().unwrap_or("").to_string(),
        )
    };
    let mut best: BTreeMap<(&str, &str), &Value> = BTreeMap::new();
    for o in objects {
        let key = (
            o["kind"].as_str().unwrap_or(""),
            o["id"].as_str().unwrap_or(""),
        );
        match best.get(&key) {
            Some(prev) if rank(prev) >= rank(o) => {}
            _ => {
                best.insert(key, o);
            }
        }
    }
    best.into_values().collect()
}

// ── helpers ──────────────────────────────────────────────────────────

fn content_records(objects: &[Value]) -> Vec<&Value> {
    objects
        .iter()
        .filter(|o| {
            serde_json::from_value::<RecordKind>(o["kind"].clone()).is_ok_and(|k| k.is_content())
        })
        .collect()
}

fn governance_records(objects: &[Value]) -> Vec<&Value> {
    objects
        .iter()
        .filter(|o| {
            serde_json::from_value::<RecordKind>(o["kind"].clone()).is_ok_and(|k| k.is_governance())
        })
        .collect()
}

fn recompute_content_snapshot(ws: &BrandWorkspace, objects: &[Value]) -> Result<Value, String> {
    let mut records: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for obj in objects {
        let kind = obj["kind"].as_str().unwrap_or("unknown").to_string();
        let id = obj["id"].as_str().unwrap_or("unknown").to_string();
        let hash_val = obj["content_hash"].as_str().unwrap_or("").to_string();
        if let Ok(k) = serde_json::from_value::<RecordKind>(obj["kind"].clone()) {
            if k.is_content() {
                records.entry(kind).or_default().insert(id, hash_val);
            }
        }
    }
    Ok(serde_json::json!({
        "schema_version": crate::record::SCHEMA_VERSION,
        "brand_id": ws.brand_id,
        "records": records,
    }))
}

fn recompute_governance_index(ws: &BrandWorkspace, objects: &[Value]) -> Result<Value, String> {
    let mut records: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for obj in objects {
        let kind = obj["kind"].as_str().unwrap_or("unknown").to_string();
        let id = obj["id"].as_str().unwrap_or("unknown").to_string();
        let hash_val = obj["content_hash"].as_str().unwrap_or("").to_string();
        if let Ok(k) = serde_json::from_value::<RecordKind>(obj["kind"].clone()) {
            if k.is_governance() {
                records.entry(kind).or_default().insert(id, hash_val);
            }
        }
    }
    Ok(serde_json::json!({
        "schema_version": crate::record::SCHEMA_VERSION,
        "brand_id": ws.brand_id,
        "records": records,
    }))
}

fn check_missing_results(content: &[&Value], governance: &[&Value], blockers: &mut Vec<String>) {
    for obj in content {
        let kind = obj["kind"].as_str().unwrap_or("");
        if kind != "check_plan" {
            continue;
        }
        let plan_id = obj["id"].as_str().unwrap_or("");

        let has_result = governance.iter().any(|g| {
            g["kind"].as_str() == Some("check_result")
                && g["data"]["check_plan_id"].as_str() == Some(plan_id)
        });
        if !has_result {
            blockers.push(format!("missing check_result for check_plan/{plan_id}"));
        }
    }
}

fn check_stale_dependencies(content: &[&Value], snapshot: &Value, stale: &mut Vec<String>) {
    let records = snapshot["records"].as_object();
    let Some(records) = records else { return };

    for obj in content {
        let Some(deps) = obj["dependencies"].as_array() else {
            continue;
        };
        let kind = obj["kind"].as_str().unwrap_or("");
        let id = obj["id"].as_str().unwrap_or("");

        for dep in deps {
            let target_kind = dep["target"]["kind"].as_str().unwrap_or("");
            let target_id = dep["target"]["id"].as_str().unwrap_or("");
            let Some(expected_hash) = dep["target"]["value_hash"].as_str() else {
                continue;
            };

            if let Some(kind_map) = records.get(target_kind) {
                if let Some(current_hash) = kind_map.get(target_id) {
                    if current_hash != expected_hash {
                        stale.push(format!(
                            "{kind}/{id} depends on {target_kind}/{target_id} (stale)"
                        ));
                    }
                }
            }
        }
    }
}

/// WO-10b: detect content records that reference forgotten evidence.
///
/// Scans all objects for tombstone records (data.type == "tombstone") and
/// collects the forgotten evidence IDs. Then checks if any content record
/// has source_refs or dependencies pointing to those evidence IDs.
fn check_stale_evidence(content: &[&Value], all_objects: &[Value], stale: &mut Vec<String>) {
    // Collect forgotten evidence IDs from tombstones.
    let mut forgotten_ids: Vec<String> = Vec::new();
    for obj in all_objects {
        if obj["data"]["type"].as_str() == Some("tombstone") {
            if let Some(refs) = obj["data"]["forgotten_evidence_refs"].as_array() {
                for r in refs {
                    if let Some(id) = r.as_str() {
                        forgotten_ids.push(id.to_string());
                    }
                }
            }
        }
    }
    if forgotten_ids.is_empty() {
        return;
    }

    // Check content records for references to forgotten evidence.
    for obj in content {
        let kind = obj["kind"].as_str().unwrap_or("");
        let id = obj["id"].as_str().unwrap_or("");

        // Check source_refs.
        if let Some(refs) = obj["source_refs"].as_array() {
            for r in refs {
                if let Some(evidence_id) = r["evidence_id"].as_str() {
                    if forgotten_ids.contains(&evidence_id.to_string()) {
                        stale.push(format!(
                            "{kind}/{id} references forgotten evidence/{evidence_id} (stale)"
                        ));
                    }
                }
            }
        }

        // Check dependencies.
        if let Some(deps) = obj["dependencies"].as_array() {
            for dep in deps {
                let target_kind = dep["target"]["kind"].as_str().unwrap_or("");
                let target_id = dep["target"]["id"].as_str().unwrap_or("");
                if target_kind == "evidence" && forgotten_ids.contains(&target_id.to_string()) {
                    stale.push(format!(
                        "{kind}/{id} depends on evidence/{target_id} (stale)"
                    ));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{BrandWorkspace, State};
    use std::path::Path;

    fn temp_ws(name: &str) -> (std::path::PathBuf, BrandWorkspace) {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("distinct-gov-{name}-{nanos}"));
        std::fs::create_dir_all(&dir).unwrap();
        let ws = BrandWorkspace::new(Path::new(&dir), "testbrand");
        std::fs::create_dir_all(ws.objects_dir()).unwrap();
        std::fs::create_dir_all(ws.snapshots_dir()).unwrap();
        std::fs::create_dir_all(ws.governance_dir()).unwrap();
        std::fs::create_dir_all(ws.evidence_objects_dir()).unwrap();
        std::fs::create_dir_all(ws.evidence_index_dir()).unwrap();
        (dir, ws)
    }

    #[test]
    fn unattested_decision_does_not_satisfy_owner_approval() {
        let plat = serde_json::json!({
            "kind": "platform", "id": "main", "content_hash": "aa",
            "created_at": "2026-10-27T00:00:00Z"
        });
        let mut dec = serde_json::json!({
            "kind": "decision", "id": "d1",
            "created_by": {"kind": "human", "id": "owner1"},
            "data": {"approved": true,
                "target": {"kind": "platform", "id": "main", "content_hash": "aa"}}
        });
        let objs = vec![plat.clone(), dec.clone()];
        let b = owner_approval_blockers(&objs, "platform");
        assert_eq!(b.len(), 1);
        assert_eq!(b[0]["code"], "missing_owner_approval");

        dec["attestation"] = serde_json::json!({"method": "terminal_code"});
        let objs = vec![plat, dec];
        assert!(owner_approval_blockers(&objs, "platform").is_empty());
        assert!(owner_approval_blockers(&objs, "check-plan-x").is_empty());
    }

    #[test]
    fn reconcile_empty_brand() {
        let (dir, ws) = temp_ws("recon-empty");
        let state = State {
            schema_version: "1.0.0".into(),
            brand_id: "testbrand".into(),
            content_snapshot_hash: None,
            governance_index_hash: None,
            generation: 0,
            name: None,
            created_at: "2026-10-27T00:00:00Z".into(),
        };
        ws.write_state(&state).unwrap();

        let verdict = reconcile_governance(&ws).unwrap();
        assert!(verdict.ready, "empty brand with no plans should be ready");
        assert!(verdict.blockers.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_check_result_is_blocker() {
        let (dir, ws) = temp_ws("recon-missing");
        let plan = serde_json::json!({
            "schema_version": "1.0.0",
            "kind": "check_plan",
            "id": "cp1",
            "brand_id": "testbrand",
            "privacy": "public",
            "data": {"scope": "all"}
        });
        ws.store_object(&plan).unwrap();

        let mut state = State {
            schema_version: "1.0.0".into(),
            brand_id: "testbrand".into(),
            content_snapshot_hash: None,
            governance_index_hash: None,
            generation: 0,
            name: None,
            created_at: "2026-10-27T00:00:00Z".into(),
        };
        ws.update_snapshot(&mut state).unwrap();
        ws.write_state(&state).unwrap();

        let verdict = reconcile_governance(&ws).unwrap();
        assert!(!verdict.ready);
        assert!(verdict
            .blockers
            .iter()
            .any(|b| b.contains("missing check_result")));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn unattested_decision_is_ignored_and_warned() {
        let (dir, ws) = temp_ws("unattested");
        let target = serde_json::json!({
            "schema_version": "1.0.0", "kind": "platform", "id": "p1",
            "brand_id": "testbrand", "privacy": "public", "data": {"x": 1}
        });
        let h = ws.store_object(&target).unwrap();
        let mk = |att: bool| {
            let mut d = serde_json::json!({
                "schema_version": "1.0.0", "kind": "decision", "id": "d1",
                "brand_id": "testbrand", "privacy": "public",
                "created_by": {"kind": "human", "id": "o1"},
                "data": {"approved": true,
                    "target": {"kind": "platform", "id": "p1", "content_hash": h.as_hex()}}
            });
            if att {
                d["attestation"] = serde_json::json!({"method": "terminal_code"});
            }
            d
        };
        let rec = ws.list_objects(None).unwrap()[0].clone();
        let idx = ApprovalIndex::build(&[rec.clone(), mk(false)]);
        assert_eq!(idx.status(&rec), ApprovalStatus::NotApproved);
        let idx = ApprovalIndex::build(&[rec.clone(), mk(true)]);
        assert!(matches!(idx.status(&rec), ApprovalStatus::Approved { .. }));

        ws.store_object(&mk(false)).unwrap();
        let mut state = State {
            schema_version: "1.0.0".into(),
            brand_id: "testbrand".into(),
            content_snapshot_hash: None,
            governance_index_hash: None,
            generation: 0,
            name: None,
            created_at: "2026-10-27T00:00:00Z".into(),
        };
        ws.update_snapshot(&mut state).unwrap();
        ws.update_governance_index(&mut state).unwrap();
        ws.write_state(&state).unwrap();
        let v = reconcile_governance(&ws).unwrap();
        assert!(v
            .warnings
            .iter()
            .any(|w| w.starts_with("unattested_decision")));
        std::fs::remove_dir_all(&dir).ok();
    }
}
