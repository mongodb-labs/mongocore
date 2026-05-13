# MCP + Claude Integration — Phase 4: Skills System

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **For implementers:** Read and follow `AGENTS.md` at the project root.
> Before committing: `cargo build` must produce ZERO warnings AND `cargo test --lib` must pass.
> If modifying shared structs (like `Config`): update ALL struct literals in `src/` AND `tests/`.

**Goal:** Implement the skills system — TOML-defined guided workflows exposed as MCP Prompts and tool-based fallbacks. Skills guide Claude through multi-step processes by orchestrating MongoCore tool calls.

**Architecture:** Skills are defined as TOML files compiled into the binary via `include_str!`. A `SkillRegistry` parses and stores them. The MCP handler responds to `prompts/list` and `prompts/get` protocol methods. A `list_skills`/`get_skill` tool pair provides fallback access.

**Tech Stack:** Rust, TOML (serde), existing MCP handler infrastructure. Skills are static definitions — no runtime modification.

**Depends on:** Phase 1 (stdio, collection_schema, ask), Phase 2 (codegen), Phase 3 (embedding tools).

---

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `src/mcp/skills/mod.rs` | Create | `SkillRegistry`, `Skill`, `SkillStep` types, TOML parsing |
| `src/mcp/skills/definitions/` | Create (dir) | TOML skill definition files |
| `src/mcp/skills/definitions/explore_dataset.toml` | Create | First skill definition |
| `src/mcp/skills/definitions/bootstrap_project.toml` | Create | Project bootstrap skill |
| `src/mcp/skills/definitions/setup_collection.toml` | Create | Collection setup skill |
| `src/mcp/skills/definitions/add_vector_search.toml` | Create | Vector search setup skill |
| `src/mcp/skills/registry.rs` | Create | Skill loading and lookup |
| `src/mcp/handler.rs` | Modify | Add `prompts/list`, `prompts/get` method handlers |
| `src/mcp/tools.rs` | Modify | Add `list_skills`, `get_skill` tool defs and handlers |
| `src/mcp/mod.rs` | Modify | Export skills module |

---

### Task 1: Define skill data types

**Files:**
- Create: `src/mcp/skills/mod.rs`
- Modify: `src/mcp/mod.rs`

- [ ] **Step 1: Create the skills module with core types**

Create `src/mcp/skills/mod.rs`:

```rust
pub mod registry;

use serde::{Deserialize, Serialize};

/// A skill is a guided workflow combining multiple tool calls.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub category: SkillCategory,
    pub arguments: Vec<SkillArgument>,
    pub steps: Vec<SkillStep>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillCategory {
    DatabaseWorkflows,
    CodeScaffolding,
    DataAnalysis,
    Operations,
}

impl SkillCategory {
    pub fn display_name(&self) -> &str {
        match self {
            Self::DatabaseWorkflows => "Database Workflows",
            Self::CodeScaffolding => "Code Scaffolding",
            Self::DataAnalysis => "Data Analysis",
            Self::Operations => "Operations",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillArgument {
    pub name: String,
    pub description: String,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillStep {
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    #[serde(default)]
    pub dynamic: bool,
    #[serde(default)]
    pub analysis: bool,
    #[serde(default)]
    pub synthesis: bool,
}

/// TOML file structure for skill definitions.
#[derive(Debug, Deserialize)]
pub struct SkillToml {
    pub skill: SkillTomlInner,
}

#[derive(Debug, Deserialize)]
pub struct SkillTomlInner {
    pub name: String,
    pub description: String,
    pub category: SkillCategory,
    #[serde(default)]
    pub arguments: Vec<SkillArgument>,
    #[serde(default)]
    pub steps: Vec<SkillStep>,
}

impl From<SkillTomlInner> for Skill {
    fn from(inner: SkillTomlInner) -> Self {
        Self {
            name: inner.name,
            description: inner.description,
            category: inner.category,
            arguments: inner.arguments,
            steps: inner.steps,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_skill_toml() {
        let toml_str = r#"
[skill]
name = "explore_dataset"
description = "Explore a database"
category = "data_analysis"

[[skill.arguments]]
name = "database"
description = "Database to explore"
required = true

[[skill.steps]]
description = "List collections"
tool = "list_collections"

[[skill.steps]]
description = "Analyze results"
analysis = true
"#;
        let parsed: SkillToml = toml::from_str(toml_str).unwrap();
        let skill = Skill::from(parsed.skill);
        assert_eq!(skill.name, "explore_dataset");
        assert_eq!(skill.category, SkillCategory::DataAnalysis);
        assert_eq!(skill.arguments.len(), 1);
        assert_eq!(skill.steps.len(), 2);
        assert_eq!(skill.steps[0].tool.as_deref(), Some("list_collections"));
        assert!(skill.steps[1].analysis);
    }

    #[test]
    fn test_skill_category_display() {
        assert_eq!(SkillCategory::DatabaseWorkflows.display_name(), "Database Workflows");
        assert_eq!(SkillCategory::CodeScaffolding.display_name(), "Code Scaffolding");
    }
}
```

- [ ] **Step 2: Export from `src/mcp/mod.rs`**

Add `pub mod skills;` to `src/mcp/mod.rs`.

- [ ] **Step 3: Run tests**

Run: `cargo test --lib mcp::skills::tests`
Expected: All pass

- [ ] **Step 4: Commit**

```bash
git add src/mcp/skills/mod.rs src/mcp/mod.rs
git commit -m "feat(mcp): add skill data types and TOML parsing"
```

---

### Task 2: Create skill definitions

**Files:**
- Create: `src/mcp/skills/definitions/explore_dataset.toml`
- Create: `src/mcp/skills/definitions/bootstrap_project.toml`
- Create: `src/mcp/skills/definitions/setup_collection.toml`
- Create: `src/mcp/skills/definitions/add_vector_search.toml`

- [ ] **Step 1: Create explore_dataset.toml**

```toml
[skill]
name = "explore_dataset"
description = "Systematically explore a MongoDB database to understand its structure, relationships, and content"
category = "data_analysis"

[[skill.arguments]]
name = "database"
description = "Database to explore"
required = true

[[skill.arguments]]
name = "focus"
description = "Optional specific question to answer about the data"
required = false

[[skill.steps]]
description = "List all collections and their document counts"
tool = "list_collections"

[[skill.steps]]
description = "Sample schema from each collection (top 5 by size)"
tool = "collection_schema"

[[skill.steps]]
description = "Compute key statistics (total docs, avg doc size, date ranges) using aggregation"
tool = "aggregate"
dynamic = true

[[skill.steps]]
description = "Identify cross-collection relationships (shared field names, ObjectId references)"
analysis = true

[[skill.steps]]
description = "Summarize: collections, key entities, relationships, size, and notable patterns"
synthesis = true
```

- [ ] **Step 2: Create bootstrap_project.toml**

```toml
[skill]
name = "bootstrap_project"
description = "Set up MongoCore in a new project: detect language, configure connection, generate example code"
category = "code_scaffolding"

[[skill.arguments]]
name = "workspace_root"
description = "Path to the project root directory"
required = true

[[skill.arguments]]
name = "database"
description = "Database to connect to"
required = false

[[skill.steps]]
description = "Detect project language and framework from workspace files"
tool = "generate_code"
dynamic = true

[[skill.steps]]
description = "Verify MongoCore connection by listing databases"
tool = "list_databases"

[[skill.steps]]
description = "Generate a minimal working example (find + insert) in the detected language"
tool = "generate_code"
dynamic = true

[[skill.steps]]
description = "Generate a typed model from an existing collection (if database specified)"
tool = "generate_model"
dynamic = true

[[skill.steps]]
description = "Provide installation instructions and next steps"
synthesis = true
```

- [ ] **Step 3: Create setup_collection.toml**

```toml
[skill]
name = "setup_collection"
description = "Design and create a new collection with schema, indexes, and typed model code"
category = "database_workflows"

[[skill.arguments]]
name = "database"
description = "Database for the new collection"
required = true

[[skill.arguments]]
name = "collection"
description = "Name for the new collection"
required = true

[[skill.arguments]]
name = "purpose"
description = "What this collection will store (used to design schema)"
required = true

[[skill.steps]]
description = "Design the schema based on the stated purpose and access patterns"
analysis = true

[[skill.steps]]
description = "Create the collection"
tool = "create_collection"

[[skill.steps]]
description = "Create indexes based on expected query patterns"
tool = "create_index"
dynamic = true

[[skill.steps]]
description = "Generate a typed model for the collection in the user's language"
tool = "generate_model"
dynamic = true

[[skill.steps]]
description = "Generate sample CRUD code showing how to use the collection"
tool = "generate_code"
dynamic = true
```

- [ ] **Step 4: Create add_vector_search.toml**

```toml
[skill]
name = "add_vector_search"
description = "Add semantic/vector search to a collection: identify text field, embed documents, create index, generate search code"
category = "code_scaffolding"

[[skill.arguments]]
name = "database"
description = "Database containing the collection"
required = true

[[skill.arguments]]
name = "collection"
description = "Collection to add vector search to"
required = true

[[skill.steps]]
description = "Inspect collection schema to identify text fields suitable for embedding"
tool = "collection_schema"

[[skill.steps]]
description = "Confirm which field to embed with the user"
analysis = true

[[skill.steps]]
description = "Embed a sample of documents to test the pipeline"
tool = "embed_and_store"
dynamic = true

[[skill.steps]]
description = "Run a test semantic search to verify results"
tool = "semantic_search"
dynamic = true

[[skill.steps]]
description = "Generate search code in the user's language with framework recommendations"
tool = "generate_code"
dynamic = true

[[skill.steps]]
description = "Provide next steps: vector index creation command, production considerations"
synthesis = true
```

- [ ] **Step 5: Commit**

```bash
mkdir -p src/mcp/skills/definitions
git add src/mcp/skills/definitions/
git commit -m "feat(mcp): add initial skill TOML definitions (4 skills)"
```

---

### Task 3: Implement skill registry

**Files:**
- Create: `src/mcp/skills/registry.rs`

- [ ] **Step 1: Create the registry with compiled-in skills**

Create `src/mcp/skills/registry.rs`:

```rust
use std::collections::HashMap;

use super::{Skill, SkillCategory, SkillToml};

/// Registry of all available skills, loaded at compile time.
pub struct SkillRegistry {
    skills: HashMap<String, Skill>,
}

impl SkillRegistry {
    /// Create a new registry with all built-in skills loaded.
    pub fn new() -> Self {
        let mut skills = HashMap::new();

        let definitions: &[(&str, &str)] = &[
            ("explore_dataset", include_str!("definitions/explore_dataset.toml")),
            ("bootstrap_project", include_str!("definitions/bootstrap_project.toml")),
            ("setup_collection", include_str!("definitions/setup_collection.toml")),
            ("add_vector_search", include_str!("definitions/add_vector_search.toml")),
        ];

        for (name, toml_str) in definitions {
            match toml::from_str::<SkillToml>(toml_str) {
                Ok(parsed) => {
                    let skill = Skill::from(parsed.skill);
                    skills.insert(name.to_string(), skill);
                }
                Err(e) => {
                    eprintln!("Failed to parse skill '{}': {}", name, e);
                }
            }
        }

        Self { skills }
    }

    /// Get all skills, optionally filtered by category.
    pub fn list(&self, category: Option<SkillCategory>) -> Vec<&Skill> {
        self.skills.values()
            .filter(|s| category.map_or(true, |c| s.category == c))
            .collect()
    }

    /// Get a specific skill by name.
    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.skills.get(name)
    }

    /// Total number of registered skills.
    pub fn len(&self) -> usize {
        self.skills.len()
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_loads_all_skills() {
        let registry = SkillRegistry::new();
        assert_eq!(registry.len(), 4);
    }

    #[test]
    fn test_registry_get_by_name() {
        let registry = SkillRegistry::new();
        let skill = registry.get("explore_dataset").unwrap();
        assert_eq!(skill.name, "explore_dataset");
        assert_eq!(skill.category, SkillCategory::DataAnalysis);
        assert!(!skill.steps.is_empty());
    }

    #[test]
    fn test_registry_filter_by_category() {
        let registry = SkillRegistry::new();
        let code_skills = registry.list(Some(SkillCategory::CodeScaffolding));
        assert!(code_skills.len() >= 2); // bootstrap_project + add_vector_search
        for skill in &code_skills {
            assert_eq!(skill.category, SkillCategory::CodeScaffolding);
        }
    }

    #[test]
    fn test_registry_list_all() {
        let registry = SkillRegistry::new();
        let all = registry.list(None);
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn test_skills_reference_valid_tools() {
        let registry = SkillRegistry::new();
        let known_tools = [
            "list_collections", "collection_schema", "aggregate",
            "create_collection", "create_index", "list_databases",
            "generate_code", "generate_model", "embed_and_store",
            "semantic_search",
        ];
        for skill in registry.list(None) {
            for step in &skill.steps {
                if let Some(ref tool) = step.tool {
                    assert!(
                        known_tools.contains(&tool.as_str()),
                        "Skill '{}' references unknown tool '{}'", skill.name, tool
                    );
                }
            }
        }
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --lib mcp::skills::registry::tests`
Expected: All 5 tests pass

- [ ] **Step 3: Commit**

```bash
git add src/mcp/skills/registry.rs
git commit -m "feat(mcp): implement skill registry with compile-time TOML loading"
```

---

### Task 4: Add `prompts/list` and `prompts/get` to MCP handler

**Files:**
- Modify: `src/mcp/handler.rs`

- [ ] **Step 1: Add SkillRegistry to McpHandler**

```rust
use super::skills::registry::SkillRegistry;

pub struct McpHandler {
    // ... existing fields ...
    skills: SkillRegistry,
}
```

Initialize in `new()`: `skills: SkillRegistry::new()`.

- [ ] **Step 2: Add method dispatch**

In `handle_request`, add:

```rust
            "prompts/list" => self.handle_prompts_list(id),
            "prompts/get" => self.handle_prompts_get(id, request.params),
```

- [ ] **Step 3: Implement handlers**

```rust
    fn handle_prompts_list(&self, id: Option<Value>) -> JsonRpcResponse {
        let prompts: Vec<Value> = self.skills.list(None)
            .iter()
            .map(|skill| {
                json!({
                    "name": skill.name,
                    "description": skill.description,
                    "arguments": skill.arguments.iter().map(|a| json!({
                        "name": a.name,
                        "description": a.description,
                        "required": a.required
                    })).collect::<Vec<_>>()
                })
            })
            .collect();
        JsonRpcResponse::success(id, json!({ "prompts": prompts }))
    }

    fn handle_prompts_get(&self, id: Option<Value>, params: Option<Value>) -> JsonRpcResponse {
        let name = match params.as_ref().and_then(|p| p.get("name")).and_then(|n| n.as_str()) {
            Some(n) => n,
            None => return JsonRpcResponse::error(id, -32602, "Missing prompt name"),
        };

        let skill = match self.skills.get(name) {
            Some(s) => s,
            None => return JsonRpcResponse::error(id, -32602, format!("Skill not found: {}", name)),
        };

        // Build the workflow guide as a prompt message
        let steps_text: String = skill.steps.iter().enumerate()
            .map(|(i, step)| {
                let prefix = if step.tool.is_some() {
                    format!("**Step {}:** [Tool: {}]", i + 1, step.tool.as_deref().unwrap_or(""))
                } else if step.analysis {
                    format!("**Step {}:** [Analysis]", i + 1)
                } else if step.synthesis {
                    format!("**Step {}:** [Synthesis]", i + 1)
                } else {
                    format!("**Step {}:**", i + 1)
                };
                format!("{} {}", prefix, step.description)
            })
            .collect::<Vec<_>>()
            .join("\n");

        let message_text = format!(
            "I'll guide you through: {}\n\n{}\n\nStarting now...",
            skill.description, steps_text
        );

        JsonRpcResponse::success(id, json!({
            "description": skill.description,
            "messages": [{
                "role": "assistant",
                "content": { "type": "text", "text": message_text }
            }]
        }))
    }
```

- [ ] **Step 4: Add tests**

```rust
    #[test]
    fn test_skills_registry_loaded() {
        let registry = super::super::skills::registry::SkillRegistry::new();
        assert!(registry.len() >= 4);
    }
```

- [ ] **Step 5: Verify and commit**

Run: `cargo build 2>&1 | grep "warning:"` — no output
Run: `cargo test --lib` — all pass

```bash
git add src/mcp/handler.rs
git commit -m "feat(mcp): add prompts/list and prompts/get handlers for skills"
```

---

### Task 5: Add `list_skills` and `get_skill` tool fallbacks

**Files:**
- Modify: `src/mcp/tools.rs`

- [ ] **Step 1: Add tool definitions**

```rust
    McpToolDefinition {
        name: "list_skills".to_string(),
        description: "List available guided workflows (skills) that orchestrate multiple tools into repeatable processes.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "category": { "type": "string", "enum": ["database_workflows", "code_scaffolding", "data_analysis", "operations"], "description": "Filter by category (optional)" }
            }
        }),
    },
    McpToolDefinition {
        name: "get_skill".to_string(),
        description: "Get the full workflow guide for a specific skill, including all steps and tool calls.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "description": "Skill name (e.g. 'explore_dataset', 'bootstrap_project')" }
            },
            "required": ["name"]
        }),
    },
```

- [ ] **Step 2: Add execution handlers**

Pass the `SkillRegistry` reference to `execute_tool` and add handlers:

```rust
        "list_skills" => {
            let category_filter = args.get("category")
                .and_then(|v| v.as_str())
                .and_then(|c| match c {
                    "database_workflows" => Some(SkillCategory::DatabaseWorkflows),
                    "code_scaffolding" => Some(SkillCategory::CodeScaffolding),
                    "data_analysis" => Some(SkillCategory::DataAnalysis),
                    "operations" => Some(SkillCategory::Operations),
                    _ => None,
                });

            let skills: Vec<Value> = skills.list(category_filter)
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

            success_result(&serde_json::to_string_pretty(&json!({ "skills": skills })).unwrap_or_default())
        }

        "get_skill" => {
            let name = match args.get("name").and_then(|v| v.as_str()) {
                Some(n) => n,
                None => return error_result("Missing required field: name"),
            };

            match skills.get(name) {
                Some(skill) => {
                    let steps: Vec<Value> = skill.steps.iter().enumerate()
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
                        "arguments": skill.arguments,
                        "steps": steps
                    });
                    success_result(&serde_json::to_string_pretty(&result).unwrap_or_default())
                }
                None => error_result(&format!("Skill not found: '{}'. Use list_skills to see available skills.", name)),
            }
        }
```

- [ ] **Step 3: Update tool count**

Update assertions from 30 to 32.

- [ ] **Step 4: Verify and commit**

Run: `cargo build 2>&1 | grep "warning:"` — no output
Run: `cargo test --lib` — all pass

```bash
git add src/mcp/tools.rs src/mcp/handler.rs tests/integration/mcp_test.rs
git commit -m "feat(mcp): add list_skills and get_skill tool fallbacks for skills system"
```

---

### Task 6: Add remaining skill definitions

**Files:**
- Create 9 more TOML files in `src/mcp/skills/definitions/`

- [ ] **Step 1: Create remaining skills**

Create `debug_slow_query.toml`, `design_schema.toml`, `build_search_pipeline.toml`, `add_crud_endpoint.toml`, `find_anomalies.toml`, `collection_health.toml`, `optimize_performance.toml`, `data_ingestion_pipeline.toml`, `migration_check.toml` following the same TOML structure as the existing 4.

- [ ] **Step 2: Register in registry.rs**

Add all new skills to the `definitions` array in `SkillRegistry::new()`.

- [ ] **Step 3: Update test assertions**

Update `test_registry_loads_all_skills` to expect 13 skills.

- [ ] **Step 4: Verify and commit**

```bash
git add src/mcp/skills/definitions/ src/mcp/skills/registry.rs
git commit -m "feat(mcp): add remaining 9 skill definitions (13 total)"
```

---

## Verification Checklist

- [ ] `cargo build 2>&1 | grep "warning:"` produces no output
- [ ] `cargo test --lib` passes all unit tests
- [ ] `prompts/list` returns all 13 skills via stdio
- [ ] `prompts/get` returns structured workflow for any skill
- [ ] `list_skills` tool returns all skills with correct categories
- [ ] `get_skill` tool returns step-by-step workflow
- [ ] All skill definitions parse without error
- [ ] All skills reference only tools that exist in the tool registry
