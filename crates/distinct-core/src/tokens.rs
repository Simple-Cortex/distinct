//! W3C DTCG token export and deterministic GUIDELINES.md (WO-13, D-H).
//!
//! Extracts platform/visual-system fields from records and produces
//! `tokens.json` in W3C Design Token Community Group format. A field is
//! exported only if it is a supported token field AND its source record's
//! current revision is approved by a valid, current owner `decision` record
//! (see `governance::ApprovalIndex`). Everything else is omitted and listed in
//! `tokens_omitted[]` with a reason. Also generates a deterministic
//! `GUIDELINES.md` from the same data.

use crate::governance::{current_revisions, ApprovalIndex, ApprovalStatus};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// Record kinds that contribute tokens.
const TOKEN_SOURCE_KINDS: &[&str] = &["platform", "system", "direction"];

/// Supported field → DTCG `$type` mapping.
///
/// This only decides which fields map to DTCG types; it does not imply
/// approval. Any other field found in a source record's `data` is listed in
/// `tokens_omitted[]` as `unsupported_field`.
const SUPPORTED_TOKEN_FIELDS: &[(&str, &str)] = &[
    // Colors
    ("primary_color", "color"),
    ("secondary_color", "color"),
    ("accent_color", "color"),
    ("background_color", "color"),
    ("text_color", "color"),
    ("surface_color", "color"),
    ("border_color", "color"),
    // Dimensions
    ("spacing", "dimension"),
    ("border_radius", "dimension"),
    ("max_width", "dimension"),
    ("type_scale", "dimension"),
    // Typography
    ("font_family", "fontFamily"),
    ("heading_font", "fontFamily"),
    ("body_font", "fontFamily"),
    ("font_weight", "fontWeight"),
    ("heading_weight", "fontWeight"),
    ("body_weight", "fontWeight"),
];

/// Why a field was not exported as a token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OmitReason {
    NotOwnerApproved,
    StaleApproval,
    UnsupportedField,
    InvalidValue,
}

impl OmitReason {
    /// Stable wire name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotOwnerApproved => "not_owner_approved",
            Self::StaleApproval => "stale_approval",
            Self::UnsupportedField => "unsupported_field",
            Self::InvalidValue => "invalid_value",
        }
    }
}

/// A source field that was not exported.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct OmittedToken {
    pub field: String,
    pub record_id: String,
    pub reason: OmitReason,
}

impl OmittedToken {
    /// `{field, record_id, reason}` JSON object.
    pub fn to_json(&self) -> Value {
        serde_json::json!({
            "field": self.field,
            "record_id": self.record_id,
            "reason": self.reason.as_str(),
        })
    }
}

/// A token that was exported, with its approving decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovedToken {
    pub field: String,
    pub record_id: String,
    pub decision_id: String,
}

/// Result of token extraction.
#[derive(Debug)]
pub struct TokenExport {
    /// The DTCG token document (JSON object).
    pub tokens: Value,
    /// Exported tokens with their approving decision, sorted by field.
    pub approved: Vec<ApprovedToken>,
    /// Source fields not exported, sorted by (field, record_id, reason).
    pub omitted: Vec<OmittedToken>,
}

impl TokenExport {
    /// `tokens_omitted[]` as JSON objects.
    pub fn omitted_json(&self) -> Vec<Value> {
        self.omitted.iter().map(OmittedToken::to_json).collect()
    }
}

/// Source records eligible for token export: current revisions of
/// `platform`/`system`/`direction` kinds whose privacy is public or internal
/// (fail-closed), in sorted (kind, id) order.
fn source_records(objects: &[Value]) -> Vec<&Value> {
    current_revisions(objects)
        .into_iter()
        .filter(|o| TOKEN_SOURCE_KINDS.contains(&o["kind"].as_str().unwrap_or("")))
        .filter(|o| matches!(o["privacy"].as_str(), Some("public" | "internal")))
        .collect()
}

/// Extract DTCG tokens from a set of brand records (all stored revisions,
/// including `decision` records, which are used to derive approvals).
///
/// Per field of each current source record: unsupported fields are omitted as
/// `unsupported_field`; supported fields without a valid owner approval of the
/// record's current revision are omitted as `not_owner_approved` (or
/// `stale_approval` when only an older revision was approved); approved
/// fields with a malformed value are omitted as `invalid_value`.
pub fn extract_tokens(objects: &[Value]) -> TokenExport {
    let supported: BTreeMap<&str, &str> = SUPPORTED_TOKEN_FIELDS.iter().copied().collect();
    let approvals = ApprovalIndex::build(objects);

    let mut tokens: BTreeMap<String, (Value, ApprovedToken)> = BTreeMap::new();
    let mut omitted: Vec<OmittedToken> = Vec::new();

    for obj in source_records(objects) {
        let kind = obj["kind"].as_str().unwrap_or("");
        let record_id = obj["id"].as_str().unwrap_or("unknown");
        let Some(data) = obj["data"].as_object() else {
            continue;
        };
        let status = approvals.status(obj);

        for (field_name, field_value) in data {
            let mut omit = |reason| {
                omitted.push(OmittedToken {
                    field: field_name.clone(),
                    record_id: record_id.to_string(),
                    reason,
                });
            };
            let Some(dtcg_type) = supported.get(field_name.as_str()) else {
                omit(OmitReason::UnsupportedField);
                continue;
            };
            let decision_id = match &status {
                ApprovalStatus::Approved { decision_id } => decision_id,
                ApprovalStatus::Stale => {
                    omit(OmitReason::StaleApproval);
                    continue;
                }
                ApprovalStatus::NotApproved => {
                    omit(OmitReason::NotOwnerApproved);
                    continue;
                }
            };
            let Some(mut token) = to_dtcg_token(dtcg_type, field_value) else {
                omit(OmitReason::InvalidValue);
                continue;
            };
            if let Some(map) = token.as_object_mut() {
                map.insert(
                    "$description".to_string(),
                    Value::String(format!(
                        "{kind}/{record_id} → {field_name} (approved by {decision_id})"
                    )),
                );
            }
            tokens.insert(
                field_name.clone(),
                (
                    token,
                    ApprovedToken {
                        field: field_name.clone(),
                        record_id: record_id.to_string(),
                        decision_id: decision_id.clone(),
                    },
                ),
            );
        }
    }

    omitted.sort();
    let mut doc = Map::new();
    let mut approved = Vec::new();
    for (name, (token, meta)) in tokens {
        doc.insert(name, token);
        approved.push(meta);
    }

    TokenExport {
        tokens: Value::Object(doc),
        approved,
        omitted,
    }
}

/// Convert a JSON value to a DTCG token of the given type.
///
/// Returns `None` if the value does not match the expected shape.
fn to_dtcg_token(dtcg_type: &str, value: &Value) -> Option<Value> {
    match dtcg_type {
        "color" => {
            let s = value.as_str()?;
            if is_valid_hex_color(s) {
                Some(serde_json::json!({
                    "$type": "color",
                    "$value": s,
                }))
            } else {
                None
            }
        }
        "dimension" => {
            let n = value.as_f64()?;
            Some(serde_json::json!({
                "$type": "dimension",
                "$value": format_dim(n),
            }))
        }
        "fontFamily" => {
            let s = value.as_str()?;
            if s.is_empty() {
                return None;
            }
            Some(serde_json::json!({
                "$type": "fontFamily",
                "$value": s,
            }))
        }
        "fontWeight" => {
            let n = value
                .as_u64()
                .or_else(|| value.as_f64().map(|f| f as u64))?;
            if n == 0 {
                return None;
            }
            Some(serde_json::json!({
                "$type": "fontWeight",
                "$value": n,
            }))
        }
        "number" => {
            let n = value.as_f64()?;
            Some(serde_json::json!({
                "$type": "number",
                "$value": n,
            }))
        }
        _ => None,
    }
}

/// Validate a hex colour string: `#rgb`, `#rrggbb`, `#rrggbbaa`.
fn is_valid_hex_color(s: &str) -> bool {
    if !s.starts_with('#') {
        return false;
    }
    let hex = &s[1..];
    match hex.len() {
        3 | 6 | 8 => hex.chars().all(|c| c.is_ascii_hexdigit()),
        _ => false,
    }
}

/// Format a dimension value — integer if whole, otherwise float.
fn format_dim(n: f64) -> String {
    if n == n.floor() && n.is_finite() {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// Generate a deterministic `GUIDELINES.md` from records and token export.
///
/// The output is byte-identical across runs given the same input records.
/// No model-generated text: every line is derived from record data.
pub fn generate_guidelines(
    brand_id: &str,
    export_kind: &str,
    objects: &[Value],
    token_export: &TokenExport,
    missing_proofs: &[String],
    missing_reviews: &[String],
) -> String {
    let mut out = String::with_capacity(4096);

    out.push_str(&format!("# {brand_id} — Guidelines\n\n"));
    out.push_str(&format!("Export kind: {export_kind}\n\n"));
    out.push_str("Status: draft — not production ready\n\n");

    // Approved tokens section (owner-approved only).
    out.push_str("## Approved tokens\n\n");
    match token_export.tokens.as_object() {
        Some(map) if !map.is_empty() => {
            let by_field: BTreeMap<&str, &ApprovedToken> = token_export
                .approved
                .iter()
                .map(|a| (a.field.as_str(), a))
                .collect();
            let mut by_type: BTreeMap<String, Vec<(&str, &Value)>> = BTreeMap::new();
            for (name, token) in map {
                let dtcg_type = token["$type"].as_str().unwrap_or("unknown").to_string();
                by_type.entry(dtcg_type).or_default().push((name, token));
            }
            for (dtcg_type, entries) in &by_type {
                out.push_str(&format!("### {dtcg_type}\n\n"));
                for (name, token) in entries {
                    let value_str = format_token_value(&token["$value"]);
                    out.push_str(&format!("- `{name}`: `{value_str}`"));
                    if let Some(a) = by_field.get(name) {
                        out.push_str(&format!(
                            " — from `{}`, approved by decision `{}`",
                            a.record_id, a.decision_id
                        ));
                    }
                    out.push('\n');
                }
                out.push('\n');
            }
        }
        _ => out.push_str("No owner-approved tokens.\n\n"),
    }

    // Omitted fields section.
    out.push_str("## Omitted fields\n\n");
    if token_export.omitted.is_empty() {
        out.push_str("No fields omitted.\n\n");
    } else {
        out.push_str("The following source fields were not exported:\n\n");
        for o in &token_export.omitted {
            out.push_str(&format!(
                "- `{}` (record `{}`): {}\n",
                o.field,
                o.record_id,
                o.reason.as_str()
            ));
        }
        out.push('\n');
    }

    // Source records and their privacy (same eligibility as token extraction).
    out.push_str("## Source records\n\n");
    let mut sources: Vec<(&str, &str, &str)> = source_records(objects)
        .into_iter()
        .map(|o| {
            (
                o["kind"].as_str().unwrap_or(""),
                o["id"].as_str().unwrap_or("unknown"),
                o["privacy"].as_str().unwrap_or("unknown"),
            )
        })
        .collect();
    sources.sort();
    if sources.is_empty() {
        out.push_str("No source records found.\n\n");
    } else {
        for (kind, id, privacy) in &sources {
            out.push_str(&format!("- `{kind}/{id}` (privacy: {privacy})\n"));
        }
        out.push('\n');
    }

    // Missing proofs.
    out.push_str("## Missing proofs\n\n");
    if missing_proofs.is_empty() {
        out.push_str("None.\n\n");
    } else {
        for p in missing_proofs {
            out.push_str(&format!("- {p}\n"));
        }
        out.push('\n');
    }

    // Missing reviews.
    out.push_str("## Missing reviews\n\n");
    if missing_reviews.is_empty() {
        out.push_str("None.\n\n");
    } else {
        for r in missing_reviews {
            out.push_str(&format!("- {r}\n"));
        }
        out.push('\n');
    }

    // Rights.
    out.push_str("## Rights\n\n");
    out.push_str("UNLICENSED — internal draft. No redistribution.\n");

    out
}

/// Format a token value for display in GUIDELINES.md.
fn format_token_value(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Stamp `content_hash` / `revision_id` as `BrandWorkspace::store_object` does.
    fn stamp(mut v: Value) -> Value {
        let h = crate::hash::content_hash(&v).unwrap();
        v["content_hash"] = json!(h.as_hex());
        v["revision_id"] = json!(crate::hash::revision_id(&h));
        v
    }

    fn record(kind: &str, id: &str, created_at: &str, data: Value) -> Value {
        stamp(json!({
            "kind": kind, "id": id, "privacy": "public",
            "created_at": created_at, "data": data,
        }))
    }

    fn decision(id: &str, actor: &str, target: &Value, extra: Value) -> Value {
        let mut data = json!({
            "approved": true,
            "target": {
                "kind": target["kind"], "id": target["id"],
                "revision_id": target["revision_id"],
            },
        });
        if let (Some(d), Some(e)) = (data.as_object_mut(), extra.as_object()) {
            d.extend(e.clone());
        }
        stamp(json!({
            "kind": "decision", "id": id, "privacy": "public",
            "created_at": "2026-10-27T01:00:00Z",
            "created_by": {"kind": actor, "id": "o1"},
            "attestation": {"method": "terminal_code", "confirmed_at": "2026-10-27T01:00:00Z"},
            "data": data,
        }))
    }

    #[test]
    fn unattested_decision_does_not_approve_tokens() {
        let p = platform(json!({"primary_color": "#ff5500"}));
        let mut d = decision("d1", "human", &p, json!({}));
        d.as_object_mut().unwrap().remove("attestation");
        let r = extract_tokens(&[p, d]);
        assert!(r.approved.is_empty());
        assert_eq!(reasons(&r), vec![("primary_color", "not_owner_approved")]);
    }

    fn reasons(r: &TokenExport) -> Vec<(&str, &str)> {
        r.omitted
            .iter()
            .map(|o| (o.field.as_str(), o.reason.as_str()))
            .collect()
    }

    fn platform(data: Value) -> Value {
        record("platform", "main", "2026-10-27T00:00:00Z", data)
    }

    #[test]
    fn unapproved_color_omitted_not_owner_approved() {
        let p = platform(json!({"primary_color": "#ff5500"}));
        let r = extract_tokens(&[p]);
        assert!(r.tokens.as_object().unwrap().is_empty());
        assert_eq!(reasons(&r), vec![("primary_color", "not_owner_approved")]);
        assert_eq!(r.omitted[0].record_id, "main");
    }

    #[test]
    fn owner_approved_color_exported() {
        let p = platform(json!({"primary_color": "#ff5500"}));
        let d = decision("d1", "human", &p, json!({}));
        let r = extract_tokens(&[p, d]);
        assert_eq!(r.tokens["primary_color"]["$type"], "color");
        assert_eq!(r.tokens["primary_color"]["$value"], "#ff5500");
        assert!(r.omitted.is_empty());
        assert_eq!(r.approved[0].decision_id, "d1");
    }

    #[test]
    fn agent_actor_approval_not_counted() {
        let p = platform(json!({"primary_color": "#ff5500"}));
        let d = decision("d1", "agent", &p, json!({}));
        let r = extract_tokens(&[p, d]);
        assert!(r.tokens.as_object().unwrap().is_empty());
        assert_eq!(reasons(&r), vec![("primary_color", "not_owner_approved")]);
    }

    #[test]
    fn model_recommendation_and_unapproved_decisions_not_counted() {
        let p = platform(json!({"primary_color": "#ff5500"}));
        let rec = decision(
            "d1",
            "human",
            &p,
            json!({"authorization_source": "model_recommendation"}),
        );
        let fix = decision("d2", "human", &p, json!({"fixture_only": true}));
        let no = decision("d3", "human", &p, json!({"approved": false}));
        let r = extract_tokens(&[p, rec, fix, no]);
        assert_eq!(reasons(&r), vec![("primary_color", "not_owner_approved")]);
    }

    #[test]
    fn approval_of_older_revision_is_stale() {
        let old = platform(json!({"primary_color": "#111111"}));
        let d = decision("d1", "human", &old, json!({}));
        let new = record(
            "platform",
            "main",
            "2026-10-27T02:00:00Z",
            json!({"primary_color": "#222222"}),
        );
        let r = extract_tokens(&[old, d, new]);
        assert!(r.tokens.as_object().unwrap().is_empty());
        assert_eq!(reasons(&r), vec![("primary_color", "stale_approval")]);
    }

    #[test]
    fn superseded_decision_not_counted() {
        let p = platform(json!({"primary_color": "#ff5500"}));
        let d1 = decision("d1", "human", &p, json!({}));
        let d2 = decision(
            "d2",
            "human",
            &p,
            json!({"approved": false, "supersedes": "d1"}),
        );
        let r = extract_tokens(&[p, d1, d2]);
        assert_eq!(reasons(&r), vec![("primary_color", "not_owner_approved")]);
    }

    #[test]
    fn unsupported_and_invalid_values_reasons() {
        let p = platform(json!({
            "primary_color": "not-a-color",
            "custom_thing": "x",
            "spacing": 16,
        }));
        let d = decision("d1", "human", &p, json!({}));
        let r = extract_tokens(&[p, d]);
        assert_eq!(r.tokens["spacing"]["$value"], "16");
        assert_eq!(
            reasons(&r),
            vec![
                ("custom_thing", "unsupported_field"),
                ("primary_color", "invalid_value")
            ]
        );
    }

    #[test]
    fn non_source_kinds_and_restricted_ignored() {
        let claim = record(
            "claim",
            "c",
            "2026-10-27T00:00:00Z",
            json!({"primary_color": "#fff"}),
        );
        let mut restricted = platform(json!({"primary_color": "#fff"}));
        restricted["privacy"] = json!("restricted");
        let d = decision("d1", "human", &restricted, json!({}));
        let r = extract_tokens(&[claim, restricted, d]);
        assert!(r.tokens.as_object().unwrap().is_empty());
        assert!(r.omitted.is_empty());
    }

    #[test]
    fn dimension_font_family_and_weight_tokens() {
        let p = platform(json!({"spacing": 16, "font_family": "Inter", "font_weight": 400}));
        let d = decision("d1", "human", &p, json!({}));
        let r = extract_tokens(&[p, d]);
        assert_eq!(r.tokens["spacing"]["$type"], "dimension");
        assert_eq!(r.tokens["font_family"]["$value"], "Inter");
        assert_eq!(r.tokens["font_weight"]["$value"], 400);
    }

    fn sample() -> Vec<Value> {
        let a = platform(json!({"primary_color": "#ff5500", "spacing": 16, "zzz": 1}));
        let b = record(
            "direction",
            "type",
            "2026-10-27T00:00:00Z",
            json!({"font_family": "Inter", "unknown_field": "x"}),
        );
        let d = decision("d1", "human", &a, json!({}));
        vec![a, b, d]
    }

    #[test]
    fn guidelines_deterministic_and_complete() {
        let objects = sample();
        let mut reversed = objects.clone();
        reversed.reverse();
        let g1 = generate_guidelines(
            "acme",
            "draft",
            &objects,
            &extract_tokens(&objects),
            &[],
            &[],
        );
        let g2 = generate_guidelines(
            "acme",
            "draft",
            &reversed,
            &extract_tokens(&reversed),
            &[],
            &[],
        );
        assert_eq!(g1, g2, "GUIDELINES.md must be byte-identical across runs");
        assert!(g1.contains("Export kind: draft"));
        assert!(g1.contains("draft — not production ready"));
        assert!(g1.contains("approved by decision `d1`"));
        assert!(g1.contains("- `font_family` (record `type`): not_owner_approved"));
        assert!(g1.contains("- `zzz` (record `main`): unsupported_field"));
    }

    #[test]
    fn guidelines_contain_rights_section() {
        let g = generate_guidelines("acme", "draft", &[], &extract_tokens(&[]), &[], &[]);
        assert!(g.contains("## Rights"));
        assert!(g.contains("UNLICENSED"));
    }

    #[test]
    fn hex_color_validation() {
        assert!(is_valid_hex_color("#fff"));
        assert!(is_valid_hex_color("#FF5500"));
        assert!(is_valid_hex_color("#ff5500cc"));
        assert!(!is_valid_hex_color("fff"));
        assert!(!is_valid_hex_color("#gggggg"));
        assert!(!is_valid_hex_color("#12345"));
    }

    #[test]
    fn omitted_sorted_by_field_then_record() {
        let a = record(
            "platform",
            "a",
            "2026-10-27T00:00:00Z",
            json!({"z_field": 1, "a_field": 2}),
        );
        let b = record(
            "system",
            "b",
            "2026-10-27T00:00:00Z",
            json!({"z_field": 3, "m_field": 4}),
        );
        let r = extract_tokens(&[b, a]);
        let got: Vec<(&str, &str)> = r
            .omitted
            .iter()
            .map(|o| (o.field.as_str(), o.record_id.as_str()))
            .collect();
        assert_eq!(
            got,
            vec![
                ("a_field", "a"),
                ("m_field", "b"),
                ("z_field", "a"),
                ("z_field", "b")
            ]
        );
    }
}
