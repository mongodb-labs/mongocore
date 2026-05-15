# MongoCore Demo Video — Design Spec

**Date:** 2026-05-14 (updated 2026-05-15)
**Format:** HTML slides (bluedusk/html-slides) with embedded terminalizer GIFs, narrated (live or AI voice)
**Duration:** Max 3 minutes
**Audience:** Mixed — MongoDB engineers + leadership/product
**Thesis:** "AI changes what a driver can be — and AI built this one in 5 days"
**Recording tool:** [terminalizer](https://github.com/faressoft/terminalizer) — terminal → GIF
**Presentation framework:** [html-slides](https://github.com/bluedusk/html-slides)
**Demo tool:** Claude Code CLI with MongoCore as MCP server

---

## Overview

A series of focused terminal GIFs, each demonstrating a single MongoCore capability via Claude Code. Embedded in an HTML slide deck with intro/closing slides providing context. The `explain` feature bridges each demo to production code — "here's what you just saw, now here's the Python to do it yourself."

---

## Slide Structure

### Slide 1 — Introduction: "What is MongoCore?" (15 seconds)

**Content:**
- One-liner: "An AI-native MongoDB driver — built by AI, in 5 days"
- Architecture diagram:

```
App (any lang) ──gRPC──▶ MongoCore (Rust sidecar) ──▶ MongoDB
AI Agent ───────MCP────▶         │
                                 └──▶ LLM (Claude/OpenAI)
```

- Key stats: Rust sidecar, 4 language clients, MCP + gRPC, NL→MQL query engine

**Voiceover:**
> "MongoCore is a Rust sidecar that gives any language a MongoDB driver — Python, TypeScript, Go, Java — from one core. What makes it different is native AI agent support via MCP. Let me show you."

---

### Slide 2 — GIF 1: Ingest + Explain (35 seconds)

**Dataset:** https://raw.githubusercontent.com/mongodb-labs/mongocore/datasets/demo/data/movies_dataset.csv (~4800 rows)

**Claude Code interaction:**

```
> Ingest this CSV into MongoDB as a new collection called movies:
  https://raw.githubusercontent.com/mongodb-labs/mongocore/datasets/demo/data/movies_dataset.csv
```

MongoCore ingests via Polars engine — schema inference, type mapping, bulk write.

```
> Show me the Python code to do that ingestion
```

Claude calls `explain_last` → displays parameterized Python function using the MongoCore client.

**Voiceover callouts:**
- "Zero code written. Natural language → data in MongoDB."
- "Polars under the hood — schema inference, type mapping, deduplication, dead letter queue."
- "And here's the Python to reproduce it — generated from what just happened."

---

### Slide 3 — GIF 2: Natural Language Queries (35 seconds)

**Claude Code interaction #1 (cold — LLM call):**

```
> What are the top-rated sci-fi movies from the 1990s?
```

MongoCore's compiled query engine translates NL → MQL via LLM, executes, returns results.

**Claude Code interaction #2 (warm — cache hit):**

```
> What about horror movies from the 2000s?
```

Template cache hit — no LLM round-trip, sub-millisecond response.

**Voiceover callouts:**
- "Natural language to MongoDB Query Language. The generated MQL is cached as a parameterized template."
- "Same template, different parameters. No LLM call. Sub-millisecond."
- "Three cache levels: in-memory, disk, and MongoDB itself."

---

### Slide 4 — GIF 3: Pipelines (40 seconds)

**Request pipeline interaction:**

```
> For every movie in the movies collection, set a field 'source' to 'movies_import'
  and 'imported_at' to today's date. Do it in a single batch.
```

MongoCore batches all operations into one gRPC round-trip, executes concurrently.

**Transactional pipeline interaction:**

```
> Create a 'curated_picks' collection. Find the top 5 highest-rated movies, insert them
  into curated_picks with a 'featured: true' tag, and update the originals to mark them
  as 'featured_elsewhere: true'. If any step fails, roll everything back.
```

Multi-step atomic pipeline — results flow between steps, automatic rollback on failure.

**Voiceover callouts:**
- "Thousands of updates in one round-trip. No chatty back-and-forth."
- "Multi-step, atomic. Results flow between steps with template syntax."
- "If any step fails — automatic rollback. No partial state."

---

### Slide 5 — GIF 4: ETL + Explain Session (40 seconds)

**Dataset:** https://raw.githubusercontent.com/mongodb-labs/mongocore/datasets/demo/data/box_office.csv (~970 rows)

**Fields kept:** Movie, LeadStudio, RottenTomatoes, AudienceScore, Genre, DomesticGross, ForeignGross, Budget, Year

**Claude Code interaction:**

```
> Ingest this CSV into the box_office collection:
  https://raw.githubusercontent.com/mongodb-labs/mongocore/datasets/demo/data/box_office.csv
  Calculate a profit field as DomesticGross + ForeignGross - Budget.
```

MongoCore ingests with transform expression to compute the derived field.

```
> Create an index on genre and profit (descending) for that collection.
```

```
> What are the most profitable Action movies?
```

Quick verification query showing the ETL worked.

```
> Show me the full Python script to reproduce everything I just did.
```

Claude calls `explain_session` → generates a complete, runnable Python script with parameterized functions for each step and a `main()` entry point.

**Voiceover callouts:**
- "Raw CSV → enriched MongoDB collection with one command."
- "The transform computed profit on the fly — no post-processing needed."
- "And here's the entire session as a Python script. Copy it, schedule it, run it when the data updates."

---

### Slide 6 — Closing: "What We Didn't Cover" (15 seconds)

**Content — features not shown:**
- Vector/semantic search with Voyage AI embeddings
- Real-time analytics dashboard (web UI)
- Multi-tenant isolation and quotas
- 4 language clients (Python, TypeScript, Go, Java)
- Directory watching with live re-ingestion
- OpenTelemetry tracing
- 35+ MCP tools, 25+ gRPC RPCs

**The closer:**
> "Built by AI. In 5 days."

End card.

---

## Datasets

### Main Demo (GIFs 1-3)

**Source:** https://raw.githubusercontent.com/mongodb-labs/mongocore/datasets/demo/data/movies_dataset.csv
**Rows:** ~4800
**Fields:** budget, revenue, genres, cast, director, ratings, etc.
**Collection:** `movies`

### ETL Demo (GIF 4)

**Source:** https://raw.githubusercontent.com/mongodb-labs/mongocore/datasets/demo/data/box_office.csv
**Rows:** ~970
**Fields:** Movie, LeadStudio, RottenTomatoes, AudienceScore, Genre, DomesticGross, ForeignGross, Budget, Year
**Derived field:** `profit = DomesticGross + ForeignGross - Budget`
**Collection:** `box_office`
**Index:** `{"genre": 1, "profit": -1}`

---

## Timing Budget

| Slide | Target | Max |
|-------|--------|-----|
| Intro | 15s | 20s |
| GIF 1: Ingest + explain_last | 35s | 40s |
| GIF 2: NL Queries | 35s | 40s |
| GIF 3: Pipelines | 40s | 45s |
| GIF 4: ETL + explain_session | 40s | 45s |
| Closing | 15s | 20s |
| **Total** | **3:00** | **3:30** |

---

## Recording Setup

### Prerequisites

1. **MongoDB running** with sample data (`just docker-up`)
2. **MongoCore built** (`cargo build --release`)
3. **Claude Code MCP config** pointing to MongoCore:
   ```json
   {
     "mcpServers": {
       "mongocore": {
         "command": "./target/release/mongocore",
         "args": ["--stdio", "--connection-uri", "mongodb://localhost:27017"],
         "env": {
           "MONGOCORE_LOG_LEVEL": "warn"
         }
       }
     }
   }
   ```
4. **terminalizer installed** (`npm install -g terminalizer`)
5. **Dataset URLs verified** (both accessible from datasets branch)
6. **Compiled query cache cleared** for GIF 2 (cold → warm transition)

### Recording Workflow

For each GIF:
1. `terminalizer record <gif-name>` — start recording
2. Run the Claude Code interaction
3. `terminalizer stop` — end recording
4. `terminalizer render <gif-name>` — generate GIF
5. Review timing, re-record if needed

### terminalizer Config Tips

- Set `cols` / `rows` to a consistent terminal size across all GIFs
- Use a clean prompt (minimal PS1)
- Set reasonable `frameDelay` to keep GIFs smooth but not too large
- Consider `quality` setting to balance file size vs clarity

---

## Presentation Assembly

Using [html-slides](https://github.com/bluedusk/html-slides):
- Each slide embeds its GIF with `<img>` tag
- Intro and closing slides are text/diagram only
- Narration recorded separately (live or AI-generated via ElevenLabs/PlayHT)
- Final video: screen-record the slideshow with narration playing

---

## Skunkworks Presentation Rules

### Do

- **Show real interactions** — no mocked or faked outputs
- **Let impressive moments breathe** — don't talk over the magic
- **Be honest about limitations** — credibility is everything with engineers in the room
- **Keep energy conversational** — not salesy, not apologetic
- **End on "5 days"** — it's the mic drop
- **Use the same MongoDB instance throughout** — continuity sells the story
- **Show the explain output** — the Python code appearing is the "bridge to production" moment

### Don't

- **Don't apologize for things that work** — confidence without arrogance
- **Don't cover every feature** — depth on 4 beats breadth on 12
- **Don't speed-talk to cram more in** — if it doesn't fit, cut it
- **Don't hide the overhead** — own it as a conscious trade-off
- **Don't over-edit** — slightly raw says "this is real"
- **Don't explain MCP protocol details** — show what it does, not how it works
- **Don't show code** (except explain output) — the whole point is you don't need code

---

## Pre-Recording Checklist

- [ ] MongoDB running and accessible
- [ ] MongoCore release binary built and configured as MCP server
- [ ] Dataset URLs verified (movies_dataset.csv and box_office.csv accessible from datasets branch)
- [ ] Claude Code connected — MCP tools listed
- [ ] Compiled query cache cleared for cold demo
- [ ] terminalizer installed and configured (consistent terminal size)
- [ ] Dry run each GIF — check timing and catch errors
- [ ] Terminal font/theme readable at GIF resolution

---

## Limitations to Acknowledge

- **Sidecar latency:** adds milliseconds vs native drivers (honest trade-off)
- **LLM dependency for NL queries:** first query requires LLM call (~500-2000ms), subsequent use cache
- **v0.1.0:** skunkworks prototype, not production-shipped software
- **Single developer + AI:** built fast, not battle-tested at scale

---

## Success Criteria

The demo succeeds if the audience walks away thinking:
1. "AI fundamentally changes what a database driver can do"
2. "This is real — not a mockup or a future vision"
3. "The explain feature bridges interactive AI → production code"
4. "One person + AI built this in 5 days — what could a team do?"
