# Query Analytics

MongoCore includes a built-in query analytics system that tracks operation performance, latency distributions, error rates, and usage patterns. This helps you identify slow queries, troubleshoot errors, and understand your application's MongoDB usage.

## Overview

Analytics are collected in real-time as operations flow through the sidecar. Events are stored in a memory-efficient ring buffer and periodically flushed to a MongoDB collection for persistent storage. You can query analytics via gRPC or the MCP server to get summary statistics.

**What's tracked:**
- Total operations and error counts
- Latency percentiles (p50, p95, p99)
- Top operations by frequency (Find, Insert, Update, etc.)
- Top collections by operation count
- Per-tenant metrics (when multi-tenant mode is enabled)

## Configuration

Analytics are controlled via the configuration file:

```toml
# config.toml

# Enable/disable analytics collection (default: true)
analytics_enabled = true

# Maximum events to buffer in memory (default: 10000)
analytics_buffer_size = 10000

# Flush interval in seconds (default: 300 = 5 minutes)
analytics_flush_interval_secs = 300
```

### Configuration Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `analytics_enabled` | boolean | `true` | Enable analytics collection |
| `analytics_buffer_size` | integer | `10000` | Ring buffer size (older events are dropped when full) |
| `analytics_flush_interval_secs` | integer | `300` | How often to persist events to MongoDB (seconds) |

**Notes:**
- Setting `analytics_enabled = false` disables collection entirely (zero overhead)
- Larger buffer sizes capture more events but use more memory
- Shorter flush intervals provide fresher data but increase write load
- Events are stored in `__mongocore.analytics` collection

## Available Metrics

### Summary Statistics

- **total_operations** - Total number of operations executed
- **total_errors** - Number of operations that failed
- **error_rate** - Percentage of failed operations (0.0 to 1.0)

### Latency Percentiles

All latencies are in milliseconds:

- **p50_latency_ms** - Median latency (50th percentile)
- **p95_latency_ms** - 95th percentile latency
- **p99_latency_ms** - 99th percentile latency

Higher percentiles reveal tail latency, which impacts user experience even if median latency is good.

### Top Operations

Ranked list of operation types by frequency:

```
[
  { operation: "Find", count: 1542 },
  { operation: "Insert", count: 823 },
  { operation: "Update", count: 412 },
  ...
]
```

**Operation types:**
- `Find`, `FindOne`, `Insert`, `InsertMany`, `Update`, `UpdateMany`
- `Delete`, `DeleteMany`, `FindAndModify`, `Aggregate`
- `Search`, `Watch`, `RunCommand`
- `BeginTransaction`, `CommitTransaction`, `AbortTransaction`
- `CreateCollection`, `CreateIndex`, `ListDatabases`, `ListCollections`

### Top Collections

Ranked list of collections by operation count (format: `database.collection`):

```
[
  { collection: "myapp.users", count: 2341 },
  { collection: "myapp.sessions", count: 1823 },
  { collection: "analytics.events", count: 945 },
  ...
]
```

## Querying Analytics

### gRPC API (All Languages)

Use the `GetAnalytics` RPC to retrieve current statistics:

#### Python

```python
from mongocore import MongoClient

async with MongoClient("localhost:50051") as client:
    analytics = await client.get_analytics()
    
    print(f"Total operations: {analytics.total_operations}")
    print(f"Error rate: {analytics.error_rate * 100:.2f}%")
    print(f"Median latency: {analytics.p50_latency_ms:.2f}ms")
    print(f"p95 latency: {analytics.p95_latency_ms:.2f}ms")
    print(f"p99 latency: {analytics.p99_latency_ms:.2f}ms")
    
    print("\nTop operations:")
    for op in analytics.top_operations:
        print(f"  {op.operation}: {op.count}")
    
    print("\nTop collections:")
    for coll in analytics.top_collections:
        print(f"  {coll.collection}: {coll.count}")
```

#### TypeScript

```typescript
import { MongoClient } from '@mongocore/client';

const client = new MongoClient('localhost:50051');
await client.connect();

const analytics = await client.getAnalytics();

console.log(`Total operations: ${analytics.totalOperations}`);
console.log(`Error rate: ${(analytics.errorRate * 100).toFixed(2)}%`);
console.log(`Median latency: ${analytics.p50LatencyMs.toFixed(2)}ms`);
console.log(`p95 latency: ${analytics.p95LatencyMs.toFixed(2)}ms`);
console.log(`p99 latency: ${analytics.p99LatencyMs.toFixed(2)}ms`);

console.log('\nTop operations:');
analytics.topOperations.forEach(op => {
  console.log(`  ${op.operation}: ${op.count}`);
});

console.log('\nTop collections:');
analytics.topCollections.forEach(coll => {
  console.log(`  ${coll.collection}: ${coll.count}`);
});
```

#### Go

```go
import (
    "context"
    "fmt"
    "github.com/rozza/mongocore/clients/go/mongocore"
)

client := mongocore.NewClient("localhost:50051")
defer client.Disconnect(ctx)

analytics, err := client.GetAnalytics(context.Background())
if err != nil {
    panic(err)
}

fmt.Printf("Total operations: %d\n", analytics.TotalOperations)
fmt.Printf("Error rate: %.2f%%\n", analytics.ErrorRate * 100)
fmt.Printf("Median latency: %.2fms\n", analytics.P50LatencyMs)
fmt.Printf("p95 latency: %.2fms\n", analytics.P95LatencyMs)
fmt.Printf("p99 latency: %.2fms\n", analytics.P99LatencyMs)

fmt.Println("\nTop operations:")
for _, op := range analytics.TopOperations {
    fmt.Printf("  %s: %d\n", op.Operation, op.Count)
}

fmt.Println("\nTop collections:")
for _, coll := range analytics.TopCollections {
    fmt.Printf("  %s: %d\n", coll.Collection, coll.Count)
}
```

#### Java

```java
import com.rozza.mongocore.MongoClient;
import com.rozza.mongocore.analytics.AnalyticsSummary;

try (MongoClient client = MongoClient.create("localhost:50051")) {
    AnalyticsSummary analytics = client.getAnalytics();
    
    System.out.println("Total operations: " + analytics.getTotalOperations());
    System.out.printf("Error rate: %.2f%%\n", analytics.getErrorRate() * 100);
    System.out.printf("Median latency: %.2fms\n", analytics.getP50LatencyMs());
    System.out.printf("p95 latency: %.2fms\n", analytics.getP95LatencyMs());
    System.out.printf("p99 latency: %.2fms\n", analytics.getP99LatencyMs());
    
    System.out.println("\nTop operations:");
    analytics.getTopOperations().forEach(op -> {
        System.out.printf("  %s: %d\n", op.getOperation(), op.getCount());
    });
    
    System.out.println("\nTop collections:");
    analytics.getTopCollections().forEach(coll -> {
        System.out.printf("  %s: %d\n", coll.getCollection(), coll.getCount());
    });
}
```

### MCP Server (AI Agents)

AI agents can use the `get_analytics` tool to retrieve analytics:

```json
{
  "name": "get_analytics",
  "arguments": {}
}
```

**Response:**

```json
{
  "total_operations": 5234,
  "total_errors": 42,
  "error_rate": 0.008,
  "p50_latency_ms": 12.5,
  "p95_latency_ms": 45.3,
  "p99_latency_ms": 89.7,
  "top_operations": [
    { "operation": "Find", "count": 3241 },
    { "operation": "Insert", "count": 1523 },
    { "operation": "Update", "count": 470 }
  ],
  "top_collections": [
    { "collection": "myapp.users", "count": 2341 },
    { "collection": "myapp.sessions", "count": 1823 },
    { "collection": "myapp.events", "count": 1070 }
  ]
}
```

## Persistence

Analytics events are automatically persisted to MongoDB for long-term storage and analysis.

**Collection:** `__mongocore.analytics`

**Schema:**

```javascript
{
  _id: ObjectId("..."),
  operation: "Find",
  database: "myapp",
  collection: "users",
  latency_ms: 15.3,
  success: true,
  timestamp: ISODate("2025-01-15T10:30:45.123Z"),
  fingerprint: "{age:Int32,email:String}",  // Query shape (optional)
  tenant_id: "acme",                         // Multi-tenant only (optional)
  document_count: 42                         // Result count (optional)
}
```

### Querying Historical Data

You can query the `__mongocore.analytics` collection directly:

```javascript
// Find all failed operations in the last hour
db.analytics.find({
  success: false,
  timestamp: { $gte: new Date(Date.now() - 3600000) }
})

// Calculate average latency by operation type
db.analytics.aggregate([
  { $group: {
    _id: "$operation",
    avg_latency: { $avg: "$latency_ms" },
    count: { $sum: 1 }
  }},
  { $sort: { avg_latency: -1 } }
])

// Top 10 slowest queries by fingerprint
db.analytics.aggregate([
  { $match: { operation: "Find", fingerprint: { $exists: true } } },
  { $group: {
    _id: "$fingerprint",
    avg_latency: { $avg: "$latency_ms" },
    count: { $sum: 1 }
  }},
  { $sort: { avg_latency: -1 } },
  { $limit: 10 }
])

// Per-tenant error rates (multi-tenant mode)
db.analytics.aggregate([
  { $match: { tenant_id: { $exists: true } } },
  { $group: {
    _id: "$tenant_id",
    total: { $sum: 1 },
    errors: { $sum: { $cond: ["$success", 0, 1] } }
  }},
  { $project: {
    tenant: "$_id",
    error_rate: { $divide: ["$errors", "$total"] }
  }}
])
```

## Interpreting Results

### Latency Percentiles

- **p50 (median)** - Half of operations complete faster, half slower
- **p95** - 95% of operations complete faster (only 5% are slower)
- **p99** - 99% of operations complete faster (only 1% are slower)

**Example interpretation:**

```
p50: 10ms   - Typical request is fast
p95: 50ms   - Most requests are fast
p99: 200ms  - Some requests are slow (investigate!)
```

If p99 is much higher than p50, you have a tail latency problem. Look for:
- Missing indexes
- Large result sets
- Complex aggregations
- Network issues
- Lock contention

### Error Rate

- **< 1%** - Healthy (errors are rare and expected)
- **1-5%** - Concerning (investigate error types)
- **> 5%** - Critical (likely a systemic issue)

High error rates indicate:
- Application bugs (invalid queries)
- Infrastructure issues (connection pool exhaustion)
- MongoDB problems (replica set down, disk full)
- Rate limiting (multi-tenant mode)

### Top Operations

Reveals your application's usage patterns:

- **High read volume** (Find, FindOne, Aggregate) - Consider caching
- **High write volume** (Insert, Update) - Ensure proper indexing
- **Many updates** - Check if you're doing partial updates vs. full replacements
- **Frequent transactions** - Transaction overhead can impact performance

### Top Collections

Identifies hot spots:

- **Uneven distribution** - Some collections dominate traffic
- **System collections** - `__mongocore.*` collections (compiled cache, analytics)
- **Unexpected collections** - Dead code or background jobs

## Overhead & Performance

Analytics collection is designed to be lightweight:

- **Memory:** Ring buffer holds last N events (default 10,000 ≈ 1-2MB RAM)
- **CPU:** Event recording is O(1), aggregation is O(N) on buffer size
- **Network:** Flush writes are batched (one write every 5 minutes by default)
- **Storage:** Events are compressed BSON documents

To minimize overhead:
- Set `analytics_enabled = false` if not needed
- Reduce `analytics_buffer_size` for memory-constrained environments
- Increase `analytics_flush_interval_secs` to reduce write load

## Multi-Tenant Analytics

When multi-tenant mode is enabled, analytics events include the `tenant_id` field. This allows you to track per-tenant usage and identify noisy neighbors.

```python
# Query per-tenant metrics from persisted analytics
pipeline = [
    {"$match": {"tenant_id": {"$exists": True}}},
    {"$group": {
        "_id": "$tenant_id",
        "total_ops": {"$sum": 1},
        "avg_latency": {"$avg": "$latency_ms"},
        "error_count": {"$sum": {"$cond": ["$success", 0, 1]}}
    }},
    {"$sort": {"total_ops": -1}}
]

results = await client["__mongocore"]["analytics"].aggregate(pipeline)
```

## Troubleshooting

### Analytics not available

If `get_analytics()` returns an error:

```
Status.UNAVAILABLE: Analytics not enabled
```

Check your config:

```toml
analytics_enabled = true  # Must be true
```

### Missing events

If recent operations don't appear in analytics:

1. Check buffer size - older events are dropped when buffer fills
2. Wait for flush interval - events are persisted every 5 minutes by default
3. Query the ring buffer snapshot (via `GetAnalytics` RPC) for real-time data

### High memory usage

If analytics is consuming too much memory:

```toml
analytics_buffer_size = 5000  # Reduce from default 10000
```

Or disable entirely:

```toml
analytics_enabled = false
```

## Related Documentation

- [Multi-Tenant](./multi-tenant.md) - Per-tenant analytics and isolation
- [Getting Started](./getting-started.md) - Configuration and setup
- [MCP Server](./mcp-server.md) - AI agent access to analytics
