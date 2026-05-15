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
| `2026-05-12-performance-benchmarks.md` | Performance benchmarking methodology |
| `2026-05-13-mcp-claude-integration-design.md` | MCP intelligent data companion (v0.8) |
| `2026-05-13-performance-uds-streaming-design.md` | UDS transport + streaming RPCs |
| `2026-05-13-pipeline-request-batching-design.md` | Request pipelining (batch N ops) |
| `2026-05-14-demo-video-design.md` | Skunkworks demo video |
| `2026-05-14-transactional-pipeline-design.md` | Transactional pipeline with result forwarding |
| `2026-05-14-web-ui-dashboard-design.md` | Embedded web dashboard |
| `2026-05-15-mcp-explain-design.md` | MCP operation explain + session recorder |

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
| `2026-05-12-performance-benchmarks-plan.md` | Complete (v0.7) |
| `2026-05-13-mcp-claude-phase1-foundation.md` | Complete (v0.8) |
| `2026-05-13-mcp-claude-phase2-codegen.md` | Complete (v0.8) |
| `2026-05-13-mcp-claude-phase3-embedding.md` | Complete (v0.8) |
| `2026-05-13-mcp-claude-phase4-skills.md` | Complete (v0.8) |
| `2026-05-13-mcp-claude-phase5-insights.md` | Complete (v0.8) |
| `2026-05-13-mcp-claude-phase6-packaging.md` | Complete (v0.8) |
| `2026-05-13-performance-uds-streaming-plan.md` | Complete (v0.9) |
| `2026-05-13-pipeline-request-batching-plan.md` | Complete (v0.9) |
| `2026-05-13-benchmark-fixes.md` | Complete |
| `2026-05-14-transactional-pipeline-plan.md` | Complete (v0.10) |
| `2026-05-14-web-ui-dashboard-plan.md` | Complete |
| `2026-05-15-demo-video-plan.md` | Complete |
| `2026-05-15-mcp-explain-plan.md` | Complete |
