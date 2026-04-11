# Local Project

The CLI ships with a built-in `local` project that manages a Meilisearch instance on your machine. It's the default project created on first run and serves as a development sandbox.

## How It Works

When you run `meilisearch local start`, the CLI:

1. Checks if Docker is available (`docker info`)
2. If Docker is available (preferred): starts a container using `getmeili/meilisearch:latest`
3. If Docker is unavailable: downloads the Meilisearch binary and runs it as a daemon

Data is stored in `~/.local/share/meilisearch/local/`.

## Commands

### Start

```bash
meilisearch local start
```

Starts the local instance. Idempotent — safe to run multiple times.

Output:
```
Starting local Meilisearch with Docker...
✓ Local Meilisearch started (http://127.0.0.1:7700)
```

### Stop

```bash
meilisearch local stop
```

### Status

```bash
meilisearch local status
```

Shows running mode (Docker or binary), status, and version.

### Logs

```bash
meilisearch local logs
meilisearch local logs -f    # follow
```

### Reset

```bash
meilisearch local reset
```

Wipes all local data and restarts fresh.

### Upgrade

```bash
meilisearch local upgrade
```

Pulls the latest Docker image (or downloads the latest binary), stops the current instance, and restarts with the new version. Meilisearch handles dumpless migration internally.

## Data Locations

| Path | Purpose |
|------|---------|
| `~/.local/share/meilisearch/local/` | Data directory |
| `~/.local/share/meilisearch/local/data.ms` | Database (binary mode) |
| `~/.local/share/meilisearch/local/meili.pid` | PID file (binary mode) |
| `~/.local/share/meilisearch/local/meilisearch.log` | Log file (binary mode) |
| `~/.local/share/meilisearch/bin/meilisearch-server` | Downloaded binary |
