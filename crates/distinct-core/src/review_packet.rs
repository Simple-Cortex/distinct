//! Review packet builder (contract §5–6, WO-11 / D-J).
//!
//! `review-packet --scope <id> --out <path>` freezes inspectable inputs
//! with the packet privacy filter and excludes builder rationale fields.

use crate::hash::{self, HexHash};
use crate::record::Privacy;
use crate::store::{BrandWorkspace, StoreError};
use serde_json::Value;

/// Fields that are builder rationale / persuasion and must be excluded.
const BUILDER_RATIONALE_FIELDS: &[&str] = &[
    "builder_rationale",
    "persuasion",
    "strategy_notes",
    "internal_reasoning",
    "model_reasoning",
    "agent_reasoning",
    "draft_notes",
    "working_hypothesis",
];

/// Build a review packet for the given scope.
///
/// Collects all records relevant to the scope, applies the privacy filter,
/// strips builder rationale fields, and writes the frozen packet to `out_path`.
pub fn build_review_packet(
    ws: &BrandWorkspace,
    scope: &str,
    out_path: &str,
    allowed_privacy: &[Privacy],
    max_bytes: usize,
) -> Result<ReviewPacketResult, StoreError> {
    let objects = ws.list_objects(None)?;

    // Filter records by scope relevance.
    let relevant = filter_by_scope(&objects, scope);

    // Apply privacy filter.
    let filtered = filter_by_privacy(&relevant, allowed_privacy);

    // Strip builder rationale fields.
    let cleaned: Vec<Value> = filtered.iter().map(strip_rationale).collect();

    // Build the packet body.
    let body = serde_json::json!({
        "scope": scope,
        "brand_id": ws.brand_id,
        "record_count": cleaned.len(),
        "records": cleaned,
        "excluded_rationale_fields": BUILDER_RATIONALE_FIELDS,
        "allowed_privacy": allowed_privacy.iter().map(|p| format!("{p:?}")).collect::<Vec<_>>(),
    });

    // Check byte budget.
    let serialized = serde_json::to_string(&body).unwrap_or_default();
    let envelope_overhead = 512;
    if serialized.len() + envelope_overhead > max_bytes {
        return Err(StoreError::Io(format!(
            "review packet exceeds budget: {} > {max_bytes}",
            serialized.len() + envelope_overhead
        )));
    }

    let packet_hash = hash::hash_json(crate::hash::DOMAIN_SCOPE, &body)
        .map_err(|e| StoreError::Hash(e.to_string()))?;

    // Write to file.
    let json =
        serde_json::to_string_pretty(&body).map_err(|e| StoreError::Serialize(e.to_string()))?;
    std::fs::write(out_path, format!("{json}\n"))
        .map_err(|e| StoreError::Io(format!("write review packet: {e}")))?;

    Ok(ReviewPacketResult {
        packet_hash,
        scope: scope.to_string(),
        record_count: cleaned.len(),
        out_path: out_path.to_string(),
        byte_length: json.len() + 1,
    })
}

/// Result of building a review packet.
#[derive(Debug, Clone)]
pub struct ReviewPacketResult {
    pub packet_hash: HexHash,
    pub scope: String,
    pub record_count: usize,
    pub out_path: String,
    pub byte_length: usize,
}

/// Filter records by scope relevance.
///
/// For scope "all", include everything. For a specific scope ID,
/// include records whose kind or id matches the scope.
fn filter_by_scope(objects: &[Value], scope: &str) -> Vec<Value> {
    if scope == "all" {
        return objects.to_vec();
    }

    objects
        .iter()
        .filter(|o| {
            let kind = o["kind"].as_str().unwrap_or("");
            let id = o["id"].as_str().unwrap_or("");
            // Match by scope as kind, id, or data scope field.
            kind == scope || id == scope || o["data"]["scope"].as_str() == Some(scope)
        })
        .cloned()
        .collect()
}

/// Apply privacy filter: only include records whose privacy class is allowed.
fn filter_by_privacy(records: &[Value], allowed: &[Privacy]) -> Vec<Value> {
    records
        .iter()
        .filter(|r| {
            let privacy_str = r["privacy"].as_str().unwrap_or("restricted");
            let record_privacy = match privacy_str {
                "public" => Some(Privacy::Public),
                "internal" => Some(Privacy::Internal),
                "confidential" => Some(Privacy::Confidential),
                "restricted" => Some(Privacy::Restricted),
                _ => None,
            };
            record_privacy.is_some_and(|p| allowed.contains(&p))
        })
        .cloned()
        .collect()
}

/// Strip builder rationale fields from a record.
fn strip_rationale(record: &Value) -> Value {
    let mut cleaned = record.clone();
    if let Some(obj) = cleaned.as_object_mut() {
        // Strip from top level.
        for field in BUILDER_RATIONALE_FIELDS {
            obj.remove(*field);
        }
        // Strip from data payload.
        if let Some(data) = obj.get_mut("data") {
            if let Some(data_obj) = data.as_object_mut() {
                for field in BUILDER_RATIONALE_FIELDS {
                    data_obj.remove(*field);
                }
            }
        }
    }
    cleaned
}

/// Parse a comma-separated privacy list.
pub fn parse_privacy_list(s: &str) -> Vec<Privacy> {
    s.split(',')
        .filter_map(|part| match part.trim() {
            "public" => Some(Privacy::Public),
            "internal" => Some(Privacy::Internal),
            "confidential" => Some(Privacy::Confidential),
            "restricted" => Some(Privacy::Restricted),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_rationale_removes_builder_fields() {
        let record = serde_json::json!({
            "kind": "claim",
            "id": "c1",
            "builder_rationale": "we think this works because...",
            "data": {
                "text": "fastest delivery",
                "strategy_notes": "internal thinking here",
            }
        });
        let cleaned = strip_rationale(&record);
        assert!(cleaned.get("builder_rationale").is_none());
        assert!(cleaned["data"].get("strategy_notes").is_none());
        assert_eq!(cleaned["data"]["text"], "fastest delivery");
    }

    #[test]
    fn filter_by_scope_all() {
        let objects = vec![
            serde_json::json!({"kind": "claim", "id": "c1"}),
            serde_json::json!({"kind": "platform", "id": "p1"}),
        ];
        let filtered = filter_by_scope(&objects, "all");
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn filter_by_scope_kind_match() {
        let objects = vec![
            serde_json::json!({"kind": "claim", "id": "c1"}),
            serde_json::json!({"kind": "platform", "id": "p1"}),
        ];
        let filtered = filter_by_scope(&objects, "claim");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0]["kind"], "claim");
    }

    #[test]
    fn filter_by_privacy_excludes_disallowed() {
        let records = vec![
            serde_json::json!({"kind": "claim", "privacy": "public"}),
            serde_json::json!({"kind": "claim", "privacy": "confidential"}),
            serde_json::json!({"kind": "claim", "privacy": "restricted"}),
        ];
        let allowed = vec![Privacy::Public, Privacy::Internal];
        let filtered = filter_by_privacy(&records, &allowed);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0]["privacy"], "public");
    }

    #[test]
    fn filter_by_privacy_missing_is_restricted() {
        let records = vec![serde_json::json!({"kind": "claim"})];
        let allowed = vec![Privacy::Public];
        let filtered = filter_by_privacy(&records, &allowed);
        assert_eq!(
            filtered.len(),
            0,
            "missing privacy should be treated as restricted"
        );
    }

    #[test]
    fn parse_privacy_list_works() {
        let list = parse_privacy_list("public,internal");
        assert_eq!(list.len(), 2);
        assert_eq!(list[0], Privacy::Public);
        assert_eq!(list[1], Privacy::Internal);
    }

    #[test]
    fn parse_privacy_list_ignores_unknown() {
        let list = parse_privacy_list("public,top_secret,internal");
        assert_eq!(list.len(), 2);
    }
}
