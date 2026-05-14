use std::collections::{HashMap, HashSet};
use std::time::Instant;

use mongodb::options::ReturnDocument;
use mongodb::ClientSession;
use serde_json::Value;

use crate::connection::pool::ConnectionPool;
use crate::defaults::{
    DEFAULT_TRANSACTION_PIPELINE_MAX_DOCS, DEFAULT_TRANSACTION_PIPELINE_MAX_RETRIES,
    DEFAULT_TRANSACTION_PIPELINE_MAX_STEPS, DEFAULT_TRANSACTION_PIPELINE_TIMEOUT_MS,
};
use crate::error::MongoCoreError;
use crate::operations::pipeline_refs::{extract_references, parse_reference, resolve_references};

/// Definition of a single pipeline step for validation and execution.
#[derive(Debug, Clone)]
pub struct PipelineStepDef {
    pub name: String,
    pub database: String,
    pub collection: String,
    pub operation_type: String,
    pub operation_json: Value,
    pub find_limit: Option<i64>,
}

/// Validate a transactional pipeline before execution.
pub fn validate_pipeline(steps: &[PipelineStepDef]) -> Result<(), MongoCoreError> {
    let err = |msg: String| MongoCoreError::TransactionPipelineError(msg);

    // Rule 1: Non-empty
    if steps.is_empty() {
        return Err(err("Pipeline must have at least one step".to_string()));
    }

    // Rule 2: Step count cap
    if steps.len() > DEFAULT_TRANSACTION_PIPELINE_MAX_STEPS {
        return Err(err(format!(
            "Pipeline exceeds maximum of {} steps",
            DEFAULT_TRANSACTION_PIPELINE_MAX_STEPS
        )));
    }

    let mut seen_names: HashSet<String> = HashSet::new();
    let mut ordered_names: Vec<String> = Vec::new();

    for step in steps {
        // Rule 7: Required fields
        if step.name.is_empty() {
            return Err(err("Step name must not be empty".to_string()));
        }
        if step.database.is_empty() {
            return Err(err(format!(
                "Step '{}': database must not be empty",
                step.name
            )));
        }
        if step.collection.is_empty() {
            return Err(err(format!(
                "Step '{}': collection must not be empty",
                step.name
            )));
        }

        // Rule 4: Valid name format (alphanumeric + underscore, starts with letter or underscore)
        if !is_valid_name(&step.name) {
            return Err(err(format!(
                "Step name '{}' is invalid: must start with a letter or underscore and contain only alphanumeric characters and underscores",
                step.name
            )));
        }

        // Rule 3: Unique step names
        if !seen_names.insert(step.name.clone()) {
            return Err(err(format!(
                "Step name '{}' is a duplicate",
                step.name
            )));
        }

        // Rule 9: No nesting
        let forbidden = ["begin_transaction", "commit_transaction", "abort_transaction"];
        if forbidden.contains(&step.operation_type.as_str()) {
            return Err(err(format!(
                "Step '{}': operation_type '{}' is not allowed in a pipeline",
                step.name, step.operation_type
            )));
        }

        // Rule 8: Find/Aggregate limit
        if let Some(limit) = step.find_limit {
            if limit > DEFAULT_TRANSACTION_PIPELINE_MAX_DOCS as i64 {
                return Err(err(format!(
                    "Step '{}': find_limit {} exceeds maximum of {}",
                    step.name, limit, DEFAULT_TRANSACTION_PIPELINE_MAX_DOCS
                )));
            }
        }

        // Rules 5 & 6: Valid references and no forward references
        let refs = extract_references(&step.operation_json);
        for ref_str in &refs {
            let (ref_step_name, _) = parse_reference(ref_str)?;
            if !seen_names.contains(&ref_step_name) {
                // Check if it's defined later (forward ref) vs not at all
                let is_forward = steps.iter().any(|s| s.name == ref_step_name);
                if is_forward {
                    return Err(err(format!(
                        "Step '{}': forward reference to step '{}' which is defined later",
                        step.name, ref_step_name
                    )));
                } else {
                    return Err(err(format!(
                        "Step '{}': references unknown step '{}'",
                        step.name, ref_step_name
                    )));
                }
            }
        }

        ordered_names.push(step.name.clone());
    }

    Ok(())
}

fn is_valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

// ─── Executor ────────────────────────────────────────────────────────────────

/// Options for transaction pipeline execution.
#[derive(Debug, Clone)]
pub struct TransactionPipelineOptions {
    pub read_concern: Option<String>,
    pub write_concern: Option<String>,
    pub max_time_ms: u64,
}

impl Default for TransactionPipelineOptions {
    fn default() -> Self {
        Self {
            read_concern: None,
            write_concern: None,
            max_time_ms: DEFAULT_TRANSACTION_PIPELINE_TIMEOUT_MS,
        }
    }
}

/// Result of executing a single step.
#[derive(Debug, Clone)]
pub struct StepResult {
    pub name: String,
    pub success: bool,
    pub result_json: Value,
}

/// Result of executing the full pipeline.
#[derive(Debug, Clone)]
pub struct PipelineExecutionResult {
    pub steps: Vec<StepResult>,
    pub total_steps: u32,
    pub steps_completed: u32,
    pub elapsed_ms: u64,
}

/// Error details when a pipeline fails.
#[derive(Debug, Clone)]
pub struct PipelineFailure {
    pub failed_step: String,
    pub step_index: u32,
    pub reason: String,
    pub steps_completed: Vec<String>,
    pub rolled_back: bool,
}

/// Execute a transactional pipeline: run validated steps sequentially within a
/// MongoDB transaction, resolving inter-step references. Retries on transient
/// transaction errors up to MAX_RETRIES times.
pub async fn execute_transaction_pipeline(
    pool: &ConnectionPool,
    steps: Vec<PipelineStepDef>,
    options: TransactionPipelineOptions,
) -> Result<PipelineExecutionResult, PipelineFailure> {
    // Validate first
    if let Err(e) = validate_pipeline(&steps) {
        return Err(PipelineFailure {
            failed_step: String::new(),
            step_index: 0,
            reason: e.to_string(),
            steps_completed: vec![],
            rolled_back: false,
        });
    }

    let start = Instant::now();

    for _attempt in 0..DEFAULT_TRANSACTION_PIPELINE_MAX_RETRIES {
        let mut session = pool.client().start_session().await.map_err(|e| {
            PipelineFailure {
                failed_step: String::new(),
                step_index: 0,
                reason: format!("Failed to start session: {}", e),
                steps_completed: vec![],
                rolled_back: false,
            }
        })?;

        let txn_options = mongodb::options::TransactionOptions::builder()
            .selection_criteria(mongodb::options::SelectionCriteria::ReadPreference(
                mongodb::options::ReadPreference::Primary,
            ))
            .build();
        session
            .start_transaction()
            .with_options(txn_options)
            .await
            .map_err(|e| PipelineFailure {
                failed_step: String::new(),
                step_index: 0,
                reason: format!("Failed to start transaction: {}", e),
                steps_completed: vec![],
                rolled_back: false,
            })?;

        let mut results_map: HashMap<String, Value> = HashMap::new();
        let mut step_results: Vec<StepResult> = Vec::new();
        let mut failed = false;
        let mut failure: Option<PipelineFailure> = None;

        for (idx, step) in steps.iter().enumerate() {
            // Check timeout
            let elapsed = start.elapsed().as_millis() as u64;
            if elapsed > options.max_time_ms {
                let _ = session.abort_transaction().await;
                return Err(PipelineFailure {
                    failed_step: step.name.clone(),
                    step_index: idx as u32,
                    reason: format!(
                        "Pipeline timeout exceeded ({}ms > {}ms)",
                        elapsed, options.max_time_ms
                    ),
                    steps_completed: step_results.iter().map(|s| s.name.clone()).collect(),
                    rolled_back: true,
                });
            }

            // Resolve references in operation_json
            let resolved_op = match resolve_references(&step.operation_json, &results_map) {
                Ok(v) => v,
                Err(e) => {
                    let _ = session.abort_transaction().await;
                    return Err(PipelineFailure {
                        failed_step: step.name.clone(),
                        step_index: idx as u32,
                        reason: format!("Reference resolution failed: {}", e),
                        steps_completed: step_results.iter().map(|s| s.name.clone()).collect(),
                        rolled_back: true,
                    });
                }
            };

            // Execute the step
            match execute_step(pool, &mut session, step, &resolved_op).await {
                Ok(result_val) => {
                    results_map.insert(step.name.clone(), result_val.clone());
                    step_results.push(StepResult {
                        name: step.name.clone(),
                        success: true,
                        result_json: result_val,
                    });
                }
                Err(e) => {
                    let _ = session.abort_transaction().await;
                    failure = Some(PipelineFailure {
                        failed_step: step.name.clone(),
                        step_index: idx as u32,
                        reason: e.to_string(),
                        steps_completed: step_results.iter().map(|s| s.name.clone()).collect(),
                        rolled_back: true,
                    });
                    failed = true;
                    break;
                }
            }
        }

        if failed {
            return Err(failure.unwrap());
        }

        // Commit
        match session.commit_transaction().await {
            Ok(()) => {
                let elapsed_ms = start.elapsed().as_millis() as u64;
                let steps_completed = step_results.len() as u32;
                return Ok(PipelineExecutionResult {
                    steps: step_results,
                    total_steps: steps.len() as u32,
                    steps_completed,
                    elapsed_ms,
                });
            }
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("TransientTransactionError") {
                    // Retry
                    continue;
                }
                return Err(PipelineFailure {
                    failed_step: String::new(),
                    step_index: steps.len() as u32,
                    reason: format!("Commit failed: {}", e),
                    steps_completed: step_results.iter().map(|s| s.name.clone()).collect(),
                    rolled_back: true,
                });
            }
        }
    }

    // Exhausted retries
    Err(PipelineFailure {
        failed_step: String::new(),
        step_index: 0,
        reason: format!(
            "Transaction failed after {} retries due to transient errors",
            DEFAULT_TRANSACTION_PIPELINE_MAX_RETRIES
        ),
        steps_completed: vec![],
        rolled_back: true,
    })
}

/// Execute a single pipeline step against the collection with the given session.
async fn execute_step(
    pool: &ConnectionPool,
    session: &mut ClientSession,
    step: &PipelineStepDef,
    resolved_op: &Value,
) -> Result<Value, MongoCoreError> {
    let coll = pool.collection(&step.database, &step.collection);

    match step.operation_type.as_str() {
        "find_one" => {
            let filter = resolved_op
                .get("filter")
                .map(bson_from_json_value)
                .unwrap_or_default();
            let mut cursor = coll.find(filter).limit(1).session(&mut *session).await?;
            if cursor.advance(&mut *session).await? {
                let doc = cursor.deserialize_current()?;
                Ok(bson_doc_to_json(&doc))
            } else {
                Ok(Value::Null)
            }
        }
        "find" => {
            let filter = resolved_op
                .get("filter")
                .map(bson_from_json_value)
                .unwrap_or_default();
            let limit = step
                .find_limit
                .unwrap_or(DEFAULT_TRANSACTION_PIPELINE_MAX_DOCS as i64);
            let mut cursor = coll
                .find(filter)
                .limit(limit)
                .session(&mut *session)
                .await?;
            let mut docs = Vec::new();
            while cursor.advance(&mut *session).await? {
                docs.push(bson_doc_to_json(&cursor.deserialize_current()?));
            }
            Ok(Value::Array(docs))
        }
        "insert" => {
            let document = resolved_op
                .get("document")
                .map(bson_from_json_value)
                .unwrap_or_default();
            let result = coll.insert_one(document).session(&mut *session).await?;
            let id_val = bson_to_json_value(&result.inserted_id);
            Ok(serde_json::json!({ "inserted_id": id_val }))
        }
        "insert_many" => {
            let docs: Vec<bson::Document> = resolved_op
                .get("documents")
                .and_then(|v| v.as_array())
                .unwrap_or(&vec![])
                .iter()
                .map(bson_from_json_value)
                .collect();
            let result = coll.insert_many(docs).session(&mut *session).await?;
            let ids: Vec<Value> = result
                .inserted_ids
                .values()
                .map(|id| bson_to_json_value(id))
                .collect();
            Ok(serde_json::json!({
                "inserted_ids": ids,
                "inserted_count": result.inserted_ids.len()
            }))
        }
        "update" => {
            let filter = resolved_op
                .get("filter")
                .map(bson_from_json_value)
                .unwrap_or_default();
            let update = resolved_op
                .get("update")
                .map(bson_from_json_value)
                .unwrap_or_default();
            let result = coll
                .update_one(filter, update)
                .session(&mut *session)
                .await?;
            let upserted_id = result
                .upserted_id
                .as_ref()
                .map(bson_to_json_value)
                .unwrap_or(Value::Null);
            Ok(serde_json::json!({
                "matched_count": result.matched_count,
                "modified_count": result.modified_count,
                "upserted_id": upserted_id
            }))
        }
        "update_many" => {
            let filter = resolved_op
                .get("filter")
                .map(bson_from_json_value)
                .unwrap_or_default();
            let update = resolved_op
                .get("update")
                .map(bson_from_json_value)
                .unwrap_or_default();
            let result = coll
                .update_many(filter, update)
                .session(&mut *session)
                .await?;
            let upserted_id = result
                .upserted_id
                .as_ref()
                .map(bson_to_json_value)
                .unwrap_or(Value::Null);
            Ok(serde_json::json!({
                "matched_count": result.matched_count,
                "modified_count": result.modified_count,
                "upserted_id": upserted_id
            }))
        }
        "delete" => {
            let filter = resolved_op
                .get("filter")
                .map(bson_from_json_value)
                .unwrap_or_default();
            let result = coll.delete_one(filter).session(&mut *session).await?;
            Ok(serde_json::json!({ "deleted_count": result.deleted_count }))
        }
        "delete_many" => {
            let filter = resolved_op
                .get("filter")
                .map(bson_from_json_value)
                .unwrap_or_default();
            let result = coll.delete_many(filter).session(&mut *session).await?;
            Ok(serde_json::json!({ "deleted_count": result.deleted_count }))
        }
        "find_and_modify" => {
            let filter = resolved_op
                .get("filter")
                .map(bson_from_json_value)
                .unwrap_or_default();
            let update = resolved_op
                .get("update")
                .map(bson_from_json_value)
                .unwrap_or_default();
            let result = coll
                .find_one_and_update(filter, update)
                .return_document(ReturnDocument::After)
                .session(&mut *session)
                .await?;
            match result {
                Some(doc) => Ok(bson_doc_to_json(&doc)),
                None => Ok(Value::Null),
            }
        }
        "aggregate" => {
            let pipeline: Vec<bson::Document> = resolved_op
                .get("pipeline")
                .and_then(|v| v.as_array())
                .unwrap_or(&vec![])
                .iter()
                .map(bson_from_json_value)
                .collect();
            let mut cursor = coll.aggregate(pipeline).session(&mut *session).await?;
            let mut docs = Vec::new();
            while cursor.advance(&mut *session).await?
                && docs.len() < DEFAULT_TRANSACTION_PIPELINE_MAX_DOCS
            {
                docs.push(bson_doc_to_json(&cursor.deserialize_current()?));
            }
            Ok(Value::Array(docs))
        }
        other => Err(MongoCoreError::TransactionPipelineError(format!(
            "Unsupported operation type: {}",
            other
        ))),
    }
}

fn bson_to_json_value(bson_val: &bson::Bson) -> Value {
    serde_json::to_value(bson_val).unwrap_or(Value::Null)
}

fn bson_from_json_value(value: &Value) -> bson::Document {
    match bson::to_bson(value) {
        Ok(bson::Bson::Document(doc)) => doc,
        _ => bson::Document::new(),
    }
}

fn bson_doc_to_json(doc: &bson::Document) -> Value {
    serde_json::to_value(doc).unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn step(name: &str, op_type: &str) -> PipelineStepDef {
        PipelineStepDef {
            name: name.to_string(),
            database: "testdb".to_string(),
            collection: "testcoll".to_string(),
            operation_type: op_type.to_string(),
            operation_json: json!({"filter": {}}),
            find_limit: None,
        }
    }

    fn step_with_ref(name: &str, op_type: &str, ref_str: &str) -> PipelineStepDef {
        PipelineStepDef {
            name: name.to_string(),
            database: "testdb".to_string(),
            collection: "testcoll".to_string(),
            operation_type: op_type.to_string(),
            operation_json: json!({"filter": {"_id": format!("{{{{{}}}}}", ref_str)}}),
            find_limit: None,
        }
    }

    #[test]
    fn test_valid_pipeline() {
        let steps = vec![
            step("find_user", "find_one"),
            step_with_ref("update_user", "update", "find_user._id"),
        ];
        assert!(validate_pipeline(&steps).is_ok());
    }

    #[test]
    fn test_empty_pipeline_rejected() {
        assert!(validate_pipeline(&[]).unwrap_err().to_string().contains("at least one step"));
    }

    #[test]
    fn test_too_many_steps_rejected() {
        let steps: Vec<_> = (0..51).map(|i| step(&format!("step_{}", i), "find_one")).collect();
        assert!(validate_pipeline(&steps).unwrap_err().to_string().contains("50"));
    }

    #[test]
    fn test_duplicate_names_rejected() {
        let steps = vec![step("find_user", "find_one"), step("find_user", "update")];
        assert!(validate_pipeline(&steps).unwrap_err().to_string().contains("duplicate"));
    }

    #[test]
    fn test_invalid_name_format_rejected() {
        let steps = vec![step("123bad", "find_one")];
        assert!(validate_pipeline(&steps).unwrap_err().to_string().contains("name"));
    }

    #[test]
    fn test_forward_reference_rejected() {
        let steps = vec![
            step_with_ref("step_a", "update", "step_b._id"),
            step("step_b", "find_one"),
        ];
        assert!(validate_pipeline(&steps).unwrap_err().to_string().contains("forward reference"));
    }

    #[test]
    fn test_unknown_reference_rejected() {
        let steps = vec![step_with_ref("step_a", "update", "nonexistent._id")];
        assert!(validate_pipeline(&steps).unwrap_err().to_string().contains("nonexistent"));
    }

    #[test]
    fn test_find_limit_over_101_rejected() {
        let mut s = step("find_step", "find");
        s.find_limit = Some(200);
        assert!(validate_pipeline(&[s]).unwrap_err().to_string().contains("101"));
    }

    #[test]
    fn test_find_limit_at_101_ok() {
        let mut s = step("find_step", "find");
        s.find_limit = Some(101);
        assert!(validate_pipeline(&[s]).is_ok());
    }

    #[test]
    fn test_missing_database_rejected() {
        let mut s = step("find_step", "find_one");
        s.database = "".to_string();
        assert!(validate_pipeline(&[s]).is_err());
    }

    #[test]
    fn test_missing_collection_rejected() {
        let mut s = step("find_step", "find_one");
        s.collection = "".to_string();
        assert!(validate_pipeline(&[s]).is_err());
    }

    #[test]
    fn test_transaction_ops_rejected() {
        let steps = vec![step("bad", "begin_transaction")];
        assert!(validate_pipeline(&steps).is_err());
    }
}
