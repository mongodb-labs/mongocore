# Demo Video Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **For implementers:** Read and follow `AGENTS.md` at the project root.
> Before committing: `cargo build` must produce ZERO warnings AND `cargo test --lib` must pass.

**Goal:** Produce 4 terminal GIFs demonstrating MongoCore features via Claude Code, and an HTML slide presentation embedding them.

**Architecture:** terminalizer records Claude Code sessions → renders to GIF → single-file HTML slide deck (html-slides) embeds the GIFs via URL references. A terminalizer config file ensures consistent terminal appearance across all recordings.

**Tech Stack:** terminalizer (npm), html-slides (single HTML file), Claude Code CLI, MongoCore MCP server

---

## File Structure

```
demo/
├── presentation.html           # Self-contained HTML slide deck
├── terminalizer.yml            # Shared terminalizer config (terminal size, theme, delays)
├── scripts/
│   ├── gif1-ingest-explain.md  # Exact prompts for GIF 1 recording
│   ├── gif2-nl-queries.md      # Exact prompts for GIF 2 recording
│   ├── gif3-pipelines.md       # Exact prompts for GIF 3 recording
│   └── gif4-etl-explain.md     # Exact prompts for GIF 4 recording
├── recordings/                 # terminalizer YAML recordings (gitignored)
└── gifs/                       # Rendered GIFs (gitignored)
```

---

### Task 1: Install and Configure terminalizer

**Files:**
- Create: `demo/terminalizer.yml`
- Create: `demo/.gitignore`

- [ ] **Step 1: Install terminalizer globally**

Run: `npm install -g terminalizer`
Expected: terminalizer command available

- [ ] **Step 2: Create shared terminalizer config**

```yaml
# demo/terminalizer.yml
# Shared config for all MongoCore demo GIF recordings

cols: 110
rows: 30

# Rendering
quality: 80
frameDelay: auto
maxIdleTime: 2000
frameBox:
  type: floating
  title: "MongoCore Demo"
  style:
    boxShadow: none
    margin: 0px

# Watermark (none)
watermark:
  imagePath: null

# Theme — dark terminal
theme:
  background: "#1e1e2e"
  foreground: "#cdd6f4"
  cursor: "#f5e0dc"
  black: "#45475a"
  red: "#f38ba8"
  green: "#a6e3a1"
  yellow: "#f9e2af"
  blue: "#89b4fa"
  magenta: "#f5c2e7"
  cyan: "#94e2d5"
  white: "#bac2de"
  brightBlack: "#585b70"
  brightRed: "#f38ba8"
  brightGreen: "#a6e3a1"
  brightYellow: "#f9e2af"
  brightBlue: "#89b4fa"
  brightMagenta: "#f5c2e7"
  brightCyan: "#94e2d5"
  brightWhite: "#a6adc8"
```

- [ ] **Step 3: Create .gitignore for recordings and rendered GIFs**

```gitignore
# demo/.gitignore
recordings/
gifs/
```

- [ ] **Step 4: Commit**

```bash
git add demo/terminalizer.yml demo/.gitignore
git commit -m "chore(demo): add terminalizer config and gitignore"
```

---

### Task 2: Write GIF Recording Scripts

**Files:**
- Create: `demo/scripts/gif1-ingest-explain.md`
- Create: `demo/scripts/gif2-nl-queries.md`
- Create: `demo/scripts/gif3-pipelines.md`
- Create: `demo/scripts/gif4-etl-explain.md`

- [ ] **Step 1: Create GIF 1 script (Ingest + Explain)**

```markdown
# GIF 1: Ingest + Explain Last

## Pre-conditions
- MongoDB running (`just docker-up`)
- MongoCore built and configured as MCP server in Claude Code
- No existing `movies` collection in the default database

## Recording

Start: `terminalizer record -c ../terminalizer.yml recordings/gif1-ingest-explain`

## Prompts

### Prompt 1: Ingest
```
Ingest this CSV into MongoDB as a new collection called movies: https://raw.githubusercontent.com/mongodb-labs/mongocore/datasets/demo/data/movies_dataset.csv
```

Wait for completion (expect ~4800 documents ingested message).

### Prompt 2: Explain
```
Show me the Python code to do that ingestion
```

Wait for `explain_last` output showing Python function.

## End

Stop recording. Render: `terminalizer render recordings/gif1-ingest-explain -o gifs/gif1-ingest-explain.gif`

## Expected Duration: ~30-35 seconds of content
```

- [ ] **Step 2: Create GIF 2 script (NL Queries)**

```markdown
# GIF 2: Natural Language Queries

## Pre-conditions
- MongoDB running with `movies` collection loaded (from GIF 1)
- Compiled query cache cleared (delete cache dir or restart MongoCore)
- LLM configured (Claude or OpenAI API key set)

## Recording

Start: `terminalizer record -c ../terminalizer.yml recordings/gif2-nl-queries`

## Prompts

### Prompt 1: Cold query (LLM call)
```
What are the top-rated sci-fi movies from the 1990s?
```

Wait for results. Note the response time (~1-2s for LLM translation + execution).

### Prompt 2: Warm query (cache hit)
```
What about horror movies from the 2000s?
```

Wait for results. Note the sub-second response (template cache hit, no LLM).

## End

Stop recording. Render: `terminalizer render recordings/gif2-nl-queries -o gifs/gif2-nl-queries.gif`

## Expected Duration: ~30-35 seconds of content
```

- [ ] **Step 3: Create GIF 3 script (Pipelines)**

```markdown
# GIF 3: Request + Transactional Pipelines

## Pre-conditions
- MongoDB running with `movies` collection loaded (from GIF 1)

## Recording

Start: `terminalizer record -c ../terminalizer.yml recordings/gif3-pipelines`

## Prompts

### Prompt 1: Request pipeline (batch update)
```
For every movie in the movies collection, set a field 'source' to 'movies_import' and 'imported_at' to today's date. Do it in a single batch.
```

Wait for completion (expect count of updated documents).

### Prompt 2: Transactional pipeline (atomic multi-step)
```
Create a 'curated_picks' collection. Find the top 5 highest-rated movies, insert them into curated_picks with a 'featured: true' tag, and update the originals to mark them as 'featured_elsewhere: true'. If any step fails, roll everything back.
```

Wait for completion (expect multi-step result with transaction confirmation).

## End

Stop recording. Render: `terminalizer render recordings/gif3-pipelines -o gifs/gif3-pipelines.gif`

## Expected Duration: ~35-40 seconds of content
```

- [ ] **Step 4: Create GIF 4 script (ETL + Explain Session)**

```markdown
# GIF 4: ETL + Explain Session

## Pre-conditions
- MongoDB running
- No existing `box_office` collection in the default database
- Fresh MCP session (restart Claude Code or clear session)

## Recording

Start: `terminalizer record -c ../terminalizer.yml recordings/gif4-etl-explain`

## Prompts

### Prompt 1: Ingest with transform
```
Ingest this CSV into the box_office collection: https://raw.githubusercontent.com/mongodb-labs/mongocore/datasets/demo/data/box_office.csv
Calculate a profit field as DomesticGross + ForeignGross - Budget.
```

Wait for completion (~970 documents ingested).

### Prompt 2: Create index
```
Create an index on genre and profit (descending) for the box_office collection.
```

Wait for index creation confirmation.

### Prompt 3: Verify query
```
What are the most profitable Action movies?
```

Wait for results showing movies with computed profit field.

### Prompt 4: Explain session
```
Show me the full Python script to reproduce everything I just did.
```

Wait for `explain_session` output showing complete Python script.

## End

Stop recording. Render: `terminalizer render recordings/gif4-etl-explain -o gifs/gif4-etl-explain.gif`

## Expected Duration: ~35-40 seconds of content
```

- [ ] **Step 5: Commit**

```bash
git add demo/scripts/
git commit -m "docs(demo): add recording scripts for all 4 GIFs"
```

---

### Task 3: Create HTML Slide Presentation

**Files:**
- Create: `demo/presentation.html`

- [ ] **Step 1: Create the HTML slide deck**

This is a self-contained HTML file using html-slides conventions. 6 slides total:

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>MongoCore — AI-Native MongoDB Driver</title>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', system-ui, sans-serif;
            background: #0f0f23;
            color: #cdd6f4;
            overflow: hidden;
            height: 100vh;
            width: 100vw;
        }
        .slide {
            display: none;
            width: 100vw;
            height: 100vh;
            padding: 60px 80px;
            flex-direction: column;
            justify-content: center;
        }
        .slide.active { display: flex; }
        .slide h1 {
            font-size: 3.2em;
            font-weight: 700;
            margin-bottom: 0.3em;
            color: #89b4fa;
        }
        .slide h2 {
            font-size: 2.2em;
            font-weight: 600;
            margin-bottom: 0.5em;
            color: #a6e3a1;
        }
        .slide p, .slide li {
            font-size: 1.4em;
            line-height: 1.6;
            color: #bac2de;
        }
        .slide ul { list-style: none; padding-left: 0; }
        .slide ul li::before {
            content: "→ ";
            color: #f9e2af;
        }
        .slide .subtitle {
            font-size: 1.6em;
            color: #f9e2af;
            margin-bottom: 1em;
        }
        .slide .architecture {
            font-family: 'JetBrains Mono', 'Fira Code', monospace;
            font-size: 1.1em;
            background: #1e1e2e;
            border: 1px solid #313244;
            border-radius: 12px;
            padding: 30px;
            margin: 20px 0;
            white-space: pre;
            line-height: 1.8;
            color: #94e2d5;
        }
        .slide .gif-container {
            display: flex;
            justify-content: center;
            align-items: center;
            flex: 1;
            margin-top: 20px;
        }
        .slide .gif-container img {
            max-width: 95%;
            max-height: 70vh;
            border-radius: 12px;
            border: 2px solid #313244;
            box-shadow: 0 20px 60px rgba(0,0,0,0.5);
        }
        .slide .stats {
            display: grid;
            grid-template-columns: repeat(3, 1fr);
            gap: 20px;
            margin-top: 30px;
        }
        .slide .stat {
            background: #1e1e2e;
            border: 1px solid #313244;
            border-radius: 12px;
            padding: 20px;
            text-align: center;
        }
        .slide .stat .number {
            font-size: 2em;
            font-weight: 700;
            color: #89b4fa;
        }
        .slide .stat .label {
            font-size: 1em;
            color: #6c7086;
            margin-top: 5px;
        }
        .slide .closer {
            font-size: 2.8em;
            font-weight: 700;
            color: #f9e2af;
            text-align: center;
            margin-top: 40px;
        }
        .slide-counter {
            position: fixed;
            bottom: 20px;
            right: 30px;
            font-size: 0.9em;
            color: #585b70;
        }
        .nav-hint {
            position: fixed;
            bottom: 20px;
            left: 30px;
            font-size: 0.8em;
            color: #45475a;
        }
    </style>
</head>
<body>

<!-- Slide 1: Introduction -->
<div class="slide active" id="slide-1">
    <h1>MongoCore</h1>
    <p class="subtitle">An AI-native MongoDB driver — built by AI, in 4 days</p>
    <div class="architecture">App (any lang) ──gRPC──▶ MongoCore (Rust sidecar) ──▶ MongoDB
AI Agent ───────MCP────▶         │
                                 └──▶ LLM (Claude/OpenAI)</div>
    <div class="stats">
        <div class="stat"><div class="number">Rust</div><div class="label">Core sidecar</div></div>
        <div class="stat"><div class="number">4</div><div class="label">Language clients</div></div>
        <div class="stat"><div class="number">MCP + gRPC</div><div class="label">Dual protocol</div></div>
        <div class="stat"><div class="number">NL→MQL</div><div class="label">Query engine</div></div>
        <div class="stat"><div class="number">35+</div><div class="label">MCP tools</div></div>
        <div class="stat"><div class="number">25+</div><div class="label">gRPC RPCs</div></div>
    </div>
</div>

<!-- Slide 2: GIF 1 — Ingest + Explain -->
<div class="slide" id="slide-2">
    <h2>Data Ingestion + Explain</h2>
    <p>Natural language → data in MongoDB → Python code to reproduce it</p>
    <div class="gif-container">
        <img src="gifs/gif1-ingest-explain.gif" alt="Demo: Ingest CSV and explain as Python code">
    </div>
</div>

<!-- Slide 3: GIF 2 — Natural Language Queries -->
<div class="slide" id="slide-3">
    <h2>Natural Language Queries</h2>
    <p>NL → MQL compilation with 3-level caching (cold then warm)</p>
    <div class="gif-container">
        <img src="gifs/gif2-nl-queries.gif" alt="Demo: Natural language queries with cache">
    </div>
</div>

<!-- Slide 4: GIF 3 — Pipelines -->
<div class="slide" id="slide-4">
    <h2>Pipelines: Batch + Transactional</h2>
    <p>Batch operations in one round-trip. Multi-step atomic workflows with rollback.</p>
    <div class="gif-container">
        <img src="gifs/gif3-pipelines.gif" alt="Demo: Request and transactional pipelines">
    </div>
</div>

<!-- Slide 5: GIF 4 — ETL + Explain Session -->
<div class="slide" id="slide-5">
    <h2>ETL Pipeline + Explain Session</h2>
    <p>Ingest with transforms → index → query → full Python script</p>
    <div class="gif-container">
        <img src="gifs/gif4-etl-explain.gif" alt="Demo: ETL pipeline and explain session">
    </div>
</div>

<!-- Slide 6: Closing -->
<div class="slide" id="slide-6">
    <h2>What We Didn't Cover</h2>
    <ul>
        <li>Vector/semantic search with Voyage AI embeddings</li>
        <li>Real-time analytics dashboard (web UI)</li>
        <li>Multi-tenant isolation and quotas</li>
        <li>Directory watching with live re-ingestion</li>
        <li>OpenTelemetry tracing</li>
    </ul>
    <div class="closer">Built by AI. In 4 days.</div>
</div>

<div class="slide-counter"></div>
<div class="nav-hint">← → arrow keys to navigate</div>

<script>
    const slides = document.querySelectorAll('.slide');
    const counter = document.querySelector('.slide-counter');
    let current = 0;

    function showSlide(n) {
        slides[current].classList.remove('active');
        current = (n + slides.length) % slides.length;
        slides[current].classList.add('active');
        counter.textContent = `${current + 1} / ${slides.length}`;
    }

    document.addEventListener('keydown', (e) => {
        if (e.key === 'ArrowRight' || e.key === ' ') showSlide(current + 1);
        if (e.key === 'ArrowLeft') showSlide(current - 1);
    });

    // Touch support
    let touchStartX = 0;
    document.addEventListener('touchstart', (e) => { touchStartX = e.touches[0].clientX; });
    document.addEventListener('touchend', (e) => {
        const diff = touchStartX - e.changedTouches[0].clientX;
        if (Math.abs(diff) > 50) {
            diff > 0 ? showSlide(current + 1) : showSlide(current - 1);
        }
    });

    showSlide(0);
</script>
</body>
</html>
```

- [ ] **Step 2: Verify presentation opens in browser**

Run: `open demo/presentation.html`
Expected: Dark-themed slide deck with arrow key navigation, 6 slides, GIF placeholders (broken images until GIFs exist)

- [ ] **Step 3: Commit**

```bash
git add demo/presentation.html
git commit -m "feat(demo): add HTML slide presentation"
```

---

### Task 4: Dry Run and Record GIFs

This task is manual — performed by the presenter, not automated.

**Pre-conditions:**
- Tasks 1-3 complete and committed
- MongoDB running (`just docker-up`)
- MongoCore built (`cargo build --release`)
- Claude Code configured with MongoCore MCP server
- LLM API key configured for compiled queries

- [ ] **Step 1: Clean slate**

```bash
# Drop any existing demo collections
mongosh --eval "db.getSiblingDB('test').movies.drop(); db.getSiblingDB('test').box_office.drop(); db.getSiblingDB('test').curated_picks.drop()"
```

- [ ] **Step 2: Record GIF 1 (Ingest + Explain)**

Follow `demo/scripts/gif1-ingest-explain.md` exactly.

```bash
cd demo
terminalizer record -c terminalizer.yml recordings/gif1-ingest-explain
# ... run prompts in Claude Code ...
# Ctrl+D to stop
terminalizer render recordings/gif1-ingest-explain -o gifs/gif1-ingest-explain.gif
```

Review: GIF should show ingestion success + Python code output. Target ~30-35s.

- [ ] **Step 3: Record GIF 2 (NL Queries)**

Clear compiled query cache (restart MongoCore), then follow `demo/scripts/gif2-nl-queries.md`.

```bash
terminalizer record -c terminalizer.yml recordings/gif2-nl-queries
# ... run prompts ...
terminalizer render recordings/gif2-nl-queries -o gifs/gif2-nl-queries.gif
```

Review: Should show visible time difference between cold and warm queries. Target ~30-35s.

- [ ] **Step 4: Record GIF 3 (Pipelines)**

Follow `demo/scripts/gif3-pipelines.md`.

```bash
terminalizer record -c terminalizer.yml recordings/gif3-pipelines
# ... run prompts ...
terminalizer render recordings/gif3-pipelines -o gifs/gif3-pipelines.gif
```

Review: Should show batch count + transactional multi-step result. Target ~35-40s.

- [ ] **Step 5: Record GIF 4 (ETL + Explain Session)**

Start a fresh Claude Code session, then follow `demo/scripts/gif4-etl-explain.md`.

```bash
terminalizer record -c terminalizer.yml recordings/gif4-etl-explain
# ... run prompts ...
terminalizer render recordings/gif4-etl-explain -o gifs/gif4-etl-explain.gif
```

Review: Should show full ETL flow + complete Python script. Target ~35-40s.

- [ ] **Step 6: Verify presentation with GIFs**

```bash
open demo/presentation.html
```

Navigate through all 6 slides. Verify:
- GIFs load and play on slides 2-5
- GIFs are readable at slide resolution
- Timing feels right for narration

- [ ] **Step 7: Re-record any GIFs that need improvement**

Common issues:
- LLM response too slow → re-record with warm cache (except GIF 2 cold demo)
- Terminal output too long → adjust terminalizer `maxIdleTime`
- Text too small → increase terminalizer `cols`/`rows` or font size

---

### Task 5: Record Narration and Produce Final Video

This task is manual — performed by the presenter.

- [ ] **Step 1: Write narration script**

Use the voiceover callouts from the design spec as a starting point. Time each section against the GIF duration.

- [ ] **Step 2: Record narration**

Options:
- **Live:** Record while presenting the slides
- **AI voice:** Use ElevenLabs or PlayHT with the script

- [ ] **Step 3: Screen-record the final presentation**

```bash
# Open presentation
open demo/presentation.html
# Start screen recording (OBS, ScreenFlow, or macOS built-in)
# Navigate slides while narration plays
# Stop recording
```

- [ ] **Step 4: Edit and export**

Trim dead time, ensure total is under 3 minutes. Export as MP4.
