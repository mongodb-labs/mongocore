# Design & Development

This directory contains the design artifacts and development history for MongoCore.

## Contents

| Path | Purpose |
|------|---------|
| [Development Log](./development-log.md) | Session history — how MongoCore was built, key decisions, debugging narratives |
| [specs/](./specs/) | Design specifications — what to build and why |
| [plans/](./plans/) | Implementation plans — step-by-step how to build it |

## Workflow

Every significant feature follows: **Brainstorm → Spec → Plan → Execute**

1. **Spec** (`specs/YYYY-MM-DD-<topic>-design.md`) — captures the problem, explores approaches, documents the chosen design
2. **Plan** (`plans/YYYY-MM-DD-<topic>-plan.md`) — step-by-step tasks with exact code, files, and commands
3. **Execute** — dispatch subagents per task or execute inline
4. **Log** — add a narrative entry to `development-log.md`

## Specs

| Spec | Area |
|------|------|
| `2026-05-11-mongocore-design.md` | Original vision and architecture |
| `2026-05-11-mongocore-v3-design.md` | Intelligent data ingestion (Polars) |
| `2026-05-12-mongocore-demo-readiness.md` | Demo: stdio MCP + restaurant dataset |
| `2026-05-12-mongocore-developer-experience.md` | AGENTS.md / CLAUDE.md |
| `2026-05-12-mongocore-integration-improvements.md` | Driver metadata, URL ingestion, OTel |
| `2026-05-12-mongocore-client-test-coverage.md` | Full 27-RPC test coverage |
| `2026-05-12-mongocore-compiled-query-testing.md` | Real LLM integration tests |
| `2026-05-12-mongocore-simplified-llm-config.md` | Direct API key config |
| `2026-05-12-mongocore-custom-llm-gateway.md` | Corporate gateway support |
| `2026-05-12-nql-mql-expanded-testing.md` | Validator hardening, injection tests |
| `2026-05-12-llm-provided-templates-and-routing.md` | Intelligent routing + templates |

## Plans

| Plan | Status |
|------|--------|
| `2026-05-11-mongocore-implementation-plan.md` | Complete (v0.1) |
| `2026-05-11-mongocore-v2-plan.md` | Complete (v0.2) |
| `2026-05-11-mongocore-v3-plan.md` | Complete (v0.3) |
| `2026-05-12-developer-experience-plan.md` | Complete (v0.4) |
| `2026-05-12-integration-improvements-plan.md` | Complete (v0.4) |
| `2026-05-12-client-test-coverage-plan.md` | Complete (v0.4) |
| `2026-05-12-compiled-query-testing-plan.md` | Complete (v0.5) |
| `2026-05-12-simplified-llm-config-plan.md` | Complete (v0.5) |
| `2026-05-12-custom-llm-gateway-plan.md` | Complete (v0.5) |
| `2026-05-12-nql-mql-expanded-testing-plan.md` | Complete (v0.5) |
| `2026-05-12-llm-templates-and-routing-plan.md` | Complete (v0.6) |
