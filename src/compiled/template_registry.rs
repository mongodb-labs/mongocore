use std::collections::HashMap;
use std::sync::RwLock;
use regex::Regex;

use super::{LlmTemplate, ParameterType};
#[cfg(test)]
use super::LlmTemplateParameter;

/// Registry that stores LLM-provided templates and matches new intents against them.
pub struct TemplateRegistry {
    /// Templates keyed by a normalized pattern string
    templates: RwLock<Vec<RegisteredTemplate>>,
}

struct RegisteredTemplate {
    /// The original intent pattern (kept for debugging/logging)
    #[allow(dead_code)]
    intent_pattern: String,
    /// Compiled regex from the pattern
    regex: Regex,
    /// Parameter names in order of capture groups
    param_names: Vec<String>,
    /// Parameter types
    param_types: Vec<ParameterType>,
    /// The MQL pattern with placeholders
    mql_pattern: serde_json::Value,
    /// The method (filter/aggregate/etc.)
    method: String,
    /// Database and collection this template applies to
    database: String,
    collection: String,
}

/// Result of a successful template match
#[derive(Debug, Clone)]
pub struct TemplateMatch {
    pub mql_json: serde_json::Value,
    pub method: String,
}

impl TemplateRegistry {
    pub fn new() -> Self {
        Self {
            templates: RwLock::new(Vec::new()),
        }
    }

    /// Register a new template from an LLM response.
    pub fn register(
        &self,
        template: &LlmTemplate,
        method: &str,
        database: &str,
        collection: &str,
    ) {
        let (regex, param_names) = Self::compile_pattern(&template.intent_pattern);
        let param_types: Vec<ParameterType> = template
            .parameters
            .iter()
            .map(|p| p.param_type.clone())
            .collect();

        let registered = RegisteredTemplate {
            intent_pattern: template.intent_pattern.clone(),
            regex,
            param_names,
            param_types,
            mql_pattern: template.mql_pattern.clone(),
            method: method.to_string(),
            database: database.to_string(),
            collection: collection.to_string(),
        };

        self.templates.write().unwrap().push(registered);
    }

    /// Try to match an intent against registered templates.
    /// Returns the substituted MQL if a match is found.
    pub fn try_match(
        &self,
        intent: &str,
        database: &str,
        collection: &str,
    ) -> Option<TemplateMatch> {
        let templates = self.templates.read().unwrap();

        for template in templates.iter() {
            // Must be same database/collection
            if template.database != database || template.collection != collection {
                continue;
            }

            if let Some(captures) = template.regex.captures(intent) {
                // Extract parameter values from capture groups
                let mut values: HashMap<String, serde_json::Value> = HashMap::new();
                for (i, name) in template.param_names.iter().enumerate() {
                    if let Some(cap) = captures.get(i + 1) {
                        let value = match &template.param_types.get(i) {
                            Some(ParameterType::Number) => {
                                if let Ok(n) = cap.as_str().parse::<f64>() {
                                    serde_json::Value::from(n)
                                } else {
                                    serde_json::Value::String(cap.as_str().to_string())
                                }
                            }
                            _ => serde_json::Value::String(cap.as_str().to_string()),
                        };
                        values.insert(name.clone(), value);
                    }
                }

                // Substitute values into MQL pattern
                let substituted = Self::substitute_mql(&template.mql_pattern, &values);
                return Some(TemplateMatch {
                    mql_json: substituted,
                    method: template.method.clone(),
                });
            }
        }

        None
    }

    /// Number of registered templates.
    pub fn len(&self) -> usize {
        self.templates.read().unwrap().len()
    }

    /// Compile an intent pattern into a regex.
    /// "find {{cuisine}} restaurants in {{location}}" →
    /// regex: "^find (.+) restaurants in (.+)$"
    /// param_names: ["cuisine", "location"]
    fn compile_pattern(pattern: &str) -> (Regex, Vec<String>) {
        let mut param_names = Vec::new();
        let mut regex_str = "^".to_string();

        let parts: Vec<&str> = pattern.split("{{").collect();
        regex_str.push_str(&regex::escape(parts[0]));

        for part in &parts[1..] {
            if let Some(end_idx) = part.find("}}") {
                let param_name = &part[..end_idx];
                let rest = &part[end_idx + 2..];
                param_names.push(param_name.to_string());
                regex_str.push_str("(.+?)");
                regex_str.push_str(&regex::escape(rest));
            }
        }
        regex_str.push('$');

        let regex = Regex::new(&regex_str).unwrap_or_else(|_| Regex::new("^$").unwrap());
        (regex, param_names)
    }

    /// Substitute {{param}} placeholders in a JSON value with actual values.
    fn substitute_mql(
        pattern: &serde_json::Value,
        values: &HashMap<String, serde_json::Value>,
    ) -> serde_json::Value {
        match pattern {
            serde_json::Value::String(s) => {
                // Check if the entire string is a placeholder
                if s.starts_with("{{") && s.ends_with("}}") {
                    let param_name = &s[2..s.len() - 2];
                    if let Some(value) = values.get(param_name) {
                        return value.clone();
                    }
                }
                // Check for inline placeholders
                let mut result = s.clone();
                for (name, value) in values {
                    let placeholder = format!("{{{{{}}}}}", name);
                    if result.contains(&placeholder) {
                        let replacement = match value {
                            serde_json::Value::String(v) => v.clone(),
                            other => other.to_string(),
                        };
                        result = result.replace(&placeholder, &replacement);
                    }
                }
                serde_json::Value::String(result)
            }
            serde_json::Value::Object(map) => {
                let mut new_map = serde_json::Map::new();
                for (key, val) in map {
                    new_map.insert(key.clone(), Self::substitute_mql(val, values));
                }
                serde_json::Value::Object(new_map)
            }
            serde_json::Value::Array(arr) => {
                serde_json::Value::Array(
                    arr.iter().map(|v| Self::substitute_mql(v, values)).collect(),
                )
            }
            other => other.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compile_pattern_single_param() {
        let (regex, names) = TemplateRegistry::compile_pattern("find {{cuisine}} restaurants");
        assert_eq!(names, vec!["cuisine"]);
        assert!(regex.is_match("find Italian restaurants"));
        assert!(regex.is_match("find Chinese restaurants"));
        assert!(!regex.is_match("count restaurants by cuisine"));
    }

    #[test]
    fn test_compile_pattern_multiple_params() {
        let (regex, names) =
            TemplateRegistry::compile_pattern("find {{cuisine}} restaurants in {{location}}");
        assert_eq!(names, vec!["cuisine", "location"]);
        let caps = regex.captures("find Italian restaurants in Manhattan").unwrap();
        assert_eq!(caps.get(1).unwrap().as_str(), "Italian");
        assert_eq!(caps.get(2).unwrap().as_str(), "Manhattan");
    }

    #[test]
    fn test_compile_pattern_numeric() {
        let (regex, names) =
            TemplateRegistry::compile_pattern("find cities with population over {{threshold}}");
        assert_eq!(names, vec!["threshold"]);
        let caps = regex.captures("find cities with population over 50000").unwrap();
        assert_eq!(caps.get(1).unwrap().as_str(), "50000");
    }

    #[test]
    fn test_substitute_mql_string_value() {
        let pattern = serde_json::json!({"cuisine": "{{cuisine_type}}"});
        let mut values = HashMap::new();
        values.insert(
            "cuisine_type".to_string(),
            serde_json::Value::String("Italian".to_string()),
        );
        let result = TemplateRegistry::substitute_mql(&pattern, &values);
        assert_eq!(result, serde_json::json!({"cuisine": "Italian"}));
    }

    #[test]
    fn test_substitute_mql_number_value() {
        let pattern = serde_json::json!({"pop": {"$gt": "{{threshold}}"}});
        let mut values = HashMap::new();
        values.insert("threshold".to_string(), serde_json::json!(50000.0));
        let result = TemplateRegistry::substitute_mql(&pattern, &values);
        // When the entire string is "{{threshold}}", replace with the value directly (as number)
        assert_eq!(result, serde_json::json!({"pop": {"$gt": 50000.0}}));
    }

    #[test]
    fn test_substitute_mql_whole_value_placeholder() {
        let pattern = serde_json::json!({"pop": {"$gt": "{{threshold}}"}});
        let mut values = HashMap::new();
        values.insert("threshold".to_string(), serde_json::json!(50000));
        let result = TemplateRegistry::substitute_mql(&pattern, &values);
        // When the entire string is "{{threshold}}", replace with the value directly
        assert_eq!(result["pop"]["$gt"], serde_json::json!(50000));
    }

    #[test]
    fn test_registry_match_and_substitute() {
        let registry = TemplateRegistry::new();
        let template = LlmTemplate {
            intent_pattern: "find {{cuisine}} restaurants in {{location}}".to_string(),
            parameters: vec![
                LlmTemplateParameter {
                    name: "cuisine".to_string(),
                    value: serde_json::json!("Italian"),
                    param_type: ParameterType::String,
                },
                LlmTemplateParameter {
                    name: "location".to_string(),
                    value: serde_json::json!("Manhattan"),
                    param_type: ParameterType::String,
                },
            ],
            mql_pattern: serde_json::json!({"cuisine": "{{cuisine}}", "borough": "{{location}}"}),
        };
        registry.register(&template, "filter", "sample_restaurants", "restaurants");

        // Should match with different values
        let result = registry
            .try_match(
                "find Chinese restaurants in Brooklyn",
                "sample_restaurants",
                "restaurants",
            )
            .unwrap();
        assert_eq!(result.method, "filter");
        assert_eq!(result.mql_json["cuisine"], "Chinese");
        assert_eq!(result.mql_json["borough"], "Brooklyn");
    }

    #[test]
    fn test_registry_no_match_different_collection() {
        let registry = TemplateRegistry::new();
        let template = LlmTemplate {
            intent_pattern: "find {{cuisine}} restaurants".to_string(),
            parameters: vec![LlmTemplateParameter {
                name: "cuisine".to_string(),
                value: serde_json::json!("Italian"),
                param_type: ParameterType::String,
            }],
            mql_pattern: serde_json::json!({"cuisine": "{{cuisine}}"}),
        };
        registry.register(&template, "filter", "sample_restaurants", "restaurants");

        // Different collection — should NOT match
        let result = registry.try_match(
            "find Italian restaurants",
            "sample_mflix",
            "movies",
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_registry_no_match_different_structure() {
        let registry = TemplateRegistry::new();
        let template = LlmTemplate {
            intent_pattern: "find {{cuisine}} restaurants in {{location}}".to_string(),
            parameters: vec![
                LlmTemplateParameter {
                    name: "cuisine".to_string(),
                    value: serde_json::json!("Italian"),
                    param_type: ParameterType::String,
                },
                LlmTemplateParameter {
                    name: "location".to_string(),
                    value: serde_json::json!("Manhattan"),
                    param_type: ParameterType::String,
                },
            ],
            mql_pattern: serde_json::json!({"cuisine": "{{cuisine}}", "borough": "{{location}}"}),
        };
        registry.register(&template, "filter", "sample_restaurants", "restaurants");

        // Different structure — should NOT match
        let result = registry.try_match(
            "count restaurants by borough",
            "sample_restaurants",
            "restaurants",
        );
        assert!(result.is_none());
    }
}
