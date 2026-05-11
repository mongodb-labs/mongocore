use regex::Regex;

use super::{ParameterType, QueryTemplate, TemplateParameter};

pub struct TemplateExtractor;

impl TemplateExtractor {
    /// Extract a template pattern and parameters from a natural language intent.
    /// Detects numeric values (like prices), quoted strings, and replaces them with placeholders.
    pub fn extract(intent: &str) -> Option<QueryTemplate> {
        let mut pattern = intent.to_string();
        let mut parameters = Vec::new();

        // Extract dollar amounts: "$50", "$100.00"
        let price_re = Regex::new(r"\$(\d+(?:\.\d+)?)").unwrap();
        for cap in price_re.captures_iter(intent) {
            let full_match = cap.get(0).unwrap().as_str();
            let param_name = format!("price_{}", parameters.len());
            let placeholder = format!("{{{}}}", param_name);
            pattern = pattern.replacen(full_match, &placeholder, 1);
            parameters.push(TemplateParameter {
                name: param_name,
                placeholder,
                value_type: ParameterType::Number,
            });
        }

        // Extract plain numbers (not already caught by dollar amounts)
        let number_re = Regex::new(r"\b(\d+(?:\.\d+)?)\b").unwrap();
        let current_pattern = pattern.clone();
        for cap in number_re.captures_iter(&current_pattern) {
            let full_match = cap.get(0).unwrap().as_str();
            // Skip if already replaced (inside a placeholder like {price_0})
            if pattern.contains(full_match) && !current_pattern.contains(&format!("{{{}", full_match))
            {
                let param_name = format!("num_{}", parameters.len());
                let placeholder = format!("{{{}}}", param_name);
                pattern = pattern.replacen(full_match, &placeholder, 1);
                parameters.push(TemplateParameter {
                    name: param_name,
                    placeholder,
                    value_type: ParameterType::Number,
                });
            }
        }

        // Extract quoted strings: "red", 'blue'
        let quoted_re = Regex::new(r#"["']([^"']+)["']"#).unwrap();
        let current_pattern = pattern.clone();
        for cap in quoted_re.captures_iter(&current_pattern) {
            let full_match = cap.get(0).unwrap().as_str();
            if pattern.contains(full_match) {
                let param_name = format!("str_{}", parameters.len());
                let placeholder = format!("{{{}}}", param_name);
                pattern = pattern.replacen(full_match, &placeholder, 1);
                parameters.push(TemplateParameter {
                    name: param_name,
                    placeholder,
                    value_type: ParameterType::String,
                });
            }
        }

        if parameters.is_empty() {
            return None; // No parameters found, not a template
        }

        Some(QueryTemplate {
            pattern,
            parameters,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_price_parameter() {
        let result = TemplateExtractor::extract("headphones under $50").unwrap();
        assert!(result.pattern.contains("{price_0}"));
        assert_eq!(result.parameters.len(), 1);
        assert!(matches!(result.parameters[0].value_type, ParameterType::Number));
    }

    #[test]
    fn extracts_number_parameter() {
        let result = TemplateExtractor::extract("find items with score above 90").unwrap();
        assert!(result.pattern.contains("{num_"));
        assert_eq!(result.parameters.len(), 1);
        assert!(matches!(result.parameters[0].value_type, ParameterType::Number));
    }

    #[test]
    fn extracts_string_parameter() {
        let result = TemplateExtractor::extract("find 'electronics' category").unwrap();
        assert!(result.pattern.contains("{str_"));
        assert_eq!(result.parameters.len(), 1);
        assert!(matches!(result.parameters[0].value_type, ParameterType::String));
    }

    #[test]
    fn returns_none_for_no_parameters() {
        let result = TemplateExtractor::extract("all documents");
        assert!(result.is_none());
    }

    #[test]
    fn multiple_parameters_in_one_intent() {
        let result =
            TemplateExtractor::extract("find 'electronics' items under $100").unwrap();
        assert!(result.parameters.len() >= 2);
    }
}
