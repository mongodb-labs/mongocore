# MongoCore: LLM-Provided Templates & Intelligent Query Routing

## Overview

Enhance the compiled query system so the LLM returns parameterized templates alongside MQL, enabling smarter cache reuse. Additionally, the LLM classifies query intent and recommends the optimal execution method (filter, aggregate, vector search, fulltext, geo), making the compiled query system an intelligent query router.

## Motivation

**Template reuse:** The current system only reuses cached queries when the NL input has numeric parameters in the same position. "Italian restaurants" and "Chinese restaurants" are separate cache entries despite having identical MQL structure. LLM-provided templates solve this — the LLM identifies which parts are variable.

**Intelligent routing:** The search handler currently uses a fixed fallback chain (vector → fulltext → filter). The LLM can classify intent better — "cozy restaurant for a date" should go to vector search, while "Italian restaurants in Manhattan" should go directly to a filter query. Caching the routing decision means subsequent queries execute at native speed.

**Real-world query patterns:** Based on MongoDB industry solutions research, the top patterns are: filtered finds, group-by aggregations, $lookup joins, full-text search, vector search, geospatial proximity, date ranges, and array operations. The system should handle all of these.

## Design

### Enhanced LLM Response Format

The LLM now returns a structured response with method routing, MQL, and a template:

```json
{
  "type": "find",
  "method": "filter",
  "filter": {"cuisine": "Italian", "borough": "Manhattan"},
  "options": {"sort": {"name": 1}, "limit": 20},
  "template": {
    "intent_pattern": "find {{cuisine_type}} restaurants in {{location}}",
    "parameters": [
      {"name": "cuisine_type", "value": "Italian", "type": "string"},
      {"name": "location", "value": "Manhattan", "type": "string"}
    ],
    "mql_pattern": {
      "filter": {"cuisine": "{{cuisine_type}}", "borough": "{{location}}"}
    }
  }
}
```

### Execution Methods

| Method | Response Fields | Use Case |
|--------|----------------|----------|
| `filter` | `filter`, `options` | Structured queries: "find X where Y" |
| `aggregate` | `pipeline` | Group-by, counts, averages, top-N, $lookup joins |
| `vector_search` | `search_query`, `pre_filter` | Semantic/vibe queries: "cozy place for a date" |
| `fulltext` | `search_query`, `pre_filter` | Keyword queries, autocomplete-style |
| `geo` | `filter` (with `$near`/`$geoWithin`) | Proximity queries: "near Times Square" |

### CompiledMql Enum Extension

```rust
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
```

### Template Structure

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmTemplate {
    pub intent_pattern: String,
    pub parameters: Vec<LlmTemplateParameter>,
    pub mql_pattern: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmTemplateParameter {
    pub name: String,
    pub value: serde_json::Value,
    pub param_type: ParameterType,  // String, Number, Boolean, Array
}
```

### Cache Lookup Flow

```
1. Hash NL intent → check L1 cache (exact match)
   → Hit: execute cached MQL directly

2. If miss: check template registry
   - For each cached template, try to match intent_pattern against new query
   - Matching: regex with {{param}} → capture groups
   - If match found: substitute parameters into mql_pattern → execute
   → Template hit: no LLM call, construct MQL from template

3. If no match: call LLM (cold path)
   - LLM returns method + MQL + template
   - Validate MQL
   - Store in cache (exact hash) AND template registry
   → Execute MQL
```

### Template Matching Algorithm

Simple regex-based matching for v1:

```rust
fn try_match_template(intent: &str, template: &LlmTemplate) -> Option<HashMap<String, String>> {
    // Convert intent_pattern "find {{cuisine_type}} restaurants in {{location}}"
    // to regex: "find (.+) restaurants in (.+)"
    // Extract parameter values from capture groups
}
```

### Prompt Changes

The system prompt sent to the LLM is updated to request the richer format:

```
Translate this natural language query into a MongoDB query.

Database: {database}
Collection: {collection}
Schema: {schema}
Intent: "{intent}"

Respond with JSON containing:
1. "type": "find" | "aggregate"
2. "method": The best execution method:
   - "filter" — for structured queries with field-based conditions
   - "aggregate" — for group-by, counts, averages, joins, top-N
   - "vector_search" — for semantic/meaning-based queries
   - "fulltext" — for keyword text search
   - "geo" — for proximity/location queries
3. The query itself (filter/pipeline/search_query depending on method)
4. "template": parameterized version for cache reuse:
   - "intent_pattern": the query with variable parts as {{param_name}}
   - "parameters": [{name, value, type}] for each variable
   - "mql_pattern": the MQL with {{param_name}} placeholders

Only output valid JSON. No explanation, no markdown.
```

### Query Patterns Supported

#### Basic Filters
- "find Italian restaurants" → filter `{cuisine: "Italian"}`
- "movies rated PG-13" → filter `{rated: "PG-13"}`
- "users without an email" → filter `{email: {$exists: false}}`

#### Array Operations
- "movies with both Action and Comedy" → filter `{genres: {$all: ["Action", "Comedy"]}}`
- "restaurants with grade A" → filter `{"grades.grade": "A"}`

#### Date Ranges
- "sales from the last month" → filter with `$gte: ISODate(...)`
- "movies from the 1990s" → filter `{year: {$gte: 1990, $lt: 2000}}`

#### Aggregations
- "count restaurants by cuisine" → `$group` by cuisine + `$sum`
- "average rating by genre" → `$unwind` genres + `$group` + `$avg`
- "top 5 directors" → `$group` + `$sort` + `$limit`

#### $lookup (Joins)
- "orders with their customer details" → `$lookup` from customers on customer_id
- Note: requires schema to know foreign key relationships

#### Geospatial
- "restaurants near Times Square" → `$near` with coordinates
- "stores within 5km of me" → `$geoWithin` with `$centerSphere`
- Note: requires geo-indexed fields in schema context

#### Vector/Semantic Search
- "cozy atmosphere for a date night" → method: vector_search
- "comfortable running shoes" → method: vector_search

#### Fulltext
- "search for 'wireless headphones'" → method: fulltext

### Backwards Compatibility

The parser handles both old and new response formats:
- If `method` is missing → default to "filter"
- If `template` is missing → fall back to current NL-side template extraction
- If `type` is "find"/"aggregate" without `method` → infer method from type

### Validator Extensions

Add validation for new method types:
- `VectorSearch`: validate `search_query` is non-empty
- `Fulltext`: validate `search_query` is non-empty
- `Geo`: validate filter contains geo operators (`$near`, `$geoWithin`, `$geoNear`)
- Existing filter/aggregate validation unchanged

### What This Does NOT Do (Read-Only)

The system only generates read queries. It will never produce:
- `updateOne`/`updateMany`
- `deleteOne`/`deleteMany`
- `insertOne`/`insertMany`
- `dropCollection`/`dropDatabase`

The prompt explicitly constrains output to find/aggregate/search operations.

## Testing

### Unit Tests (template matching)
- Template regex matching with single parameter
- Template matching with multiple parameters
- Template matching with numeric parameter types
- No match when structure differs
- Parameter type validation (string vs number)

### Integration Tests (LLM, conditional)

**Filter routing tests:**
- "find Italian restaurants in Manhattan" → method: filter
- "movies from the 1990s" → method: filter with date range
- "restaurants with grade A" → method: filter with array query

**Aggregate routing tests:**
- "count restaurants by borough" → method: aggregate
- "top 5 directors by average rating" → method: aggregate
- "total sales by store location" → method: aggregate

**Search routing tests:**
- "cozy restaurant for a romantic dinner" → method: vector_search
- "good atmosphere quiet" → method: vector_search

**Geo routing tests (if geo-indexed data available):**
- "restaurants near coordinates 40.7, -74.0" → method: geo

**Template reuse tests:**
- "Italian restaurants" → cache miss → LLM call → template stored
- "Chinese restaurants" → template match → no LLM call → substituted MQL
- "restaurants in Manhattan" → template match → substituted MQL
- Verify LLM called only once for the group

**$lookup test:**
- "orders with customer names" → aggregate with $lookup stage

### Injection Safety (via translator)
- Existing injection tests still pass (validator unchanged for filter/aggregate)
- New methods (vector_search, fulltext, geo) have their own validation

## Implementation Scope

| File | Change |
|------|--------|
| `src/compiled/mod.rs` | Extend CompiledMql enum, add LlmTemplate types |
| `src/compiled/translator.rs` | Update parser for new format, add template registry, matching logic |
| `src/compiled/template.rs` | LLM template matching (regex-based) — replaces or augments current NL extractor |
| `src/compiled/providers/claude.rs` | Update prompt for new response format |
| `src/compiled/providers/openai.rs` | Update prompt for new response format |
| `src/compiled/providers/gateway.rs` | Update prompt for new response format |
| `src/compiled/validator.rs` | Add validation for VectorSearch/Fulltext/Geo methods |
| `tests/integration/compiled_llm_test.rs` | Add routing + template reuse tests |
| `docs/compiled-queries.md` | Document new capabilities |

## Won't Build (Deferred to Roadmap)

### Query Capabilities

| Area | Priority | Reason to defer |
|------|----------|-----------------|
| Window functions ($setWindowFields) | High | Moving averages, running totals, rankings — most common analytics NL pattern. Needs allowlist update + prompt engineering. |
| $graphLookup | High | Recursive hierarchy traversal ("who reports to whom"). Fundamentally different from $lookup. Risky for LLM without guardrails. |
| Hybrid search with RRF scoring | High | Vector + fulltext with reciprocal rank fusion — industry standard for RAG. Needs search RPC integration. |
| Bucketing ($bucket/$bucketAuto) | Medium | "Histogram of order values", "group by age ranges". Allowlist update + prompt examples. |
| Time-series ($densify/$fill) | Medium | Gap-filling for IoT/financial data. Specialized use case. |
| Atlas Search compound/fuzzy/facets | Medium | Autocomplete, fuzzy matching, faceted results. Needs search RPC integration. |
| Multi-$lookup with sub-pipelines | Medium | Complex nested syntax, high LLM error rate. |
| $unionWith (multi-collection) | Low | Security concerns with cross-collection access. |
| Pagination/cursor state | Low | State management complexity. |
| Write/update operations | Low | Security boundary — reads only. |

### Enterprise Features

| Area | Priority | Reason to defer |
|------|----------|-----------------|
| Query explanation & transparency | High | Show generated MQL, confidence score, alternative interpretations. Trust requirement for enterprise adoption. |
| Audit trail / query lineage | High | Full lineage: NL input → MQL → execution stats → who ran it. SOC2/HIPAA/GDPR compliance. |
| Multi-tenant auto-scoping | High | Auto-inject tenant filter into generated MQL. Architectural change. |
| Role-based field redaction | Medium | Generated MQL must respect field-level access ($redact). Needs RBAC integration. |
| Query governance/guardrails | Medium | Prevent expensive queries (enforce $limit, index-aware generation). |
| Query cost estimation | Low | "How expensive will this query be?" before execution. |
| Embedding-based template matching | Medium | Semantic similarity for template lookup (beyond regex). |
| Index-aware query hints | Low | Would need index metadata in context. |

### Infrastructure

| Area | Priority | Reason to defer |
|------|----------|-----------------|
| Search RPC integration | High | Wire compiled query translator into search handler as intelligent router. Own spec. |
| Change stream subscriptions via NL | Low | Different paradigm (subscription vs query). |

## Future Roadmap Additions

Add to README Future Roadmap table:
- **Search RPC Integration** — Wire compiled query translator into search handler as intelligent router
- **Query Explanation** — Show generated MQL, confidence scores, and alternative interpretations
- **Graph Queries** — $graphLookup support with safety constraints for recursive traversal
- **Hybrid Search (RRF)** — Vector + fulltext with reciprocal rank fusion scoring
- **Window Functions** — Moving averages, running totals, rankings via $setWindowFields
- **Enterprise Compliance** — Audit trail, multi-tenant scoping, role-based field redaction

## Success Criteria

- [ ] LLM returns `method` field for routing (filter/aggregate/vector_search/fulltext/geo)
- [ ] LLM returns `template` with intent_pattern and parameters
- [ ] Template registry stores and matches cached templates
- [ ] "Italian restaurants" → "Chinese restaurants" reuses template (no second LLM call)
- [ ] CompiledMql enum extended with VectorSearch, Fulltext, Geo variants
- [ ] Backwards compatible — old LLM responses still parse correctly
- [ ] All existing tests pass unchanged
- [ ] New integration tests verify routing decisions
- [ ] New integration tests verify template reuse across semantic variants
- [ ] $lookup queries produce valid pipelines
- [ ] Geo queries produce valid `$near`/`$geoWithin` filters
- [ ] Prompt updated for all 3 providers (Claude, OpenAI, Gateway)
