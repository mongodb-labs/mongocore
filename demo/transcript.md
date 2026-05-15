# MongoCore Demo — Slide Key Points & Transcript

## Slide 1 — "What is MongoCore?"

> "MongoCore is a Rust sidecar that gives any language a MongoDB driver — Python, TypeScript, Go, Java — from one core. What makes it different is native AI agent support via MCP. Let me show you."

---

## Slide 2 — Ingest + Explain

> "Zero code written. Natural language → data in MongoDB. Polars under the hood — schema inference, type mapping, deduplication, dead letter queue. And here's the Python to reproduce it — generated from what just happened."

---

## Slide 3 — Natural Language Queries

> "Natural language to MongoDB Query Language. The generated MQL is cached as a parameterized template. Same template, different parameters. No LLM call. Sub-millisecond. Three cache levels: in-memory, disk, and MongoDB itself."

---

## Slide 4 — Pipelines

> "Thousands of updates in one round-trip. No chatty back-and-forth. Multi-step, atomic. Results flow between steps with template syntax. If any step fails — automatic rollback. No partial state."

---

## Slide 5 — ETL + Explain Session

> "Raw CSV → enriched MongoDB collection with one command. The transform computed profit on the fly — no post-processing needed. And here's the entire session as a Python script. Copy it, schedule it, run it when the data updates."

---

## Slide 6 — Closing

- **What we didn't show:**
  - Vector/semantic search with Voyage AI
  - 35+ MCP tools, 25+ gRPC RPCs
  - Multi-tenant isolation
  - OpenTelemetry tracing
  - 4 language clients
  - Directory watching with live re-ingestion

**The mic drop:**
> "Built by AI. In 5 days."

