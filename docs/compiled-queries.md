# Compiled Queries — Intelligent NL→MQL

MongoCore's compiled query system translates natural language into optimized MongoDB queries using an LLM, then intelligently caches both the query structure and execution strategy. Subsequent queries — even with different parameters — execute at native speed without any LLM call.

## The Innovation

Traditional approaches treat NL→MQL as simple text-to-query translation. MongoCore goes further:

1. **Intelligent routing** — The LLM classifies your intent and chooses the optimal execution method (direct filter, aggregation pipeline, vector search, full-text search, or geospatial)
2. **Parameterized templates** — The LLM identifies which parts of your query are variable, enabling automatic reuse. "Italian restaurants" and "Chinese restaurants" share a single cached template.
3. **Multi-level caching** — Translated queries are cached in memory, on disk, and in Atlas — so your entire fleet benefits from a single LLM call
4. **Defense-in-depth** — All generated MQL passes through a security validator that blocks dangerous operators regardless of what the LLM produces

The result: **pay once for the LLM translation, then execute at native MongoDB speed forever** — even for queries the system has never seen before, as long as they match a cached template pattern.

## How It Works

```
"find Italian restaurants in Manhattan"
        │
        ▼
┌────────────────────────────┐
│  1. Exact Cache Lookup     │ ← hash(intent + db + collection)
│     (L1 memory → L2 disk   │
│      → L3 Atlas)           │
└────────────────────────────┘
        │ miss
        ▼
┌────────────────────────────┐
│  2. Template Matching      │ ← "find {{cuisine}} restaurants in {{location}}"
│     (regex pattern match)  │   → substitute params → execute
└────────────────────────────┘
        │ miss
        ▼
┌────────────────────────────┐
│  3. LLM Translation        │ ← one-time cost
│     Returns:               │
│     • Execution method     │   (filter/aggregate/vector/fulltext/geo)
│     • MQL query            │
│     • Parameterized        │
│       template             │
└────────────────────────────┘
        │
        ▼
  Validated → Cached at all levels → Executed
```

### Template Reuse in Action

```
1st call: "find Italian restaurants in Manhattan"
  → LLM returns: method=filter, filter={cuisine:"Italian", borough:"Manhattan"}
  → Template: "find {{cuisine}} restaurants in {{location}}"
  → Cached ✓

2nd call: "find Chinese restaurants in Brooklyn"
  → Template match! Pattern: "find {{cuisine}} restaurants in {{location}}"
  → Substitute: cuisine="Chinese", location="Brooklyn"
  → Execute: find({cuisine:"Chinese", borough:"Brooklyn"})
  → NO LLM call ✓
```

## Execution Methods

The LLM classifies each query into the optimal execution strategy:

| Method | When Used | Example Query |
|--------|-----------|---------------|
| **filter** | Structured field-based queries | "find Italian restaurants in Manhattan" |
| **aggregate** | Group-by, counts, averages, joins, top-N | "count restaurants by cuisine" |
| **vector_search** | Semantic/meaning-based queries | "cozy restaurant for a romantic dinner" |
| **fulltext** | Keyword text search | "search for 'wireless headphones'" |
| **geo** | Proximity/location queries | "restaurants near Times Square" |

### Query Patterns Supported

**Direct Filters:**
- "find Italian restaurants" → `{cuisine: "Italian"}`
- "movies from the 1990s" → `{year: {$gte: 1990, $lt: 2000}}`
- "users without an email" → `{email: {$exists: false}}`
- "movies with both Action and Comedy" → `{genres: {$all: ["Action", "Comedy"]}}`

**Aggregations:**
- "count restaurants by borough" → `$group` + `$sum`
- "average rating by genre" → `$unwind` + `$group` + `$avg`
- "top 5 directors by rating" → `$group` + `$sort` + `$limit`
- "orders with customer details" → `$lookup` (join)

**Geospatial:**
- "restaurants near Times Square" → `$near` with coordinates
- "stores within 5km" → `$geoWithin` + `$centerSphere`

**Semantic (Vector Search):**
- "cozy atmosphere for a date night" → vector embedding + `$vectorSearch`
- "comfortable running shoes" → semantic similarity

**Full-Text:**
- "search for 'noise cancelling headphones'" → Atlas `$search`

## Configuration

```toml
# Direct API key (auto-detects provider)
ANTHROPIC_API_KEY = "your-api-key-here"

# Or OpenAI
# OPENAI_API_KEY = "your-api-key-here"

# Enable L3 Atlas cache (shared across instances)
compiled_cache_sync = true
```

## Custom LLM Gateway

For organizations using corporate AI gateways, proxies, or self-hosted endpoints:

```toml
LLM_BASE_URL = "https://my-ai-gateway.example.com/anthropic/v1/messages"
LLM_API_KEY = "your-gateway-api-key"
LLM_AUTH_HEADER = "api-key"
LLM_MODEL = "claude-sonnet-4-6"
LLM_PROVIDER_TYPE = "anthropic"  # or "openai"
```

| Field | Description | Default |
|-------|-------------|---------|
| `LLM_BASE_URL` | Full URL for the LLM endpoint | — (activates gateway mode) |
| `LLM_API_KEY` | API key sent in the auth header | — |
| `LLM_AUTH_HEADER` | HTTP header name for the API key | `api-key` |
| `LLM_MODEL` | Model identifier to send in requests | `claude-sonnet-4-6` |
| `LLM_PROVIDER_TYPE` | Request/response format: `anthropic` or `openai` | `anthropic` |

When `LLM_BASE_URL` is set, MongoCore uses the gateway for all NL→MQL translations.

## Usage

### Python

```python
from mongocore import MongoClient

async with MongoClient() as client:
    restaurants = client["sample_restaurants"]["restaurants"]

    # Natural language → intelligently routed to the right execution method
    results = await restaurants.compiled_query("find Italian restaurants in Manhattan")

    # Second call with different params → uses cached template, NO LLM call
    results = await restaurants.compiled_query("find Chinese restaurants in Brooklyn")

    # Aggregation — routed to aggregate pipeline automatically
    results = await restaurants.compiled_query("count restaurants by cuisine type")

    # Semantic search — routed to vector search
    results = await restaurants.compiled_query("cozy spot for a date night")
```

### TypeScript

```typescript
const restaurants = client.db('sample_restaurants').collection('restaurants');

// Filter query — cached after first call
const results = await restaurants.compiledQuery('find Italian restaurants in Manhattan');

// Template reuse — no LLM call
const results2 = await restaurants.compiledQuery('find Thai restaurants in Queens');

// Aggregation
const stats = await restaurants.compiledQuery('average grade score by borough');
```

### Go

```go
restaurants := client.Database("sample_restaurants").Collection("restaurants")

// Intelligent routing — LLM picks the best execution method
results, _ := restaurants.CompiledQuery(ctx, "top 5 cuisines by number of restaurants")

// Template reuse on subsequent calls
results2, _ := restaurants.CompiledQuery(ctx, "top 10 cuisines by number of restaurants")
```

### Java

```java
MongoCollection restaurants = client.getDatabase("sample_restaurants").getCollection("restaurants");

// Natural language with automatic caching
List<Document> results = restaurants.compiledQuery(
    "find restaurants with grade A in Manhattan"
);
```

## Cache Architecture

| Level | Storage | Latency | Scope | Survives Restart |
|-------|---------|---------|-------|-----------------|
| L1 | In-memory (RwLock HashMap) | ~0ms | Single process | No |
| L2 | Disk (JSON files) | ~1ms | Single machine | Yes |
| L3 | Atlas collection | ~5-50ms | All instances | Yes |
| Template | In-memory registry | ~0ms | Single process | No (rebuilt from L2/L3) |
| LLM | API call | ~500-2000ms | — | — |

### Cache Keys

- **Exact match:** `hash(intent + database + collection)` — same query string = instant hit
- **Template match:** Regex pattern on `intent_pattern` — different values, same structure = template hit
- **Routing cached:** The execution method decision is cached alongside the MQL — subsequent calls don't re-evaluate

### Cache Sync

When `compiled_cache_sync = true`, every new translation (including its template) is stored in `__mongocore.compiled_queries` on your Atlas cluster. Other MongoCore instances warm from this shared cache on startup.

## Safety & Validation

All LLM-generated MQL passes through a validation layer before execution. This provides defense-in-depth — even if an LLM produces dangerous output (due to prompt injection or hallucination), the validator blocks it.

### Blocked Filter Operators

| Operator | Risk | Reason |
|----------|------|--------|
| `$where` | JavaScript execution | Allows arbitrary JS code to run on the server. An attacker could exfiltrate data or cause denial of service. |
| `$function` | JavaScript execution | Server-side JS function evaluation. Same risks as `$where` — arbitrary code execution. |
| `$accumulator` | JavaScript execution | Custom accumulator with JS `init`/`accumulate`/`merge` functions. Arbitrary code execution in aggregation. |

### Blocked Aggregation Stages

| Stage | Risk | Reason |
|-------|------|--------|
| `$out` | Data exfiltration/overwrite | Writes pipeline results to a collection. Could overwrite production data. |
| `$merge` | Data modification | Merges pipeline results into an existing collection. |
| `$collStats` | Information disclosure | Exposes internal collection statistics. |
| `$currentOp` | Information disclosure | Exposes running operations and connection info. |
| `$listSessions` | Information disclosure | Lists active sessions. |
| `$planCacheStats` | Information disclosure | Exposes query plan cache internals. |

### Allowed Aggregation Stages

Only these stages are permitted: `$match`, `$project`, `$sort`, `$limit`, `$skip`, `$group`, `$lookup`, `$unwind`, `$vectorSearch`, `$search`, `$count`, `$addFields`, `$set`

Any stage not in this allowlist is rejected. This is a whitelist approach — safe by default.

### Recursive Validation

The validator inspects nested documents at any depth — `$where` buried inside `$and`/`$or` clauses is still caught.

### What This Protects Against

1. **Prompt injection** — User tricks LLM into producing `$where` or `$out` → validator catches it
2. **LLM hallucination** — Unexpected operators → blocked by allowlist
3. **Operator injection** — MongoDB operators embedded in NL → validator blocks execution
4. **Data exfiltration** — `$out`/`$merge` → blocked unconditionally
5. **Code execution** — `$where`/`$function`/`$accumulator` → all blocked

### Read-Only Guarantee

The compiled query system only generates read operations (find, aggregate, search). It will never produce updates, deletes, inserts, or destructive operations. The prompt explicitly constrains output, and the validator enforces it.

## Error Handling

| Scenario | Behavior |
|----------|----------|
| LLM unavailable | Returns error (cached queries still work) |
| Unparseable LLM response | Returns error, no execution |
| MQL validation failure | Returns error with reason |
| Template substitution error | Falls through to LLM call |
| Empty results | Returns empty set (not an error) |
