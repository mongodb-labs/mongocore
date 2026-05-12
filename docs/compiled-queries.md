# Compiled Queries

Compiled queries let you express MongoDB operations in natural language. MongoCore translates your intent to MQL using an LLM, then caches the result at multiple levels so subsequent calls are instant.

## How It Works

```
"find active users older than 30"
        │
        ▼
┌───────────────────────┐
│  L1: In-Memory Cache  │ ← fastest, per-process LRU
│  (DashMap)            │
└───────────────────────┘
        │ miss
        ▼
┌───────────────────────┐
│  L2: Disk Cache       │ ← survives restarts, local filesystem
│  (JSON files)         │
└───────────────────────┘
        │ miss
        ▼
┌───────────────────────┐
│  L3: Atlas Cache      │ ← shared across instances
│  (__mongocore DB)     │
└───────────────────────┘
        │ miss
        ▼
┌───────────────────────┐
│  LLM Translation      │ ← Claude/Anthropic API
│  (one-time cost)      │
└───────────────────────┘
        │
        ▼
  Cached at all levels
```

After the first translation, the query is stored at all cache levels. The hash of (intent + database + collection) determines cache identity.

## Configuration

```toml
# mongocore.toml
ANTHROPIC_API_KEY = "your-api-key-here"
compiled_cache_sync = true  # Enable L3 Atlas cache
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

Or via environment variables:

```bash
export LLM_BASE_URL="https://my-ai-gateway.example.com/anthropic/v1/messages"
export LLM_API_KEY="your-gateway-api-key"
export LLM_AUTH_HEADER="api-key"
export LLM_MODEL="claude-sonnet-4-6"
export LLM_PROVIDER_TYPE="anthropic"
```

### Configuration

| Field | Description | Default |
|-------|-------------|---------|
| `LLM_BASE_URL` | Full URL for the LLM endpoint | — (activates gateway mode) |
| `LLM_API_KEY` | API key sent in the auth header | — |
| `LLM_AUTH_HEADER` | HTTP header name for the API key | `api-key` |
| `LLM_MODEL` | Model identifier to send in requests | `claude-sonnet-4-6` |
| `LLM_PROVIDER_TYPE` | Request/response format: `anthropic` or `openai` | `anthropic` |

### Precedence

When `LLM_BASE_URL` is set, MongoCore uses the gateway for all NL→MQL translations. Direct `ANTHROPIC_API_KEY` or `OPENAI_API_KEY` are ignored for compiled queries (but still used for other features if configured).

### Examples

**Anthropic via gateway:**
```toml
LLM_BASE_URL = "https://my-ai-gateway.example.com/anthropic/v1/messages"
LLM_API_KEY = "gw-key-123"
LLM_AUTH_HEADER = "api-key"
LLM_MODEL = "claude-sonnet-4-6"
LLM_PROVIDER_TYPE = "anthropic"
```

**OpenAI via gateway:**
```toml
LLM_BASE_URL = "https://my-ai-gateway.example.com/openai/v1/chat/completions"
LLM_API_KEY = "gw-key-456"
LLM_AUTH_HEADER = "api-key"
LLM_MODEL = "gpt-5.1"
LLM_PROVIDER_TYPE = "openai"
```

## Usage

### Python

```python
from mongocore import MongoClient

async with MongoClient() as client:
    users = client["myapp"]["users"]

    # Natural language query — first call hits LLM, subsequent calls use cache
    results = await users.compiled_query(
        "find active users who signed up this month"
    )

    # With parameters — templates are extracted automatically
    results = await users.compiled_query(
        "find users older than 25 in the engineering department"
    )

    # The above might extract a template like:
    # "find users older than {age_0} in the {department_0} department"
    # Allowing future queries with different values to reuse the cached MQL
```

### TypeScript

```typescript
const users = client.db('myapp').collection('users');

// Natural language query
const results = await users.compiledQuery(
  'find active users who signed up this month'
);

// Parameterized — MongoCore extracts templates automatically
const results2 = await users.compiledQuery(
  'find products cheaper than $50 in the electronics category'
);
```

### Go

```go
users := client.Database("myapp").Collection("users")

results, err := users.CompiledQuery(ctx, "find active users who signed up this month")
if err != nil {
    log.Fatal(err)
}

for _, doc := range results {
    fmt.Println(doc)
}
```

### Java

```java
MongoCollection users = client.getDatabase("myapp").getCollection("users");

List<Document> results = users.compiledQuery(
    "find active users who signed up this month"
);
```

## Template Extraction

When a compiled query contains literal values (numbers, strings), MongoCore automatically extracts them into a parameterized template. This means:

```
Intent: "find items under $50"
Template: "find items under {price_0}"
Parameter: price_0 = 50 (type: Number)
```

Future queries like "find items under $100" match the same template and reuse the cached MQL structure, substituting the new value without calling the LLM again.

## Cache Behavior

| Level | Storage | Latency | Scope | Survives Restart |
|-------|---------|---------|-------|-----------------|
| L1 | In-memory (DashMap) | ~0ms | Single process | No |
| L2 | Disk (JSON files) | ~1ms | Single machine | Yes |
| L3 | Atlas collection | ~5-50ms | All instances | Yes |
| LLM | API call | ~500-2000ms | — | — |

### Cache Sync

When `compiled_cache_sync = true`, every new translation is stored in the `__mongocore.compiled_queries` collection on your Atlas cluster. Other MongoCore instances can read from this shared cache, meaning a query only needs LLM translation once across your entire fleet.

## Output Format

The LLM returns structured JSON that MongoCore parses into one of:

```rust
enum CompiledMql {
    Find { filter, options },
    Aggregate { pipeline },
}
```

This structured output is what gets cached and executed — not raw query strings.

## Error Handling

If the LLM is unavailable or returns unparseable output, MongoCore returns an error rather than executing a potentially incorrect query. The cache hierarchy means this only affects the first call for a given intent — cached queries always work regardless of LLM availability.
