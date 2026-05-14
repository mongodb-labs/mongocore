# Multi-Tenant Support

MongoCore supports running a single sidecar instance shared across multiple tenants. Each tenant gets isolated resources, rate limiting, and optional separate connection pools, making it safe to host multiple applications or customers on one MongoCore deployment.

## Overview

Multi-tenant mode allows you to:

- **Share a single sidecar** across multiple applications or customers
- **Isolate resources** with per-tenant connection pools, caches, and rate limits
- **Override connection URIs** to route tenants to different MongoDB clusters
- **Track usage** with per-tenant analytics and metrics
- **Control costs** by consolidating infrastructure

**Tenant identification** is done via the `x-tenant-id` gRPC metadata header. If no tenant ID is provided, the request uses the default (single-tenant) configuration.

## Configuration

Enable multi-tenant mode and define tenants in `config.toml`:

```toml
# config.toml

connection_uri = "mongodb://localhost:27017"  # Default URI
multi_tenant_enabled = true

[[tenants]]
tenant_id = "acme"
max_connections = 20
rate_limit_ops_per_sec = 500

[[tenants]]
tenant_id = "beta"
max_connections = 5
rate_limit_ops_per_sec = 100
connection_uri = "mongodb://beta-cluster:27017"  # Override connection

[[tenants]]
tenant_id = "gamma"
max_connections = 10
max_cache_entries = 1000
```

### Per-Tenant Options

| Option | Type | Required | Description |
|--------|------|----------|-------------|
| `tenant_id` | string | Yes | Unique tenant identifier |
| `max_connections` | integer | No | Connection pool size (default: global pool) |
| `max_cache_entries` | integer | No | Compiled query cache size (default: unlimited) |
| `rate_limit_ops_per_sec` | integer | No | Operations per second limit (default: no limit) |
| `connection_uri` | string | No | Override MongoDB URI (default: global URI) |

**Notes:**
- `tenant_id` must be unique across all tenants
- If `connection_uri` is not specified, tenant uses the global connection URI
- Rate limiting uses a sliding window counter with per-second reset
- Cache isolation prevents tenants from seeing each other's compiled queries

## Tenant Identification

Clients must send the `x-tenant-id` metadata header with every gRPC request:

### Python

```python
from mongocore import MongoClient

# Create client with tenant ID
client = MongoClient("localhost:50051", tenant_id="acme")

async with client:
    users = client["myapp"]["users"]
    await users.insert_one({"name": "Alice", "tenant": "acme"})
    docs = await users.find({"tenant": "acme"})
```

The tenant ID is automatically included in all gRPC calls.

### TypeScript

```typescript
import { MongoClient } from '@mongocore/client';

// Create client with tenant ID
const client = new MongoClient('localhost:50051', {
  tenantId: 'beta'
});

await client.connect();
const users = client.db('myapp').collection('users');
await users.insertOne({ name: 'Bob', tenant: 'beta' });
```

### Go

```go
import (
    "context"
    "github.com/rozza/mongocore/clients/go/mongocore"
)

// Create client with tenant ID
client := mongocore.NewClientWithTenant("localhost:50051", "gamma")
defer client.Disconnect(ctx)

users := client.Database("myapp").Collection("users")
users.InsertOne(ctx, bson.D{{Key: "name", Value: "Charlie"}, {Key: "tenant", Value: "gamma"}})
```

### Java

```java
import com.rozza.mongocore.MongoClient;
import com.rozza.mongocore.MongoClientOptions;

// Create client with tenant ID
MongoClientOptions options = MongoClientOptions.builder()
    .tenantId("acme")
    .build();

try (MongoClient client = MongoClient.create("localhost:50051", options)) {
    MongoCollection users = client.getDatabase("myapp").getCollection("users");
    users.insertOne(new Document("name", "Diana").append("tenant", "acme"));
}
```

## Isolation Guarantees

MongoCore provides the following isolation between tenants:

### 1. Connection Pool Isolation

When `max_connections` is specified, each tenant gets a dedicated connection pool:

```toml
[[tenants]]
tenant_id = "acme"
max_connections = 20  # Acme gets 20 connections

[[tenants]]
tenant_id = "beta"
max_connections = 5   # Beta gets 5 connections
```

**Benefits:**
- Prevents one tenant from exhausting all connections
- Ensures fair resource distribution
- Isolates connection errors (if Acme's pool is exhausted, Beta is unaffected)

**Without per-tenant pools:** All tenants share the global connection pool.

### 2. Compiled Query Cache Partitioning

Each tenant's compiled queries are stored in a separate cache partition:

```toml
[[tenants]]
tenant_id = "acme"
max_cache_entries = 1000  # Acme cache limited to 1000 entries

[[tenants]]
tenant_id = "beta"
max_cache_entries = 500   # Beta cache limited to 500 entries
```

**Benefits:**
- Prevents cache pollution (tenant A can't evict tenant B's cached queries)
- Security: tenants cannot see each other's natural language queries
- Fairness: no single tenant can dominate the cache

**Without cache partitioning:** All tenants share the global cache (single-tenant mode).

### 3. Rate Limiting

Tenants can be rate-limited to prevent abuse:

```toml
[[tenants]]
tenant_id = "beta"
rate_limit_ops_per_sec = 100  # Beta limited to 100 ops/sec
```

**How it works:**
- Sliding window counter with per-second reset
- Requests are rejected with `RESOURCE_EXHAUSTED` status when limit exceeded
- Rate limit applies to all gRPC operations (Find, Insert, Update, etc.)

**Without rate limiting:** No per-tenant quotas are enforced.

### 4. Connection URI Override

Tenants can connect to different MongoDB clusters:

```toml
[[tenants]]
tenant_id = "enterprise"
connection_uri = "mongodb://enterprise-cluster.example.com:27017"

[[tenants]]
tenant_id = "starter"
connection_uri = "mongodb://starter-cluster.example.com:27017"
```

**Use cases:**
- Multi-region deployments (tenant A in us-east, tenant B in eu-west)
- Data residency requirements (tenant data must stay in specific regions)
- Performance tiers (premium tenants on dedicated clusters)

**Without URI override:** All tenants connect to the global `connection_uri`.

### 5. Analytics Tracking

When multi-tenant mode is enabled, analytics events include the `tenant_id` field:

```javascript
{
  operation: "Find",
  database: "myapp",
  collection: "users",
  latency_ms: 15.3,
  success: true,
  tenant_id: "acme"  // Tenant identifier
}
```

This allows you to:
- Track per-tenant usage and costs
- Identify noisy neighbors
- Analyze tenant-specific performance

## Example Setup

Here's a complete multi-tenant configuration for a SaaS application:

```toml
# config.toml

# Global defaults
connection_uri = "mongodb://shared-cluster:27017"
grpc_port = 50051
mcp_port = 3000
log_level = "info"
compiled_cache_sync = true
analytics_enabled = true

# Enable multi-tenant mode
multi_tenant_enabled = true

# Free tier tenant (limited resources)
[[tenants]]
tenant_id = "free_user_1"
max_connections = 5
max_cache_entries = 100
rate_limit_ops_per_sec = 50

# Pro tier tenant (moderate resources)
[[tenants]]
tenant_id = "pro_user_1"
max_connections = 20
max_cache_entries = 500
rate_limit_ops_per_sec = 200

# Enterprise tier tenant (dedicated cluster)
[[tenants]]
tenant_id = "enterprise_corp"
max_connections = 50
max_cache_entries = 2000
rate_limit_ops_per_sec = 1000
connection_uri = "mongodb://enterprise-dedicated:27017"

# Development tenant (shared cluster, no limits)
[[tenants]]
tenant_id = "dev_sandbox"
max_connections = 10
```

## Usage Patterns

### Pattern 1: Shared Database, Tenant Field

All tenants use the same database/collection, with a `tenant` field to partition data:

```python
# Acme tenant
client = MongoClient("localhost:50051", tenant_id="acme")
users = client["shared"]["users"]
await users.insert_one({"name": "Alice", "tenant": "acme"})
await users.find({"tenant": "acme"})  # Only Acme's users

# Beta tenant
client = MongoClient("localhost:50051", tenant_id="beta")
users = client["shared"]["users"]
await users.insert_one({"name": "Bob", "tenant": "beta"})
await users.find({"tenant": "beta"})  # Only Beta's users
```

**Pros:**
- Simple to set up
- Easy to query across tenants (for admin purposes)

**Cons:**
- Requires discipline to always filter by `tenant`
- Risk of data leakage if filter is forgotten
- Not suitable for strict data isolation requirements

### Pattern 2: Separate Databases

Each tenant gets a dedicated database:

```python
# Acme tenant
client = MongoClient("localhost:50051", tenant_id="acme")
users = client["acme_db"]["users"]  # Acme's database
await users.insert_one({"name": "Alice"})

# Beta tenant
client = MongoClient("localhost:50051", tenant_id="beta")
users = client["beta_db"]["users"]  # Beta's database
await users.insert_one({"name": "Bob"})
```

**Pros:**
- Strong isolation (database-level ACLs possible)
- Easy to backup/restore per tenant
- Simpler queries (no tenant field required)

**Cons:**
- More MongoDB databases to manage
- Cannot easily query across tenants

### Pattern 3: Separate Clusters

Each tenant (or tenant tier) gets a separate MongoDB cluster:

```toml
[[tenants]]
tenant_id = "free_tier"
connection_uri = "mongodb://free-cluster:27017"

[[tenants]]
tenant_id = "enterprise_tier"
connection_uri = "mongodb://enterprise-cluster:27017"
```

**Pros:**
- Complete isolation (different hardware, regions, backup policies)
- Per-tenant scaling and tuning
- Meets data residency requirements

**Cons:**
- Higher infrastructure costs
- More operational complexity

## Rate Limiting

When a tenant exceeds their rate limit, requests are rejected:

```python
client = MongoClient("localhost:50051", tenant_id="beta")

try:
    await client["myapp"]["users"].find({})
except ResourceExhausted as e:
    print(e)  # "Rate limit exceeded for tenant 'beta'"
```

**Rate limit status:**
- gRPC status: `RESOURCE_EXHAUSTED`
- HTTP status (MCP): `429 Too Many Requests`

**Rate limit algorithm:**
- Sliding window counter with per-second reset
- Window size = 1 second
- Max operations per window = `rate_limit_ops_per_sec`
- Counter resets when a new second begins

**Example:** `rate_limit_ops_per_sec = 100`
- Tenant can do 100 operations per second sustained
- Counter resets each second window
- After hitting the limit, requests are rejected until the next window

## Monitoring & Analytics

Query per-tenant usage from the analytics collection:

```javascript
// Total operations per tenant
db.analytics.aggregate([
  { $match: { tenant_id: { $exists: true } } },
  { $group: {
    _id: "$tenant_id",
    total_ops: { $sum: 1 }
  }},
  { $sort: { total_ops: -1 } }
])

// Per-tenant error rates
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
  }},
  { $sort: { error_rate: -1 } }
])

// Per-tenant latency percentiles
db.analytics.aggregate([
  { $match: { tenant_id: "acme" } },
  { $group: {
    _id: null,
    latencies: { $push: "$latency_ms" }
  }},
  // ... compute percentiles in application code
])
```

## Security Considerations

### Tenant ID Validation

MongoCore does not validate tenant IDs. If a client sends `tenant_id = "unknown"`, the request will fail if no matching tenant is configured:

```toml
# Only "acme" and "beta" are valid
[[tenants]]
tenant_id = "acme"

[[tenants]]
tenant_id = "beta"
```

Requests with `tenant_id = "unknown"` will be rejected.

### Authentication

MongoCore does not authenticate tenants. You must implement authentication in your application layer:

1. Authenticate user (JWT, OAuth, etc.)
2. Extract tenant ID from user context
3. Pass tenant ID to MongoCore client

**Do not trust tenant IDs from client requests directly!** Always derive the tenant ID server-side.

### Data Isolation

MongoCore provides **resource isolation** (connection pools, caches, rate limits), not **data isolation**. To ensure tenants cannot access each other's data:

1. **Use separate databases** per tenant (Pattern 2 above)
2. **Use separate MongoDB clusters** per tenant (Pattern 3 above)
3. **Use MongoDB ACLs** to restrict database access
4. **Always filter by tenant ID** in queries (if using shared database)

MongoCore does not prevent a tenant from querying another tenant's database if they share a connection URI.

## Troubleshooting

### Tenant not found

```
Error: Unknown tenant 'xyz'
```

Solution: Add the tenant to `config.toml` and restart MongoCore.

### Rate limit exceeded

```
Status.RESOURCE_EXHAUSTED: Rate limit exceeded for tenant 'beta'
```

Solutions:
- Increase `rate_limit_ops_per_sec` for the tenant
- Implement client-side backoff/retry
- Optimize queries to reduce operation count

### Connection pool exhausted

```
Error: Connection pool exhausted for tenant 'acme'
```

Solutions:
- Increase `max_connections` for the tenant
- Investigate slow queries (check analytics)
- Add more MongoCore sidecar instances (load balance)

### High memory usage

If multi-tenant mode uses too much memory:

- Reduce `max_cache_entries` per tenant
- Reduce `analytics_buffer_size` (global setting)
- Disable analytics for specific tenants (not yet supported - use global setting)

## Migration Guide

### Single-Tenant to Multi-Tenant

1. Add `multi_tenant_enabled = true` to `config.toml`
2. Define tenants with `[[tenants]]` blocks
3. Update clients to send `x-tenant-id` metadata header
4. Test with one tenant before rolling out to all
5. Monitor analytics for per-tenant usage

### Multi-Tenant to Single-Tenant

1. Set `multi_tenant_enabled = false`
2. Remove `[[tenants]]` blocks from `config.toml`
3. Remove `tenant_id` from client configurations
4. Restart MongoCore (all requests use global pool)

## Related Documentation

- [Analytics](./analytics.md) - Per-tenant usage tracking
- [Getting Started](./getting-started.md) - Configuration and setup
- [Client Libraries](./client-libraries.md) - Language-specific tenant ID configuration
