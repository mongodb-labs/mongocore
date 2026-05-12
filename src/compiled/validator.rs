use bson::Document;

pub struct MqlValidator;

/// Allowed aggregation stages
const ALLOWED_STAGES: &[&str] = &[
    "$match",
    "$project",
    "$sort",
    "$limit",
    "$skip",
    "$group",
    "$lookup",
    "$unwind",
    "$vectorSearch",
    "$search",
    "$count",
    "$addFields",
    "$set",
];

/// Blocked stages (dangerous operations)
const BLOCKED_STAGES: &[&str] = &[
    "$out",
    "$merge",
    "$collStats",
    "$currentOp",
    "$listSessions",
    "$planCacheStats",
];

impl MqlValidator {
    /// Validate a filter document. Returns Ok(()) if safe, Err with reason if not.
    pub fn validate_filter(filter: &Document) -> Result<(), String> {
        Self::check_dangerous_operators(filter)
    }

    /// Validate an aggregation pipeline. Returns Ok(()) if all stages are allowed.
    pub fn validate_pipeline(pipeline: &[Document]) -> Result<(), String> {
        for (i, stage) in pipeline.iter().enumerate() {
            let stage_name = stage
                .keys()
                .next()
                .ok_or_else(|| format!("Stage {} is empty", i))?;

            if BLOCKED_STAGES.contains(&stage_name.as_str()) {
                return Err(format!("Blocked stage '{}' at position {}", stage_name, i));
            }

            if !ALLOWED_STAGES.contains(&stage_name.as_str()) {
                return Err(format!(
                    "Unknown stage '{}' at position {} — not in allowlist",
                    stage_name, i
                ));
            }

            // Check for dangerous operators nested within stage content
            Self::check_dangerous_operators(stage)?;
        }
        Ok(())
    }

    /// Operators that allow arbitrary code execution
    const DANGEROUS_OPERATORS: &'static [&'static str] = &["$where", "$function", "$accumulator"];

    fn check_dangerous_operators(doc: &Document) -> Result<(), String> {
        for (key, value) in doc.iter() {
            if Self::DANGEROUS_OPERATORS.contains(&key.as_str()) {
                return Err(format!(
                    "'{}' operator is not allowed (code execution risk)",
                    key
                ));
            }
            // Recursively check nested documents
            if let Some(nested) = value.as_document() {
                Self::check_dangerous_operators(nested)?;
            }
            // Check arrays for nested documents
            if let Some(arr) = value.as_array() {
                for item in arr {
                    if let Some(nested) = item.as_document() {
                        Self::check_dangerous_operators(nested)?;
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bson::doc;

    #[test]
    fn valid_filter_passes() {
        let filter = doc! { "name": "Alice", "age": { "$gt": 25 } };
        assert!(MqlValidator::validate_filter(&filter).is_ok());
    }

    #[test]
    fn filter_with_where_is_rejected() {
        let filter = doc! { "$where": "this.a > 1" };
        let result = MqlValidator::validate_filter(&filter);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("$where"));
    }

    #[test]
    fn valid_pipeline_passes() {
        let pipeline = vec![
            doc! { "$match": { "status": "active" } },
            doc! { "$sort": { "name": 1 } },
            doc! { "$limit": 10 },
        ];
        assert!(MqlValidator::validate_pipeline(&pipeline).is_ok());
    }

    #[test]
    fn pipeline_with_out_is_blocked() {
        let pipeline = vec![
            doc! { "$match": { "status": "active" } },
            doc! { "$out": "results" },
        ];
        let result = MqlValidator::validate_pipeline(&pipeline);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("$out"));
    }

    #[test]
    fn pipeline_with_merge_is_blocked() {
        let pipeline = vec![doc! { "$merge": { "into": "output" } }];
        let result = MqlValidator::validate_pipeline(&pipeline);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("$merge"));
    }

    #[test]
    fn unknown_stage_is_rejected() {
        let pipeline = vec![doc! { "$fakeStage": {} }];
        let result = MqlValidator::validate_pipeline(&pipeline);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not in allowlist"));
    }

    #[test]
    fn empty_stage_is_rejected() {
        let pipeline = vec![doc! {}];
        let result = MqlValidator::validate_pipeline(&pipeline);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    #[test]
    fn nested_where_in_subdocument_is_caught() {
        let filter = doc! {
            "status": "active",
            "$and": [
                { "$where": "this.x > 1" }
            ]
        };
        let result = MqlValidator::validate_filter(&filter);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("$where"));
    }

    #[test]
    fn function_operator_in_filter_is_blocked() {
        let filter = doc! {
            "$expr": {
                "$function": {
                    "body": "function() { return true; }",
                    "args": [],
                    "lang": "js"
                }
            }
        };
        let result = MqlValidator::validate_filter(&filter);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("$function"));
    }

    #[test]
    fn accumulator_operator_in_filter_is_blocked() {
        let filter = doc! {
            "$expr": {
                "$accumulator": {
                    "init": "function() { return 0; }",
                    "accumulate": "function(state, val) { return state + val; }",
                    "merge": "function(a, b) { return a + b; }",
                    "lang": "js"
                }
            }
        };
        let result = MqlValidator::validate_filter(&filter);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("$accumulator"));
    }

    #[test]
    fn function_in_pipeline_addfields_is_blocked() {
        let pipeline = vec![doc! {
            "$addFields": {
                "computed": {
                    "$function": {
                        "body": "function(x) { return x * 2; }",
                        "args": ["$value"],
                        "lang": "js"
                    }
                }
            }
        }];
        // Pipeline stage is allowed ($addFields), but nested content has $function
        // This requires the pipeline validator to also check nested operators
        let result = MqlValidator::validate_pipeline(&pipeline);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("$function"));
    }

    #[test]
    fn deeply_nested_where_is_caught() {
        let filter = doc! {
            "$and": [{
                "$or": [{
                    "$and": [{
                        "$where": "this.x > 1"
                    }]
                }]
            }]
        };
        let result = MqlValidator::validate_filter(&filter);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("$where"));
    }

    #[test]
    fn regex_in_filter_is_allowed() {
        let filter = doc! { "name": { "$regex": "^test", "$options": "i" } };
        assert!(MqlValidator::validate_filter(&filter).is_ok());
    }

    #[test]
    fn function_operator_at_top_level_is_blocked() {
        let filter = doc! { "$function": { "body": "return true", "args": [], "lang": "js" } };
        let result = MqlValidator::validate_filter(&filter);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("$function"));
    }

    #[test]
    fn valid_geo_filter_passes() {
        let filter = doc! {
            "location": {
                "$near": {
                    "$geometry": { "type": "Point", "coordinates": [-73.97, 40.77] },
                    "$maxDistance": 5000
                }
            }
        };
        assert!(MqlValidator::validate_filter(&filter).is_ok());
    }
}
