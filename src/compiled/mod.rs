pub mod cache;
pub mod hasher;
pub mod providers;
pub mod template;
pub mod translator;
pub mod validator;

use bson::Document;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledQuery {
    pub hash: String,
    pub intent: String,
    pub collection: String,
    pub database: String,
    pub mql: CompiledMql,
    pub template: Option<QueryTemplate>,
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
