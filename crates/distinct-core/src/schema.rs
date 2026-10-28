//! JSON Schema validation with draft 2020-12, local-only references.
//!
//! Uses the `jsonschema` crate with `default-features=false` and
//! `resolve-file` only — no HTTP/TLS. Validates closed-field schemas
//! with version 1.0.0 and rejects unknown schema versions.

use serde_json::Value;

/// Schema validation errors.
#[derive(Debug, Clone)]
pub struct SchemaError {
    /// Human-readable error messages.
    pub errors: Vec<String>,
}

impl std::fmt::Display for SchemaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "schema validation failed: {}", self.errors.join("; "))
    }
}

impl std::error::Error for SchemaError {}

/// Validate a JSON value against a JSON Schema (draft 2020-12).
///
/// Returns `Ok(())` if valid, or `Err(SchemaError)` with all violations.
pub fn validate(schema: &Value, instance: &Value) -> Result<(), SchemaError> {
    let validator = jsonschema::validator_for(schema).map_err(|e| SchemaError {
        errors: vec![format!("invalid schema: {e}")],
    })?;

    let result = validator.validate(instance);
    match result {
        Ok(()) => Ok(()),
        Err(error) => {
            let msgs: Vec<String> = error.to_string().lines().map(|l| l.to_string()).collect();
            Err(SchemaError { errors: msgs })
        }
    }
}

/// Check that a schema declares the expected version.
///
/// Rejects unknown major/minor versions (contract section 4).
pub fn check_schema_version(schema: &Value, expected: &str) -> Result<(), SchemaError> {
    let version = schema
        .get("schema_version")
        .or_else(|| schema.get("$schema"))
        .and_then(|v| v.as_str());

    match version {
        Some(v) if v == expected || v.contains("2020-12") => Ok(()),
        Some(v) => Err(SchemaError {
            errors: vec![format!("unsupported schema version: {v}")],
        }),
        None => Err(SchemaError {
            errors: vec!["schema missing version field".into()],
        }),
    }
}

/// Validate that a JSON text has no duplicate keys before parsing.
///
/// Combines duplicate-key rejection with schema validation.
pub fn validate_json_text(schema: &Value, json_text: &str) -> Result<Value, SchemaError> {
    // Step 1: Reject duplicate keys
    crate::dup_keys::reject_duplicate_keys(json_text).map_err(|e| SchemaError {
        errors: vec![e.to_string()],
    })?;

    // Step 2: Parse
    let instance: Value = serde_json::from_str(json_text).map_err(|e| SchemaError {
        errors: vec![format!("JSON parse error: {e}")],
    })?;

    // Step 3: Validate against schema
    validate(schema, &instance)?;

    Ok(instance)
}

/// Maximum nesting depth for JSON documents (contract section 4 size/depth limits).
pub const MAX_DEPTH: usize = 32;

/// Maximum size for a JSON document in bytes (contract section 4).
pub const MAX_JSON_SIZE: usize = 10 * 1024 * 1024; // 10 MB

/// Check that a JSON text does not exceed size/depth limits.
pub fn check_limits(json_text: &str) -> Result<(), SchemaError> {
    if json_text.len() > MAX_JSON_SIZE {
        return Err(SchemaError {
            errors: vec![format!(
                "JSON size {} exceeds maximum {}",
                json_text.len(),
                MAX_JSON_SIZE
            )],
        });
    }
    // Depth check: count max nesting by tracking braces/brackets
    let mut depth: usize = 0;
    let mut max_depth: usize = 0;
    for b in json_text.bytes() {
        match b {
            b'{' | b'[' => {
                depth += 1;
                if depth > max_depth {
                    max_depth = depth;
                }
            }
            b'}' | b']' => {
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }
    if max_depth > MAX_DEPTH {
        return Err(SchemaError {
            errors: vec![format!(
                "JSON nesting depth {max_depth} exceeds maximum {MAX_DEPTH}"
            )],
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_instance_passes() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" }
            },
            "required": ["name"],
            "additionalProperties": false
        });
        let instance = serde_json::json!({"name": "test"});
        assert!(validate(&schema, &instance).is_ok());
    }

    #[test]
    fn missing_required_field_fails() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" }
            },
            "required": ["name"]
        });
        let instance = serde_json::json!({});
        let err = validate(&schema, &instance).unwrap_err();
        assert!(!err.errors.is_empty());
    }

    #[test]
    fn additional_property_rejected() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" }
            },
            "additionalProperties": false
        });
        let instance = serde_json::json!({"name": "test", "extra": true});
        let err = validate(&schema, &instance).unwrap_err();
        assert!(!err.errors.is_empty());
    }

    #[test]
    fn duplicate_keys_rejected_before_parse() {
        let schema = serde_json::json!({"type": "object"});
        let json_text = r#"{"a":1,"a":2}"#;
        let err = validate_json_text(&schema, json_text).unwrap_err();
        assert!(err.errors[0].contains("duplicate key"));
    }

    #[test]
    fn size_limit_enforced() {
        let huge = "x".repeat(MAX_JSON_SIZE + 1);
        let json_text = format!(r#"{{"data":"{huge}"}}"#);
        let err = check_limits(&json_text).unwrap_err();
        assert!(err.errors[0].contains("exceeds maximum"));
    }

    #[test]
    fn depth_limit_enforced() {
        // Build a deeply nested JSON
        let mut json = String::from(r#"{"a":"#);
        for _ in 0..MAX_DEPTH + 1 {
            json.push_str(r#"{"a":"#);
        }
        json.push('1');
        for _ in 0..MAX_DEPTH + 1 {
            json.push('}');
        }
        json.push('}');
        let err = check_limits(&json).unwrap_err();
        assert!(err.errors[0].contains("nesting depth"));
    }

    #[test]
    fn schema_version_check() {
        let schema = serde_json::json!({
            "schema_version": "1.0.0",
            "type": "object"
        });
        assert!(check_schema_version(&schema, "1.0.0").is_ok());

        let bad = serde_json::json!({
            "schema_version": "2.0.0",
            "type": "object"
        });
        assert!(check_schema_version(&bad, "1.0.0").is_err());
    }
}
