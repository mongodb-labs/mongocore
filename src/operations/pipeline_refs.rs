use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;

use crate::error::MongoCoreError;

#[derive(Debug, Clone, PartialEq)]
pub enum PathSegment {
    Field(String),
    Index(usize),
    Wildcard,
}

/// Parse "step_name.field[0].sub" into (step_name, [Field("field"), Index(0), Field("sub")])
pub fn parse_reference(reference: &str) -> Result<(String, Vec<PathSegment>), MongoCoreError> {
    let err = |msg: String| MongoCoreError::TransactionPipelineError(msg);

    if reference.is_empty() {
        return Err(err("Empty reference".to_string()));
    }

    // Split on first '.' or '[' to get step name
    let step_end = reference
        .find(|c: char| c == '.' || c == '[')
        .unwrap_or(reference.len());

    let step_name = &reference[..step_end];
    if step_name.is_empty() {
        return Err(err("Empty step name in reference".to_string()));
    }

    let remainder = &reference[step_end..];
    if remainder.is_empty() {
        return Ok((step_name.to_string(), vec![]));
    }

    let mut segments = Vec::new();
    let mut rest = remainder;

    while !rest.is_empty() {
        if rest.starts_with('.') {
            rest = &rest[1..];
            // Read field name until next '.' or '['
            let field_end = rest
                .find(|c: char| c == '.' || c == '[')
                .unwrap_or(rest.len());
            if field_end == 0 {
                return Err(err(format!("Empty field name in reference: {}", reference)));
            }
            segments.push(PathSegment::Field(rest[..field_end].to_string()));
            rest = &rest[field_end..];
        } else if rest.starts_with('[') {
            let bracket_end = rest.find(']').ok_or_else(|| {
                err(format!("Unclosed bracket in reference: {}", reference))
            })?;
            let inside = &rest[1..bracket_end];
            if inside == "*" {
                segments.push(PathSegment::Wildcard);
            } else {
                let idx: usize = inside.parse().map_err(|_| {
                    err(format!("Invalid array index '{}' in reference: {}", inside, reference))
                })?;
                segments.push(PathSegment::Index(idx));
            }
            rest = &rest[bracket_end + 1..];
        } else {
            return Err(err(format!("Unexpected character in reference: {}", reference)));
        }
    }

    Ok((step_name.to_string(), segments))
}

/// Find all {{...}} references in a JSON value
pub fn extract_references(value: &Value) -> Vec<String> {
    let re = Regex::new(r"\{\{([^}]+)\}\}").unwrap();
    let mut refs = Vec::new();
    extract_refs_recursive(value, &re, &mut refs);
    refs
}

fn extract_refs_recursive(value: &Value, re: &Regex, refs: &mut Vec<String>) {
    match value {
        Value::String(s) => {
            for cap in re.captures_iter(s) {
                let r = cap[1].trim().to_string();
                if !refs.contains(&r) {
                    refs.push(r);
                }
            }
        }
        Value::Array(arr) => {
            for item in arr {
                extract_refs_recursive(item, re, refs);
            }
        }
        Value::Object(map) => {
            for (_key, val) in map {
                extract_refs_recursive(val, re, refs);
            }
        }
        _ => {}
    }
}

/// Resolve all {{...}} references in a JSON value using the results map
pub fn resolve_references(
    value: &Value,
    results: &HashMap<String, Value>,
) -> Result<Value, MongoCoreError> {
    let re = Regex::new(r"\{\{([^}]+)\}\}").unwrap();
    resolve_value(value, results, &re)
}

fn resolve_value(
    value: &Value,
    results: &HashMap<String, Value>,
    re: &Regex,
) -> Result<Value, MongoCoreError> {
    match value {
        Value::String(s) => resolve_string(s, results, re),
        Value::Array(arr) => {
            let resolved: Result<Vec<Value>, _> = arr
                .iter()
                .map(|v| resolve_value(v, results, re))
                .collect();
            Ok(Value::Array(resolved?))
        }
        Value::Object(map) => {
            let mut result = serde_json::Map::new();
            for (key, val) in map {
                result.insert(key.clone(), resolve_value(val, results, re)?);
            }
            Ok(Value::Object(result))
        }
        other => Ok(other.clone()),
    }
}

fn resolve_string(
    s: &str,
    results: &HashMap<String, Value>,
    re: &Regex,
) -> Result<Value, MongoCoreError> {
    // Check if the entire string is a single reference (type preservation)
    let trimmed = s.trim();
    if trimmed.starts_with("{{") && trimmed.ends_with("}}") {
        let inner = &trimmed[2..trimmed.len() - 2];
        // Make sure there's no other {{ in between (it's truly a single ref)
        if !inner.contains("{{") && !inner.contains("}}") {
            let resolved = resolve_single_ref(inner.trim(), results)?;
            return Ok(resolved);
        }
    }

    // Otherwise, interpolate all references as strings
    let mut result = s.to_string();
    for cap in re.captures_iter(s) {
        let full_match = &cap[0];
        let ref_str = cap[1].trim();
        let resolved = resolve_single_ref(ref_str, results)?;
        let string_val = match &resolved {
            Value::String(s) => s.clone(),
            Value::Null => "null".to_string(),
            other => other.to_string(),
        };
        result = result.replace(full_match, &string_val);
    }

    Ok(Value::String(result))
}

fn resolve_single_ref(
    ref_str: &str,
    results: &HashMap<String, Value>,
) -> Result<Value, MongoCoreError> {
    let (step_name, path) = parse_reference(ref_str)?;

    let step_result = results.get(&step_name).ok_or_else(|| {
        MongoCoreError::TransactionPipelineError(format!(
            "Step '{}' not found in results",
            step_name
        ))
    })?;

    if path.is_empty() {
        return Ok(step_result.clone());
    }

    traverse_path(step_result, &path, ref_str)
}

fn traverse_path(
    value: &Value,
    path: &[PathSegment],
    ref_str: &str,
) -> Result<Value, MongoCoreError> {
    if path.is_empty() {
        return Ok(value.clone());
    }

    let segment = &path[0];
    let remaining = &path[1..];

    match segment {
        PathSegment::Field(field) => {
            // Special case: .length on arrays
            if field == "length" {
                if let Value::Array(arr) = value {
                    return Ok(Value::Number(arr.len().into()));
                }
            }

            match value {
                Value::Object(map) => {
                    let field_value = map.get(field).ok_or_else(|| {
                        MongoCoreError::TransactionPipelineError(format!(
                            "Field '{}' not found in reference '{}'",
                            field, ref_str
                        ))
                    })?;
                    traverse_path(field_value, remaining, ref_str)
                }
                _ => Err(MongoCoreError::TransactionPipelineError(format!(
                    "Cannot access field '{}' on non-object value in reference '{}'",
                    field, ref_str
                ))),
            }
        }
        PathSegment::Index(idx) => match value {
            Value::Array(arr) => {
                let item = arr.get(*idx).ok_or_else(|| {
                    MongoCoreError::TransactionPipelineError(format!(
                        "Array index {} out of bounds in reference '{}'",
                        idx, ref_str
                    ))
                })?;
                traverse_path(item, remaining, ref_str)
            }
            _ => Err(MongoCoreError::TransactionPipelineError(format!(
                "Cannot index non-array value in reference '{}'",
                ref_str
            ))),
        },
        PathSegment::Wildcard => match value {
            Value::Array(arr) => {
                let results: Result<Vec<Value>, _> = arr
                    .iter()
                    .map(|item| traverse_path(item, remaining, ref_str))
                    .collect();
                Ok(Value::Array(results?))
            }
            _ => Err(MongoCoreError::TransactionPipelineError(format!(
                "Cannot use wildcard on non-array value in reference '{}'",
                ref_str
            ))),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_simple_field() {
        let (step, path) = parse_reference("find_user._id").unwrap();
        assert_eq!(step, "find_user");
        assert_eq!(path, vec![PathSegment::Field("_id".to_string())]);
    }

    #[test]
    fn test_parse_nested_field() {
        let (step, path) = parse_reference("find_user.address.city").unwrap();
        assert_eq!(step, "find_user");
        assert_eq!(
            path,
            vec![
                PathSegment::Field("address".to_string()),
                PathSegment::Field("city".to_string()),
            ]
        );
    }

    #[test]
    fn test_parse_array_index() {
        let (step, path) = parse_reference("find_users[0]._id").unwrap();
        assert_eq!(step, "find_users");
        assert_eq!(
            path,
            vec![
                PathSegment::Index(0),
                PathSegment::Field("_id".to_string())
            ]
        );
    }

    #[test]
    fn test_parse_wildcard() {
        let (step, path) = parse_reference("find_users[*].email").unwrap();
        assert_eq!(step, "find_users");
        assert_eq!(
            path,
            vec![
                PathSegment::Wildcard,
                PathSegment::Field("email".to_string())
            ]
        );
    }

    #[test]
    fn test_parse_full_result() {
        let (step, path) = parse_reference("find_users").unwrap();
        assert_eq!(step, "find_users");
        assert!(path.is_empty());
    }

    #[test]
    fn test_parse_length() {
        let (step, path) = parse_reference("find_users.length").unwrap();
        assert_eq!(step, "find_users");
        assert_eq!(path, vec![PathSegment::Field("length".to_string())]);
    }

    #[test]
    fn test_extract_references_from_json() {
        let val = json!({
            "filter": {"_id": "{{find_user._id}}"},
            "update": {"$set": {"city": "{{find_user.address.city}}"}}
        });
        let refs = extract_references(&val);
        assert_eq!(refs.len(), 2);
        assert!(refs.contains(&"find_user._id".to_string()));
        assert!(refs.contains(&"find_user.address.city".to_string()));
    }

    #[test]
    fn test_resolve_simple_reference() {
        let mut results = HashMap::new();
        results.insert(
            "find_user".to_string(),
            json!({"_id": "abc123", "name": "Alice"}),
        );
        let input = json!({"user_id": "{{find_user._id}}"});
        let resolved = resolve_references(&input, &results).unwrap();
        assert_eq!(resolved, json!({"user_id": "abc123"}));
    }

    #[test]
    fn test_resolve_preserves_type() {
        let mut results = HashMap::new();
        results.insert(
            "update_step".to_string(),
            json!({"modified_count": 5}),
        );
        let input = json!({"count": "{{update_step.modified_count}}"});
        let resolved = resolve_references(&input, &results).unwrap();
        assert_eq!(resolved, json!({"count": 5}));
    }

    #[test]
    fn test_resolve_inline_interpolation() {
        let mut results = HashMap::new();
        results.insert("find_user".to_string(), json!({"name": "Alice"}));
        let input = json!({"msg": "User {{find_user.name}} deactivated"});
        let resolved = resolve_references(&input, &results).unwrap();
        assert_eq!(resolved, json!({"msg": "User Alice deactivated"}));
    }

    #[test]
    fn test_resolve_wildcard_pluck() {
        let mut results = HashMap::new();
        results.insert(
            "find_users".to_string(),
            json!([
                {"_id": "a", "name": "Alice"},
                {"_id": "b", "name": "Bob"},
            ]),
        );
        let input = json!({"ids": "{{find_users[*]._id}}"});
        let resolved = resolve_references(&input, &results).unwrap();
        assert_eq!(resolved, json!({"ids": ["a", "b"]}));
    }

    #[test]
    fn test_resolve_full_passthrough() {
        let mut results = HashMap::new();
        results.insert(
            "find_expired".to_string(),
            json!([
                {"_id": "a", "status": "expired"},
                {"_id": "b", "status": "expired"},
            ]),
        );
        let input = json!("{{find_expired}}");
        let resolved = resolve_references(&input, &results).unwrap();
        assert_eq!(
            resolved,
            json!([
                {"_id": "a", "status": "expired"},
                {"_id": "b", "status": "expired"},
            ])
        );
    }

    #[test]
    fn test_resolve_length() {
        let mut results = HashMap::new();
        results.insert(
            "find_users".to_string(),
            json!([{"_id": "a"}, {"_id": "b"}, {"_id": "c"}]),
        );
        let input = json!({"count": "{{find_users.length}}"});
        let resolved = resolve_references(&input, &results).unwrap();
        assert_eq!(resolved, json!({"count": 3}));
    }

    #[test]
    fn test_resolve_missing_step_errors() {
        let results = HashMap::new();
        let input = json!({"id": "{{nonexistent._id}}"});
        assert!(resolve_references(&input, &results).is_err());
    }

    #[test]
    fn test_resolve_missing_field_errors() {
        let mut results = HashMap::new();
        results.insert("find_user".to_string(), json!({"_id": "abc"}));
        let input = json!({"email": "{{find_user.email}}"});
        assert!(resolve_references(&input, &results).is_err());
    }
}
