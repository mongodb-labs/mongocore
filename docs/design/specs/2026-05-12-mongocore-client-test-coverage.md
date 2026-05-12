# MongoCore: Full Client Integration Test Coverage

## Overview

Add integration tests for all 27 gRPC RPCs to all 4 client libraries (Python, TypeScript, Go, Java), fix Java test output (currently silent), and add an AGENTS.md rule enforcing test coverage parity.

## Motivation

Currently each client tests 11 of 27 RPCs. The missing 16 operations have no client-level integration coverage — bugs in client wrappers for these methods would go undetected. All clients should test every RPC to ensure the thin wrapper layer works correctly end-to-end.

## Current State

**Tested (11):** Find, FindOne, Insert, InsertMany, Update, Delete, DeleteMany, Aggregate, Watch, Search, ListDatabases

**Missing (16):** UpdateMany, FindAndModify, BeginTransaction, CommitTransaction, AbortTransaction, CreateCollection, CreateIndex, ListCollections, RunCommand, GetAnalytics, Ingest, GetIngestStatus, ListIngestJobs, CancelIngest, WatchDirectory, StopWatch

## Design

### 1. Fix Java Test Output

Change justfile `test-java` recipe from:
```
cd clients/java && mvn test -Dtest=IntegrationTest -q
```
To:
```
cd clients/java && mvn test -Dtest=IntegrationTest
```

This produces per-test pass/fail output matching the other clients.

### 2. Shared Test Fixture

Create `clients/test_fixtures/sample.csv` for ingestion tests:
```csv
name,age,city
Alice,30,NYC
Bob,25,LA
Charlie,35,Chicago
```

All 4 clients reference this file by relative path during ingestion tests.

### 3. New Integration Tests (All Clients)

Each client adds these 16 tests to reach full 27-RPC coverage:

| Test Name | RPC | Behavior |
|-----------|-----|----------|
| `update_many` | UpdateMany | Insert 3 docs, update all matching a filter, verify modified count |
| `find_and_modify` | FindAndModify | Insert doc, find-and-update atomically, verify returned doc is the modified version |
| `list_collections` | ListCollections | Create a collection, verify it appears in the list |
| `create_collection` | CreateCollection | Create a new collection, verify success |
| `create_index` | CreateIndex | Create an index on a field, verify success |
| `run_command` | RunCommand | Execute `{ ping: 1 }`, verify `ok: 1.0` in result |
| `get_analytics` | GetAnalytics | Call get_analytics, verify response has total_operations field |
| `transaction_commit` | BeginTransaction + CommitTransaction | Begin txn, insert doc, commit, verify doc persisted |
| `transaction_abort` | BeginTransaction + AbortTransaction | Begin txn, insert doc, abort, verify doc NOT persisted |
| `ingest_csv` | Ingest | Ingest the shared CSV fixture, verify documents appear in collection |
| `ingest_status` | GetIngestStatus | Start an ingest, query its status, verify job_id and status fields |
| `list_ingest_jobs` | ListIngestJobs | After ingestion, list jobs, verify at least one returned |
| `cancel_ingest` | CancelIngest | Start an ingest, cancel it, verify cancellation acknowledged |
| `watch_directory` | WatchDirectory | Start a directory watch, verify watch_id returned |
| `stop_watch` | StopWatch | Start then stop a directory watch, verify success |

### 4. Test Isolation

Each test uses a unique collection name (e.g., `test_update_many_<uuid>` or language-specific prefix) to avoid interference between tests running in parallel or sequentially.

Ingestion tests use a dedicated database (e.g., `mongocore_ingest_test`) and clean up after themselves.

### 5. AGENTS.md Rule

Add to the existing "Don'ts" section:
```
- Don't add a gRPC RPC without adding integration tests for it in ALL 4 client test suites
```

And add a new "Testing Rules" section after "Don'ts":
```
## Testing Rules

- Every gRPC RPC must have a corresponding integration test in each client library (Python, TypeScript, Go, Java)
- Client integration tests must produce verbose per-test output (no silent/quiet modes)
- New tests should follow the existing pattern in each client's test file
- Use unique collection names per test to avoid interference
```

## Implementation Scope

| File | Change |
|------|--------|
| `justfile` | Remove `-q` from Java test recipe |
| `clients/test_fixtures/sample.csv` | Create shared CSV fixture |
| `clients/python/tests/test_integration.py` | Add 16 new test functions |
| `clients/typescript/tests/integration.test.ts` | Add 16 new test blocks |
| `clients/go/mongocore/integration_test.go` | Add 16 new test functions |
| `clients/java/src/test/java/com/mongocore/IntegrationTest.java` | Add 16 new test methods |
| `AGENTS.md` | Add testing rules section |

## Rust Unit Test Coverage Assessment

208 unit tests across all modules. Coverage is well-distributed:

**Well-covered:** analytics (19), compiled (34), config (7), connection (4), ingestion (55), mcp (26), operations (27), search (11), tenant (19), voyage (4)

**Modules without unit tests (appropriate):**
- `mod.rs` re-export files — no logic, just pub use
- `grpc::service`, `grpc::server` — RPC handlers are integration-tested
- `compiled::providers::{claude,openai}` — external API clients, integration-only
- `ingestion::engine` — orchestrator with DB deps, integration-tested
- `ingestion::types` — pure struct definitions with Default impls, no logic
- `ingestion::watch` — filesystem-dependent, integration-tested
- `analytics::persistence` — DB-dependent flush logic, integration-tested
- `voyage::batch` — HTTP-dependent batching, integration-tested
- `defaults`, `error` — constants and enum definitions

No new Rust unit tests are needed. The gaps are all in modules that require external dependencies (DB, filesystem, HTTP APIs) and are appropriately covered by integration tests.

## Won't Build

- No new client library methods (all RPCs already exposed)
- No proto changes
- No Rust sidecar changes

## Success Criteria

- [ ] `just test-java` produces verbose per-test output
- [ ] All 4 clients have 27 integration tests (one per RPC)
- [ ] All tests pass with `just test-clients`
- [ ] Shared CSV fixture exists at `clients/test_fixtures/sample.csv`
- [ ] AGENTS.md contains the testing rules section
- [ ] Test names are consistent across clients (same operations tested in same order)
