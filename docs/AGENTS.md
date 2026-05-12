# Documentation Agent Guide

Rules for AI agents (or humans) editing documentation in this `docs/` folder.

## Writing Style

- Clear, concise, scannable — prefer tables and bullet points over paragraphs
- Use active voice ("MongoCore validates..." not "validation is performed by...")
- Code examples should be copy-pasteable (no `...` elision unless clearly a snippet)
- Every doc should be self-contained — a reader shouldn't need to read 3 other docs first
- No emojis unless the user explicitly requests them

## When to Update Docs

| Trigger | What to update |
|---------|---------------|
| New gRPC RPC added | `mcp-server.md` (tool count), `roadmap.md` (if version-relevant) |
| New config field | `getting-started.md` (env var table), `testing.md` (if test-relevant), `config.test.toml.example` |
| New feature shipped | `roadmap.md` (move from future to version), `design/development-log.md` (add entry) |
| New client method | `client-libraries.md`, relevant operation doc (crud, aggregation, etc.) |
| Architecture change | `README.md` (project structure), `docs/README.md` (features list) |
| New doc created | `docs/README.md` (add to table), main `README.md` (if it has a docs link) |

## Cross-References to Maintain

These docs reference each other — when updating one, check if others need updating:

- `README.md` ↔ `docs/README.md` (features lists should match)
- `compiled-queries.md` ↔ `roadmap.md` (capabilities vs future work)
- `testing.md` ↔ `config.test.toml.example` (config fields must match)
- `getting-started.md` ↔ `quick-start.md` (config examples should be consistent)
- `roadmap.md` ↔ `design/development-log.md` (completed items move between them)

## Doc Structure Conventions

- Each doc starts with a `# Title` and one-sentence description
- Configuration docs show both TOML and env var forms
- Operation docs show all 4 languages (Python, TypeScript, Go, Java)
- Tables use `|` alignment — keep columns readable
- Code blocks specify language: ` ```toml `, ` ```rust `, ` ```python `, etc.

## Files in This Folder

| File | Purpose | Update frequency |
|------|---------|-----------------|
| `README.md` | Documentation index | When docs are added/removed |
| `quick-start.md` | Language examples + config | Rarely (stable) |
| `getting-started.md` | Full config reference | When config fields change |
| `testing.md` | Test setup and commands | When test infra changes |
| `compiled-queries.md` | NL→MQL system docs | When compiled query system changes |
| `roadmap.md` | Version history + future | Every version release |
| `design/development-log.md` | Session history narrative | After significant work |
| `opentelemetry.md` | OTel setup | When OTel config changes |
| Operation docs | CRUD, aggregation, etc. | When APIs change |

## Tone

Match the existing doc tone: technically precise, assumes the reader is a developer, no hand-holding on basics (they know what MongoDB is). Explain MongoCore-specific concepts clearly.

## Development Log

When completing significant work, add a brief narrative entry to `design/development-log.md`:
- What was the problem/question?
- What approach was taken?
- What was learned?
- Keep it conversational — this is a session history, not a changelog.

## Don'ts

- Don't duplicate content across docs — link instead
- Don't document implementation details (that's code comments) — document user-facing behavior
- Don't let config examples drift from `config.test.toml.example`
- Don't add a doc without adding it to `docs/README.md`
