# meilisearch CLI — Build Instructions

## Step 0 — Read these before writing any code
- PRD: ./PRD.md (read this fully first)
- API: https://www.meilisearch.com/docs/reference/api
- OpenAPI spec: https://www.meilisearch.com/assets/open-api/meilisearch-openapi.json
- Reference 1: https://raw.githubusercontent.com/irevoire/mieli/main/src/main.rs
- Reference 2: https://raw.githubusercontent.com/meilisearch/meilisearch-importer/main/src/main.rs

## Environment
Meilisearch running in Docker on http://localhost:7700 (no auth).
Verify: curl http://localhost:7700/health

## Build order
1. cargo new meilisearch-cli --bin + Cargo.toml with all deps
2. Config module — ~/.config/meilisearch/config.toml + unit tests
3. HTTP client module + unit tests
4. All API commands from OpenAPI spec + integration tests against localhost:7700
5. meilisearch local (Docker preferred, binary fallback)
6. meilisearch import + integration test
7. meilisearch clone + meilisearch promote (export route) + integration tests
8. meilisearch settings edit ($EDITOR integration)
9. meilisearch search -i (ratatui TUI)
10. meilisearch chat -i (ratatui TUI, streaming)
11. cargo test && cargo clippy -- -D warnings && cargo fmt --check
12. README.md + docs/local-project.md + docs/promote.md + docs/interactive-mode.md + CHANGELOG.md + CONTRIBUTING.md
13. cargo build --release → write DONE.md

## Rules
- cargo build after every module, fix all errors before continuing
- Integration tests: unique index names (uuid suffix), always clean up
- anyhow::Result everywhere
- Never ask for confirmation
