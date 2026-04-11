# Meilisearch CLI

The official Meilisearch CLI — a single Rust binary that covers everything from day-to-day instance management to advanced data operations, with an interactive TUI mode for search and chat.

## Installation

```bash
# From source
cargo install --path .

# The binary is named `meilisearch`
meilisearch --help
```

## Quick Start

```bash
# Check server health
meilisearch health

# Create an index and import data
meilisearch index create movies --primary-key id
meilisearch import movies --file movies.json

# Search
meilisearch search movies "gatsby"

# Interactive search TUI
meilisearch search movies -i
```

## Project Management

The CLI supports multiple Meilisearch instances through named projects stored in `~/.config/meilisearch/config.toml`.

```bash
# Add a remote project
meilisearch project add production --url https://my-instance.meilisearch.io --api-key masterKey123

# Switch default project
meilisearch project use production

# List all projects
meilisearch project list

# Target a specific project for one command
meilisearch --project staging health
```

## Commands

### Index Management

```bash
meilisearch index list
meilisearch index create <uid> [--primary-key <key>]
meilisearch index get <uid>
meilisearch index delete <uid>
meilisearch index stats <uid>
meilisearch index swap <index-a> <index-b>
```

### Document Management

```bash
meilisearch document add <uid> --file <path>
meilisearch document add <uid> < data.json          # stdin
meilisearch document get <uid> <id>
meilisearch document list <uid> [--limit 20]
meilisearch document delete <uid> <id>
meilisearch document delete-all <uid>
```

### Search

```bash
meilisearch search <uid> <query> [--filter <expr>] [--facets <...>] [--limit <n>]
meilisearch search <uid> -i                         # interactive TUI
```

### Settings

```bash
meilisearch settings get <uid>
meilisearch settings update <uid> --file settings.json
meilisearch settings reset <uid>
meilisearch settings edit <uid>                     # opens $EDITOR
meilisearch settings diff <uid>
```

### Tasks

```bash
meilisearch task list [--status <status>] [--type <type>]
meilisearch task get <id>
meilisearch task cancel <uids>
meilisearch task delete <uids>
meilisearch task wait <id> [--timeout 60000]
meilisearch task watch <id>
```

### API Keys

```bash
meilisearch key list
meilisearch key get <key>
meilisearch key create --actions search --indexes movies
meilisearch key delete <key>
```

### Import

```bash
meilisearch import <uid> --file data.json
meilisearch import <uid> --file data.ndjson --batch-size 10485760
meilisearch import <uid> --file data.csv
meilisearch import <uid> < stdin.ndjson
```

### Clone

```bash
meilisearch clone <source-uid> <dest-uid>
meilisearch clone <source-uid> <dest-uid> --from production --to staging
```

### Promote

```bash
meilisearch promote --from local --to production
meilisearch promote --from local --to staging --indexes products,categories
meilisearch promote --from local --to production --dry-run
```

### Local Instance

```bash
meilisearch local start       # Docker preferred, binary fallback
meilisearch local stop
meilisearch local restart
meilisearch local status
meilisearch local logs [-f]
meilisearch local reset       # wipe data, restart fresh
meilisearch local upgrade     # upgrade to latest version
```

### Dumps & Snapshots

```bash
meilisearch dump create
meilisearch dump snapshot
```

### Chat

```bash
meilisearch chat "What products do you have?"
meilisearch chat -i                                 # interactive TUI
```

### Server Info

```bash
meilisearch health
meilisearch version
meilisearch stats
```

## Output Formatting

```bash
meilisearch search movies "query"              # pretty-printed JSON
meilisearch --raw search movies "query"        # compact JSON, pipeable
meilisearch --quiet health                     # errors only
```

## Configuration

Config file: `~/.config/meilisearch/config.toml`

```toml
default = "local"

[projects.local]
type = "local"
url = "http://127.0.0.1:7700"

[projects.production]
url = "https://my-instance.meilisearch.io"
api_key = "masterKey123"
```

## Development

```bash
# Build
cargo build

# Run tests (requires Meilisearch on localhost:7700)
cargo test

# Lint
cargo clippy -- -D warnings
cargo fmt --check
```

## License

MIT
