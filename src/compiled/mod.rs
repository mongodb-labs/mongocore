pub mod cache;
pub mod hasher;
pub mod providers;
pub mod template;
pub mod template_registry;
pub mod translator;
pub mod validator;

pub use cache::CacheStats;

use bson::Document;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledQuery {
    pub hash: String,
    pub intent: String,
    pub collection: String,
    pub database: String,
    pub mql: CompiledMql,
    pub template: Option<QueryTemplate>,       // NL-side extraction (existing)
    pub llm_template: Option<LlmTemplate>,     // LLM-provided template (new)
    pub created_at: i64, // unix timestamp
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompiledMql {
    Find {
        filter: Document,
        options: Option<Document>,
    },
    Aggregate {
        pipeline: Vec<Document>,
    },
    VectorSearch {
        search_query: String,
        pre_filter: Option<Document>,
    },
    Fulltext {
        search_query: String,
        pre_filter: Option<Document>,
    },
    Geo {
        filter: Document,
        options: Option<Document>,
    },
}

impl CompiledMql {
    /// Return the execution method name for this MQL variant.
    pub fn method(&self) -> &str {
        match self {
            Self::Find { .. } => "filter",
            Self::Aggregate { .. } => "aggregate",
            Self::VectorSearch { .. } => "vector_search",
            Self::Fulltext { .. } => "fulltext",
            Self::Geo { .. } => "geo",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryTemplate {
    pub pattern: String, // e.g. "find {category} under ${price}"
    pub parameters: Vec<TemplateParameter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateParameter {
    pub name: String,
    pub placeholder: String, // e.g. "$price"
    pub value_type: ParameterType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ParameterType {
    Number,
    String,
    Boolean,
}

/// Template provided by the LLM for parameterized cache reuse.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmTemplate {
    /// NL pattern with {{param}} placeholders: "find {{cuisine}} restaurants in {{location}}"
    pub intent_pattern: String,
    /// Parameter values extracted by the LLM
    pub parameters: Vec<LlmTemplateParameter>,
    /// MQL with {{param}} placeholders (serialized JSON)
    pub mql_pattern: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmTemplateParameter {
    pub name: String,
    pub value: serde_json::Value,
    pub param_type: ParameterType,
}
