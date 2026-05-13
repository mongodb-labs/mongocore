# MCP + Claude Integration — Phase 5: Insights

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **For implementers:** Read and follow `AGENTS.md` at the project root.
> Before committing: `cargo build` must produce ZERO warnings AND `cargo test --lib` must pass.
> If modifying shared structs (like `Config`): update ALL struct literals in `src/` AND `tests/`.

**Goal:** Add `suggest_indexes` and `slow_queries` tools that analyze the analytics ring buffer to surface performance insights. Add schema caching as an MCP resource.

**Architecture:** The existing `AnalyticsCollector` (`src/analytics/`) already records operation latencies and filter patterns. These tools query that buffer to identify optimization opportunities. The schema resource caches `collection_schema` results.

**Tech Stack:** Existing analytics collector, existing MCP resource infrastructure.

**Depends on:** Phase 1 (collection_schema), existing analytics module.

---

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `src/mcp/tools.rs` | Modify | Add `suggest_indexes`, `slow_queries` tool defs and handlers |
| `src/mcp/resources.rs` | Modify | Add `mongocore://schema/{database}/{collection}` resource |
| `src/mcp/handler.rs` | Modify | Update tool count |

---

### Task 1: Implement `suggest_indexes` tool

**Files:**
- Modify: `src/mcp/tools.rs`

- [ ] **Step 1: Add tool definition**

```rust
    McpToolDefinition {
        name: "suggest_indexes".to_string(),
        description: "Analyze recent query patterns from analytics and recommend missing indexes for better performance.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "database": { "type": "string", "description": "Database to analyze (optional — all if omitted)" },
                "collection": { "type": "string", "description": "Collection to analyze (optional — all in database if omitted)" }
            }
        }),
    },
```

- [ ] **Step 2: Add execution handler**

```rust
        "suggest_indexes" => {
            let analytics = match analytics {
                Some(a) => a,
                None => return error_result("Analytics not enabled. Enable analytics in config to use suggest_indexes."),
            };

            let database_filter = args.get("database").and_then(|v| v.as_str());
            let collection_filter = args.get("collection").and_then(|v| v.as_str());

            let stats = analytics.get_stats();
            let mut suggestions: Vec<Value> = Vec::new();

            // Analyze top operations for filter patterns
            for op in &stats.top_operations {
                if let Some(db) = database_filter {
                    if !op.operation.contains(db) { continue; }
                }
                if let Some(coll) = collection_filter {
                    if !op.operation.contains(coll) { continue; }
                }

                // Operations with high latency that use filters are candidates for indexing
                if op.avg_latency_ms > 100.0 && op.count > 5 {
                    suggestions.push(json!({
                        "operation": op.operation,
                        "avg_latency_ms": op.avg_latency_ms,
                        "call_count": op.count,
                        "suggestion": format!("This operation averages {}ms. Consider adding an index on the filter fields.", op.avg_latency_ms as u64),
                        "impact": "high"
                    }));
                }
            }

            if suggestions.is_empty() {
                let result = json!({
                    "message": "No index suggestions. Either queries are well-indexed or there isn't enough analytics data yet.",
                    "total_operations_analyzed": stats.total_operations
                });
                success_result(&serde_json::to_string_pretty(&result).unwrap_or_default())
            } else {
                let result = json!({
                    "suggestions": suggestions,
                    "total_operations_analyzed": stats.total_operations
                });
                success_result(&serde_json::to_string_pretty(&result).unwrap_or_default())
            }
        }
```

- [ ] **Step 3: Verify and commit**

```bash
git add src/mcp/tools.rs
git commit -m "feat(mcp): add suggest_indexes tool analyzing analytics for index recommendations"
```

---

### Task 2: Implement `slow_queries` tool

**Files:**
- Modify: `src/mcp/tools.rs`

- [ ] **Step 1: Add tool definition**

```rust
    McpToolDefinition {
        name: "slow_queries".to_string(),
        description: "Surface the slowest queries from analytics with their latency, frequency, and optimization suggestions.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "database": { "type": "string", "description": "Filter by database (optional)" },
                "threshold_ms": { "type": "integer", "description": "Minimum latency threshold in ms (default: p95 from analytics)" },
                "limit": { "type": "integer", "description": "Max results to return (default 10)", "default": 10 }
            }
        }),
    },
```

- [ ] **Step 2: Add execution handler**

```rust
        "slow_queries" => {
            let analytics = match analytics {
                Some(a) => a,
                None => return error_result("Analytics not enabled. Enable analytics in config to use slow_queries."),
            };

            let database_filter = args.get("database").and_then(|v| v.as_str());
            let threshold_ms = args.get("threshold_ms").and_then(|v| v.as_f64());
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

            let stats = analytics.get_stats();

            // Use p95 as default threshold if not specified
            let threshold = threshold_ms.unwrap_or(stats.p95_latency_ms);

            let mut slow_ops: Vec<Value> = stats.top_operations.iter()
                .filter(|op| {
                    op.avg_latency_ms >= threshold
                        && database_filter.map_or(true, |db| op.operation.contains(db))
                })
                .take(limit)
                .map(|op| json!({
                    "operation": op.operation,
                    "avg_latency_ms": op.avg_latency_ms,
                    "max_latency_ms": op.max_latency_ms,
                    "call_count": op.count,
                    "suggestion": if op.avg_latency_ms > 1000.0 {
                        "Very slow (>1s). Check if an index exists, or if the query scans too many documents."
                    } else if op.avg_latency_ms > 100.0 {
                        "Moderately slow. An index on filter fields would likely help."
                    } else {
                        "Above threshold but not critical. Monitor for regression."
                    }
                }))
                .collect();

            let result = json!({
                "threshold_ms": threshold,
                "slow_queries": slow_ops,
                "total_above_threshold": slow_ops.len(),
                "p50_latency_ms": stats.p50_latency_ms,
                "p95_latency_ms": stats.p95_latency_ms,
                "p99_latency_ms": stats.p99_latency_ms
            });
            success_result(&serde_json::to_string_pretty(&result).unwrap_or_default())
        }
```

- [ ] **Step 3: Update tool count and verify**

Update tool count from 32 to 34.

Run: `cargo build 2>&1 | grep "warning:"` — no output
Run: `cargo test --lib` — all pass

- [ ] **Step 4: Commit**

```bash
git add src/mcp/tools.rs src/mcp/handler.rs tests/integration/mcp_test.rs
git commit -m "feat(mcp): add slow_queries tool for performance analysis"
```

---

### Task 3: Add schema resource

**Files:**
- Modify: `src/mcp/resources.rs`

- [ ] **Step 1: Add schema resource definition and handler**

Add `mongocore://schema/{database}/{collection}` to `resource_definitions()` and implement the read handler that calls the same sampling logic as `collection_schema`.

- [ ] **Step 2: Verify and commit**

```bash
git add src/mcp/resources.rs
git commit -m "feat(mcp): add schema resource URI for cached collection schema access"
```

---

### Task 4: Update roadmap — mark v0.8 complete

**Files:**
- Modify: `docs/roadmap.md`

- [ ] **Step 1: Add v0.8 to version history table**

Add after the v0.7 row:

```
| **v0.8** | MCP + Claude Integration: Intelligent Data Companion | **Complete** |
```

- [ ] **Step 2: Add v0.8 changelog section**

Add after the `## v0.7 — Performance Benchmarking` section:

```markdown
## v0.8 — MCP + Claude Integration

- **Stdio MCP transport** — `--stdio` flag for Claude Desktop/Code integration, JSON-RPC over stdin/stdout
- **`ask` tool** — Natural language questions → MQL → execute → return answer with generated query and confidence
- **`explain_query` tool** — NL → MQL translation with execution plan, no execution (safe for expensive queries)
- **`collection_schema` tool** — Sample documents and infer schema (field types, cardinality, examples)
- **MCP sampling** — Zero-config LLM: uses Claude itself via MCP sampling protocol when no API key configured
- **Code generation** — `generate_code`, `generate_model`, `generate_index` tools with Tera templates for Python, TypeScript, Go, Java
- **Language/framework detection** — Auto-detect from workspace (pyproject.toml, package.json, go.mod, pom.xml/build.gradle.kts)
- **Composable skill recommendations** — Detects framework (FastAPI, Express, Spring, etc.) and recommends combining with framework-specific skills
- **Embedding pipeline** — `embed_and_store`, `semantic_search`, `ingest_and_embed` tools wiring Voyage AI + Polars + $vectorSearch
- **Skills system** — 13 guided workflows (MCP Prompts protocol + `list_skills`/`get_skill` tool fallback)
- **Insights tools** — `suggest_indexes` and `slow_queries` analyzing analytics ring buffer
- **Schema resource** — `mongocore://schema/{database}/{collection}` MCP resource
- **34 MCP tools total** — up from 21 in v0.7
```

- [ ] **Step 3: Remove MCP + Claude Integration from Future Roadmap table**

Remove the row:
```
| MCP + Claude Integration (v0.8) | Intelligent Data Companion: ... |
```

- [ ] **Step 4: Commit**

```bash
git add docs/roadmap.md
git commit -m "docs: mark v0.8 MCP + Claude Integration as complete in roadmap"
```

---

### Task 5: Add v0.8 entry to development log

**Files:**
- Modify: `docs/design/development-log.md`

- [ ] **Step 1: Add Session 3 entry**

Add after the `## Statistics & Final State` section (before `## What's Next`):

```markdown
---

## Session 3: The Intelligent Data Companion (v0.8)

### The Vision: "Groundbreaking MCP"

The session opened with a research question: "What exists in the database MCP space, and how do we go beyond it?" A survey of existing servers (the official MongoDB MCP with ~45 admin tools, PostgreSQL's minimal single `query` tool, various novel patterns like semantic decision logging and intelligent routing) revealed a gap: nobody had built an **intelligent data companion** — something that understands your data, answers questions in natural language, generates application code, and proactively suggests optimizations.

MongoCore already had the building blocks (NL→MQL compiled queries, Voyage AI embeddings, analytics ring buffer, Polars ingestion) that no other MCP server possessed. The design challenge was composition, not invention.

### The Zero-Config Insight

**The key conversation:** "Is there a way for the sidecar to use the CLI's LLM rather than needing a separate LLM configuration?"

This led to the hybrid LLM strategy: when MongoCore runs inside Claude as an MCP server, it uses MCP sampling to ask Claude itself to generate MQL. No API key needed. The template cache ensures subsequent similar queries don't need any LLM call at all. Result: users add 3 lines to their MCP config and immediately get NL→MQL capability.

### Skills as a Force Multiplier

**The conversation that elevated the design:** "Could this MCP service use skills as much as possible for repeatable processes?"

Instead of just exposing raw tools, MongoCore ships guided workflows (skills) that orchestrate multiple tool calls into coherent processes. The `add_vector_search` skill chains schema inspection → embedding → index creation → code generation into one conversation. 13 skills across 4 categories, exposed via both MCP Prompts protocol (native Claude Desktop UI) and tool-based fallbacks.

### Composable Skill Recommendations

**The final architectural insight:** "When a user asks to generate MongoDB code, the skill/MCP recommends combining with whatever skill is foremost for that language and framework."

Rather than encoding every framework's patterns (which would rot), MongoCore detects the stack (FastAPI, Express, Spring Boot, etc.) and **recommends** a framework-specific skill by name. Claude handles finding and invoking it. Falls back to LLM general knowledge for unknown frameworks. MongoCore stays the data expert; framework knowledge lives elsewhere.

### The Numbers

| Metric | Before (v0.7) | After (v0.8) |
|--------|---------------|--------------|
| MCP tools | 21 | 34 |
| MCP skills | 0 | 13 |
| MCP resources | 3 | 4 |
| Supported MCP methods | 4 | 6 (+ prompts/list, prompts/get) |
| Code generation languages | 0 | 4 |
| Embedding tools | 0 | 3 |

### Reflection

This session was about **composition over invention**. Every major feature (ask, codegen, embedding, skills) combined existing subsystems in new ways rather than building from scratch. The stdio transport was the only truly new infrastructure — everything else was plumbing between existing capabilities.

The design process worked well: brainstorm → clarifying questions → 3 approaches → user picks → present sections for approval → write spec → write plan. Each decision point surfaced requirements that wouldn't have emerged from a solo design session (build.gradle.kts support, the MCP sampling insight, composable skill recommendations).
```

- [ ] **Step 2: Update the Statistics table**

Update the counts in the `Statistics & Final State` section to reflect v0.8:
- MCP tools: 21 → 34
- Design specs: 12 → 13
- Implementation plans: 12 → 18
- Versions: v0.1 → v0.6 → v0.1 → v0.8

- [ ] **Step 3: Commit**

```bash
git add docs/design/development-log.md
git commit -m "docs: add v0.8 session narrative to development log"
```

---

## Verification Checklist

- [ ] `cargo build 2>&1 | grep "warning:"` produces no output
- [ ] `cargo test --lib` passes
- [ ] `suggest_indexes` returns suggestions when analytics has slow operations
- [ ] `slow_queries` returns operations above threshold
- [ ] Both tools return helpful messages when analytics is not enabled
- [ ] Schema resource responds to `resources/read`
- [ ] `docs/roadmap.md` shows v0.8 in version history and changelog, removed from future roadmap
- [ ] `docs/design/development-log.md` has Session 3 entry covering v0.8 design process
