# Changelog

## [0.5.0] - 2026-10-02

> **Agent-friendly release**: `msc` can now be driven reliably by scripts, CI jobs and AI agents. Output is JSON whenever stdout is not a terminal, errors are structured with stable exit codes, and every write can wait for its task. See [`docs/agents.md`](docs/agents.md).

### Added

- **Structured errors and exit codes**: failures exit with `1` (general), `2` (usage), `3` (auth), `4` (not found), `5` (API error), `6` (network), `7` (task failed) or `8` (timeout). In JSON mode the error is printed on stderr as `{"error": {"kind", "code", "message", "exitCode", "httpStatus", "type", "link", "hint"}}`, keeping Meilisearch's error `type` and documentation `link`.
- **`--wait` and `--wait-timeout`** global options (also `MSC_WAIT`): any write that enqueues a task waits for it and prints the finished task. A failed task exits with code `7`.
- **`--url` and `--api-key`** global options, with `MSC_URL`, `MSC_API_KEY` and `MSC_PROJECT` environment variables. `--url` bypasses the config file entirely. `MSC_CONFIG` overrides the config file path.
- **`--json`** (`-j`, alias of `--raw`) and **`--pretty`** to force either output mode.
- **`--select <PATHS>`**: keep only the given dot-path fields, applied to each element of `results`/`hits` lists.
- **`--dry-run`** on `index delete`, `document delete-all`, `document delete-by-filter`, `settings update`, `settings reset`, `task cancel`, `task delete`, `key delete`, `clone` and `local reset`. `settings update --dry-run` shows only the settings that would change.
- **Idempotent flags**: `index create --if-not-exists`, `index delete --if-exists`, `key delete --if-exists`, `project add --if-not-exists` / `--force`, `project remove --if-exists`.
- **`msc api <METHOD> <PATH>`**: call any API route with the CLI's auth, error handling, exit codes, `--wait` and `--select`. The body comes from `--data`, `--file` or stdin; `--param KEY=VALUE` adds query parameters.
- **`search --body <JSON>`**: merge any search parameter (`hybrid`, `showRankingScore`, `matchingStrategy`, …) over the flags.
- **`msc whoami`**: shows the resolved server, project and API key source, and checks reachability and authentication (exit `6` or `3` on failure).
- **`--table` now renders tables.** Lists become rows and single objects become key/value rows. The flag was previously accepted but ignored.
- **`msc schema`**: prints the full command tree (arguments, types, defaults, env vars, exit codes) as JSON.
- **`msc mcp`**: a Model Context Protocol server over stdio exposing every command as a tool, with `--read-only` and `--tools` filters.
- **Agent skill bundled in the binary**: `msc skill install` writes it to `~/.claude/skills/msc` (`--local` for `./.claude/skills`, `--dir` for any agent), alongside `skill list`, `skill show` and `skill uninstall`. `install.sh` installs it when `~/.claude` exists, and `self-update` keeps it current.
- **Claude Code plugin**: the repository is a plugin marketplace. `/plugin marketplace add meilisearch/meilisearch-cli` then `/plugin install meilisearch-cli@meilisearch` installs the skill and the MCP server together.
- **`--format json|ndjson|csv`** on `document add`, `document update` and `import`, for NDJSON and CSV on stdin.
- **`import --no-wait`** to return once batches are enqueued, and **`import --events`** for NDJSON progress events.
- **`chat --events`** for NDJSON answer events; non-interactive `chat` in JSON mode returns `{answer, sources, model, workspace}`, including the documents used as sources.
- **`project update --url / --api-key / --remove-api-key`** for non-interactive updates.
- **`similar --offset` and `--embedder`**, **`key list --offset` and `--limit`**.
- **Help text for every argument**, so `--help`, `msc schema` and MCP tools describe them all.

### Changed

- **Output is JSON when stdout is not a terminal.** Piped or captured output is compact JSON without needing `--raw`. Use `--pretty` to get human output in a pipe.
- **Status messages moved to stderr** (`Waiting for task…`, `✓ Settings synced`, progress bars) and are silenced in JSON mode, so stdout only contains the result. Docker output from `msc local` no longer leaks to stdout.
- **Every command returns a result**: `import`, `clone`, `promote`, `project *`, `local *`, `key delete` and `log stop` print a JSON object in JSON mode instead of prose.
- **`version` prints to stdout** (JSON in JSON mode) instead of stderr, and only checks for CLI updates in a terminal.
- **`task wait` exits with code `7`** when the task failed or was canceled (the task is still printed on stdout). Its `--timeout` now defaults to `--wait-timeout` (5 minutes). `task watch` emits NDJSON events in JSON mode.
- **`project add --url` and `--api-key` are now global options.** The command line is unchanged, but the values can also come from `MSC_URL` / `MSC_API_KEY`.
- **Commands never prompt or open a TUI without a terminal**: `project add`, `project update`, `settings edit`, `search -i` and `chat -i` fail with exit code `2` and a hint instead of hanging. Reading documents or JSON from stdin errors when stdin is a terminal.
- **`project list` JSON output** is `{"default": ..., "results": [...]}` and never includes API keys.
- **`settings diff`** now compares current settings against a settings file (`--against <PATH>` or stdin) and lists only the changed keys.
- **`import` batches JSON arrays** by size, like NDJSON, and returns a summary with `taskUids` and `indexedDocuments`.
- **`chat`** requires a message when not using `-i` (it previously defaulted to "Hello").
- **Task polling** uses exponential backoff (50 ms up to 1 s) instead of a fixed 250 ms interval.

### Fixed

- **CI lints tests too**: `cargo clippy --all-targets -- -D warnings`.
- **`log stream`** now streams logs to stdout instead of trying to parse the stream as a single JSON response.
- **`multi-search`** accepts a full request body (`{"queries": [...], "federation": {...}}`) in addition to a bare queries array.
- **`promote`** now fails when a fallback document copy task fails, instead of silently continuing.
- **API error responses that are not JSON** (for example from a proxy) are reported with their HTTP status instead of a parse error.

## [0.4.0] - 2026-04-15

> **Breaking change**: the CLI binary has been renamed from `meilisearch` to `msc`. A one-time migration moves your existing config to the new location on first run, so projects and API keys are preserved. You may need to update shell aliases, scripts, and shell completions to use the new name.

### Changed

- **Binary renamed from `meilisearch` to `msc`** (short for **M**eili**s**earch **C**LI) to avoid the name collision with the Meilisearch server binary. All invocations now use `msc` instead of `meilisearch` — for example, `msc search movies "query"`, `msc local start`, `msc project list`.
- **Config directory moved** from `~/.config/meilisearch/` to `~/.config/msc/`. On first run, the old `config.toml` is automatically moved to the new location and the message `Migrated config from … to …` is printed. No manual action is required.
- **Local data directory moved** from `~/.local/share/meilisearch/` to `~/.local/share/msc/`. This affects the local project's data directory and the cached Meilisearch server binary used when Docker is unavailable. Users of `msc local` with an existing binary-mode install should re-run `msc local upgrade` or move the directory manually.
- **Release artifacts renamed** from `meilisearch-<version>-<target>.tar.gz` to `msc-<version>-<target>.tar.gz`. The `install.sh` script and `msc self-update` use the new naming automatically.
- **Shell completion filenames** updated to match the new binary name: `_msc` (zsh), `msc` (bash), and `msc.fish` (fish). Re-run `install.sh` or regenerate completions with `msc completions <shell>` to refresh them.
- **Self-update message** now reads `Run \`msc self-update\` to upgrade.` instead of referencing the old binary name.

### Migration guide

1. Download the new release (or run `msc self-update` once you're already on a build that reads `msc-*.tar.gz` artifacts).
2. The first time you run any `msc` command, your old `~/.config/meilisearch/config.toml` is moved to `~/.config/msc/config.toml` automatically. Verify with `msc project list`.
3. If you had shell completions installed from the old binary, remove the stale files (`_meilisearch`, `meilisearch`, `meilisearch.fish`) and re-run `install.sh` or `msc completions <shell>`.
4. Update any scripts, aliases, or CI jobs that invoke `meilisearch` to use `msc` instead.

## [0.3.0] - 2026-04-11

### Added

- **Interactive settings TUI**: `meilisearch settings edit <index> -i` opens a full TUI for managing all 21 settings sub-resources with checkbox editors, reordering, string list editing, select menus, and number inputs
- **Sub-resource editing**: `meilisearch settings edit <index> synonyms` opens `$EDITOR` on a single sub-resource instead of the full settings blob
- **CLI reference documentation**: comprehensive `docs/reference.md` covering all 24 commands, 42+ subcommands, every argument, flag, and default value
- **README screenshots**: interactive search, chat, settings overview, and settings editor screenshots

### Fixed

- **JSON field ordering**: search results and settings now preserve server field order instead of sorting alphabetically (serde_json `preserve_order` feature)

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
