# Changelog

## [0.2.0] - 2026-04-11

### Added

- **Shell completions**: `meilisearch completions <shell>` generates completions for bash, zsh, and fish
- **Auto-install completions**: the install script now detects your shell and installs completions automatically
- **Grouped help output**: `--help` now organizes commands into categories (Data, Search, Operations, Server, Configuration) for better discoverability

### Improved

- **Clearer argument names**: all positional arguments now show descriptive names in usage lines (`<INDEX_UID>`, `<DOCUMENT_ID>`, `<API_KEY>`, `<BATCH_UID>`) instead of generic `<UID>`
- **Command ordering**: commands are logically grouped in the enum to match the help output categories

## [0.1.0] - 2026-04-10

### Added

- **Index management**: create, list, get, delete, update, stats, swap
- **Document management**: add, update, get, list, fetch, delete, delete-all, delete-by-filter, delete-batch, edit-by-function
- **Search**: full-text search with filters, facets, sort, highlighting
- **Multi-search**: search across multiple indexes in a single request
- **Facet search**: dedicated facet search endpoint
- **Similar documents**: find similar documents by ID
- **Settings management**: get, update, reset (all settings or individual sub-resources), interactive editor (`$EDITOR`), diff view
- **Task management**: list, get, cancel, delete, wait, watch
- **Batch management**: list, get
- **API key management**: list, get, create, update, delete
- **Import**: bulk import from JSON, NDJSON, CSV with progress bar and automatic batching
- **Clone**: clone indexes within a project
- **Promote**: promote indexes between projects (cross-instance)
- **Dumps and snapshots**: create dumps and snapshots
- **Log management**: update stderr target, stream logs, stop streaming
- **Network configuration**: get and update network settings
- **Experimental features**: get and toggle experimental features
- **Prometheus metrics**: fetch raw metrics endpoint
- **Interactive search TUI**: real-time search-as-you-type interface (ratatui)
- **Interactive chat TUI**: streaming chat with your data, supports `/compact`, `/clear`, `/system`
- **Local instance management**: start/stop/restart Meilisearch locally (Docker preferred, binary fallback), logs, reset, upgrade
- **Project management**: multi-project credential store (`~/.config/meilisearch/config.toml`)
- **Self-update**: check for updates and auto-update from GitHub Releases
- **CI/CD**: GitHub Actions for check, test, and cross-platform release builds (linux/macOS x86_64/aarch64)
- **Install script**: one-liner install with platform detection
- **Output modes**: pretty JSON (default), `--raw` for piping, `--quiet` for errors only
