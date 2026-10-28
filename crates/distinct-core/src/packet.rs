//! Bounded task packet builder (contract §6, WO-04).
//!
//! A packet is a self-contained JSON document that conveys brand context
//! to an external consumer within a fixed byte budget.

use crate::hash::{self, HexHash, DOMAIN_PACKET};
use crate::record::Privacy;
use serde_json::{Map, Value};

/// Minimum permitted byte cap.
pub const MIN_CAP: usize = 1024;
/// Maximum permitted byte cap.
pub const MAX_CAP: usize = 1_048_576;
/// Default byte cap when the caller does not specify one.
pub const DEFAULT_CAP: usize = 16_384;

/// Anchor section names in declaration order.
pub const ANCHOR_KEYS: &[&str] = &[
    "platform_fields",
    "request",
    "protected_features",
    "constraints",
    "contradictions",
    "unknowns",
    "method_version",
    "source_refs",
    "output_contract",
];

/// Map a scope field name to the record kinds that populate it.
fn scope_to_record_kinds(field: &str) -> &[&str] {
    match field {
        "platform_fields" => &["platform"],
        "request" => &["direction", "system"],
        "protected_features" => &["claim"],
        "constraints" => &["voice", "current_brand"],
        "contradictions" => &["research_item"],
        "unknowns" => &["logo_intent", "logo_finding"],
        "method_version" => &["project"],
        "source_refs" => &["source"],
        "output_contract" => &["test_plan", "asset"],
        _ => &[],
    }
}

/// Options for building a packet.
pub struct PacketOptions<'a> {
    pub task: &'a str,
    pub brand_id: &'a str,
    pub snapshot_hash: Option<&'a str>,
    pub scope: Vec<String>,
    pub destination: Option<&'a str>,
    pub allowed_privacy: Vec<Privacy>,
    pub max_bytes: usize,
    pub objects: Vec<Value>,
}

/// Outcome of a packet build attempt.
#[derive(Debug)]
pub enum PacketResult {
    /// Packet built successfully.
    Ok {
        envelope: Value,
        packet_hash: HexHash,
        body: Value,
    },
    /// Cap below 1024 — invalid input, no source data leaked.
    CapTooSmall,
    /// Required records excluded by privacy filter.
    PrivacyDisallowed { blockers: Vec<String> },
    /// Essential content exceeds the cap.
    BudgetInsufficient {
        min_required_bytes: usize,
        split_proposal: Vec<String>,
    },
}

/// Build a bounded task packet from the given options.
pub fn build_packet(opts: &PacketOptions<'_>) -> PacketResult {
    if opts.max_bytes < MIN_CAP {
        return PacketResult::CapTooSmall;
    }

    // Require explicit destination when allowing confidential or restricted records.
    if opts.destination.is_none()
        && (opts.allowed_privacy.contains(&Privacy::Confidential)
            || opts.allowed_privacy.contains(&Privacy::Restricted))
    {
        return PacketResult::PrivacyDisallowed {
            blockers: vec![
                "confidential/restricted records require an explicit destination".into(),
            ],
        };
    }

    // Filter objects by privacy, then exclude quotes without consent.
    let after_privacy = filter_by_privacy(&opts.objects, &opts.allowed_privacy);
    let filtered: Vec<Value> = after_privacy
        .into_iter()
        .filter(|obj| !crate::record::is_quote_without_consent(obj))
        .collect();

    // Check that every requested scope field has at least one matching record.
    if let Some(blockers) = check_scope_coverage(&opts.scope, &filtered) {
        return PacketResult::PrivacyDisallowed { blockers };
    }

    // Group by kind.
    let mut by_kind: Map<String, Value> = Map::new();
    for obj in &filtered {
        let kind = obj["kind"].as_str().unwrap_or("unknown").to_string();
        if !by_kind.contains_key(&kind) {
            by_kind.insert(kind.clone(), Value::Array(vec![]));
        }
        if let Some(arr) = by_kind.get_mut(&kind).and_then(|v| v.as_array_mut()) {
            arr.push(obj.clone());
        }
    }

    // Build anchors from scope.
    let mut anchors = Map::new();
    for field in &opts.scope {
        let kinds = scope_to_record_kinds(field);
        let mut items = Vec::new();
        for kind in kinds {
            if let Some(records) = by_kind.get(*kind) {
                if let Some(arr) = records.as_array() {
                    items.extend(arr.iter().cloned());
                }
            }
        }
        anchors.insert(field.clone(), Value::Array(items));
    }

    // Collect examples from all filtered objects.
    let examples = collect_examples(&filtered);

    // Try with examples first.
    let body_with = build_body(opts, &anchors, &examples);
    let envelope_with = build_envelope(
        &hash::hash_json(DOMAIN_PACKET, &body_with)
            .map(|h| h.as_hex().to_string())
            .unwrap_or_default(),
        &body_with,
    );
    let full_with = format!(
        "{}\n",
        serde_json::to_string_pretty(&envelope_with).unwrap_or_default()
    );

    if full_with.len() <= opts.max_bytes {
        let hash = hash::hash_json(DOMAIN_PACKET, &body_with).unwrap();
        return PacketResult::Ok {
            envelope: envelope_with,
            packet_hash: hash,
            body: body_with,
        };
    }

    // Drop examples, try essential only.
    let body_essential = build_body(opts, &anchors, &[]);
    let envelope_essential = build_envelope(
        &hash::hash_json(DOMAIN_PACKET, &body_essential)
            .map(|h| h.as_hex().to_string())
            .unwrap_or_default(),
        &body_essential,
    );
    let full_essential = format!(
        "{}\n",
        serde_json::to_string_pretty(&envelope_essential).unwrap_or_default()
    );

    if full_essential.len() <= opts.max_bytes {
        let hash = hash::hash_json(DOMAIN_PACKET, &body_essential).unwrap();
        return PacketResult::Ok {
            envelope: envelope_essential,
            packet_hash: hash,
            body: body_essential,
        };
    }

    // Essential still over — budget_insufficient.
    let min_required = full_essential.len();
    let split = build_split_proposal(&anchors, opts.max_bytes);

    PacketResult::BudgetInsufficient {
        min_required_bytes: min_required,
        split_proposal: split,
    }
}

// ── helpers ──────────────────────────────────────────────────────────

fn filter_by_privacy(objects: &[Value], allowed: &[Privacy]) -> Vec<Value> {
    objects
        .iter()
        .filter(|obj| {
            // Missing or non-string privacy is treated as restricted (fail-closed).
            let p = match obj["privacy"].as_str() {
                Some(s) => s,
                None => return allowed.contains(&Privacy::Restricted),
            };
            let record_privacy = match p {
                "public" => Privacy::Public,
                "internal" => Privacy::Internal,
                "confidential" => Privacy::Confidential,
                "restricted" => Privacy::Restricted,
                _ => return allowed.contains(&Privacy::Restricted),
            };
            allowed.contains(&record_privacy)
        })
        .cloned()
        .collect()
}

fn check_scope_coverage(scope: &[String], filtered: &[Value]) -> Option<Vec<String>> {
    let mut blockers = Vec::new();
    for field in scope {
        let kinds = scope_to_record_kinds(field);
        if kinds.is_empty() {
            continue;
        }
        // Are there records of these kinds in the full store that got filtered?
        let has_matching = filtered.iter().any(|obj| {
            let kind = obj["kind"].as_str().unwrap_or("");
            kinds.contains(&kind)
        });
        if !has_matching {
            blockers.push(format!("scope field '{}' has no allowed records", field));
        }
    }
    if blockers.is_empty() {
        None
    } else {
        Some(blockers)
    }
}

fn collect_examples(objects: &[Value]) -> Vec<Value> {
    let mut examples = Vec::new();
    for obj in objects {
        if let Some(data_examples) = obj.get("data").and_then(|d| d.get("examples")) {
            if let Some(arr) = data_examples.as_array() {
                examples.extend(arr.iter().cloned());
            }
        }
    }
    examples
}

fn build_body(opts: &PacketOptions<'_>, anchors: &Map<String, Value>, examples: &[Value]) -> Value {
    let mut body = Map::new();
    body.insert("task".into(), Value::String(opts.task.to_string()));
    body.insert("brand_id".into(), Value::String(opts.brand_id.to_string()));
    body.insert(
        "snapshot_hash".into(),
        opts.snapshot_hash
            .map(|s| Value::String(s.to_string()))
            .unwrap_or(Value::Null),
    );
    body.insert(
        "scope".into(),
        Value::Array(
            opts.scope
                .iter()
                .map(|s| Value::String(s.clone()))
                .collect(),
        ),
    );
    body.insert(
        "destination".into(),
        opts.destination
            .map(|s| Value::String(s.to_string()))
            .unwrap_or(Value::Null),
    );
    body.insert(
        "allowed_privacy".into(),
        Value::Array(
            opts.allowed_privacy
                .iter()
                .map(|p| serde_json::to_value(p).unwrap())
                .collect(),
        ),
    );
    body.insert("anchors".into(), Value::Object(anchors.clone()));
    body.insert("examples".into(), Value::Array(examples.to_vec()));
    Value::Object(body)
}

fn build_envelope(packet_hash: &str, body: &Value) -> Value {
    let mut env = Map::new();
    env.insert("packet_hash".into(), Value::String(packet_hash.to_string()));
    for (k, v) in body.as_object().unwrap() {
        env.insert(k.clone(), v.clone());
    }
    env.insert(
        "counts".into(),
        Value::Object({
            let mut c = Map::new();
            let anchor_count = body["anchors"].as_object().map(|a| a.len()).unwrap_or(0);
            c.insert("anchor_sections".into(), Value::Number(anchor_count.into()));
            let example_count = body["examples"].as_array().map(|a| a.len()).unwrap_or(0);
            c.insert("examples".into(), Value::Number(example_count.into()));
            c
        }),
    );
    Value::Object(env)
}

/// Public wrapper for the split-proposal builder (used by the CLI budget error).
pub fn build_split_proposal_public(anchors: &Map<String, Value>, cap: usize) -> Vec<String> {
    build_split_proposal(anchors, cap)
}

fn build_split_proposal(anchors: &Map<String, Value>, cap: usize) -> Vec<String> {
    let total: usize = anchors
        .values()
        .map(|v| serde_json::to_string(v).unwrap_or_default().len())
        .sum();
    let per_group = (total / cap) + 1;
    let mut groups: Vec<Vec<String>> = vec![Vec::new(); per_group];
    let mut group_sizes = vec![0usize; per_group];

    for (key, value) in anchors {
        let size = serde_json::to_string(value).unwrap_or_default().len();
        // Find the group with the smallest current size.
        let mut min_idx = 0;
        for i in 1..per_group {
            if group_sizes[i] < group_sizes[min_idx] {
                min_idx = i;
            }
        }
        groups[min_idx].push(key.clone());
        group_sizes[min_idx] += size;
    }

    groups
        .iter()
        .enumerate()
        .map(|(i, keys)| format!("group_{}: {}", i + 1, keys.join(",")))
        .collect()
}

/// Parse a comma-separated privacy list.
pub fn parse_privacy_list(input: &str) -> Vec<Privacy> {
    input
        .split(',')
        .filter_map(|s| match s.trim() {
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

    fn sample_objects() -> Vec<Value> {
        vec![
            serde_json::json!({
                "schema_version": "1.0.0",
                "kind": "project",
                "id": "main",
                "brand_id": "acme",
                "privacy": "public",
                "data": {"name": "Acme Brand"}
            }),
            serde_json::json!({
                "schema_version": "1.0.0",
                "kind": "platform",
                "id": "web",
                "brand_id": "acme",
                "privacy": "public",
                "data": {"name": "Website"}
            }),
            serde_json::json!({
                "schema_version": "1.0.0",
                "kind": "claim",
                "id": "quality",
                "brand_id": "acme",
                "privacy": "internal",
                "data": {"text": "Best quality"}
            }),
            serde_json::json!({
                "schema_version": "1.0.0",
                "kind": "source",
                "id": "interview",
                "brand_id": "acme",
                "privacy": "public",
                "data": {"text": "Secret strategy"}
            }),
        ]
    }

    #[test]
    fn cap_too_small_returns_cap_too_small() {
        let opts = PacketOptions {
            task: "t",
            brand_id: "acme",
            snapshot_hash: None,
            scope: vec!["method_version".into()],
            destination: None,
            allowed_privacy: vec![Privacy::Public],
            max_bytes: 512,
            objects: sample_objects(),
        };
        assert!(matches!(build_packet(&opts), PacketResult::CapTooSmall));
    }

    #[test]
    fn privacy_disallowed_when_no_destination() {
        // scope includes "protected_features" which maps to "claim" (internal).
        // With no destination only public is allowed → claim is excluded → blocker.
        let opts = PacketOptions {
            task: "t",
            brand_id: "acme",
            snapshot_hash: None,
            scope: vec!["protected_features".into()],
            destination: None,
            allowed_privacy: vec![Privacy::Public],
            max_bytes: DEFAULT_CAP,
            objects: sample_objects(),
        };
        match build_packet(&opts) {
            PacketResult::PrivacyDisallowed { blockers } => {
                assert!(!blockers.is_empty());
            }
            other => panic!("expected PrivacyDisallowed, got {other:?}"),
        }
    }

    #[test]
    fn parse_privacy_list_works() {
        let list = parse_privacy_list("public,internal");
        assert_eq!(list, vec![Privacy::Public, Privacy::Internal]);
    }

    #[test]
    fn packet_missing_privacy_excluded() {
        let objects = vec![serde_json::json!({
            "schema_version": "1.0.0",
            "kind": "project",
            "id": "main",
            "brand_id": "acme",
            "data": {"name": "No privacy field"}
        })];
        let opts = PacketOptions {
            task: "t",
            brand_id: "acme",
            snapshot_hash: None,
            scope: vec![],
            destination: None,
            allowed_privacy: vec![Privacy::Public],
            max_bytes: DEFAULT_CAP,
            objects,
        };
        match build_packet(&opts) {
            PacketResult::Ok { body, .. } => {
                let examples = body["examples"].as_array().unwrap();
                assert!(
                    examples.is_empty(),
                    "record with missing privacy must be excluded (treated as restricted)"
                );
            }
            other => panic!("expected Ok with empty records, got {other:?}"),
        }
    }

    #[test]
    fn import_requires_privacy() {
        // Missing privacy → treated as restricted, not in allowed list → excluded.
        let objects_no_privacy = vec![serde_json::json!({
            "schema_version": "1.0.0",
            "kind": "project",
            "id": "main",
            "brand_id": "acme",
            "data": {}
        })];
        let opts = PacketOptions {
            task: "t",
            brand_id: "acme",
            snapshot_hash: None,
            scope: vec![],
            destination: None,
            allowed_privacy: vec![Privacy::Public, Privacy::Internal],
            max_bytes: DEFAULT_CAP,
            objects: objects_no_privacy,
        };
        match build_packet(&opts) {
            PacketResult::Ok { body, .. } => {
                assert!(body["examples"].as_array().unwrap().is_empty());
            }
            other => panic!("expected Ok, got {other:?}"),
        }
    }

    #[test]
    fn packet_unconsented_quote_excluded() {
        let objects = vec![
            serde_json::json!({
                "schema_version": "1.0.0",
                "kind": "project",
                "id": "main",
                "brand_id": "acme",
                "privacy": "public",
                "data": {"name": "Acme"}
            }),
            serde_json::json!({
                "schema_version": "1.0.0",
                "kind": "source",
                "id": "interview",
                "brand_id": "acme",
                "privacy": "public",
                "data": {"quote": "A quote", "consent": false}
            }),
        ];
        let opts = PacketOptions {
            task: "t",
            brand_id: "acme",
            snapshot_hash: None,
            scope: vec![],
            destination: None,
            allowed_privacy: vec![Privacy::Public],
            max_bytes: DEFAULT_CAP,
            objects,
        };
        match build_packet(&opts) {
            PacketResult::Ok { body, .. } => {
                let examples = body["examples"].as_array().unwrap();
                assert!(
                    examples.is_empty(),
                    "source with quote but no consent must be excluded"
                );
            }
            other => panic!("expected Ok, got {other:?}"),
        }
    }

    #[test]
    fn allow_confidential_requires_destination() {
        let opts = PacketOptions {
            task: "t",
            brand_id: "acme",
            snapshot_hash: None,
            scope: vec![],
            destination: None,
            allowed_privacy: vec![Privacy::Public, Privacy::Confidential],
            max_bytes: DEFAULT_CAP,
            objects: vec![],
        };
        assert!(
            matches!(build_packet(&opts), PacketResult::PrivacyDisallowed { .. }),
            "allowing confidential without destination must be rejected"
        );
    }
}
