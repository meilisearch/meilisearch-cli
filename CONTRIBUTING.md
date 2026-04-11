# Contributing

## Prerequisites

- Rust (latest stable)
- Meilisearch running on `http://localhost:7700` (for integration tests)

## Setup

```bash
# Start Meilisearch (Docker)
docker run -d --name meilisearch-local -p 7700:7700 getmeili/meilisearch:latest

# Build
cargo build

# Run tests
cargo test

# Lint
cargo clippy -- -D warnings
cargo fmt --check
```

## Project Structure

```
src/
├── main.rs           # Entry point
├── config.rs         # Project credential management (~/.config/meilisearch/config.toml)
├── client.rs         # HTTP client wrapper for Meilisearch REST API
├── commands/
│   ├── mod.rs        # CLI definition (clap), command routing
│   ├── index.rs      # Index CRUD commands
│   ├── document.rs   # Document CRUD commands
│   ├── search.rs     # Search command (standard + interactive)
│   ├── settings.rs   # Settings get/update/reset/edit/diff
│   ├── task.rs       # Task list/get/cancel/wait/watch
│   ├── key.rs        # API key management
│   ├── project.rs    # Project add/remove/list/use/current
│   ├── local.rs      # Local instance management (Docker/binary)
│   ├── import.rs     # Bulk document import
│   ├── clone.rs      # Index cloning across projects
│   ├── promote.rs    # Promote local to remote
│   ├── dump.rs       # Dump and snapshot creation
│   ├── chat.rs       # Chat completions (standard + interactive)
│   └── health.rs     # Health check
└── tui/
    ├── mod.rs
    ├── search.rs     # Interactive search TUI (ratatui)
    └── chat.rs       # Interactive chat TUI (ratatui)
tests/
└── integration.rs    # Integration tests against running Meilisearch
```

## Guidelines

- Use `anyhow::Result` for all error handling
- Integration tests must use unique index names (UUID suffix) and clean up after themselves
- Run `cargo clippy -- -D warnings` before submitting — zero warnings policy
- Run `cargo fmt` before committing
