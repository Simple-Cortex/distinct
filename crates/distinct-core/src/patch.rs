//! Typed patch operations for content records (contract §5, WO-11 / D-J).
//!
//! `patch --plan <file> --dry-run|--apply` applies a list of typed
//! add/replace/remove operations with exact parent, scope, and impact
//! preview. Approval remains a separate governance step.

use crate::record::RecordKind;
use crate::store::{BrandWorkspace, StoreError};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Validate that a kind string is a known content kind (not governance).
///
/// Governance kinds (decision, review, check_result) are writable only
/// through their own dedicated commands, not through patch.
fn require_content_kind(kind: &str, op_index: usize) -> Result<(), String> {
    let parsed: RecordKind = serde_json::from_str(&format!("\"{kind}\""))
        .map_err(|_| format!("operation[{op_index}]: unknown kind '{kind}'"))?;
    if !parsed.is_content() {
        return Err(format!(
            "operation[{op_index}]: kind '{kind}' is a governance kind and cannot be modified by patch"
        ));
    }
    Ok(())
}

/// A single patch operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum PatchOp {
    /// Add a new record. Requires `kind`, `id`, `data`, `parent_hash`.
    Add {
        kind: String,
        id: String,
        parent_hash: Option<String>,
        data: Value,
    },
    /// Replace an existing record's data. Requires `kind`, `id`, `parent_hash`, `data`.
    Replace {
        kind: String,
        id: String,
        parent_hash: String,
        data: Value,
    },
    /// Remove a record by marking it superseded. Requires `kind`, `id`, `parent_hash`.
    Remove {
        kind: String,
        id: String,
        parent_hash: String,
    },
}

/// A patch plan: a list of operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchPlan {
    pub brand_id: String,
    pub operations: Vec<PatchOp>,
}

/// Result of a dry-run preview.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchPreview {
    pub operation_count: usize,
    pub adds: usize,
    pub replaces: usize,
    pub removes: usize,
    pub affected_kinds: Vec<String>,
    pub affected_ids: Vec<String>,
    pub warnings: Vec<String>,
}

/// Result of applying a patch plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchResult {
    pub applied: usize,
    pub hashes: Vec<String>,
    pub warnings: Vec<String>,
}

/// Validate a patch plan from JSON.
pub fn parse_patch_plan(json: &Value) -> Result<PatchPlan, String> {
    let brand_id = json["brand_id"]
        .as_str()
        .ok_or_else(|| "missing required field: brand_id".to_string())?
        .to_string();

    let ops_array = json["operations"]
        .as_array()
        .ok_or_else(|| "missing required field: operations (must be array)".to_string())?;

    if ops_array.is_empty() {
        return Err("operations must not be empty".into());
    }

    let mut operations = Vec::new();
    for (i, op_val) in ops_array.iter().enumerate() {
        let op_str = op_val["op"].as_str().unwrap_or("?");
        match op_str {
            "add" => {
                let kind = op_val["kind"]
                    .as_str()
                    .ok_or_else(|| format!("operation[{i}]: missing 'kind'"))?
                    .to_string();
                require_content_kind(&kind, i)?;
                let id = op_val["id"]
                    .as_str()
                    .ok_or_else(|| format!("operation[{i}]: missing 'id'"))?
                    .to_string();
                let data = op_val["data"].clone();
                if data.is_null() {
                    return Err(format!("operation[{i}]: add requires 'data'"));
                }
                let parent_hash = op_val["parent_hash"].as_str().map(|s| s.to_string());
                operations.push(PatchOp::Add {
                    kind,
                    id,
                    parent_hash,
                    data,
                });
            }
            "replace" => {
                let kind = op_val["kind"]
                    .as_str()
                    .ok_or_else(|| format!("operation[{i}]: missing 'kind'"))?
                    .to_string();
                require_content_kind(&kind, i)?;
                let id = op_val["id"]
                    .as_str()
                    .ok_or_else(|| format!("operation[{i}]: missing 'id'"))?
                    .to_string();
                let parent_hash = op_val["parent_hash"]
                    .as_str()
                    .ok_or_else(|| format!("operation[{i}]: replace requires 'parent_hash'"))?
                    .to_string();
                let data = op_val["data"].clone();
                if data.is_null() {
                    return Err(format!("operation[{i}]: replace requires 'data'"));
                }
                operations.push(PatchOp::Replace {
                    kind,
                    id,
                    parent_hash,
                    data,
                });
            }
            "remove" => {
                let kind = op_val["kind"]
                    .as_str()
                    .ok_or_else(|| format!("operation[{i}]: missing 'kind'"))?
                    .to_string();
                require_content_kind(&kind, i)?;
                let id = op_val["id"]
                    .as_str()
                    .ok_or_else(|| format!("operation[{i}]: missing 'id'"))?
                    .to_string();
                let parent_hash = op_val["parent_hash"]
                    .as_str()
                    .ok_or_else(|| format!("operation[{i}]: remove requires 'parent_hash'"))?
                    .to_string();
                operations.push(PatchOp::Remove {
                    kind,
                    id,
                    parent_hash,
                });
            }
            other => return Err(format!("operation[{i}]: unknown op '{other}'")),
        }
    }

    Ok(PatchPlan {
        brand_id,
        operations,
    })
}

/// The current (head) revision of `kind`/`id`, chosen the same way approvals are
/// (`governance::current_revisions`), never the first match of an unordered listing.
fn head_revision<'a>(objects: &'a [Value], kind: &str, id: &str) -> Option<&'a Value> {
    crate::governance::current_revisions(objects)
        .into_iter()
        .find(|o| o["kind"].as_str() == Some(kind) && o["id"].as_str() == Some(id))
}

/// Generate a dry-run preview of a patch plan.
pub fn preview_patch(plan: &PatchPlan, ws: &BrandWorkspace) -> Result<PatchPreview, StoreError> {
    let objects = ws.list_objects(None)?;
    let mut preview = PatchPreview {
        operation_count: plan.operations.len(),
        adds: 0,
        replaces: 0,
        removes: 0,
        affected_kinds: Vec::new(),
        affected_ids: Vec::new(),
        warnings: Vec::new(),
    };

    for op in &plan.operations {
        match op {
            PatchOp::Add { kind, id, .. } => {
                preview.adds += 1;
                if !preview.affected_kinds.contains(kind) {
                    preview.affected_kinds.push(kind.clone());
                }
                preview.affected_ids.push(id.clone());

                // Warn if a record with the same kind+id already exists.
                let exists = objects
                    .iter()
                    .any(|o| o["kind"].as_str() == Some(kind) && o["id"].as_str() == Some(id));
                if exists {
                    preview.warnings.push(format!(
                        "add: {kind}/{id} already exists; will create a new revision"
                    ));
                }
            }
            PatchOp::Replace {
                kind,
                id,
                parent_hash,
                ..
            } => {
                preview.replaces += 1;
                if !preview.affected_kinds.contains(kind) {
                    preview.affected_kinds.push(kind.clone());
                }
                preview.affected_ids.push(id.clone());

                // Verify parent exists.
                let parent_exists = objects.iter().any(|o| {
                    o["kind"].as_str() == Some(kind)
                        && o["id"].as_str() == Some(id)
                        && o["content_hash"].as_str() == Some(parent_hash)
                });
                if !parent_exists {
                    preview.warnings.push(format!(
                        "replace: {kind}/{id} parent_hash {parent_hash} not found"
                    ));
                }
            }
            PatchOp::Remove {
                kind,
                id,
                parent_hash,
            } => {
                preview.removes += 1;
                if !preview.affected_kinds.contains(kind) {
                    preview.affected_kinds.push(kind.clone());
                }
                preview.affected_ids.push(id.clone());

                let parent_exists = objects.iter().any(|o| {
                    o["kind"].as_str() == Some(kind)
                        && o["id"].as_str() == Some(id)
                        && o["content_hash"].as_str() == Some(parent_hash)
                });
                if !parent_exists {
                    preview.warnings.push(format!(
                        "remove: {kind}/{id} parent_hash {parent_hash} not found"
                    ));
                }
            }
        }
    }

    Ok(preview)
}

/// Apply a patch plan: execute each operation and store resulting records.
///
/// `patch --apply` never approves anything — it only creates/replaces records.
/// Approval is a separate governance step.
pub fn apply_patch(
    plan: &PatchPlan,
    ws: &BrandWorkspace,
    created_at: &str,
) -> Result<PatchResult, StoreError> {
    let mut result = PatchResult {
        applied: 0,
        hashes: Vec::new(),
        warnings: Vec::new(),
    };

    for op in &plan.operations {
        match op {
            PatchOp::Add { kind, id, data, .. } => {
                let record = serde_json::json!({
                    "schema_version": crate::record::SCHEMA_VERSION,
                    "kind": kind,
                    "id": id,
                    "brand_id": plan.brand_id,
                    "created_at": created_at,
                    "created_by": {
                        "kind": "agent",
                        "id": "distinct-rs",
                    },
                    "privacy": "internal",
                    "dependencies": [],
                    "limitations": [],
                    "data": data,
                });
                let hash = ws.store_object(&record)?;
                result.hashes.push(hash.as_hex().to_string());
                result.applied += 1;
            }
            PatchOp::Replace {
                kind,
                id,
                parent_hash,
                data,
                ..
            } => {
                // WO-15: validate parent_hash matches the current content hash.
                let objects = ws.list_objects(None)?;
                let current = head_revision(&objects, kind, id);
                match current {
                    Some(rec) if rec["content_hash"].as_str() == Some(parent_hash) => {
                        // OK, proceed
                    }
                    Some(rec) => {
                        let current_hash = rec["content_hash"].as_str().unwrap_or("?");
                        return Err(StoreError::Conflict(format!(
                            "{kind}/{id} parent_hash {parent_hash} does not match current content hash {current_hash}"
                        )));
                    }
                    None => {
                        return Err(StoreError::Conflict(format!(
                            "{kind}/{id} not found for parent_hash {parent_hash}"
                        )));
                    }
                }
                // WO-15: preserve the original record's privacy.
                let original_privacy = current
                    .and_then(|r| r["privacy"].as_str())
                    .unwrap_or("internal");
                let record = serde_json::json!({
                    "schema_version": crate::record::SCHEMA_VERSION,
                    "kind": kind,
                    "id": id,
                    "brand_id": plan.brand_id,
                    "created_at": created_at,
                    "created_by": {
                        "kind": "agent",
                        "id": "distinct-rs",
                    },
                    "privacy": original_privacy,
                    "dependencies": [],
                    "limitations": [],
                    "data": data,
                    // Lineage: the head is the revision no other revision supersedes.
                    "supersedes": parent_hash,
                });
                let hash = ws.store_object(&record)?;
                result.hashes.push(hash.as_hex().to_string());
                result.applied += 1;
            }
            PatchOp::Remove {
                kind,
                id,
                parent_hash,
                ..
            } => {
                // WO-15: validate parent_hash matches the current content hash.
                let objects = ws.list_objects(None)?;
                let current = head_revision(&objects, kind, id);
                match current {
                    Some(rec) if rec["content_hash"].as_str() == Some(parent_hash) => {
                        // OK, proceed
                    }
                    Some(rec) => {
                        let current_hash = rec["content_hash"].as_str().unwrap_or("?");
                        return Err(StoreError::Conflict(format!(
                            "{kind}/{id} parent_hash {parent_hash} does not match current content hash {current_hash}"
                        )));
                    }
                    None => {
                        return Err(StoreError::Conflict(format!(
                            "{kind}/{id} not found for parent_hash {parent_hash}"
                        )));
                    }
                }
                // Immutable store: remove = store a tombstone marker.
                let record = serde_json::json!({
                    "schema_version": crate::record::SCHEMA_VERSION,
                    "kind": kind,
                    "id": id,
                    "brand_id": plan.brand_id,
                    "created_at": created_at,
                    "created_by": {
                        "kind": "agent",
                        "id": "distinct-rs",
                    },
                    "privacy": "internal",
                    "dependencies": [],
                    "limitations": ["tombstone: record removed by patch"],
                    "data": {
                        "removed": true,
                        "removed_at": created_at,
                    },
                    "supersedes": parent_hash,
                });
                let hash = ws.store_object(&record)?;
                result.hashes.push(hash.as_hex().to_string());
                result.applied += 1;
            }
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_add_operation() {
        let json = serde_json::json!({
            "brand_id": "acme",
            "operations": [{
                "op": "add",
                "kind": "claim",
                "id": "claim-1",
                "data": {"text": "fastest delivery"},
            }],
        });
        let plan = parse_patch_plan(&json).unwrap();
        assert_eq!(plan.brand_id, "acme");
        assert_eq!(plan.operations.len(), 1);
        assert!(matches!(&plan.operations[0], PatchOp::Add { kind, .. } if kind == "claim"));
    }

    #[test]
    fn parse_replace_requires_parent_hash() {
        let json = serde_json::json!({
            "brand_id": "acme",
            "operations": [{
                "op": "replace",
                "kind": "claim",
                "id": "claim-1",
                "data": {"text": "updated"},
            }],
        });
        let err = parse_patch_plan(&json).unwrap_err();
        assert!(err.contains("parent_hash"));
    }

    #[test]
    fn parse_remove_requires_parent_hash() {
        let json = serde_json::json!({
            "brand_id": "acme",
            "operations": [{
                "op": "remove",
                "kind": "claim",
                "id": "claim-1",
            }],
        });
        let err = parse_patch_plan(&json).unwrap_err();
        assert!(err.contains("parent_hash"));
    }

    #[test]
    fn parse_unknown_op_rejected() {
        let json = serde_json::json!({
            "brand_id": "acme",
            "operations": [{
                "op": "delete_all",
                "kind": "claim",
                "id": "claim-1",
            }],
        });
        let err = parse_patch_plan(&json).unwrap_err();
        assert!(err.contains("unknown op"));
    }

    #[test]
    fn empty_operations_rejected() {
        let json = serde_json::json!({
            "brand_id": "acme",
            "operations": [],
        });
        let err = parse_patch_plan(&json).unwrap_err();
        assert!(err.contains("must not be empty"));
    }

    #[test]
    fn preview_counts_operations() {
        let plan = PatchPlan {
            brand_id: "acme".into(),
            operations: vec![
                PatchOp::Add {
                    kind: "claim".into(),
                    id: "c1".into(),
                    parent_hash: None,
                    data: serde_json::json!({"text": "new"}),
                },
                PatchOp::Remove {
                    kind: "claim".into(),
                    id: "c2".into(),
                    parent_hash: "abc".into(),
                },
            ],
        };
        let dir = std::env::temp_dir().join(format!(
            "distinct-patch-preview-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let ws = BrandWorkspace::new(&dir, "acme");
        std::fs::create_dir_all(ws.objects_dir()).unwrap();

        let preview = preview_patch(&plan, &ws).unwrap();
        assert_eq!(preview.adds, 1);
        assert_eq!(preview.removes, 1);
        assert_eq!(preview.operation_count, 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_rejects_governance_kind_add() {
        let json = serde_json::json!({
            "brand_id": "acme",
            "operations": [{
                "op": "add",
                "kind": "decision",
                "id": "d1",
                "data": {"text": "test"},
            }],
        });
        let err = parse_patch_plan(&json).unwrap_err();
        assert!(
            err.contains("governance"),
            "expected governance error, got: {err}"
        );
        assert!(err.contains("operation[0]"), "error should name op index");
    }

    #[test]
    fn parse_rejects_governance_kind_replace() {
        let json = serde_json::json!({
            "brand_id": "acme",
            "operations": [{
                "op": "replace",
                "kind": "review",
                "id": "r1",
                "parent_hash": "abc123",
                "data": {"text": "test"},
            }],
        });
        let err = parse_patch_plan(&json).unwrap_err();
        assert!(
            err.contains("governance"),
            "expected governance error, got: {err}"
        );
    }

    #[test]
    fn parse_rejects_governance_kind_remove() {
        let json = serde_json::json!({
            "brand_id": "acme",
            "operations": [{
                "op": "remove",
                "kind": "check_result",
                "id": "cr1",
                "parent_hash": "abc123",
            }],
        });
        let err = parse_patch_plan(&json).unwrap_err();
        assert!(
            err.contains("governance"),
            "expected governance error, got: {err}"
        );
    }

    #[test]
    fn parse_accepts_content_kinds() {
        let json = serde_json::json!({
            "brand_id": "acme",
            "operations": [
                {"op": "add", "kind": "claim", "id": "c1", "data": {"text": "ok"}},
                {"op": "add", "kind": "test_plan", "id": "tp1", "data": {"q": "ok"}},
                {"op": "add", "kind": "check_plan", "id": "cp1", "data": {"scope": "all"}},
            ],
        });
        let plan = parse_patch_plan(&json).unwrap();
        assert_eq!(plan.operations.len(), 3);
    }
}
