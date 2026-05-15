use bson::Document;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use super::context;
use super::session::SessionRecorder;

use crate::analytics::AnalyticsCollector;
use crate::compiled::translator::CompiledQueryTranslator;
use crate::connection::pool::ConnectionPool;
use crate::defaults::DEFAULT_PIPELINE_MAX_OPS;
use crate::ingestion::engine::IngestionEngine;
use crate::ingestion::types::*;
use crate::ingestion::watch::{DirectoryWatcher, WatchConfig};
use crate::mcp::codegen::{detect, index_gen, model_gen, query_gen, Language};
use crate::mcp::skills::registry::SkillRegistry;
use crate::operations::{FindOptions, IndexOptions, Operations, RawCommandOptions, ValidationMode};

use super::safety::SafetyConfig;
use super::types::{McpContent, McpToolCallResult, McpToolDefinition};

/// Maximum number of documents returned by find operations (safety control for AI agents).
const MAX_FIND_LIMIT: i64 = 100;

/// Return all MCP tool definitions with their JSON Schema input schemas.
pub fn tool_definitions() -> Vec<McpToolDefinition> {
    vec![
        McpToolDefinition {
            name: "find".to_string(),
            description: "Find documents in a MongoDB collection matching a filter".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Database name" },
                    "collection": { "type": "string", "description": "Collection name" },
                    "filter": { "type": "object", "description": "Query filter (default: {})" },
                    "limit": { "type": "integer", "description": "Maximum documents to return (max 100)" },
                    "skip": { "type": "integer", "description": "Number of documents to skip" }
                },
                "required": ["database", "collection"]
            }),
        },
        McpToolDefinition {
            name: "find_one".to_string(),
            description: "Find a single document in a MongoDB collection matching a filter"
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Database name" },
                    "collection": { "type": "string", "description": "Collection name" },
                    "filter": { "type": "object", "description": "Query filter (default: {})" }
                },
                "required": ["database", "collection"]
            }),
        },
        McpToolDefinition {
            name: "insert".to_string(),
            description: "Insert a single document into a MongoDB collection".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Database name" },
                    "collection": { "type": "string", "description": "Collection name" },
                    "document": { "type": "object", "description": "Document to insert" }
                },
                "required": ["database", "collection", "document"]
            }),
        },
        McpToolDefinition {
            name: "insert_many".to_string(),
            description: "Insert multiple documents into a MongoDB collection".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Database name" },
                    "collection": { "type": "string", "description": "Collection name" },
                    "documents": {
                        "type": "array",
                        "items": { "type": "object" },
                        "description": "Array of documents to insert"
                    }
                },
                "required": ["database", "collection", "documents"]
            }),
        },
        McpToolDefinition {
            name: "update".to_string(),
            description: "Update the first document matching a filter in a MongoDB collection"
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Database name" },
                    "collection": { "type": "string", "description": "Collection name" },
                    "filter": { "type": "object", "description": "Query filter to match documents" },
                    "update": { "type": "object", "description": "Update operations to apply" }
                },
                "required": ["database", "collection", "filter", "update"]
            }),
        },
        McpToolDefinition {
            name: "update_many".to_string(),
            description: "Update all documents matching a filter in a MongoDB collection"
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Database name" },
                    "collection": { "type": "string", "description": "Collection name" },
                    "filter": { "type": "object", "description": "Query filter to match documents" },
                    "update": { "type": "object", "description": "Update operations to apply" }
                },
                "required": ["database", "collection", "filter", "update"]
            }),
        },
        McpToolDefinition {
            name: "delete".to_string(),
            description: "Delete the first document matching a filter in a MongoDB collection"
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Database name" },
                    "collection": { "type": "string", "description": "Collection name" },
                    "filter": { "type": "object", "description": "Query filter to match the document to delete" }
                },
                "required": ["database", "collection", "filter"]
            }),
        },
        McpToolDefinition {
            name: "delete_many".to_string(),
            description: "Delete all documents matching a filter in a MongoDB collection"
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Database name" },
                    "collection": { "type": "string", "description": "Collection name" },
                    "filter": { "type": "object", "description": "Query filter to match documents to delete" }
                },
                "required": ["database", "collection", "filter"]
            }),
        },
        McpToolDefinition {
            name: "aggregate".to_string(),
            description: "Execute an aggregation pipeline on a MongoDB collection".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Database name" },
                    "collection": { "type": "string", "description": "Collection name" },
                    "pipeline": {
                        "type": "array",
                        "items": { "type": "object" },
                        "description": "Array of aggregation pipeline stages"
                    }
                },
                "required": ["database", "collection", "pipeline"]
            }),
        },
        McpToolDefinition {
            name: "create_collection".to_string(),
            description: "Create a new collection in a MongoDB database".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Database name" },
                    "collection": { "type": "string", "description": "Collection name to create" }
                },
                "required": ["database", "collection"]
            }),
        },
        McpToolDefinition {
            name: "create_index".to_string(),
            description: "Create an index on a MongoDB collection".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Database name" },
                    "collection": { "type": "string", "description": "Collection name" },
                    "keys": { "type": "object", "description": "Index key specification (e.g. {\"field\": 1})" },
                    "unique": { "type": "boolean", "description": "Whether the index enforces uniqueness" }
                },
                "required": ["database", "collection", "keys"]
            }),
        },
        McpToolDefinition {
            name: "list_databases".to_string(),
            description: "List all databases on the MongoDB server".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        McpToolDefinition {
            name: "list_collections".to_string(),
            description: "List all collections in a MongoDB database".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Database name" }
                },
                "required": ["database"]
            }),
        },
        McpToolDefinition {
            name: "run_command".to_string(),
            description: "Execute an arbitrary MongoDB command on a database".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Database name" },
                    "command": { "type": "object", "description": "MongoDB command document" },
                    "allow_all": { "type": "boolean", "description": "Allow all commands including dangerous ones (default: false)" }
                },
                "required": ["database", "command"]
            }),
        },
        McpToolDefinition {
            name: "get_analytics".to_string(),
            description: "Get analytics summary including operation counts, error rates, and latency percentiles".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "window_seconds": { "type": "integer", "description": "Time window in seconds (optional)" }
                },
                "required": []
            }),
        },
        McpToolDefinition {
            name: "ingest".to_string(),
            description: "Start an ingestion job to load data from a local file or remote URL into a MongoDB collection. Supports local paths and http/https URLs (e.g. CSV files hosted on GitHub).".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file_path": { "type": "string", "description": "Local file path or URL (http/https) to ingest" },
                    "database": { "type": "string", "description": "Target database name" },
                    "collection": { "type": "string", "description": "Target collection name" },
                    "format": { "type": "string", "enum": ["auto", "csv", "json", "ndjson", "parquet"], "description": "File format (default: auto-detect)" },
                    "dedup_key": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Fields to use for deduplication"
                    },
                    "conflict_strategy": { "type": "string", "enum": ["skip", "overwrite", "merge"], "description": "How to handle duplicate documents (default: skip)" },
                    "batch_size": { "type": "integer", "description": "Number of documents per batch (default: 1000)" },
                    "expressions": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Transform expressions to apply before ingestion. Supported: rename(old, new), drop(col1, col2), filter(col > val), cast(col, Type), select(col1, col2), compute(new_name, col1 + col2 - col3)"
                    },
                    "schema_overrides": { "type": "object", "description": "Field name to BSON type overrides (e.g. {\"age\": \"int32\"})" },
                    "sample_size": { "type": "integer", "description": "Number of rows to sample for schema inference (default: 1000)" }
                },
                "required": ["file_path", "database", "collection"]
            }),
        },
        McpToolDefinition {
            name: "ingest_status".to_string(),
            description: "Get the status of an ingestion job".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "job_id": { "type": "string", "description": "The ingestion job ID" }
                },
                "required": ["job_id"]
            }),
        },
        McpToolDefinition {
            name: "list_ingest_jobs".to_string(),
            description: "List all ingestion jobs".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        McpToolDefinition {
            name: "cancel_ingest".to_string(),
            description: "Cancel a running ingestion job".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "job_id": { "type": "string", "description": "The ingestion job ID to cancel" }
                },
                "required": ["job_id"]
            }),
        },
        McpToolDefinition {
            name: "watch_directory".to_string(),
            description: "Start watching a directory for new files and auto-ingest them into a MongoDB collection".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Directory path to watch" },
                    "database": { "type": "string", "description": "Target database name" },
                    "collection": { "type": "string", "description": "Target collection name" },
                    "file_pattern": { "type": "string", "description": "Glob pattern for files to watch (default: *)" },
                    "conflict_strategy": { "type": "string", "enum": ["skip", "overwrite", "merge"], "description": "How to handle duplicate documents (default: skip)" },
                    "dedup_key": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Fields to use for deduplication"
                    }
                },
                "required": ["path", "database", "collection"]
            }),
        },
        McpToolDefinition {
            name: "stop_watch".to_string(),
            description: "Stop watching a directory".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "watch_id": { "type": "string", "description": "The watch ID to stop" }
                },
                "required": ["watch_id"]
            }),
        },
        McpToolDefinition {
            name: "pipeline".to_string(),
            description: "Execute multiple independent operations concurrently in a single round-trip. All operations are validated before execution — if any violates safety rules, the entire pipeline is rejected.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "operations": {
                        "type": "array",
                        "description": "List of operations to execute concurrently",
                        "items": {
                            "type": "object",
                            "properties": {
                                "op": {
                                    "type": "string",
                                    "enum": ["find", "find_one", "insert", "insert_many", "update", "update_many", "delete", "delete_many", "aggregate", "find_and_modify", "run_command", "search", "create_collection", "create_index", "list_databases", "list_collections", "begin_transaction", "commit_transaction", "abort_transaction", "get_analytics"],
                                    "description": "Operation type"
                                },
                                "database": { "type": "string", "description": "Database name" },
                                "collection": { "type": "string", "description": "Collection name" },
                                "filter": { "type": "object", "description": "Query filter" },
                                "document": { "type": "object", "description": "Document to insert" },
                                "documents": { "type": "array", "description": "Documents for insert_many" },
                                "pipeline": { "type": "array", "description": "Aggregation pipeline stages" },
                                "update": { "type": "object", "description": "Update specification" },
                                "command": { "type": "object", "description": "Raw command document" },
                                "options": { "type": "object", "description": "Operation-specific options (limit, skip, sort, projection, upsert)" }
                            },
                            "required": ["op"]
                        },
                        "maxItems": 100
                    }
                },
                "required": ["operations"]
            }),
        },
        McpToolDefinition {
            name: "collection_schema".to_string(),
            description: "Sample documents from a collection and infer the schema (field names, BSON types, cardinality, example values)".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Database name" },
                    "collection": { "type": "string", "description": "Collection name" },
                    "sample_size": { "type": "integer", "description": "Number of documents to sample (default 100)", "default": 100 }
                },
                "required": ["database", "collection"]
            }),
        },
        McpToolDefinition {
            name: "ask".to_string(),
            description: "Ask a natural language question about your data — this is the preferred tool for querying. Automatically translates your question to an optimal MQL query, executes it, and returns results. Use this instead of manually constructing find or aggregate queries.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "question": { "type": "string", "description": "Natural language question about your data" },
                    "database": { "type": "string", "description": "Database to query" },
                    "collection": { "type": "string", "description": "Collection to query (optional — auto-detect if omitted)" }
                },
                "required": ["question", "database"]
            }),
        },
        McpToolDefinition {
            name: "explain_query".to_string(),
            description: "Translate a natural language question to MQL and show the execution plan without running it. Safe for expensive queries.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "question": { "type": "string", "description": "Natural language question" },
                    "database": { "type": "string", "description": "Database name" },
                    "collection": { "type": "string", "description": "Collection name (optional)" }
                },
                "required": ["question", "database"]
            }),
        },
        McpToolDefinition {
            name: "generate_code".to_string(),
            description: "Generate ready-to-run MongoCore client code for a query. Detects your project language and framework, provides composable skill recommendations.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Database name" },
                    "collection": { "type": "string", "description": "Collection name" },
                    "operation": { "type": "string", "enum": ["find", "aggregate", "insert"], "description": "Operation type (default: find)" },
                    "filter": { "type": "object", "description": "MQL filter for find operations" },
                    "pipeline": { "type": "array", "description": "Aggregation pipeline stages" },
                    "language": { "type": "string", "enum": ["python", "typescript", "go", "java"], "description": "Target language (auto-detected if omitted)" },
                    "workspace_root": { "type": "string", "description": "Path to workspace root for language/framework detection" }
                },
                "required": ["database", "collection"]
            }),
        },
        McpToolDefinition {
            name: "generate_model".to_string(),
            description: "Generate a typed data model (Pydantic, TypeScript interface, Go struct, Java record) from a collection's inferred schema.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Database name" },
                    "collection": { "type": "string", "description": "Collection name" },
                    "language": { "type": "string", "enum": ["python", "typescript", "go", "java"], "description": "Target language (auto-detected if omitted)" },
                    "workspace_root": { "type": "string", "description": "Path to workspace root for language detection" },
                    "sample_size": { "type": "integer", "description": "Documents to sample for schema inference (default 100)" }
                },
                "required": ["database", "collection"]
            }),
        },
        McpToolDefinition {
            name: "generate_index".to_string(),
            description: "Analyze a query filter and generate index creation code with an explanation of why the index helps.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "filter": { "type": "object", "description": "MQL filter to analyze for index suggestion" },
                    "database": { "type": "string", "description": "Database name" },
                    "collection": { "type": "string", "description": "Collection name" },
                    "language": { "type": "string", "enum": ["python", "typescript", "go", "java"], "description": "Target language (auto-detected if omitted)" },
                    "workspace_root": { "type": "string", "description": "Path to workspace root for language detection" }
                },
                "required": ["filter", "database", "collection"]
            }),
        },
        McpToolDefinition {
            name: "embed_and_store".to_string(),
            description: "Embed text fields in documents using Voyage AI and store them with vector embeddings in MongoDB.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "documents": { "type": "array", "items": { "type": "object" }, "description": "Array of documents to embed and store" },
                    "database": { "type": "string", "description": "Target database" },
                    "collection": { "type": "string", "description": "Target collection" },
                    "embed_field": { "type": "string", "description": "Field name containing text to embed" }
                },
                "required": ["documents", "database", "collection", "embed_field"]
            }),
        },
        McpToolDefinition {
            name: "semantic_search".to_string(),
            description: "Search for documents semantically similar to a query using vector embeddings. Requires a vector search index on the collection.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Natural language search query" },
                    "database": { "type": "string", "description": "Database name" },
                    "collection": { "type": "string", "description": "Collection name" },
                    "limit": { "type": "integer", "description": "Max results (default 10)", "default": 10 },
                    "index_name": { "type": "string", "description": "Vector search index name (default: vector_index)", "default": "vector_index" },
                    "path": { "type": "string", "description": "Field path of the embedding (default: _embedding)", "default": "_embedding" }
                },
                "required": ["query", "database", "collection"]
            }),
        },
        McpToolDefinition {
            name: "ingest_and_embed".to_string(),
            description: "Parse a file or URL (CSV/JSON/NDJSON/Parquet), embed a specified text field using Voyage AI, and store all documents with vector embeddings. For large files, use 'ingest' followed by 'embed_and_store' instead.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file_path": { "type": "string", "description": "Local file path or URL (http/https) to ingest" },
                    "database": { "type": "string", "description": "Target database" },
                    "collection": { "type": "string", "description": "Target collection" },
                    "embed_field": { "type": "string", "description": "Field containing text to embed" },
                    "format": { "type": "string", "enum": ["csv", "json", "ndjson", "parquet"], "description": "File format (auto-detected from extension if omitted)" }
                },
                "required": ["file_path", "database", "collection", "embed_field"]
            }),
        },
        McpToolDefinition {
            name: "list_skills".to_string(),
            description: "List available guided workflows (skills) that orchestrate multiple tools into repeatable processes.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "category": { "type": "string", "enum": ["database_workflows", "code_scaffolding", "data_analysis", "operations"], "description": "Filter by category (optional)" }
                },
                "required": []
            }),
        },
        McpToolDefinition {
            name: "get_skill".to_string(),
            description: "Get the full workflow guide for a specific skill, including all steps and tool calls.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Skill name (e.g. explore_dataset, bootstrap_project)" }
                },
                "required": ["name"]
            }),
        },
        McpToolDefinition {
            name: "suggest_indexes".to_string(),
            description: "Analyze recent query patterns from analytics and recommend missing indexes for better performance.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Filter by database (optional — all if omitted)" },
                    "collection": { "type": "string", "description": "Filter by collection (optional)" }
                },
                "required": []
            }),
        },
        McpToolDefinition {
            name: "slow_queries".to_string(),
            description: "Surface the slowest queries from analytics with their latency, frequency, and optimization suggestions.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Filter by database (optional)" },
                    "threshold_ms": { "type": "number", "description": "Minimum latency threshold in ms (default: p95 from analytics)" },
                    "limit": { "type": "integer", "description": "Max results (default 10)", "default": 10 }
                },
                "required": []
            }),
        },
        McpToolDefinition {
            name: "transaction_pipeline".to_string(),
            description: "Execute multiple dependent operations atomically in a transaction. Steps run sequentially and can reference results from prior steps using {{step_name.field}} syntax. Reference examples: {{find_top._id}} gets the _id from a find_one result, {{find_top[0]._id}} gets first result's _id from find, {{find_top[*]._id}} gets all _ids as an array, {{find_top}} gets the full result. For insert_many results use {{insert_step.inserted_ids}}.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "steps": {
                        "type": "array",
                        "description": "Ordered list of operations to execute in a transaction",
                        "items": {
                            "type": "object",
                            "properties": {
                                "name": { "type": "string", "description": "Unique step name (used in {{name.field}} references)" },
                                "database": { "type": "string", "description": "Database name" },
                                "collection": { "type": "string", "description": "Collection name" },
                                "operation": { "type": "string", "enum": ["find_one", "find", "insert", "insert_many", "update", "update_many", "delete", "delete_many", "find_and_modify", "aggregate"], "description": "Operation type" },
                                "params": { "type": "object", "description": "Operation parameters (filter, document, update, pipeline)" }
                            },
                            "required": ["name", "database", "collection", "operation", "params"]
                        }
                    },
                    "options": {
                        "type": "object",
                        "description": "Transaction options",
                        "properties": {
                            "read_concern": { "type": "string" },
                            "write_concern": { "type": "string" },
                            "max_time_ms": { "type": "integer" }
                        }
                    }
                },
                "required": ["steps"]
            }),
        },
        McpToolDefinition {
            name: "explain_last".to_string(),
            description: "Generate reusable MongoCore client code for a recent operation. Produces a parameterized function in the specified language.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "language": {
                        "type": "string",
                        "enum": ["python", "typescript", "go", "java"],
                        "description": "Target programming language"
                    },
                    "offset": {
                        "type": "integer",
                        "description": "How many operations back (0 = most recent)",
                        "default": 0
                    }
                },
                "required": ["language"]
            }),
        },
        McpToolDefinition {
            name: "explain_session".to_string(),
            description: "Generate a complete MongoCore client script reproducing all operations performed in this session. Produces parameterized functions with a main entry point.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "language": {
                        "type": "string",
                        "enum": ["python", "typescript", "go", "java"],
                        "description": "Target programming language"
                    }
                },
                "required": ["language"]
            }),
        },
    ]
}

/// Execute an MCP tool by name with the given JSON arguments.
pub async fn execute_tool(
    operations: &Operations,
    pool: &ConnectionPool,
    analytics: Option<&Arc<AnalyticsCollector>>,
    ingestion: Option<&Arc<IngestionEngine>>,
    watcher: Option<&Arc<DirectoryWatcher>>,
    safety: &SafetyConfig,
    translator: Option<&Arc<CompiledQueryTranslator>>,
    voyage: Option<&Arc<crate::voyage::client::VoyageClient>>,
    skills: &SkillRegistry,
    session: &Arc<Mutex<SessionRecorder>>,
    name: &str,
    arguments: &Value,
) -> McpToolCallResult {
    let start = std::time::Instant::now();
    let result = execute_tool_inner(operations, pool, analytics, ingestion, watcher, safety, translator, voyage, skills, session, name, arguments).await;

    // Record analytics for data operations
    if let Some(analytics) = analytics {
        if let Some(op_kind) = tool_name_to_operation_kind(name) {
            let database = arguments.get("database").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let collection = arguments.get("collection").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let success = !result.is_error;
            analytics.record(crate::analytics::AnalyticsEvent::new(
                op_kind,
                database,
                collection,
                start.elapsed(),
                success,
            ));
        }
    }

    // Enrich response with _context
    enrich_result(result, name, arguments)
}

/// Wrap a tool result with _context metadata.
/// For text responses that are valid JSON objects, inserts _context as a field.
/// For non-JSON or array responses, wraps in {"result": ..., "_context": ...}.
fn enrich_result(result: McpToolCallResult, tool_name: &str, args: &Value) -> McpToolCallResult {
    let context = context::build_context(tool_name, args);

    let enriched_content: Vec<McpContent> = result
        .content
        .into_iter()
        .map(|content| {
            if let Ok(mut parsed) = serde_json::from_str::<Value>(&content.text) {
                if let Some(obj) = parsed.as_object_mut() {
                    if !obj.contains_key("_context") {
                        obj.insert("_context".to_string(), context.clone());
                    }
                    McpContent {
                        type_: content.type_,
                        text: serde_json::to_string_pretty(&parsed).unwrap_or(content.text),
                    }
                } else {
                    // Array or scalar — wrap
                    let wrapped = json!({
                        "result": parsed,
                        "_context": context
                    });
                    McpContent {
                        type_: content.type_,
                        text: serde_json::to_string_pretty(&wrapped).unwrap_or(content.text),
                    }
                }
            } else {
                // Non-JSON text (error messages) — wrap
                let original_text = content.text.clone();
                let wrapped = json!({
                    "message": content.text,
                    "_context": context
                });
                McpContent {
                    type_: "text".to_string(),
                    text: serde_json::to_string_pretty(&wrapped).unwrap_or(original_text),
                }
            }
        })
        .collect();

    McpToolCallResult {
        content: enriched_content,
        is_error: result.is_error,
    }
}

fn tool_name_to_operation_kind(name: &str) -> Option<crate::analytics::OperationKind> {
    use crate::analytics::OperationKind;
    match name {
        "find" => Some(OperationKind::Find),
        "find_one" => Some(OperationKind::FindOne),
        "insert" => Some(OperationKind::Insert),
        "insert_many" => Some(OperationKind::InsertMany),
        "update" => Some(OperationKind::Update),
        "update_many" => Some(OperationKind::UpdateMany),
        "delete" => Some(OperationKind::Delete),
        "delete_many" => Some(OperationKind::DeleteMany),
        "aggregate" => Some(OperationKind::Aggregate),
        "run_command" => Some(OperationKind::RunCommand),
        "search" | "semantic_search" => Some(OperationKind::Search),
        "ask" | "explain_query" => Some(OperationKind::Search),
        "list_databases" => Some(OperationKind::ListDatabases),
        "list_collections" => Some(OperationKind::ListCollections),
        "create_collection" => Some(OperationKind::CreateCollection),
        "create_index" => Some(OperationKind::CreateIndex),
        "pipeline" => Some(OperationKind::Pipeline),
        _ => None,
    }
}

async fn execute_tool_inner(
    operations: &Operations,
    pool: &ConnectionPool,
    analytics: Option<&Arc<AnalyticsCollector>>,
    ingestion: Option<&Arc<IngestionEngine>>,
    watcher: Option<&Arc<DirectoryWatcher>>,
    safety: &SafetyConfig,
    translator: Option<&Arc<CompiledQueryTranslator>>,
    voyage: Option<&Arc<crate::voyage::client::VoyageClient>>,
    skills: &SkillRegistry,
    session: &Arc<Mutex<SessionRecorder>>,
    name: &str,
    arguments: &Value,
) -> McpToolCallResult {
    match name {
        "find" => execute_find(operations, arguments).await,
        "find_one" => execute_find_one(operations, arguments).await,
        "insert" => execute_insert(operations, arguments).await,
        "insert_many" => execute_insert_many(operations, arguments).await,
        "update" => execute_update(operations, arguments).await,
        "update_many" => execute_update_many(operations, arguments).await,
        "delete" => execute_delete(operations, arguments).await,
        "delete_many" => execute_delete_many(operations, arguments).await,
        "aggregate" => execute_aggregate(operations, arguments).await,
        "create_collection" => execute_create_collection(operations, arguments).await,
        "create_index" => execute_create_index(operations, arguments).await,
        "list_databases" => execute_list_databases(pool).await,
        "list_collections" => execute_list_collections(pool, arguments).await,
        "run_command" => execute_run_command(pool, arguments).await,
        "get_analytics" => execute_get_analytics(analytics).await,
        "ingest" => execute_ingest(ingestion, pool, arguments).await,
        "ingest_status" => execute_ingest_status(ingestion, arguments).await,
        "list_ingest_jobs" => execute_list_ingest_jobs(ingestion).await,
        "cancel_ingest" => execute_cancel_ingest(ingestion, arguments).await,
        "watch_directory" => execute_watch_directory(watcher, arguments).await,
        "stop_watch" => execute_stop_watch(watcher, arguments).await,
        "pipeline" => {
            execute_pipeline(operations, pool, analytics, ingestion, watcher, safety, translator, voyage, skills, session, arguments)
                .await
        }
        "collection_schema" => execute_collection_schema(operations, arguments).await,
        "generate_code" => execute_generate_code(operations, arguments).await,
        "generate_model" => execute_generate_model(operations, arguments).await,
        "generate_index" => execute_generate_index(arguments).await,
        "ask" => {
            let question = match arguments.get("question").and_then(|v| v.as_str()) {
                Some(q) => q,
                None => return error_result("Missing required field: question".to_string()),
            };
            let database = match arguments.get("database").and_then(|v| v.as_str()) {
                Some(d) => d,
                None => return error_result("Missing required field: database".to_string()),
            };
            let collection = match arguments.get("collection").and_then(|v| v.as_str()) {
                Some(c) => c,
                None => return error_result("Collection is required for 'ask' (auto-detection not yet implemented)".to_string()),
            };

            let translator = match translator {
                Some(t) => t,
                None => return error_result("Natural language queries require an LLM provider. Configure ANTHROPIC_API_KEY or use MongoCore within Claude (MCP sampling). Use 'find' or 'aggregate' tools directly.".to_string()),
            };

            // Fetch a sample document to provide schema context to the translator
            let sample_docs = match operations.find(database, collection, bson::doc! {}, Some(FindOptions { limit: Some(1), skip: None, sort: None, projection: None })).await {
                Ok(docs) => docs.iter().map(|d| serde_json::to_string(d).unwrap_or_default()).collect(),
                Err(_) => vec![],
            };
            let context = crate::compiled::providers::TranslationContext {
                sample_documents: sample_docs,
                available_indexes: vec![],
                schema_hint: None,
            };
            let translate_start = std::time::Instant::now();

            match translator.translate(question, database, collection, &context).await {
                Ok(compiled) => {
                    let translate_ms = translate_start.elapsed().as_millis();
                    let from_cache = translate_ms < 50; // cache/template hits are near-instant; LLM calls take seconds

                    // Execute the compiled query
                    let exec_start = std::time::Instant::now();
                    let exec_result = match &compiled.mql {
                        crate::compiled::CompiledMql::Find { filter, .. } => {
                            let find_opts = Some(FindOptions {
                                limit: Some(MAX_FIND_LIMIT),
                                skip: None,
                                sort: None,
                                projection: None,
                            });
                            operations.find(database, collection, filter.clone(), find_opts).await
                        }
                        crate::compiled::CompiledMql::Aggregate { pipeline } => {
                            operations.aggregate(database, collection, pipeline.clone()).await
                        }
                        _ => Ok(vec![]),
                    };

                    let execution_time_ms = exec_start.elapsed().as_millis();
                    match exec_result {
                        Ok(docs) => {
                            let compiled_query = match &compiled.mql {
                                crate::compiled::CompiledMql::Find { filter, .. } => {
                                    json!({"method": "find", "filter": filter})
                                }
                                crate::compiled::CompiledMql::Aggregate { pipeline } => {
                                    json!({"method": "aggregate", "pipeline": pipeline})
                                }
                                _ => json!({"method": compiled.mql.method()}),
                            };
                            let result = json!({
                                "documents": docs.iter().take(20).map(|d| {
                                    serde_json::to_value(d).unwrap_or(Value::Null)
                                }).collect::<Vec<_>>(),
                                "count": docs.len(),
                                "query": {
                                    "method": compiled.mql.method(),
                                    "intent": compiled.intent
                                },
                                "execution_time_ms": execution_time_ms,
                                "from_cache": from_cache,
                                "_context": {
                                    "operation": "ask",
                                    "database": database,
                                    "collection": collection,
                                    "question": question,
                                    "compiled_query": compiled_query
                                }
                            });
                            success_result(serde_json::to_string_pretty(&result).unwrap_or_default())
                        }
                        Err(e) => error_result(format!("Query execution failed: {}", e)),
                    }
                }
                Err(e) => error_result(format!("Translation failed: {}. Use 'find' or 'aggregate' tools directly.", e)),
            }
        }
        "explain_query" => {
            let question = match arguments.get("question").and_then(|v| v.as_str()) {
                Some(q) => q,
                None => return error_result("Missing required field: question".to_string()),
            };
            let database = match arguments.get("database").and_then(|v| v.as_str()) {
                Some(d) => d,
                None => return error_result("Missing required field: database".to_string()),
            };
            let collection = match arguments.get("collection").and_then(|v| v.as_str()) {
                Some(c) => c,
                None => return error_result("Collection is required for 'explain_query' (auto-detection not yet implemented)".to_string()),
            };

            let translator = match translator {
                Some(t) => t,
                None => return error_result("Query explanation requires an LLM provider. Configure ANTHROPIC_API_KEY or use MongoCore within Claude. Use 'find' or 'aggregate' tools directly.".to_string()),
            };

            let context = crate::compiled::providers::TranslationContext::default();
            let translate_start = std::time::Instant::now();

            match translator.translate(question, database, collection, &context).await {
                Ok(compiled) => {
                    let from_cache = translate_start.elapsed().as_millis() < 50;

                    let mql_json = match &compiled.mql {
                        crate::compiled::CompiledMql::Find { filter, options } => json!({
                            "method": "find",
                            "filter": serde_json::to_value(filter).unwrap_or(json!({})),
                            "options": options.as_ref().map(|o| serde_json::to_value(o).unwrap_or(json!({})))
                        }),
                        crate::compiled::CompiledMql::Aggregate { pipeline } => json!({
                            "method": "aggregate",
                            "pipeline": pipeline.iter().map(|s| serde_json::to_value(s).unwrap_or(json!({}))).collect::<Vec<_>>()
                        }),
                        other => json!({ "method": other.method() }),
                    };

                    let result = json!({
                        "query": mql_json,
                        "intent": compiled.intent,
                        "from_cache": from_cache,
                        "note": "Query was NOT executed. Use 'ask' to execute it."
                    });
                    success_result(serde_json::to_string_pretty(&result).unwrap_or_default())
                }
                Err(e) => error_result(format!("Translation failed: {}. Use 'find' or 'aggregate' tools directly.", e)),
            }
        }
        "embed_and_store" => {
            let voyage = match voyage {
                Some(v) => v,
                None => return error_result("Embedding requires VOYAGE_API_KEY configuration".to_string()),
            };

            let documents = match arguments.get("documents").and_then(|v| v.as_array()) {
                Some(docs) => docs,
                None => return error_result("Missing required field: documents (must be an array)".to_string()),
            };
            let database = match arguments.get("database").and_then(|v| v.as_str()) {
                Some(d) => d,
                None => return error_result("Missing required field: database".to_string()),
            };
            let collection_name = match arguments.get("collection").and_then(|v| v.as_str()) {
                Some(c) => c,
                None => return error_result("Missing required field: collection".to_string()),
            };
            let embed_field = match arguments.get("embed_field").and_then(|v| v.as_str()) {
                Some(f) => f,
                None => return error_result("Missing required field: embed_field".to_string()),
            };

            // Extract text from embed_field for each document
            let texts: Vec<String> = documents.iter()
                .filter_map(|doc| doc.get(embed_field).and_then(|v| v.as_str()).map(|s| s.to_string()))
                .collect();

            if texts.is_empty() {
                return error_result(format!("No documents contain text in field '{}'", embed_field));
            }

            // Batch embed via Voyage AI
            let embeddings = match voyage.embed(texts).await {
                Ok(result) => result.embeddings,
                Err(e) => return error_result(format!("Embedding failed: {}", e)),
            };

            // Build documents with _embedding field appended
            let db = pool.client().database(database);
            let coll = db.collection::<bson::Document>(collection_name);
            let mut bson_docs: Vec<bson::Document> = Vec::new();

            for (i, doc_val) in documents.iter().enumerate() {
                let mut bson_doc = match bson::to_document(doc_val) {
                    Ok(d) => d,
                    Err(e) => return error_result(format!("Invalid document at index {}: {}", i, e)),
                };
                if let Some(embedding) = embeddings.get(i) {
                    let bson_vec: Vec<bson::Bson> = embedding.iter()
                        .map(|&f| bson::Bson::Double(f))
                        .collect();
                    bson_doc.insert("_embedding", bson::Bson::Array(bson_vec));
                }
                bson_docs.push(bson_doc);
            }

            // Insert all documents
            match coll.insert_many(bson_docs).await {
                Ok(result) => {
                    let dimensions = embeddings.first().map(|e| e.len()).unwrap_or(0);
                    let resp = json!({
                        "documents_stored": result.inserted_ids.len(),
                        "embeddings_generated": embeddings.len(),
                        "embedding_dimensions": dimensions,
                        "database": database,
                        "collection": collection_name
                    });
                    success_result(serde_json::to_string_pretty(&resp).unwrap_or_default())
                }
                Err(e) => error_result(format!("Insert failed: {}", e)),
            }
        }
        "semantic_search" => {
            let voyage = match voyage {
                Some(v) => v,
                None => return error_result("Semantic search requires VOYAGE_API_KEY configuration".to_string()),
            };

            let query = match arguments.get("query").and_then(|v| v.as_str()) {
                Some(q) => q,
                None => return error_result("Missing required field: query".to_string()),
            };
            let database = match arguments.get("database").and_then(|v| v.as_str()) {
                Some(d) => d,
                None => return error_result("Missing required field: database".to_string()),
            };
            let collection_name = match arguments.get("collection").and_then(|v| v.as_str()) {
                Some(c) => c,
                None => return error_result("Missing required field: collection".to_string()),
            };
            let limit = arguments.get("limit").and_then(|v| v.as_i64()).unwrap_or(10);
            let index_name = arguments.get("index_name").and_then(|v| v.as_str()).unwrap_or("vector_index");
            let path = arguments.get("path").and_then(|v| v.as_str()).unwrap_or("_embedding");

            // Embed the query text
            let embedding_result = match voyage.embed(vec![query.to_string()]).await {
                Ok(r) => r,
                Err(e) => return error_result(format!("Failed to embed query: {}", e)),
            };

            let query_vector = match embedding_result.embeddings.first() {
                Some(v) => v.clone(),
                None => return error_result("Embedding returned no vectors".to_string()),
            };

            // Build $vectorSearch aggregation pipeline
            let vector_bson: Vec<bson::Bson> = query_vector.iter()
                .map(|&f| bson::Bson::Double(f))
                .collect();

            let pipeline = vec![
                bson::doc! {
                    "$vectorSearch": {
                        "index": index_name,
                        "path": path,
                        "queryVector": vector_bson,
                        "numCandidates": (limit * 10) as i64,
                        "limit": limit
                    }
                },
                bson::doc! {
                    "$addFields": {
                        "score": { "$meta": "vectorSearchScore" }
                    }
                },
            ];

            let db = pool.client().database(database);
            let coll = db.collection::<bson::Document>(collection_name);

            match coll.aggregate(pipeline).await {
                Ok(mut cursor) => {
                    let mut results = Vec::new();
                    while cursor.advance().await.unwrap_or(false) {
                        if let Ok(doc) = cursor.deserialize_current() {
                            results.push(serde_json::to_value(&doc).unwrap_or(json!(null)));
                        }
                    }
                    let resp = json!({
                        "results": results,
                        "count": results.len(),
                        "query": query,
                        "database": database,
                        "collection": collection_name
                    });
                    success_result(serde_json::to_string_pretty(&resp).unwrap_or_default())
                }
                Err(e) => error_result(format!(
                    "Vector search failed: {}. Ensure a vector search index named '{}' exists on the collection.",
                    e, index_name
                )),
            }
        }
        "ingest_and_embed" => {
            match voyage {
                Some(_) => {},
                None => return error_result("Embedding requires VOYAGE_API_KEY configuration".to_string()),
            };

            let file_path = match arguments.get("file_path").and_then(|v| v.as_str()) {
                Some(p) => p,
                None => return error_result("Missing required field: file_path".to_string()),
            };
            let database = match arguments.get("database").and_then(|v| v.as_str()) {
                Some(d) => d,
                None => return error_result("Missing required field: database".to_string()),
            };
            let collection_name = match arguments.get("collection").and_then(|v| v.as_str()) {
                Some(c) => c,
                None => return error_result("Missing required field: collection".to_string()),
            };
            let embed_field = match arguments.get("embed_field").and_then(|v| v.as_str()) {
                Some(f) => f,
                None => return error_result("Missing required field: embed_field".to_string()),
            };

            let path = std::path::Path::new(file_path);
            if !path.exists() {
                return error_result(format!("File not found: {}", file_path));
            }

            // For this initial implementation, guide users to the two-step approach
            let resp = json!({
                "status": "not_yet_implemented",
                "message": "The unified ingest+embed pipeline is planned. For now, use these steps:",
                "workaround": [
                    "1. Use the 'ingest' tool to load the file into a collection",
                    format!("2. Use 'find' to read documents from '{}.{}'", database, collection_name),
                    format!("3. Use 'embed_and_store' with the documents and embed_field='{}'", embed_field)
                ],
                "file_path": file_path,
                "file_exists": true,
                "database": database,
                "collection": collection_name,
                "embed_field": embed_field
            });
            success_result(serde_json::to_string_pretty(&resp).unwrap_or_default())
        }
        "list_skills" => {
            let category_filter = arguments.get("category")
                .and_then(|v| v.as_str())
                .and_then(|c| match c {
                    "database_workflows" => Some(crate::mcp::skills::SkillCategory::DatabaseWorkflows),
                    "code_scaffolding" => Some(crate::mcp::skills::SkillCategory::CodeScaffolding),
                    "data_analysis" => Some(crate::mcp::skills::SkillCategory::DataAnalysis),
                    "operations" => Some(crate::mcp::skills::SkillCategory::Operations),
                    _ => None,
                });

            let skill_list: Vec<serde_json::Value> = skills.list(category_filter)
                .iter()
                .map(|s| json!({
                    "name": s.name,
                    "description": s.description,
                    "category": s.category.display_name(),
                    "arguments": s.arguments.iter().map(|a| json!({
                        "name": a.name,
                        "required": a.required
                    })).collect::<Vec<_>>(),
                    "step_count": s.steps.len()
                }))
                .collect();

            success_result(serde_json::to_string_pretty(&json!({ "skills": skill_list })).unwrap_or_default())
        }

        "get_skill" => {
            let name = match arguments.get("name").and_then(|v| v.as_str()) {
                Some(n) => n,
                None => return error_result("Missing required field: name".to_string()),
            };

            match skills.get(name) {
                Some(skill) => {
                    let steps: Vec<serde_json::Value> = skill.steps.iter().enumerate()
                        .map(|(i, step)| {
                            let mut s = json!({
                                "step": i + 1,
                                "description": step.description
                            });
                            if let Some(ref tool) = step.tool {
                                s["tool"] = json!(tool);
                            }
                            if step.dynamic { s["dynamic"] = json!(true); }
                            if step.analysis { s["type"] = json!("analysis"); }
                            if step.synthesis { s["type"] = json!("synthesis"); }
                            s
                        })
                        .collect();

                    let result = json!({
                        "name": skill.name,
                        "description": skill.description,
                        "category": skill.category.display_name(),
                        "arguments": skill.arguments.iter().map(|a| json!({
                            "name": a.name,
                            "description": a.description,
                            "required": a.required
                        })).collect::<Vec<serde_json::Value>>(),
                        "steps": steps
                    });
                    success_result(serde_json::to_string_pretty(&result).unwrap_or_default())
                }
                None => error_result(format!("Skill not found: '{}'. Use list_skills to see available skills.", name)),
            }
        }
        "suggest_indexes" => {
            let analytics = match analytics {
                Some(a) => a,
                None => return error_result("Analytics not enabled. Enable analytics in config to use suggest_indexes.".to_string()),
            };

            let database_filter = arguments.get("database").and_then(|v| v.as_str());
            let collection_filter = arguments.get("collection").and_then(|v| v.as_str());

            let events = analytics.snapshot();
            let summary = crate::analytics::aggregator::aggregate(&events);

            // Find collections with high operation count (potential index candidates)
            let suggestions: Vec<serde_json::Value> = summary.top_collections.iter()
                .filter(|(name, _count)| {
                    if let Some(db) = database_filter {
                        if !name.starts_with(&format!("{}.", db)) { return false; }
                    }
                    if let Some(coll) = collection_filter {
                        if !name.ends_with(&format!(".{}", coll)) { return false; }
                    }
                    true
                })
                .filter(|(_name, count)| *count >= 5)
                .map(|(name, count)| {
                    json!({
                        "collection": name,
                        "operation_count": count,
                        "suggestion": format!("Collection '{}' has {} operations. Consider adding indexes on frequently filtered fields.", name, count),
                        "recommendation": "Use 'collection_schema' to inspect fields, then 'generate_index' to create indexes for your query patterns."
                    })
                })
                .collect();

            if suggestions.is_empty() {
                success_result(serde_json::to_string_pretty(&json!({
                    "message": "No index suggestions. Either queries are well-indexed or there isn't enough analytics data yet.",
                    "total_operations_analyzed": summary.total_operations,
                    "tip": "Run more queries and try again, or use 'generate_index' with a specific filter to get index recommendations."
                })).unwrap_or_default())
            } else {
                success_result(serde_json::to_string_pretty(&json!({
                    "suggestions": suggestions,
                    "total_operations_analyzed": summary.total_operations,
                    "p95_latency_ms": summary.p95_latency_ms
                })).unwrap_or_default())
            }
        }
        "slow_queries" => {
            let analytics = match analytics {
                Some(a) => a,
                None => return error_result("Analytics not enabled. Enable analytics in config to use slow_queries.".to_string()),
            };

            let database_filter = arguments.get("database").and_then(|v| v.as_str());
            let threshold_ms = arguments.get("threshold_ms").and_then(|v| v.as_f64());
            let limit = arguments.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

            let events = analytics.snapshot();
            let summary = crate::analytics::aggregator::aggregate(&events);

            let threshold = threshold_ms.unwrap_or(summary.p95_latency_ms);

            // Find slow events above threshold
            let mut slow_events: Vec<&crate::analytics::types::AnalyticsEvent> = events.iter()
                .filter(|e| {
                    let latency_ms = e.latency.as_secs_f64() * 1000.0;
                    latency_ms >= threshold
                        && database_filter.map_or(true, |db| e.database == db)
                })
                .collect();

            // Sort by latency descending
            slow_events.sort_by(|a, b| b.latency.partial_cmp(&a.latency).unwrap_or(std::cmp::Ordering::Equal));
            slow_events.truncate(limit);

            let slow_queries: Vec<serde_json::Value> = slow_events.iter()
                .map(|e| {
                    let latency_ms = e.latency.as_secs_f64() * 1000.0;
                    json!({
                        "operation": format!("{:?}", e.operation),
                        "database": e.database,
                        "collection": e.collection,
                        "latency_ms": latency_ms,
                        "success": e.success,
                        "suggestion": if latency_ms > 1000.0 {
                            "Very slow (>1s). Check for missing indexes or full collection scans."
                        } else if latency_ms > 100.0 {
                            "Moderately slow. An index on filter fields would likely help."
                        } else {
                            "Above threshold. Monitor for regression."
                        }
                    })
                })
                .collect();

            success_result(serde_json::to_string_pretty(&json!({
                "threshold_ms": threshold,
                "slow_queries": slow_queries,
                "count": slow_queries.len(),
                "p50_latency_ms": summary.p50_latency_ms,
                "p95_latency_ms": summary.p95_latency_ms,
                "p99_latency_ms": summary.p99_latency_ms,
                "total_operations": summary.total_operations
            })).unwrap_or_default())
        }
        "transaction_pipeline" => {
            execute_transaction_pipeline_tool(pool, safety, arguments).await
        }
        "explain_last" => execute_explain_last(session, arguments),
        "explain_session" => execute_explain_session(session, arguments),
        _ => error_result(format!("Unknown tool: {}", name)),
    }
}

fn parse_language(args: &Value) -> Result<super::codegen::Language, McpToolCallResult> {
    use super::codegen::Language;
    match args.get("language").and_then(|v| v.as_str()) {
        Some("python") => Ok(Language::Python),
        Some("typescript") => Ok(Language::TypeScript),
        Some("go") => Ok(Language::Go),
        Some("java") => Ok(Language::Java),
        Some(other) => Err(error_result(format!(
            "Unsupported language: {}. Use python, typescript, go, or java.",
            other
        ))),
        None => Err(error_result(
            "Missing required field: language".to_string(),
        )),
    }
}

fn execute_explain_last(
    session: &Arc<Mutex<SessionRecorder>>,
    args: &Value,
) -> McpToolCallResult {
    let language = match parse_language(args) {
        Ok(l) => l,
        Err(e) => return e,
    };
    let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

    let session_guard = match session.lock() {
        Ok(s) => s,
        Err(_) => return error_result("Failed to access session state".to_string()),
    };

    if session_guard.is_empty() {
        return error_result("No operations recorded in this session yet.".to_string());
    }

    let record = match session_guard.get_last(offset) {
        Some(r) => r,
        None => {
            return error_result(format!(
                "Offset {} is out of bounds. Session has {} operations.",
                offset,
                session_guard.len()
            ))
        }
    };

    match super::codegen::session_gen::generate_single_operation_code(
        language,
        &record.tool_name,
        &record.params,
    ) {
        Ok(code) => {
            let result = json!({
                "code": code,
                "language": args.get("language").unwrap_or(&Value::Null),
                "operation": record.tool_name,
            });
            success_result(serde_json::to_string_pretty(&result).unwrap_or_default())
        }
        Err(e) => error_result(format!("Code generation failed: {}", e)),
    }
}

fn execute_explain_session(
    session: &Arc<Mutex<SessionRecorder>>,
    args: &Value,
) -> McpToolCallResult {
    let language = match parse_language(args) {
        Ok(l) => l,
        Err(e) => return e,
    };

    let session_guard = match session.lock() {
        Ok(s) => s,
        Err(_) => return error_result("Failed to access session state".to_string()),
    };

    if session_guard.is_empty() {
        return error_result("No operations recorded in this session yet.".to_string());
    }

    let operations = session_guard.get_all();

    match super::codegen::session_gen::generate_session_script(language, operations) {
        Ok(code) => {
            let op_names: Vec<&str> = operations.iter().map(|o| o.tool_name.as_str()).collect();
            let result = json!({
                "code": code,
                "language": args.get("language").unwrap_or(&Value::Null),
                "operation_count": operations.len(),
                "operations": op_names,
            });
            success_result(serde_json::to_string_pretty(&result).unwrap_or_default())
        }
        Err(e) => error_result(format!("Code generation failed: {}", e)),
    }
}

async fn execute_transaction_pipeline_tool(
    pool: &ConnectionPool,
    safety: &SafetyConfig,
    args: &Value,
) -> McpToolCallResult {
    use crate::operations::transaction_pipeline::{
        execute_transaction_pipeline, PipelineStepDef, TransactionPipelineOptions,
    };

    if let Err(reason) = safety.check_tool_allowed("transaction_pipeline") {
        return error_result(reason);
    }

    let steps_arr = match args.get("steps").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => return error_result("Missing required field: steps".to_string()),
    };

    if steps_arr.is_empty() {
        return error_result("Pipeline must contain at least one step".to_string());
    }

    let steps: Vec<PipelineStepDef> = steps_arr
        .iter()
        .map(|s| {
            let name = s
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let database = s
                .get("database")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let collection = s
                .get("collection")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let operation = s
                .get("operation")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let params = s.get("params").cloned().unwrap_or(json!({}));
            let find_limit = params.get("limit").and_then(|v| v.as_i64());

            PipelineStepDef {
                name,
                database,
                collection,
                operation_type: operation,
                operation_json: params,
                find_limit,
            }
        })
        .collect();

    let options = match args.get("options") {
        Some(opts) => TransactionPipelineOptions {
            read_concern: opts
                .get("read_concern")
                .and_then(|v| v.as_str())
                .map(String::from),
            write_concern: opts
                .get("write_concern")
                .and_then(|v| v.as_str())
                .map(String::from),
            max_time_ms: opts
                .get("max_time_ms")
                .and_then(|v| v.as_u64())
                .unwrap_or(crate::defaults::DEFAULT_TRANSACTION_PIPELINE_TIMEOUT_MS),
        },
        None => TransactionPipelineOptions::default(),
    };

    match execute_transaction_pipeline(pool, steps, options).await {
        Ok(result) => {
            let response = json!({
                "steps": result.steps.iter().map(|s| json!({
                    "name": s.name,
                    "success": s.success,
                    "result": s.result_json,
                })).collect::<Vec<_>>(),
                "summary": {
                    "total_steps": result.total_steps,
                    "steps_completed": result.steps_completed,
                    "elapsed_ms": result.elapsed_ms,
                }
            });
            McpToolCallResult {
                content: vec![McpContent {
                    type_: "text".to_string(),
                    text: response.to_string(),
                }],
                is_error: false,
            }
        }
        Err(failure) => error_result(
            json!({
                "failed_step": failure.failed_step,
                "step_index": failure.step_index,
                "reason": failure.reason,
                "steps_completed": failure.steps_completed,
                "rolled_back": failure.rolled_back,
            })
            .to_string(),
        ),
    }
}

// --- Helper functions ---

fn error_result(message: String) -> McpToolCallResult {
    McpToolCallResult {
        content: vec![McpContent {
            type_: "text".to_string(),
            text: message,
        }],
        is_error: true,
    }
}

fn success_result(text: String) -> McpToolCallResult {
    McpToolCallResult {
        content: vec![McpContent {
            type_: "text".to_string(),
            text,
        }],
        is_error: false,
    }
}

fn get_str<'a>(args: &'a Value, field: &str) -> Result<&'a str, McpToolCallResult> {
    args.get(field)
        .and_then(|v| v.as_str())
        .ok_or_else(|| error_result(format!("Missing required field: {}", field)))
}

fn json_to_document(value: &Value) -> Result<Document, McpToolCallResult> {
    let bson_val = bson::to_bson(value)
        .map_err(|e| error_result(format!("Failed to convert to BSON: {}", e)))?;
    bson_val.as_document().cloned().ok_or_else(|| {
        error_result("Expected a JSON object convertible to BSON document".to_string())
    })
}

fn json_to_documents(value: &Value) -> Result<Vec<Document>, McpToolCallResult> {
    let arr = value
        .as_array()
        .ok_or_else(|| error_result("Expected a JSON array".to_string()))?;
    arr.iter().map(json_to_document).collect()
}

// --- Tool executors ---

async fn execute_find(operations: &Operations, args: &Value) -> McpToolCallResult {
    let db = match get_str(args, "database") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let coll = match get_str(args, "collection") {
        Ok(v) => v,
        Err(e) => return e,
    };

    let filter = if let Some(f) = args.get("filter") {
        match json_to_document(f) {
            Ok(d) => d,
            Err(e) => return e,
        }
    } else {
        Document::new()
    };

    let mut limit = args
        .get("limit")
        .and_then(|v| v.as_i64())
        .unwrap_or(MAX_FIND_LIMIT);
    if limit > MAX_FIND_LIMIT || limit <= 0 {
        limit = MAX_FIND_LIMIT;
    }

    let skip = args.get("skip").and_then(|v| v.as_u64());

    let options = Some(FindOptions {
        limit: Some(limit),
        skip,
        sort: None,
        projection: None,
    });

    match operations.find(db, coll, filter, options).await {
        Ok(docs) => {
            let json_docs: Vec<Value> = docs
                .iter()
                .map(|d| serde_json::to_value(d).unwrap_or(Value::Null))
                .collect();
            let text =
                serde_json::to_string_pretty(&json_docs).unwrap_or_else(|_| "[]".to_string());
            success_result(text)
        }
        Err(e) => error_result(format!("find failed: {}", e)),
    }
}

async fn execute_find_one(operations: &Operations, args: &Value) -> McpToolCallResult {
    let db = match get_str(args, "database") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let coll = match get_str(args, "collection") {
        Ok(v) => v,
        Err(e) => return e,
    };

    let filter = if let Some(f) = args.get("filter") {
        match json_to_document(f) {
            Ok(d) => d,
            Err(e) => return e,
        }
    } else {
        Document::new()
    };

    match operations.find_one(db, coll, filter).await {
        Ok(Some(doc)) => {
            let text = serde_json::to_string_pretty(&doc).unwrap_or_else(|_| "null".to_string());
            success_result(text)
        }
        Ok(None) => success_result("null".to_string()),
        Err(e) => error_result(format!("find_one failed: {}", e)),
    }
}

async fn execute_insert(operations: &Operations, args: &Value) -> McpToolCallResult {
    let db = match get_str(args, "database") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let coll = match get_str(args, "collection") {
        Ok(v) => v,
        Err(e) => return e,
    };

    let document = match args.get("document") {
        Some(d) => match json_to_document(d) {
            Ok(doc) => doc,
            Err(e) => return e,
        },
        None => return error_result("Missing required field: document".to_string()),
    };

    match operations.insert(db, coll, document).await {
        Ok(result) => {
            let text = json!({
                "insertedId": result.inserted_id.to_string()
            })
            .to_string();
            success_result(text)
        }
        Err(e) => error_result(format!("insert failed: {}", e)),
    }
}

async fn execute_insert_many(operations: &Operations, args: &Value) -> McpToolCallResult {
    let db = match get_str(args, "database") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let coll = match get_str(args, "collection") {
        Ok(v) => v,
        Err(e) => return e,
    };

    let documents = match args.get("documents") {
        Some(d) => match json_to_documents(d) {
            Ok(docs) => docs,
            Err(e) => return e,
        },
        None => return error_result("Missing required field: documents".to_string()),
    };

    match operations.insert_many(db, coll, documents).await {
        Ok(result) => {
            let ids: Vec<String> = result
                .inserted_ids
                .values()
                .map(|id| id.to_string())
                .collect();
            let text = json!({
                "insertedIds": ids,
                "insertedCount": ids.len()
            })
            .to_string();
            success_result(text)
        }
        Err(e) => error_result(format!("insert_many failed: {}", e)),
    }
}

async fn execute_update(operations: &Operations, args: &Value) -> McpToolCallResult {
    let db = match get_str(args, "database") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let coll = match get_str(args, "collection") {
        Ok(v) => v,
        Err(e) => return e,
    };

    let filter = match args.get("filter") {
        Some(f) => match json_to_document(f) {
            Ok(d) => d,
            Err(e) => return e,
        },
        None => return error_result("Missing required field: filter".to_string()),
    };

    let update = match args.get("update") {
        Some(u) => match json_to_document(u) {
            Ok(d) => d,
            Err(e) => return e,
        },
        None => return error_result("Missing required field: update".to_string()),
    };

    match operations.update(db, coll, filter, update).await {
        Ok(result) => {
            let text = json!({
                "matchedCount": result.matched_count,
                "modifiedCount": result.modified_count
            })
            .to_string();
            success_result(text)
        }
        Err(e) => error_result(format!("update failed: {}", e)),
    }
}

async fn execute_update_many(operations: &Operations, args: &Value) -> McpToolCallResult {
    let db = match get_str(args, "database") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let coll = match get_str(args, "collection") {
        Ok(v) => v,
        Err(e) => return e,
    };

    let filter = match args.get("filter") {
        Some(f) => match json_to_document(f) {
            Ok(d) => d,
            Err(e) => return e,
        },
        None => return error_result("Missing required field: filter".to_string()),
    };

    let update = match args.get("update") {
        Some(u) => match json_to_document(u) {
            Ok(d) => d,
            Err(e) => return e,
        },
        None => return error_result("Missing required field: update".to_string()),
    };

    match operations.update_many(db, coll, filter, update).await {
        Ok(result) => {
            let text = json!({
                "matchedCount": result.matched_count,
                "modifiedCount": result.modified_count
            })
            .to_string();
            success_result(text)
        }
        Err(e) => error_result(format!("update_many failed: {}", e)),
    }
}

async fn execute_delete(operations: &Operations, args: &Value) -> McpToolCallResult {
    let db = match get_str(args, "database") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let coll = match get_str(args, "collection") {
        Ok(v) => v,
        Err(e) => return e,
    };

    let filter = match args.get("filter") {
        Some(f) => match json_to_document(f) {
            Ok(d) => d,
            Err(e) => return e,
        },
        None => return error_result("Missing required field: filter".to_string()),
    };

    match operations.delete(db, coll, filter).await {
        Ok(result) => {
            let text = json!({
                "deletedCount": result.deleted_count
            })
            .to_string();
            success_result(text)
        }
        Err(e) => error_result(format!("delete failed: {}", e)),
    }
}

async fn execute_delete_many(operations: &Operations, args: &Value) -> McpToolCallResult {
    let db = match get_str(args, "database") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let coll = match get_str(args, "collection") {
        Ok(v) => v,
        Err(e) => return e,
    };

    let filter = match args.get("filter") {
        Some(f) => match json_to_document(f) {
            Ok(d) => d,
            Err(e) => return e,
        },
        None => return error_result("Missing required field: filter".to_string()),
    };

    match operations.delete_many(db, coll, filter).await {
        Ok(result) => {
            let text = json!({
                "deletedCount": result.deleted_count
            })
            .to_string();
            success_result(text)
        }
        Err(e) => error_result(format!("delete_many failed: {}", e)),
    }
}

async fn execute_aggregate(operations: &Operations, args: &Value) -> McpToolCallResult {
    let db = match get_str(args, "database") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let coll = match get_str(args, "collection") {
        Ok(v) => v,
        Err(e) => return e,
    };

    let pipeline = match args.get("pipeline") {
        Some(p) => match json_to_documents(p) {
            Ok(docs) => docs,
            Err(e) => return e,
        },
        None => return error_result("Missing required field: pipeline".to_string()),
    };

    match operations.aggregate(db, coll, pipeline).await {
        Ok(docs) => {
            let json_docs: Vec<Value> = docs
                .iter()
                .map(|d| serde_json::to_value(d).unwrap_or(Value::Null))
                .collect();
            let text =
                serde_json::to_string_pretty(&json_docs).unwrap_or_else(|_| "[]".to_string());
            success_result(text)
        }
        Err(e) => error_result(format!("aggregate failed: {}", e)),
    }
}

async fn execute_create_collection(operations: &Operations, args: &Value) -> McpToolCallResult {
    let db = match get_str(args, "database") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let coll = match get_str(args, "collection") {
        Ok(v) => v,
        Err(e) => return e,
    };

    match operations.create_collection(db, coll).await {
        Ok(()) => success_result(json!({ "ok": 1, "collection": coll }).to_string()),
        Err(e) => error_result(format!("create_collection failed: {}", e)),
    }
}

async fn execute_create_index(operations: &Operations, args: &Value) -> McpToolCallResult {
    let db = match get_str(args, "database") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let coll = match get_str(args, "collection") {
        Ok(v) => v,
        Err(e) => return e,
    };

    let keys = match args.get("keys") {
        Some(k) => match json_to_document(k) {
            Ok(d) => d,
            Err(e) => return e,
        },
        None => return error_result("Missing required field: keys".to_string()),
    };

    let unique = args.get("unique").and_then(|v| v.as_bool());
    let options = if unique.is_some() {
        Some(IndexOptions {
            name: None,
            unique,
            sparse: None,
        })
    } else {
        None
    };

    match operations.create_index(db, coll, keys, options).await {
        Ok(index_name) => success_result(json!({ "ok": 1, "indexName": index_name }).to_string()),
        Err(e) => error_result(format!("create_index failed: {}", e)),
    }
}

async fn execute_list_databases(pool: &ConnectionPool) -> McpToolCallResult {
    match pool.client().list_database_names().await {
        Ok(names) => success_result(
            serde_json::to_string_pretty(&json!({ "databases": names }))
                .unwrap_or_else(|_| "[]".to_string()),
        ),
        Err(e) => error_result(format!("list_databases failed: {}", e)),
    }
}

async fn execute_list_collections(pool: &ConnectionPool, args: &Value) -> McpToolCallResult {
    let db = match get_str(args, "database") {
        Ok(v) => v,
        Err(e) => return e,
    };

    match pool.database(db).list_collection_names().await {
        Ok(names) => success_result(
            serde_json::to_string_pretty(&json!({ "collections": names }))
                .unwrap_or_else(|_| "[]".to_string()),
        ),
        Err(e) => error_result(format!("list_collections failed: {}", e)),
    }
}

async fn execute_run_command(pool: &ConnectionPool, args: &Value) -> McpToolCallResult {
    let db = match get_str(args, "database") {
        Ok(v) => v,
        Err(e) => return e,
    };

    let command = match args.get("command") {
        Some(c) => match json_to_document(c) {
            Ok(doc) => doc,
            Err(e) => return e,
        },
        None => return error_result("Missing required field: command".to_string()),
    };

    // Determine validation mode based on allow_all parameter
    let allow_all = args.get("allow_all").and_then(|v| v.as_bool()).unwrap_or(false);
    let validation_mode = if allow_all {
        ValidationMode::AllowAll
    } else {
        ValidationMode::BlockDangerous
    };

    let options = RawCommandOptions { validation_mode };

    match crate::operations::raw::run_command(pool, db, command, &options).await {
        Ok(result) => {
            // Convert BSON Document to JSON Value
            let json_result = serde_json::to_value(&result).unwrap_or(Value::Null);
            let text = serde_json::to_string_pretty(&json_result)
                .unwrap_or_else(|_| "{}".to_string());
            success_result(text)
        }
        Err(e) => error_result(format!("run_command failed: {}", e)),
    }
}

async fn execute_get_analytics(analytics: Option<&Arc<AnalyticsCollector>>) -> McpToolCallResult {
    let analytics = match analytics {
        Some(a) => a,
        None => return error_result("Analytics not enabled".to_string()),
    };

    let events = analytics.snapshot();
    let summary = crate::analytics::aggregator::aggregate(&events);

    let top_operations: Vec<Value> = summary.top_operations.iter().map(|(op, count)| {
        json!({
            "operation": format!("{:?}", op),
            "count": count
        })
    }).collect();

    let top_collections: Vec<Value> = summary.top_collections.iter().map(|(coll, count)| {
        json!({
            "collection": coll,
            "count": count
        })
    }).collect();

    let result = json!({
        "total_operations": summary.total_operations,
        "total_errors": summary.total_errors,
        "error_rate": summary.error_rate,
        "p50_latency_ms": summary.p50_latency_ms,
        "p95_latency_ms": summary.p95_latency_ms,
        "p99_latency_ms": summary.p99_latency_ms,
        "top_operations": top_operations,
        "top_collections": top_collections
    });

    let text = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string());
    success_result(text)
}

// --- Ingestion tool executors ---

fn parse_file_format(s: &str) -> FileFormat {
    match s.to_lowercase().as_str() {
        "csv" => FileFormat::Csv,
        "json" => FileFormat::Json,
        "ndjson" => FileFormat::NdJson,
        "parquet" => FileFormat::Parquet,
        _ => FileFormat::Auto,
    }
}

fn parse_conflict_strategy(s: &str) -> ConflictStrategy {
    match s.to_lowercase().as_str() {
        "overwrite" => ConflictStrategy::Overwrite,
        "merge" => ConflictStrategy::Merge,
        _ => ConflictStrategy::Skip,
    }
}

async fn execute_ingest(
    engine: Option<&Arc<IngestionEngine>>,
    pool: &ConnectionPool,
    args: &Value,
) -> McpToolCallResult {
    let engine = match engine {
        Some(e) => e,
        None => return error_result("Ingestion engine not enabled".to_string()),
    };

    let file_path = match get_str(args, "file_path") {
        Ok(v) => v.to_string(),
        Err(e) => return e,
    };
    let database = match get_str(args, "database") {
        Ok(v) => v.to_string(),
        Err(e) => return e,
    };
    let collection = match get_str(args, "collection") {
        Ok(v) => v.to_string(),
        Err(e) => return e,
    };

    let format = args
        .get("format")
        .and_then(|v| v.as_str())
        .map(parse_file_format)
        .unwrap_or(FileFormat::Auto);

    let dedup_key: Vec<String> = args
        .get("dedup_key")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();

    let conflict_strategy = args
        .get("conflict_strategy")
        .and_then(|v| v.as_str())
        .map(parse_conflict_strategy)
        .unwrap_or_default();

    let batch_size = args
        .get("batch_size")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
        .unwrap_or(1000);

    let expressions: Vec<String> = args
        .get("expressions")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();

    let schema_overrides: HashMap<String, String> = args
        .get("schema_overrides")
        .and_then(|v| v.as_object())
        .map(|obj| {
            obj.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default();

    let sample_size = args
        .get("sample_size")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
        .unwrap_or(1000);

    let options = IngestOptions {
        file_path,
        database,
        collection,
        format,
        dedup_key,
        conflict_strategy,
        batch_size,
        expressions,
        schema_overrides,
        sample_size,
        ..Default::default()
    };

    match engine.ingest(pool.client(), options).await {
        Ok(job) => {
            let result = json!({
                "job_id": job.job_id,
                "status": format!("{:?}", job.status),
                "total_rows": job.total_rows,
                "file_path": job.file_path,
                "database": job.database,
                "collection": job.collection
            });
            success_result(serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string()))
        }
        Err(e) => error_result(format!("ingest failed: {}", e)),
    }
}

async fn execute_ingest_status(
    engine: Option<&Arc<IngestionEngine>>,
    args: &Value,
) -> McpToolCallResult {
    let engine = match engine {
        Some(e) => e,
        None => return error_result("Ingestion engine not enabled".to_string()),
    };

    let job_id = match get_str(args, "job_id") {
        Ok(v) => v,
        Err(e) => return e,
    };

    match engine.get_status(job_id).await {
        Ok(Some(job)) => {
            let result = json!({
                "job_id": job.job_id,
                "status": format!("{:?}", job.status),
                "total_rows": job.total_rows,
                "rows_processed": job.rows_processed,
                "rows_inserted": job.rows_inserted,
                "rows_skipped": job.rows_skipped,
                "rows_failed": job.rows_failed,
                "started_at": job.started_at.to_rfc3339(),
                "completed_at": job.completed_at.map(|t| t.to_rfc3339()),
                "error": job.error
            });
            success_result(serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string()))
        }
        Ok(None) => error_result(format!("Job not found: {}", job_id)),
        Err(e) => error_result(format!("ingest_status failed: {}", e)),
    }
}

async fn execute_list_ingest_jobs(
    engine: Option<&Arc<IngestionEngine>>,
) -> McpToolCallResult {
    let engine = match engine {
        Some(e) => e,
        None => return error_result("Ingestion engine not enabled".to_string()),
    };

    match engine.list_jobs().await {
        Ok(jobs) => {
            let jobs_json: Vec<Value> = jobs
                .iter()
                .map(|job| {
                    json!({
                        "job_id": job.job_id,
                        "file_path": job.file_path,
                        "database": job.database,
                        "collection": job.collection,
                        "status": format!("{:?}", job.status),
                        "total_rows": job.total_rows,
                        "rows_processed": job.rows_processed,
                        "started_at": job.started_at.to_rfc3339()
                    })
                })
                .collect();
            success_result(
                serde_json::to_string_pretty(&json!({ "jobs": jobs_json }))
                    .unwrap_or_else(|_| "[]".to_string()),
            )
        }
        Err(e) => error_result(format!("list_ingest_jobs failed: {}", e)),
    }
}

async fn execute_cancel_ingest(
    engine: Option<&Arc<IngestionEngine>>,
    args: &Value,
) -> McpToolCallResult {
    let engine = match engine {
        Some(e) => e,
        None => return error_result("Ingestion engine not enabled".to_string()),
    };

    let job_id = match get_str(args, "job_id") {
        Ok(v) => v,
        Err(e) => return e,
    };

    match engine.cancel(job_id).await {
        Ok(()) => success_result(json!({ "ok": 1, "job_id": job_id, "status": "Cancelled" }).to_string()),
        Err(e) => error_result(format!("cancel_ingest failed: {}", e)),
    }
}

async fn execute_watch_directory(
    watcher: Option<&Arc<DirectoryWatcher>>,
    args: &Value,
) -> McpToolCallResult {
    let watcher = match watcher {
        Some(w) => w,
        None => return error_result("Directory watcher not enabled".to_string()),
    };

    let path = match get_str(args, "path") {
        Ok(v) => v.to_string(),
        Err(e) => return e,
    };
    let database = match get_str(args, "database") {
        Ok(v) => v.to_string(),
        Err(e) => return e,
    };
    let collection = match get_str(args, "collection") {
        Ok(v) => v.to_string(),
        Err(e) => return e,
    };

    let file_pattern = args
        .get("file_pattern")
        .and_then(|v| v.as_str())
        .unwrap_or("*")
        .to_string();

    let conflict_strategy = args
        .get("conflict_strategy")
        .and_then(|v| v.as_str())
        .map(parse_conflict_strategy)
        .unwrap_or_default();

    let dedup_key: Vec<String> = args
        .get("dedup_key")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();

    let config = WatchConfig {
        path: std::path::PathBuf::from(path),
        file_pattern,
        database,
        collection,
        conflict_strategy,
        dedup_key,
        debounce_ms: 2000,
    };

    match watcher.start_watch(config).await {
        Ok(watch_id) => success_result(json!({ "ok": 1, "watch_id": watch_id }).to_string()),
        Err(e) => error_result(format!("watch_directory failed: {}", e)),
    }
}

async fn execute_stop_watch(
    watcher: Option<&Arc<DirectoryWatcher>>,
    args: &Value,
) -> McpToolCallResult {
    let watcher = match watcher {
        Some(w) => w,
        None => return error_result("Directory watcher not enabled".to_string()),
    };

    let watch_id = match get_str(args, "watch_id") {
        Ok(v) => v,
        Err(e) => return e,
    };

    match watcher.stop_watch(watch_id).await {
        Ok(()) => success_result(json!({ "ok": 1, "watch_id": watch_id, "status": "stopped" }).to_string()),
        Err(e) => error_result(format!("stop_watch failed: {}", e)),
    }
}

// --- Pipeline executor ---

async fn execute_pipeline(
    operations: &Operations,
    pool: &ConnectionPool,
    analytics: Option<&Arc<AnalyticsCollector>>,
    ingestion: Option<&Arc<IngestionEngine>>,
    watcher: Option<&Arc<DirectoryWatcher>>,
    safety: &SafetyConfig,
    translator: Option<&Arc<CompiledQueryTranslator>>,
    voyage: Option<&Arc<crate::voyage::client::VoyageClient>>,
    skills: &SkillRegistry,
    session: &Arc<Mutex<SessionRecorder>>,
    args: &Value,
) -> McpToolCallResult {
    let ops = match args.get("operations").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => return error_result("Missing required field: operations".to_string()),
    };

    if ops.is_empty() {
        return error_result("Pipeline must contain at least one operation".to_string());
    }

    if ops.len() > DEFAULT_PIPELINE_MAX_OPS {
        return error_result(format!(
            "Pipeline exceeds maximum of {} operations (got {})",
            DEFAULT_PIPELINE_MAX_OPS,
            ops.len()
        ));
    }

    // All-or-nothing safety validation
    if let Err(reason) = safety.check_pipeline_allowed(ops) {
        return error_result(reason);
    }

    // Execute all operations concurrently
    let futures: Vec<_> = ops
        .iter()
        .map(|op| {
            let op_type = op.get("op").and_then(|v| v.as_str()).unwrap_or("unknown");
            async move {
                let result = execute_tool(
                    operations, pool, analytics, ingestion, watcher, safety, translator, voyage, skills, session, op_type, op,
                )
                .await;
                (op_type.to_string(), result)
            }
        })
        .collect();

    let results = futures::future::join_all(futures).await;

    let mut succeeded = 0u64;
    let mut failed = 0u64;
    let mut result_entries: Vec<Value> = Vec::with_capacity(results.len());

    for (i, (op_type, result)) in results.into_iter().enumerate() {
        if result.is_error {
            failed += 1;
        } else {
            succeeded += 1;
        }

        let content_text = result
            .content
            .first()
            .map(|c| c.text.clone())
            .unwrap_or_default();

        result_entries.push(json!({
            "index": i,
            "op": op_type,
            "success": !result.is_error,
            "content": content_text
        }));
    }

    let output = json!({
        "results": result_entries,
        "succeeded": succeeded,
        "failed": failed,
        "total": succeeded + failed
    });

    success_result(serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string()))
}

// --- Schema inference helpers ---

use std::collections::HashSet;

struct FieldInfo {
    types: HashSet<String>,
    count: usize,
    example: Option<Value>,
}

impl FieldInfo {
    fn new() -> Self {
        Self {
            types: HashSet::new(),
            count: 0,
            example: None,
        }
    }
}

fn collect_fields(doc: &Document, prefix: &str, fields: &mut HashMap<String, FieldInfo>) {
    for (key, value) in doc.iter() {
        let field_path = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{}.{}", prefix, key)
        };

        let info = fields.entry(field_path.clone()).or_insert_with(FieldInfo::new);
        info.count += 1;
        info.types.insert(bson_type_name(value));

        if info.example.is_none() {
            info.example = Some(bson_to_example_json(value));
        }

        // Recurse into nested documents
        if let bson::Bson::Document(nested) = value {
            collect_fields(nested, &field_path, fields);
        }
    }
}

fn bson_type_name(value: &bson::Bson) -> String {
    match value {
        bson::Bson::Double(_) => "double",
        bson::Bson::String(_) => "string",
        bson::Bson::Array(_) => "array",
        bson::Bson::Document(_) => "document",
        bson::Bson::Boolean(_) => "bool",
        bson::Bson::Null => "null",
        bson::Bson::RegularExpression(_) => "regex",
        bson::Bson::JavaScriptCode(_) => "javascript",
        bson::Bson::JavaScriptCodeWithScope(_) => "javascriptWithScope",
        bson::Bson::Int32(_) => "int32",
        bson::Bson::Int64(_) => "int64",
        bson::Bson::Timestamp(_) => "timestamp",
        bson::Bson::Binary(_) => "binData",
        bson::Bson::ObjectId(_) => "objectId",
        bson::Bson::DateTime(_) => "date",
        bson::Bson::Symbol(_) => "symbol",
        bson::Bson::Decimal128(_) => "decimal",
        bson::Bson::Undefined => "undefined",
        bson::Bson::MaxKey => "maxKey",
        bson::Bson::MinKey => "minKey",
        bson::Bson::DbPointer(_) => "dbPointer",
    }
    .to_string()
}

fn bson_to_example_json(value: &bson::Bson) -> Value {
    match value {
        bson::Bson::Double(v) => json!(v),
        bson::Bson::String(v) => {
            if v.len() > 50 {
                json!(format!("{}...", &v[..50]))
            } else {
                json!(v)
            }
        }
        bson::Bson::Array(arr) => {
            if arr.is_empty() {
                json!([])
            } else {
                json!([bson_to_example_json(&arr[0])])
            }
        }
        bson::Bson::Document(_) => json!("{ ... }"),
        bson::Bson::Boolean(v) => json!(v),
        bson::Bson::Null => Value::Null,
        bson::Bson::Int32(v) => json!(v),
        bson::Bson::Int64(v) => json!(v),
        bson::Bson::ObjectId(oid) => json!(oid.to_string()),
        bson::Bson::DateTime(dt) => json!(dt.to_string()),
        bson::Bson::Decimal128(d) => json!(d.to_string()),
        _ => json!("..."),
    }
}

async fn execute_collection_schema(operations: &Operations, args: &Value) -> McpToolCallResult {
    let db = match get_str(args, "database") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let coll = match get_str(args, "collection") {
        Ok(v) => v,
        Err(e) => return e,
    };

    let sample_size = args
        .get("sample_size")
        .and_then(|v| v.as_i64())
        .unwrap_or(100);

    // Run $sample aggregation to get random documents
    let pipeline = vec![bson::doc! { "$sample": { "size": sample_size } }];

    match operations.aggregate(db, coll, pipeline).await {
        Ok(docs) => {
            let mut fields: HashMap<String, FieldInfo> = HashMap::new();

            for doc in &docs {
                collect_fields(doc, "", &mut fields);
            }

            // Build the result
            let mut fields_array: Vec<Value> = fields
                .into_iter()
                .map(|(name, info)| {
                    let types: Vec<String> = info.types.into_iter().collect();
                    json!({
                        "name": name,
                        "types": types,
                        "count": info.count,
                        "example": info.example
                    })
                })
                .collect();

            // Sort by field name for consistent output
            fields_array.sort_by(|a, b| {
                a.get("name")
                    .and_then(|v| v.as_str())
                    .cmp(&b.get("name").and_then(|v| v.as_str()))
            });

            let result = json!({
                "database": db,
                "collection": coll,
                "documents_sampled": docs.len(),
                "fields": fields_array
            });

            let text = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string());
            success_result(text)
        }
        Err(e) => error_result(format!("collection_schema failed: {}", e)),
    }
}

// --- Codegen tool helpers ---

fn resolve_language(args: &Value) -> Language {
    if let Some(lang_str) = args.get("language").and_then(|v| v.as_str()) {
        match lang_str {
            "python" => return Language::Python,
            "typescript" => return Language::TypeScript,
            "go" => return Language::Go,
            "java" => return Language::Java,
            _ => {}
        }
    }
    if let Some(root) = args.get("workspace_root").and_then(|v| v.as_str()) {
        if let Some(stack) = detect::detect_stack(std::path::Path::new(root)) {
            return stack.language;
        }
    }
    Language::Python // default
}

async fn execute_generate_code(_operations: &Operations, args: &Value) -> McpToolCallResult {
    let db = match get_str(args, "database") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let coll = match get_str(args, "collection") {
        Ok(v) => v,
        Err(e) => return e,
    };

    let language = resolve_language(args);
    let operation = args
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("find");

    // Build MQL value from filter or pipeline
    let mql = if operation == "aggregate" {
        if let Some(pipeline) = args.get("pipeline") {
            json!({"pipeline": pipeline})
        } else {
            json!({"pipeline": []})
        }
    } else {
        if let Some(filter) = args.get("filter") {
            json!({"filter": filter})
        } else {
            json!({"filter": {}})
        }
    };

    match query_gen::generate_query_code(language, db, coll, operation, &mql, "localhost:50051") {
        Ok(code) => {
            let mut result = json!({
                "code": code,
                "language": language.display_name(),
            });

            // Detect framework if workspace_root provided
            if let Some(root) = args.get("workspace_root").and_then(|v| v.as_str()) {
                if let Some(stack) = detect::detect_stack(std::path::Path::new(root)) {
                    result["framework_detected"] = json!(stack.framework.display_name());
                    if let Some(rec) = stack.framework.skill_recommendation() {
                        result["recommendation"] = json!(rec);
                    }
                }
            }

            success_result(serde_json::to_string_pretty(&result).unwrap_or_default())
        }
        Err(e) => error_result(format!("generate_code failed: {}", e)),
    }
}

async fn execute_generate_model(operations: &Operations, args: &Value) -> McpToolCallResult {
    let db = match get_str(args, "database") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let coll = match get_str(args, "collection") {
        Ok(v) => v,
        Err(e) => return e,
    };

    let language = resolve_language(args);
    let sample_size = args
        .get("sample_size")
        .and_then(|v| v.as_i64())
        .unwrap_or(100);

    // Sample documents using $sample aggregation (same pattern as collection_schema)
    let pipeline = vec![bson::doc! { "$sample": { "size": sample_size } }];

    match operations.aggregate(db, coll, pipeline).await {
        Ok(docs) => {
            // Collect field names and types from sampled docs
            let mut field_map: HashMap<String, String> = HashMap::new();
            for doc in &docs {
                for (key, value) in doc.iter() {
                    field_map.entry(key.clone()).or_insert_with(|| {
                        bson_type_to_model_type(value)
                    });
                }
            }

            let mut fields: Vec<(String, String)> = field_map.into_iter().collect();
            fields.sort_by(|a, b| a.0.cmp(&b.0));

            let model = model_gen::generate_model(language, coll, &fields);
            let fields_count = fields.len();

            let result = json!({
                "model": model,
                "language": language.display_name(),
                "collection": coll,
                "fields_count": fields_count,
            });

            success_result(serde_json::to_string_pretty(&result).unwrap_or_default())
        }
        Err(e) => error_result(format!("generate_model failed: {}", e)),
    }
}

/// Map a BSON value to a model type string used by model_gen.
fn bson_type_to_model_type(value: &bson::Bson) -> String {
    match value {
        bson::Bson::Double(_) => "Double",
        bson::Bson::String(_) => "String",
        bson::Bson::Array(_) => "Array",
        bson::Bson::Document(_) => "Document",
        bson::Bson::Boolean(_) => "Boolean",
        bson::Bson::Int32(_) => "Int32",
        bson::Bson::Int64(_) => "Int64",
        bson::Bson::ObjectId(_) => "ObjectId",
        bson::Bson::DateTime(_) => "DateTime",
        _ => "String",
    }
    .to_string()
}

async fn execute_generate_index(args: &Value) -> McpToolCallResult {
    let filter_value = match args.get("filter") {
        Some(f) => f.clone(),
        None => return error_result("Missing required field: filter".to_string()),
    };
    let db = match get_str(args, "database") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let coll = match get_str(args, "collection") {
        Ok(v) => v,
        Err(e) => return e,
    };

    let language = resolve_language(args);
    let suggestion = index_gen::suggest_index(language, db, coll, &filter_value);

    let result = json!({
        "index_spec": suggestion.index_spec,
        "fields": suggestion.fields,
        "code": suggestion.code,
        "explanation": suggestion.explanation,
        "language": language.display_name(),
    });

    success_result(serde_json::to_string_pretty(&result).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definitions_count() {
        let tools = tool_definitions();
        assert_eq!(tools.len(), 38);
    }

    #[test]
    fn test_tool_definitions_have_required_fields() {
        let tools = tool_definitions();
        for tool in &tools {
            assert!(!tool.name.is_empty());
            assert!(!tool.description.is_empty());
            assert_eq!(tool.input_schema["type"], "object");
            assert!(tool.input_schema.get("properties").is_some());
            assert!(tool.input_schema.get("required").is_some());
        }
    }

    #[test]
    fn test_tool_definitions_names() {
        let tools = tool_definitions();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"find"));
        assert!(names.contains(&"find_one"));
        assert!(names.contains(&"insert"));
        assert!(names.contains(&"insert_many"));
        assert!(names.contains(&"update"));
        assert!(names.contains(&"update_many"));
        assert!(names.contains(&"delete"));
        assert!(names.contains(&"delete_many"));
        assert!(names.contains(&"aggregate"));
        assert!(names.contains(&"create_collection"));
        assert!(names.contains(&"create_index"));
        assert!(names.contains(&"list_databases"));
        assert!(names.contains(&"list_collections"));
        assert!(names.contains(&"run_command"));
        assert!(names.contains(&"get_analytics"));
        assert!(names.contains(&"ingest"));
        assert!(names.contains(&"ingest_status"));
        assert!(names.contains(&"list_ingest_jobs"));
        assert!(names.contains(&"cancel_ingest"));
        assert!(names.contains(&"watch_directory"));
        assert!(names.contains(&"stop_watch"));
        assert!(names.contains(&"collection_schema"));
    }

    #[test]
    fn test_json_to_document_valid() {
        let val = json!({"name": "test", "age": 30});
        let doc = json_to_document(&val).unwrap();
        assert_eq!(doc.get_str("name").unwrap(), "test");
        assert_eq!(doc.get_i64("age").unwrap(), 30);
    }

    #[test]
    fn test_json_to_document_invalid() {
        let val = json!("not an object");
        let result = json_to_document(&val);
        assert!(result.is_err());
    }

    #[test]
    fn test_json_to_documents_valid() {
        let val = json!([{"a": 1}, {"b": 2}]);
        let docs = json_to_documents(&val).unwrap();
        assert_eq!(docs.len(), 2);
    }

    #[test]
    fn test_json_to_documents_invalid() {
        let val = json!("not an array");
        let result = json_to_documents(&val);
        assert!(result.is_err());
    }

    #[test]
    fn test_error_result() {
        let result = error_result("something went wrong".to_string());
        assert!(result.is_error);
        assert_eq!(result.content.len(), 1);
        assert_eq!(result.content[0].text, "something went wrong");
    }

    #[test]
    fn test_success_result() {
        let result = success_result("ok".to_string());
        assert!(!result.is_error);
        assert_eq!(result.content.len(), 1);
        assert_eq!(result.content[0].text, "ok");
    }
}
