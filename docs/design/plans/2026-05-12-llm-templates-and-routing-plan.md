# LLM-Provided Templates & Intelligent Routing — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **For implementers:** Read and follow `AGENTS.md` at the project root.
> Before committing: `just test-all` must pass (this runs all Rust tests + all client tests).
> If modifying client libraries: verify imports work and run `just test-clients`.
> If modifying shared structs (like `Config`): update ALL struct literals in `src/` AND `tests/`.

**Goal:** Extend the compiled query system with LLM-provided templates for smarter cache reuse and intelligent method routing (filter/aggregate/vector_search/fulltext/geo).

**Architecture:** Extend `CompiledMql` enum with new variants, add `LlmTemplate` types alongside existing `QueryTemplate`, build a template registry with regex matching, update the translator to parse the enriched LLM response format and check templates before calling the LLM, and update all 3 provider prompts to request the new format.

**Tech Stack:** Rust, regex, serde_json, existing compiled query infrastructure, existing LLM providers.

**Branch:** `feat/llm-provided-templates` — do NOT push to origin.

---

## File Structure

| Action | Path | Responsibility |
|--------|------|----------------|
| Modify | `src/compiled/mod.rs` | Extend CompiledMql enum, add LlmTemplate types |
| Create | `src/compiled/template_registry.rs` | Template registry with regex matching |
| Modify | `src/compiled/template.rs` | Keep existing NL extractor, add integration point |
| Modify | `src/compiled/translator.rs` | New parser, template registry integration, routing |
| Modify | `src/compiled/validator.rs` | Add validation for VectorSearch/Fulltext/Geo |
| Modify | `src/compiled/providers/claude.rs` | Updated prompt |
| Modify | `src/compiled/providers/openai.rs` | Updated prompt |
| Modify | `src/compiled/providers/gateway.rs` | Updated prompt |
| Modify | `tests/integration/compiled_llm_test.rs` | New routing + template reuse tests |

---

## Task 1: Extend CompiledMql Enum and Add LlmTemplate Types

**Files:**
- Modify: `src/compiled/mod.rs`

- [ ] **Step 1: Extend CompiledMql with new variants**

In `src/compiled/mod.rs`, replace the `CompiledMql` enum:

```rust
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
```

- [ ] **Step 2: Add LlmTemplate types**

Add after the existing `ParameterType` enum:

```rust
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
```

- [ ] **Step 3: Update CompiledQuery to hold both template types**

Change the `template` field in `CompiledQuery`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledQuery {
    pub hash: String,
    pub intent: String,
    pub collection: String,
    pub database: String,
    pub mql: CompiledMql,
    pub template: Option<QueryTemplate>,       // NL-side extraction (existing)
    pub llm_template: Option<LlmTemplate>,     // LLM-provided template (new)
    pub created_at: i64,
}
```

- [ ] **Step 4: Run tests to verify compilation**

Run: `cargo test --lib compiled`
Expected: Existing tests may need minor adjustments if they construct CompiledQuery without the new field. Add `llm_template: None` to any struct literals that fail.

- [ ] **Step 5: Fix any struct literals**

Search for `CompiledQuery {` in `src/compiled/translator.rs` and `tests/integration/compiled_test.rs`. Add `llm_template: None,` to each.

- [ ] **Step 6: Run all tests**

Run: `cargo test --lib`
Expected: All pass

- [ ] **Step 7: Commit**

```bash
git add src/compiled/mod.rs src/compiled/translator.rs tests/
git commit -m "feat(compiled): extend CompiledMql with VectorSearch/Fulltext/Geo and add LlmTemplate types"
```

---

## Task 2: Create Template Registry

**Files:**
- Create: `src/compiled/template_registry.rs`
- Modify: `src/compiled/mod.rs` (add `pub mod template_registry;`)

- [ ] **Step 1: Create template_registry.rs with types and tests**

```rust
use std::collections::HashMap;
use std::sync::RwLock;
use regex::Regex;

use super::{CompiledMql, LlmTemplate, LlmTemplateParameter, ParameterType};

/// Registry that stores LLM-provided templates and matches new intents against them.
pub struct TemplateRegistry {
    /// Templates keyed by a normalized pattern string
    templates: RwLock<Vec<RegisteredTemplate>>,
}

struct RegisteredTemplate {
    /// The original intent pattern: "find {{cuisine}} restaurants in {{location}}"
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
        assert_eq!(result, serde_json::json!({"pop": {"$gt": "50000"}}));
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
```

- [ ] **Step 2: Export module**

Add to `src/compiled/mod.rs`:
```rust
pub mod template_registry;
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib template_registry`
Expected: All 8 tests pass

- [ ] **Step 4: Commit**

```bash
git add src/compiled/template_registry.rs src/compiled/mod.rs
git commit -m "feat(compiled): add template registry with regex matching"
```

---

## Task 3: Update LLM Prompts

**Files:**
- Modify: `src/compiled/providers/claude.rs`
- Modify: `src/compiled/providers/openai.rs`
- Modify: `src/compiled/providers/gateway.rs`

- [ ] **Step 1: Create a shared prompt builder function**

Since all 3 providers use the same prompt logic, extract it. Add a new file `src/compiled/providers/prompt.rs`:

```rust
use super::TranslationContext;

/// Build the system/user prompt for NL→MQL translation with routing and templates.
pub fn build_translation_prompt(
    intent: &str,
    database: &str,
    collection: &str,
    context: &TranslationContext,
) -> String {
    let mut prompt = format!(
        "Translate this natural language query into a MongoDB query.\n\n\
         Database: {}\nCollection: {}\nIntent: \"{}\"\n\n",
        database, collection, intent
    );
    if let Some(ref schema) = context.schema_hint {
        prompt.push_str(&format!("Schema: {}\n\n", schema));
    }
    if !context.sample_documents.is_empty() {
        prompt.push_str("Sample documents:\n");
        for doc in &context.sample_documents {
            prompt.push_str(&format!("  {}\n", doc));
        }
        prompt.push('\n');
    }
    if !context.available_indexes.is_empty() {
        prompt.push_str("Available indexes:\n");
        for idx in &context.available_indexes {
            prompt.push_str(&format!("  {}\n", idx));
        }
        prompt.push('\n');
    }
    prompt.push_str(
        "Respond with ONLY valid JSON containing:\n\
         1. \"type\": \"find\" or \"aggregate\"\n\
         2. \"method\": The best execution method:\n\
            - \"filter\" — structured queries with field-based conditions\n\
            - \"aggregate\" — group-by, counts, averages, joins, top-N\n\
            - \"vector_search\" — semantic/meaning-based queries\n\
            - \"fulltext\" — keyword text search\n\
            - \"geo\" — proximity/location queries\n\
         3. The query (\"filter\" for find, \"pipeline\" for aggregate, \"search_query\" for search methods)\n\
         4. \"template\" (optional): parameterized version for cache reuse:\n\
            - \"intent_pattern\": the query with variable parts as {{param_name}}\n\
            - \"parameters\": [{\"name\": \"...\", \"value\": ..., \"type\": \"string\"|\"number\"|\"boolean\"}]\n\
            - \"mql_pattern\": the MQL with {{param_name}} placeholders\n\n\
         No explanation, no markdown. Only valid JSON.",
    );
    prompt
}
```

- [ ] **Step 2: Export prompt module and update providers**

Add `pub mod prompt;` to `src/compiled/providers/mod.rs`.

Update `claude.rs` to use the shared prompt:
```rust
fn build_prompt(...) -> String {
    super::prompt::build_translation_prompt(intent, database, collection, context)
}
```

Same for `openai.rs` and `gateway.rs`.

- [ ] **Step 3: Run tests**

Run: `cargo test --lib`
Expected: All pass (prompt output is the same structure, just different wording)

- [ ] **Step 4: Commit**

```bash
git add src/compiled/providers/
git commit -m "feat(compiled): update LLM prompts with routing and template instructions"
```

---

## Task 4: Update Translator with New Parser and Template Integration

**Files:**
- Modify: `src/compiled/translator.rs`

- [ ] **Step 1: Add template registry to translator**

Add the template registry field:
```rust
use super::template_registry::TemplateRegistry;

pub struct CompiledQueryTranslator {
    cache: CacheHierarchy,
    provider: Option<Box<dyn LlmProvider>>,
    template_registry: TemplateRegistry,
}
```

Update `new()`:
```rust
pub fn new(
    pool: Option<ConnectionPool>,
    provider: Option<Box<dyn LlmProvider>>,
    cache_dir: Option<PathBuf>,
) -> Self {
    let cache = CacheHierarchy::new(pool, cache_dir);
    Self {
        cache,
        provider,
        template_registry: TemplateRegistry::new(),
    }
}
```

- [ ] **Step 2: Update translate() to check template registry**

Insert template registry check between cache check and LLM call:

```rust
pub async fn translate(...) -> Result<CompiledQuery, TranslateError> {
    let hash = QueryHasher::hash(intent, database, collection, None);

    // 1. Check exact cache
    if let Some(cached) = self.cache.get(&hash).await {
        return Ok(cached);
    }

    // 2. Check template registry
    if let Some(template_match) = self.template_registry.try_match(intent, database, collection) {
        let mql = Self::parse_method_response(&template_match.mql_json, &template_match.method)?;
        // Validate
        self.validate_mql(&mql)?;
        let template = TemplateExtractor::extract(intent);
        let compiled = CompiledQuery {
            hash: hash.clone(),
            intent: intent.to_string(),
            collection: collection.to_string(),
            database: database.to_string(),
            mql,
            template,
            llm_template: None,
            created_at: now(),
        };
        self.cache.put(&compiled).await;
        return Ok(compiled);
    }

    // 3. Call LLM
    let provider = self.provider.as_ref().ok_or(TranslateError::NoProvider)?;
    let response = provider.translate(intent, database, collection, context).await
        .map_err(TranslateError::Llm)?;
    let parsed = Self::parse_llm_response(&response)?;
    self.validate_mql(&parsed.mql)?;

    // Register template if LLM provided one
    if let Some(ref llm_tmpl) = parsed.llm_template {
        self.template_registry.register(llm_tmpl, parsed.mql.method(), database, collection);
    }

    let template = TemplateExtractor::extract(intent);
    let compiled = CompiledQuery {
        hash: hash.clone(),
        intent: intent.to_string(),
        collection: collection.to_string(),
        database: database.to_string(),
        mql: parsed.mql,
        template,
        llm_template: parsed.llm_template,
        created_at: now(),
    };
    self.cache.put(&compiled).await;
    Ok(compiled)
}
```

- [ ] **Step 3: Update parse_llm_response to handle new format**

The parser should handle both old format (just type+filter/pipeline) and new format (with method+template):

```rust
struct ParsedLlmResponse {
    mql: CompiledMql,
    llm_template: Option<LlmTemplate>,
}

fn parse_llm_response(response: &str) -> Result<ParsedLlmResponse, TranslateError> {
    let value: serde_json::Value = serde_json::from_str(response)
        .map_err(|e| TranslateError::ParseError(format!("Invalid JSON: {}", e)))?;

    let method = value["method"].as_str().unwrap_or("filter");
    let mql = Self::parse_method_response(&value, method)?;

    // Parse template if present
    let llm_template = value.get("template").and_then(|t| {
        serde_json::from_value::<LlmTemplate>(t.clone()).ok()
    });

    Ok(ParsedLlmResponse { mql, llm_template })
}

fn parse_method_response(value: &serde_json::Value, method: &str) -> Result<CompiledMql, TranslateError> {
    match method {
        "filter" | "geo" => {
            let filter_val = &value["filter"];
            let filter: Document = bson::to_document(filter_val)
                .map_err(|e| TranslateError::ParseError(format!("Invalid filter: {}", e)))?;
            let options = value.get("options")
                .and_then(|o| bson::to_document(o).ok());
            if method == "geo" {
                Ok(CompiledMql::Geo { filter, options })
            } else {
                Ok(CompiledMql::Find { filter, options })
            }
        }
        "aggregate" => {
            let pipeline_val = value["pipeline"].as_array()
                .ok_or_else(|| TranslateError::ParseError("Missing pipeline".to_string()))?;
            let pipeline: Vec<Document> = pipeline_val.iter()
                .map(|v| bson::to_document(v).map_err(|e| TranslateError::ParseError(e.to_string())))
                .collect::<Result<_, _>>()?;
            Ok(CompiledMql::Aggregate { pipeline })
        }
        "vector_search" => {
            let search_query = value["search_query"].as_str()
                .unwrap_or(value["query"].as_str().unwrap_or(""))
                .to_string();
            let pre_filter = value.get("pre_filter")
                .and_then(|f| bson::to_document(f).ok());
            Ok(CompiledMql::VectorSearch { search_query, pre_filter })
        }
        "fulltext" => {
            let search_query = value["search_query"].as_str()
                .unwrap_or(value["query"].as_str().unwrap_or(""))
                .to_string();
            let pre_filter = value.get("pre_filter")
                .and_then(|f| bson::to_document(f).ok());
            Ok(CompiledMql::Fulltext { search_query, pre_filter })
        }
        _ => {
            // Default to find for backwards compatibility
            let filter_val = &value["filter"];
            let filter: Document = bson::to_document(filter_val)
                .unwrap_or_default();
            Ok(CompiledMql::Find { filter, options: None })
        }
    }
}
```

- [ ] **Step 4: Add validate_mql helper**

```rust
fn validate_mql(&self, mql: &CompiledMql) -> Result<(), TranslateError> {
    match mql {
        CompiledMql::Find { filter, .. } | CompiledMql::Geo { filter, .. } => {
            MqlValidator::validate_filter(filter).map_err(TranslateError::Validation)?;
        }
        CompiledMql::Aggregate { pipeline } => {
            MqlValidator::validate_pipeline(pipeline).map_err(TranslateError::Validation)?;
        }
        CompiledMql::VectorSearch { search_query, .. } | CompiledMql::Fulltext { search_query, .. } => {
            if search_query.is_empty() {
                return Err(TranslateError::Validation("Empty search query".to_string()));
            }
        }
    }
    Ok(())
}
```

- [ ] **Step 5: Add template_registry_size method**

```rust
pub fn template_registry_size(&self) -> usize {
    self.template_registry.len()
}
```

- [ ] **Step 6: Update existing tests**

The existing unit tests in translator.rs construct `CompiledMql::Find` directly — these should still work. The `parse_llm_response` tests may need updating since it now returns `ParsedLlmResponse` instead of `CompiledMql`.

- [ ] **Step 7: Run all tests**

Run: `cargo test --lib`
Expected: All pass

- [ ] **Step 8: Commit**

```bash
git add src/compiled/translator.rs
git commit -m "feat(compiled): integrate template registry and new parser into translator"
```

---

## Task 5: Update Validator for New Variants

**Files:**
- Modify: `src/compiled/validator.rs`

- [ ] **Step 1: Add validation test for geo filter**

```rust
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
```

- [ ] **Step 2: Run tests — should pass (geo uses existing filter validation)**

Run: `cargo test --lib validator`
Expected: PASS (no new validation logic needed for geo — it's just a filter with geo operators)

- [ ] **Step 3: Commit**

```bash
git add src/compiled/validator.rs
git commit -m "test(compiled): add geo filter validation test"
```

---

## Task 6: Add Integration Tests for Routing and Template Reuse

**Files:**
- Modify: `tests/integration/compiled_llm_test.rs`

- [ ] **Step 1: Add routing verification tests**

Add tests that verify the LLM returns appropriate methods and that template reuse works with the new system. These use the existing `load_test_config()` + `llm_tests_enabled()` pattern.

Key tests to add:
- `test_llm_routing_filter_query` — "find Italian restaurants" → verify `compiled.mql.method() == "filter"`
- `test_llm_routing_aggregate_query` — "count restaurants by borough" → verify method is "aggregate"
- `test_llm_template_registry_reuse` — "find Italian restaurants in Manhattan" then "find Chinese restaurants in Brooklyn" → verify `template_registry_size() == 1` and LLM called only once via the registry

- [ ] **Step 2: Verify compilation**

Run: `cargo test --test integration compiled_llm -- --list`

- [ ] **Step 3: Commit**

```bash
git add tests/integration/compiled_llm_test.rs
git commit -m "test(compiled): add routing and template registry integration tests"
```

---

## Task 7: Verification

- [ ] **Step 1: Run all unit tests**

```bash
cargo test --lib
```
Expected: All pass (223+ with new template registry tests)

- [ ] **Step 2: Verify integration tests compile**

```bash
cargo test --test integration --no-run
```

- [ ] **Step 3: Verify tests skip without LLM**

```bash
unset TEST_LLM_INTEGRATION
cargo test --test integration compiled_llm -- --nocapture 2>&1 | grep -c "ok"
```
Expected: All pass (skip and return ok)

- [ ] **Step 4: Commit any fixes**

---

## Implementation Order

```
Task 1: Types (foundation — everything depends on this)
Task 2: Template registry (depends on types from Task 1)
Task 3: Prompts (independent of Task 2, depends on Task 1)
Task 4: Translator update (depends on Tasks 1, 2, 3)
Task 5: Validator (independent, small)
Task 6: Integration tests (depends on Task 4)
Task 7: Verification (depends on all)
```

Tasks 2, 3, and 5 can be parallelized after Task 1.

---

## Definition of Done

- [ ] `CompiledMql` has 5 variants: Find, Aggregate, VectorSearch, Fulltext, Geo
- [ ] `LlmTemplate` type stores intent_pattern, parameters, and mql_pattern
- [ ] Template registry matches intents via regex and substitutes parameters
- [ ] Translator checks template registry before calling LLM
- [ ] LLM-provided templates are registered after translation
- [ ] All 3 provider prompts request method + template in response
- [ ] Parser handles both old and new LLM response formats (backwards compatible)
- [ ] Template registry tests pass (8+ unit tests)
- [ ] All existing tests pass unchanged
- [ ] Integration tests verify routing and template reuse
