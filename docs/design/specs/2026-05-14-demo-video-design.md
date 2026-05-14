# MongoCore Demo Video — Design Spec

**Date:** 2026-05-14
**Format:** 3-minute video (slides + live demo clips via Claude Desktop MCP)
**Audience:** Mixed — MongoDB engineers + leadership/product
**Thesis:** "AI changes what a driver can be — and AI built this one in 4 days"

---

## Overview

A single continuous narrative filmed in Claude Desktop, showing one realistic workflow that naturally demonstrates four revolutionary features: data ingestion, natural language queries, request pipelines, and transactional pipelines. Bookended by brief slides that establish context and deliver the mic-drop.

The video is honest about capabilities and limitations. It shows real interactions, not mocked outputs.

---

## Structure

### Opening — "What is this?" (20 seconds)

**Format:** 1-2 slides + voiceover

**Slide 1:** Title card
> "MongoCore — an AI-native MongoDB driver, built by AI, in 4 days"

**Slide 2:** Architecture flash
```
App (any lang) ──gRPC──▶ MongoCore (Rust sidecar) ──▶ MongoDB
AI Agent ───────MCP────▶         │
                                 └──▶ LLM (Claude/OpenAI)
```

**Voiceover (paraphrased):**
> "MongoCore is a Rust sidecar that gives any language a MongoDB driver — Python, TypeScript, Go, Java — from one core. But what makes it different is native AI agent support via MCP. Let me show you what that means."

**Transition:** Cut to Claude Desktop, already connected to MongoCore via MCP.

---

### Act 1 — Ingestion: "Get data in" (40 seconds)

**Setup:** A movie dataset CSV from GitHub (~4800 rows with budget, revenue, genres, cast, director, ratings). The sample_mflix movies collection is already loaded.

**Dataset:** https://raw.githubusercontent.com/rashida048/Datasets/refs/heads/master/movie_dataset.csv

**Claude interaction:**
> User: "Ingest this CSV into MongoDB as a new collection called box_office: https://raw.githubusercontent.com/rashida048/Datasets/refs/heads/master/movie_dataset.csv"

**What happens (visible in Claude):**
- Claude calls the `ingest` MCP tool
- MongoCore's Polars engine reads the CSV, infers schema, maps types to BSON
- Data flows into MongoDB
- Claude reports success with row count and inferred schema

**Voiceover callouts:**
- "Polars under the hood — schema inference, type mapping, deduplication, dead letter queue for bad rows. All automatic."
- "Supports CSV, JSON, NDJSON, Parquet — from local files, HTTP URLs, or cloud storage."

**Key point:** Zero code written. Natural language → data in MongoDB.

---

### Act 2 — Natural Language Queries: "Ask questions" (45 seconds)

**Claude interaction #1:**
> User: "What are the top-rated sci-fi movies from the 1990s?"

**What happens:**
- Claude calls the `ask` MCP tool
- MongoCore translates NL → MQL (via LLM), executes, returns results
- Claude presents the answer naturally

**Voiceover callout:**
> "That query went through our compiled query engine — natural language to MongoDB Query Language. The generated MQL is now cached as a parameterized template."

**Claude interaction #2 (template reuse):**
> User: "What about horror movies from the 2000s?"

**What happens:**
- Template cache hit — no LLM call
- Sub-millisecond response

**Voiceover callout:**
> "Same template, different parameters. No LLM round-trip. Sub-millisecond. The cache has three levels: in-memory, disk, and MongoDB itself."

**Key point:** First query pays the LLM cost. Every variant after is near-instant.

---

### Act 3 — Request Pipelines: "Batch operations" (40 seconds)

**Claude interaction:**
> User: "For every movie in the box_office collection, set a field 'source' to 'box_office_import' and 'imported_at' to today's date. Do it in a single batch."

**What happens:**
- Claude calls the pipeline MCP tool
- MongoCore batches all update operations into a single gRPC round-trip
- Executes them concurrently on the server side
- Reports completion with count

**Voiceover callouts:**
- "That was [N] update operations in one round-trip. No chatty back-and-forth between client and database."
- "Pipelines can mix operation types — finds, updates, inserts, deletes — all in one batch with concurrent execution."

**Key point:** Eliminates network round-trip overhead for bulk operations.

---

### Act 4 — Transactional Pipelines: "Atomic multi-step workflows" (35 seconds)

**Claude interaction:**
> User: "Create a 'curated_picks' collection. Find the top 5 highest-rated movies, insert them into curated_picks with a 'featured: true' tag, and update the originals to mark them as 'featured_elsewhere: true'. If any step fails, roll everything back."

**What happens:**
- Claude calls the transactional pipeline MCP tool
- MongoCore runs a multi-step atomic pipeline:
  1. Create collection
  2. Find top 5 (result stored)
  3. Insert into curated_picks using `{{find_step.documents}}`
  4. Update originals using `{{find_step.documents._id}}`
- All wrapped in a transaction — automatic rollback on failure

**Voiceover callouts:**
- "Multi-step, atomic. Results flow between steps with template syntax."
- "If any step fails — automatic rollback. No partial state."

**Key point:** Complex workflows that would require manual transaction management — expressed in natural language.

---

### Closing — "What you just saw" (20 seconds)

**Format:** Slide or voiceover over final Claude screen

**The numbers (quick flash):**
- 35 MCP tools
- 25 gRPC RPCs
- 4 language clients (Python, TypeScript, Go, Java)
- 3-level query cache
- Real-time analytics dashboard
- Multi-tenant isolation

**The honesty beat:**
> "There's a trade-off. The sidecar adds milliseconds of latency compared to native drivers. In return, you get AI-native capabilities that are impossible in a traditional driver architecture."

**The closer:**
> "Built by AI. In 4 days."

End card.

---

## Skunkworks Presentation Rules

### Do

- **Show real interactions** — no mocked or faked outputs
- **Let impressive moments breathe** — don't talk over the magic
- **Be honest about limitations** — credibility is everything with engineers in the room
- **Keep energy conversational** — not salesy, not apologetic
- **End on "4 days"** — it's the mic drop
- **Use the same MongoDB instance throughout** — continuity sells the story
- **Show Claude thinking** — the MCP tool calls appearing is part of the wow factor

### Don't

- **Don't apologize for things that work** — confidence without arrogance
- **Don't cover every feature** — depth on 4 beats breadth on 12
- **Don't speed-talk to cram more in** — if it doesn't fit, cut it
- **Don't hide the overhead** — own it as a conscious trade-off
- **Don't over-edit** — slightly raw says "this is real"
- **Don't explain MCP protocol details** — show what it does, not how it works
- **Don't show code** — the whole point is you don't need code

---

## Pre-Recording Checklist

1. **MongoDB running** with sample_mflix loaded (`just docker-up`)
2. **MongoCore built** and configured as MCP server in Claude Desktop
3. **Dataset URL verified** — https://raw.githubusercontent.com/rashida048/Datasets/refs/heads/master/movie_dataset.csv (~4800 rows, budget/revenue/genres/cast/director)
4. **Claude Desktop connected** — verify MCP tools are listed
5. **Compiled query cache cleared** — so Act 2 shows a genuine cold → warm transition
6. **Screen recording configured** — capture Claude Desktop window only
7. **Dry run** — do the full flow once to check timing and catch errors

---

## Timing Budget

| Segment | Target | Max |
|---------|--------|-----|
| Opening slides | 20s | 25s |
| Act 1: Ingestion | 40s | 45s |
| Act 2: NL Queries | 45s | 50s |
| Act 3: Pipelines | 40s | 45s |
| Act 4: Transactional Pipelines | 35s | 40s |
| Closing | 20s | 25s |
| **Total** | **3:20** | **3:50** |

Note: Budget is slightly over 3 minutes to allow for cuts. Editing will tighten pauses and LLM wait times.

---

## Tooling Recommendations

| Tool | Purpose | Cost |
|------|---------|------|
| **OBS Studio** | Screen recording with region capture | Free |
| **ScreenFlow** | Record + edit in one (Mac) | $169 |
| **DaVinci Resolve** | Professional editing, transitions, text overlays | Free |
| **Keynote** | Intro/closing slides | Free (Mac) |
| **ElevenLabs** | AI voiceover from script | $5-22/mo |
| **PlayHT** | AI voiceover alternative | $31/mo |
| **Descript** | Edit video by editing transcript, remove pauses | $24/mo |
| **Claude** | Write and refine the voiceover script | Already here |

---

## Limitations to Acknowledge

- **Sidecar latency:** adds milliseconds vs native drivers (honest trade-off)
- **LLM dependency for NL queries:** first query requires LLM call (~500-2000ms), subsequent queries use cache
- **v0.1.0:** this is a skunkworks prototype, not production-shipped software
- **Single developer + AI:** built fast, not battle-tested at scale

---

## Success Criteria

The demo succeeds if the audience walks away thinking:
1. "AI fundamentally changes what a database driver can do"
2. "This is real — not a mockup or a future vision"
3. "One person + AI built this in 4 days — what could a team do?"
