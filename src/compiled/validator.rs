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
                return Err(format!(
                    "Blocked stage '{}' at position {}",
                    stage_name, i
                ));
            }

            if !ALLOWED_STAGES.contains(&stage_name.as_str()) {
                return Err(format!(
                    "Unknown stage '{}' at position {} — not in allowlist",
                    stage_name, i
                ));
            }
        }
        Ok(())
    }

    fn check_dangerous_operators(doc: &Document) -> Result<(), String> {
        for (key, value) in doc.iter() {
            if key == "$where" {
                return Err(
                    "$where operator is not allowed (code injection risk)".to_string()
                );
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
}
