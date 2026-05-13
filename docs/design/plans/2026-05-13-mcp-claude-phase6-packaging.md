# MCP + Claude Integration — Phase 6: Packaging & Distribution

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **For implementers:** Read and follow `AGENTS.md` at the project root.
> Before committing: `cargo build` must produce ZERO warnings AND `cargo test --lib` must pass.

**Goal:** Package MongoCore for distribution via GitHub Releases (cross-platform binaries), Homebrew, and `cargo install`. Create getting-started documentation for MCP users.

**Architecture:** GitHub Actions workflow builds release binaries for macOS (arm64, x86_64) and Linux (x86_64, arm64). A Homebrew formula downloads from GitHub Releases. Documentation provides copy-paste MCP config for Claude Desktop and Claude Code.

**Tech Stack:** GitHub Actions, cross-compilation (cross-rs or native runners), Homebrew formula DSL, Markdown docs.

**Depends on:** All prior phases (the binary must include all MCP features).

---

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `.github/workflows/release.yml` | Create | Cross-platform build + GitHub Release upload |
| `Formula/mongocore.rb` | Create | Homebrew formula |
| `docs/mcp-setup.md` | Create | Getting-started guide for MCP users |
| `Cargo.toml` | Modify | Ensure `[[bin]]` section, add metadata for crates.io |

---

### Task 1: Create GitHub Actions release workflow

**Files:**
- Create: `.github/workflows/release.yml`

- [ ] **Step 1: Write the release workflow**

Create `.github/workflows/release.yml`:

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

permissions:
  contents: write

jobs:
  build:
    strategy:
      matrix:
        include:
          - os: macos-14
            target: aarch64-apple-darwin
            artifact: mongocore-darwin-arm64
          - os: macos-13
            target: x86_64-apple-darwin
            artifact: mongocore-darwin-x86_64
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            artifact: mongocore-linux-x86_64
          - os: ubuntu-latest
            target: aarch64-unknown-linux-gnu
            artifact: mongocore-linux-arm64
            cross: true

    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Install cross (for ARM Linux)
        if: matrix.cross
        run: cargo install cross

      - name: Build
        run: |
          if [ "${{ matrix.cross }}" = "true" ]; then
            cross build --release --target ${{ matrix.target }}
          else
            cargo build --release --target ${{ matrix.target }}
          fi

      - name: Package binary
        run: |
          mkdir -p dist
          cp target/${{ matrix.target }}/release/mongocore dist/${{ matrix.artifact }}
          chmod +x dist/${{ matrix.artifact }}
          tar -czf dist/${{ matrix.artifact }}.tar.gz -C dist ${{ matrix.artifact }}

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: ${{ matrix.artifact }}
          path: dist/${{ matrix.artifact }}.tar.gz

  release:
    needs: build
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Download all artifacts
        uses: actions/download-artifact@v4
        with:
          path: artifacts

      - name: Create GitHub Release
        uses: softprops/action-gh-release@v1
        with:
          files: artifacts/**/*.tar.gz
          generate_release_notes: true
```

- [ ] **Step 2: Commit**

```bash
mkdir -p .github/workflows
git add .github/workflows/release.yml
git commit -m "ci: add cross-platform release workflow for GitHub Releases"
```

---

### Task 2: Create Homebrew formula

**Files:**
- Create: `Formula/mongocore.rb`

- [ ] **Step 1: Write the formula**

Create `Formula/mongocore.rb`:

```ruby
class Mongocore < Formula
  desc "AI-native MongoDB driver sidecar with MCP support"
  homepage "https://github.com/mongodb/mongocore"
  version "0.8.0"

  on_macos do
    on_arm do
      url "https://github.com/mongodb/mongocore/releases/download/v#{version}/mongocore-darwin-arm64.tar.gz"
      # sha256 will be filled after first release
    end
    on_intel do
      url "https://github.com/mongodb/mongocore/releases/download/v#{version}/mongocore-darwin-x86_64.tar.gz"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/mongodb/mongocore/releases/download/v#{version}/mongocore-linux-arm64.tar.gz"
    end
    on_intel do
      url "https://github.com/mongodb/mongocore/releases/download/v#{version}/mongocore-linux-x86_64.tar.gz"
    end
  end

  def install
    bin.install "mongocore"
  end

  test do
    assert_match "mongocore", shell_output("#{bin}/mongocore --help")
  end
end
```

- [ ] **Step 2: Commit**

```bash
mkdir -p Formula
git add Formula/mongocore.rb
git commit -m "chore: add Homebrew formula for mongocore"
```

---

### Task 3: Create MCP setup documentation

**Files:**
- Create: `docs/mcp-setup.md`

- [ ] **Step 1: Write the getting-started guide**

Create `docs/mcp-setup.md`:

```markdown
# MongoCore MCP Setup

Use MongoCore as an MCP server with Claude Desktop or Claude Code to query, explore, and generate code for your MongoDB databases.

## Installation

**Homebrew (macOS):**
```bash
brew install mongocore
```

**GitHub Releases (all platforms):**
Download from [Releases](https://github.com/mongodb/mongocore/releases) and add to your PATH.

**From source:**
```bash
cargo install mongocore
# or: git clone + cargo build --release
```

## Claude Desktop Configuration

Add to your Claude Desktop MCP settings (`~/Library/Application Support/Claude/claude_desktop_config.json`):

**Minimal (local MongoDB):**
```json
{
  "mcpServers": {
    "mongocore": {
      "command": "mongocore",
      "args": ["--stdio"]
    }
  }
}
```

**With connection string:**
```json
{
  "mcpServers": {
    "mongocore": {
      "command": "mongocore",
      "args": ["--stdio", "--connection-uri", "mongodb+srv://user:pass@cluster.mongodb.net/mydb"]
    }
  }
}
```

**With embedding support (Voyage AI):**
```json
{
  "mcpServers": {
    "mongocore": {
      "command": "mongocore",
      "args": ["--stdio", "--connection-uri", "mongodb://localhost:27017"],
      "env": {
        "VOYAGE_API_KEY": "your-voyage-api-key"
      }
    }
  }
}
```

## Claude Code Configuration

Add to your project's `.claude/settings.json` or global settings:

```json
{
  "mcpServers": {
    "mongocore": {
      "command": "mongocore",
      "args": ["--stdio", "--connection-uri", "mongodb://localhost:27017"]
    }
  }
}
```

## What You Can Do

Once configured, Claude can:

| Capability | Example |
|-----------|---------|
| **Ask your data** | "How many restaurants in Brooklyn have a grade A?" |
| **Generate code** | "Write Python code to find all users who signed up last week" |
| **Explore schemas** | "What does the restaurants collection look like?" |
| **Semantic search** | "Find articles about caching strategies" |
| **Get insights** | "Which queries are slowest? Do I need any indexes?" |
| **Guided workflows** | "Help me add vector search to my articles collection" |

## Connection Priority

MongoCore resolves the connection URI in this order:
1. `--connection-uri` CLI flag
2. `MONGODB_URI` environment variable
3. `connection_uri` in config TOML file
4. `mongodb://localhost:27017` (default)

## Zero-Config LLM

When running inside Claude, MongoCore uses Claude itself (via MCP sampling) for natural language → MQL translation. No separate LLM API key needed.

If you want to use MongoCore standalone (not inside Claude), configure an API key:
```json
{
  "env": {
    "ANTHROPIC_API_KEY": "your-key"
  }
}
```

## Available Skills

MongoCore includes guided workflows accessible via Claude's prompt selector:

- **explore_dataset** — Systematically explore a database
- **bootstrap_project** — Set up MongoCore in a new project
- **setup_collection** — Design and create a collection with indexes
- **add_vector_search** — Add semantic search to a collection
- **debug_slow_query** — Identify and fix slow queries
- **and more...**

Use `list_skills` to see all available workflows.

## Troubleshooting

| Problem | Solution |
|---------|----------|
| "Cannot connect to MongoDB" | Check your connection URI and that MongoDB is running |
| "Embedding requires VOYAGE_API_KEY" | Add VOYAGE_API_KEY to env in MCP config |
| "NL queries require an LLM" | Use MongoCore inside Claude (automatic) or set ANTHROPIC_API_KEY |
| Tools not appearing | Restart Claude Desktop after config changes |
```

- [ ] **Step 2: Update docs/README.md**

Add `mcp-setup.md` to the documentation index.

- [ ] **Step 3: Commit**

```bash
git add docs/mcp-setup.md docs/README.md
git commit -m "docs: add MCP setup guide for Claude Desktop and Claude Code"
```

---

### Task 4: Update Cargo.toml for crates.io publishing

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add metadata**

Ensure these fields exist in `[package]`:

```toml
[package]
name = "mongocore"
description = "AI-native MongoDB driver sidecar with MCP support"
repository = "https://github.com/mongodb/mongocore"
license = "Apache-2.0"
keywords = ["mongodb", "mcp", "ai", "database", "grpc"]
categories = ["database", "command-line-utilities"]
```

- [ ] **Step 2: Verify `cargo publish --dry-run`**

Run: `cargo publish --dry-run`
Expected: No errors (may warn about missing fields)

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "chore: add crates.io metadata to Cargo.toml"
```

---

## Verification Checklist

- [ ] `.github/workflows/release.yml` syntax validates (`act` or yamllint)
- [ ] Homebrew formula references correct URLs
- [ ] `docs/mcp-setup.md` covers all MCP config variants
- [ ] `cargo build --release` succeeds
- [ ] `./target/release/mongocore --stdio` starts correctly
- [ ] `./target/release/mongocore --help` shows the new `--stdio` flag
