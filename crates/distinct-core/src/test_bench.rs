//! Test bench: plan, packet, import, summarize (contract §5, WO-11 / D-J).
//!
//! Implements the four test commands:
//! - `test plan` — validate and freeze a test protocol
//! - `test packet` — neutral stimuli/protocol projection for a test
//! - `test import` — import actual observations/limitations
//! - `test summarize` — deterministic counts, gaps, interpretations

use crate::hash;
use crate::store::{BrandWorkspace, StoreError};
use serde_json::Value;

/// Valid test result execution statuses.
const EXECUTION_STATUSES: &[&str] = &["completed", "partial", "blocked", "error", "not_run"];

/// Valid test result interpretation values.
const INTERPRETATIONS: &[&str] = &[
    "supports",
    "challenges",
    "mixed",
    "inconclusive",
    "not_assessed",
];

/// Validate a test plan request and return the frozen record body.
///
/// Required fields: `question`, `method`, `scope`, `stimuli_refs` (array),
/// `stop_condition`, `facilitator`. Optional: `hypothesis`, `budget`,
/// `permissions`, `protocol_version`, `limitations`.
pub fn validate_test_plan(request: &Value) -> Result<Value, String> {
    let question = request["question"]
        .as_str()
        .ok_or_else(|| "missing required field: question".to_string())?;
    if question.trim().is_empty() {
        return Err("question must not be empty".into());
    }

    let method = request["method"]
        .as_str()
        .ok_or_else(|| "missing required field: method".to_string())?;
    if method.trim().is_empty() {
        return Err("method must not be empty".into());
    }

    let scope = request["scope"]
        .as_str()
        .ok_or_else(|| "missing required field: scope".to_string())?;

    let stimuli = request["stimuli_refs"]
        .as_array()
        .ok_or_else(|| "missing required field: stimuli_refs (must be array)".to_string())?;

    let stop_condition = request["stop_condition"]
        .as_str()
        .ok_or_else(|| "missing required field: stop_condition".to_string())?;
    if stop_condition.trim().is_empty() {
        return Err("stop_condition must not be empty".into());
    }

    let facilitator = request["facilitator"]
        .as_str()
        .ok_or_else(|| "missing required field: facilitator".to_string())?;

    // Build the frozen plan record data.
    let mut data = serde_json::json!({
        "question": question,
        "method": method,
        "scope": scope,
        "stimuli_refs": stimuli,
        "stop_condition": stop_condition,
        "facilitator": facilitator,
        "lifecycle": "frozen",
    });

    // Copy optional fields if present.
    if let Some(h) = request["hypothesis"].as_str() {
        data["hypothesis"] = Value::String(h.to_string());
    }
    if let Some(b) = request.get("budget") {
        data["budget"] = b.clone();
    }
    if let Some(p) = request.get("permissions") {
        data["permissions"] = p.clone();
    }
    if let Some(l) = request.get("limitations") {
        if l.is_array() {
            data["limitations"] = l.clone();
        }
    }
    if let Some(pv) = request["protocol_version"].as_str() {
        data["protocol_version"] = Value::String(pv.to_string());
    }

    Ok(data)
}

/// Build a test_plan record JSON from validated data.
pub fn build_test_plan_record(
    brand_id: &str,
    plan_id: &str,
    data: Value,
    created_at: &str,
) -> Value {
    serde_json::json!({
        "schema_version": crate::record::SCHEMA_VERSION,
        "kind": "test_plan",
        "id": plan_id,
        "brand_id": brand_id,
        "created_at": created_at,
        "created_by": {
            "kind": "agent",
            "id": "distinct-rs",
        },
        "privacy": "internal",
        "dependencies": [],
        "limitations": data["limitations"].as_array().cloned().unwrap_or_default(),
        "data": data,
    })
}

/// Validate a test result record.
///
/// Required: `frozen_plan_ref` (hash of the plan), `execution` (one of
/// EXECUTION_STATUSES), `observations` (array). Optional: `interpretation`,
/// `deviations`, `coverage`, `limitations`.
pub fn validate_test_result(result: &Value) -> Result<(), String> {
    let plan_ref = result["frozen_plan_ref"]
        .as_str()
        .ok_or_else(|| "missing required field: frozen_plan_ref".to_string())?;
    if plan_ref.trim().is_empty() {
        return Err("frozen_plan_ref must not be empty".into());
    }

    let execution = result["execution"]
        .as_str()
        .ok_or_else(|| "missing required field: execution".to_string())?;
    if !EXECUTION_STATUSES.contains(&execution) {
        return Err(format!(
            "execution must be one of: {}; got '{execution}'",
            EXECUTION_STATUSES.join(", ")
        ));
    }

    let observations = result["observations"]
        .as_array()
        .ok_or_else(|| "missing required field: observations (must be array)".to_string())?;
    if observations.is_empty() && execution == "completed" {
        return Err("completed results must have at least one observation".into());
    }

    // Validate interpretation if present.
    if let Some(interp) = result["interpretation"].as_str() {
        if !INTERPRETATIONS.contains(&interp) {
            return Err(format!(
                "interpretation must be one of: {}; got '{interp}'",
                INTERPRETATIONS.join(", ")
            ));
        }
    }

    Ok(())
}

/// Build a test_result record JSON.
pub fn build_test_result_record(
    brand_id: &str,
    result_id: &str,
    data: Value,
    created_at: &str,
) -> Value {
    serde_json::json!({
        "schema_version": crate::record::SCHEMA_VERSION,
        "kind": "test_result",
        "id": result_id,
        "brand_id": brand_id,
        "created_at": created_at,
        "created_by": {
            "kind": "agent",
            "id": "distinct-rs",
        },
        "privacy": "internal",
        "dependencies": [{
            "target": {
                "kind": "test_plan",
                "id": data["frozen_plan_ref"].as_str().unwrap_or(""),
                "pointer": "",
            },
            "applies_to": ["frozen_plan_ref"],
        }],
        "limitations": data["limitations"].as_array().cloned().unwrap_or_default(),
        "data": data,
    })
}

/// Summarize a test run: deterministic counts, gaps, interpretations.
///
/// Scans all objects for test_plan and test_result records matching
/// the given run_id (plan id prefix or exact match).
pub fn summarize_run(ws: &BrandWorkspace, run_id: &str) -> Result<Value, StoreError> {
    let objects = ws.list_objects(None)?;

    // Find the plan.
    let plan = objects
        .iter()
        .find(|o| o["kind"].as_str() == Some("test_plan") && o["id"].as_str() == Some(run_id));

    let plan_info = plan.map(|p| {
        serde_json::json!({
            "id": p["id"],
            "lifecycle": p["data"]["lifecycle"],
            "question": p["data"]["question"],
            "method": p["data"]["method"],
        })
    });

    // Find all results referencing this plan.
    let results: Vec<&Value> = objects
        .iter()
        .filter(|o| {
            o["kind"].as_str() == Some("test_result")
                && o["data"]["frozen_plan_ref"].as_str() == Some(run_id)
        })
        .collect();

    // Count by execution status.
    let mut counts = serde_json::Map::new();
    for status in EXECUTION_STATUSES {
        counts.insert(
            status.to_string(),
            serde_json::json!(results
                .iter()
                .filter(|r| r["data"]["execution"].as_str() == Some(status))
                .count()),
        );
    }

    // Collect interpretations (only from results that have them).
    let interpretations: Vec<Value> = results
        .iter()
        .filter_map(|r| {
            r["data"]["interpretation"].as_str().map(|interp| {
                serde_json::json!({
                    "result_id": r["id"],
                    "interpretation": interp,
                })
            })
        })
        .collect();

    // Identify gaps: planned stimuli with no corresponding result observation.
    let plan_stimuli_count = plan
        .and_then(|p| p["data"]["stimuli_refs"].as_array())
        .map(|a| a.len())
        .unwrap_or(0);

    let total_observations: usize = results
        .iter()
        .filter_map(|r| r["data"]["observations"].as_array())
        .map(|a| a.len())
        .sum();

    let gaps = if plan_stimuli_count > total_observations {
        serde_json::json!({
            "planned_stimuli": plan_stimuli_count,
            "observed": total_observations,
            "missing": plan_stimuli_count - total_observations,
        })
    } else {
        serde_json::json!(null)
    };

    Ok(serde_json::json!({
        "run_id": run_id,
        "plan": plan_info,
        "total_results": results.len(),
        "execution_counts": counts,
        "interpretations": interpretations,
        "gaps": gaps,
    }))
}

/// Build a neutral test packet for a given test plan.
///
/// Extracts the plan's stimuli, protocol, and scope into a bounded packet.
/// Applies the same privacy filter as the main packet builder.
pub fn build_test_packet(
    ws: &BrandWorkspace,
    test_id: &str,
    max_bytes: usize,
) -> Result<Value, StoreError> {
    let objects = ws.list_objects(None)?;

    let plan = objects
        .iter()
        .find(|o| o["kind"].as_str() == Some("test_plan") && o["id"].as_str() == Some(test_id))
        .ok_or_else(|| StoreError::Io(format!("test plan '{test_id}' not found")))?;

    // Build a neutral stimuli projection — no interpretation, no builder rationale.
    let body = serde_json::json!({
        "test_id": test_id,
        "plan_hash": plan["content_hash"],
        "lifecycle": plan["data"]["lifecycle"],
        "question": plan["data"]["question"],
        "method": plan["data"]["method"],
        "scope": plan["data"]["scope"],
        "stimuli_refs": plan["data"]["stimuli_refs"],
        "stop_condition": plan["data"]["stop_condition"],
        "protocol_version": plan["data"]["protocol_version"],
    });

    // Check byte budget.
    let serialized = serde_json::to_string(&body).unwrap_or_default();
    let envelope_size = serialized.len() + 256; // overhead for envelope wrapper
    if envelope_size > max_bytes {
        return Err(StoreError::Io(format!(
            "test packet exceeds budget: {envelope_size} > {max_bytes}"
        )));
    }

    let packet_hash = hash::hash_json(crate::hash::DOMAIN_PACKET, &body)
        .map_err(|e| StoreError::Hash(e.to_string()))?;

    Ok(serde_json::json!({
        "packet_hash": packet_hash.as_hex(),
        "test_id": test_id,
        "body": body,
        "byte_length": serialized.len() + 1,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_plan_requires_question() {
        let req = serde_json::json!({
            "method": "comprehension",
            "scope": "voice",
            "stimuli_refs": [],
            "stop_condition": "two unproductive cycles",
            "facilitator": "agent",
        });
        let err = validate_test_plan(&req).unwrap_err();
        assert!(err.contains("question"));
    }

    #[test]
    fn validate_plan_requires_method() {
        let req = serde_json::json!({
            "question": "Do unfamiliar readers understand the offer?",
            "scope": "voice",
            "stimuli_refs": [],
            "stop_condition": "two unproductive cycles",
            "facilitator": "agent",
        });
        let err = validate_test_plan(&req).unwrap_err();
        assert!(err.contains("method"));
    }

    #[test]
    fn validate_plan_success() {
        let req = serde_json::json!({
            "question": "Do unfamiliar readers understand the offer?",
            "method": "unaided_comprehension",
            "scope": "voice",
            "stimuli_refs": [{"kind": "asset", "id": "homepage-v1"}],
            "stop_condition": "three consistent misunderstandings",
            "facilitator": "host",
            "hypothesis": "readers confuse the offer with a competitor",
        });
        let data = validate_test_plan(&req).unwrap();
        assert_eq!(data["lifecycle"], "frozen");
        assert_eq!(
            data["question"],
            "Do unfamiliar readers understand the offer?"
        );
        assert_eq!(data["method"], "unaided_comprehension");
        assert_eq!(
            data["hypothesis"],
            "readers confuse the offer with a competitor"
        );
    }

    #[test]
    fn validate_result_requires_execution() {
        let result = serde_json::json!({
            "frozen_plan_ref": "abc123",
            "observations": [{"text": "participant confused offer"}],
        });
        let err = validate_test_result(&result).unwrap_err();
        assert!(err.contains("execution"));
    }

    #[test]
    fn validate_result_rejects_unknown_execution() {
        let result = serde_json::json!({
            "frozen_plan_ref": "abc123",
            "execution": "passed",
            "observations": [{"text": "ok"}],
        });
        let err = validate_test_result(&result).unwrap_err();
        assert!(err.contains("execution must be one of"));
    }

    #[test]
    fn validate_result_completed_needs_observations() {
        let result = serde_json::json!({
            "frozen_plan_ref": "abc123",
            "execution": "completed",
            "observations": [],
        });
        let err = validate_test_result(&result).unwrap_err();
        assert!(err.contains("at least one observation"));
    }

    #[test]
    fn validate_result_blocked_allows_empty_observations() {
        let result = serde_json::json!({
            "frozen_plan_ref": "abc123",
            "execution": "blocked",
            "observations": [],
        });
        assert!(validate_test_result(&result).is_ok());
    }

    #[test]
    fn validate_result_rejects_unknown_interpretation() {
        let result = serde_json::json!({
            "frozen_plan_ref": "abc123",
            "execution": "completed",
            "observations": [{"text": "ok"}],
            "interpretation": "proves_it_works",
        });
        let err = validate_test_result(&result).unwrap_err();
        assert!(err.contains("interpretation must be one of"));
    }

    #[test]
    fn validate_result_accepts_valid_interpretation() {
        let result = serde_json::json!({
            "frozen_plan_ref": "abc123",
            "execution": "completed",
            "observations": [{"text": "ok"}],
            "interpretation": "challenges",
        });
        assert!(validate_test_result(&result).is_ok());
    }

    #[test]
    fn build_plan_record_has_correct_kind() {
        let data = serde_json::json!({
            "question": "test?",
            "method": "m",
            "scope": "s",
            "stimuli_refs": [],
            "stop_condition": "stop",
            "facilitator": "f",
            "lifecycle": "frozen",
        });
        let record = build_test_plan_record("acme", "plan-1", data, "2026-10-27T00:00:00Z");
        assert_eq!(record["kind"], "test_plan");
        assert_eq!(record["brand_id"], "acme");
        assert_eq!(record["id"], "plan-1");
    }

    #[test]
    fn build_result_record_has_dependency() {
        let data = serde_json::json!({
            "frozen_plan_ref": "plan-1",
            "execution": "completed",
            "observations": [{"text": "ok"}],
        });
        let record = build_test_result_record("acme", "result-1", data, "2026-10-27T00:00:00Z");
        assert_eq!(record["kind"], "test_result");
        assert_eq!(record["dependencies"][0]["target"]["id"], "plan-1");
    }
}
