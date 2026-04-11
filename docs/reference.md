# CLI Reference

Complete reference for every command, subcommand, and option in the Meilisearch CLI.

## Global Options

These options apply to all commands.

| Option | Description |
|--------|-------------|
| `--project <NAME>` | Target a specific project (overrides default) |
| `-r, --raw` | Output raw JSON with no formatting |
| `-t, --table` | Output as table |
| `-q, --quiet` | Quiet mode (errors only) |
| `-h, --help` | Print help |
| `-V, --version` | Print version |

---

## Data

### `index` - Manage indexes

#### `index list`

List all indexes.

```bash
meilisearch index list [--offset <N>] [--limit <N>]
```

| Option | Description |
|--------|-------------|
| `--offset <N>` | Number of indexes to skip |
| `--limit <N>` | Maximum number of indexes to return |

#### `index create`

Create an index.

```bash
meilisearch index create <INDEX_UID> [--primary-key <KEY>]
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<INDEX_UID>` | Yes | Unique identifier for the index |

| Option | Description |
|--------|-------------|
| `--primary-key <KEY>` | Primary key field name |

#### `index get`

Get index info.

```bash
meilisearch index get <INDEX_UID>
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<INDEX_UID>` | Yes | Index to retrieve |

#### `index delete`

Delete an index.

```bash
meilisearch index delete <INDEX_UID>
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<INDEX_UID>` | Yes | Index to delete |

#### `index stats`

Show index stats.

```bash
meilisearch index stats <INDEX_UID>
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<INDEX_UID>` | Yes | Index to get stats for |

#### `index update`

Update an index (change primary key).

```bash
meilisearch index update <INDEX_UID> [--primary-key <KEY>]
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<INDEX_UID>` | Yes | Index to update |

| Option | Description |
|--------|-------------|
| `--primary-key <KEY>` | New primary key field name |

#### `index swap`

Swap two indexes.

```bash
meilisearch index swap <INDEX_A> <INDEX_B>
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<INDEX_A>` | Yes | First index |
| `<INDEX_B>` | Yes | Second index |

---

### `document` - Manage documents

#### `document add`

Add or replace documents.

```bash
meilisearch document add <INDEX_UID> [--file <PATH>] [--primary-key <KEY>]
```

Reads from stdin if `--file` is not provided. Supports JSON, NDJSON (`.ndjson`, `.jsonl`), and CSV (`.csv`) formats.

| Argument | Required | Description |
|----------|----------|-------------|
| `<INDEX_UID>` | Yes | Target index |

| Option | Description |
|--------|-------------|
| `--file <PATH>` | Path to the documents file |
| `--primary-key <KEY>` | Primary key field name |

#### `document update`

Add or update documents (partial update).

```bash
meilisearch document update <INDEX_UID> [--file <PATH>] [--primary-key <KEY>]
```

Same options as `document add`. Uses PUT instead of POST, so existing documents are partially updated.

#### `document get`

Get a single document.

```bash
meilisearch document get <INDEX_UID> <DOCUMENT_ID>
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<INDEX_UID>` | Yes | Target index |
| `<DOCUMENT_ID>` | Yes | Document ID to retrieve |

#### `document list`

List documents.

```bash
meilisearch document list <INDEX_UID> [--offset <N>] [--limit <N>] [--fields <FIELDS>]
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<INDEX_UID>` | Yes | Target index |

| Option | Description |
|--------|-------------|
| `--offset <N>` | Number of documents to skip |
| `--limit <N>` | Maximum number of documents to return |
| `--fields <FIELDS>` | Comma-separated list of fields to return |

#### `document fetch`

Fetch documents with POST (supports filter).

```bash
meilisearch document fetch <INDEX_UID> [--filter <EXPR>] [--offset <N>] [--limit <N>] [--fields <F1,F2>]
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<INDEX_UID>` | Yes | Target index |

| Option | Description |
|--------|-------------|
| `--filter <EXPR>` | Filter expression |
| `--offset <N>` | Number of documents to skip |
| `--limit <N>` | Maximum number of documents to return |
| `--fields <F1,F2>` | Comma-separated list of fields to return |

#### `document delete`

Delete a single document.

```bash
meilisearch document delete <INDEX_UID> <DOCUMENT_ID>
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<INDEX_UID>` | Yes | Target index |
| `<DOCUMENT_ID>` | Yes | Document ID to delete |

#### `document delete-all`

Delete all documents in an index.

```bash
meilisearch document delete-all <INDEX_UID>
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<INDEX_UID>` | Yes | Target index |

#### `document delete-by-filter`

Delete documents matching a filter.

```bash
meilisearch document delete-by-filter <INDEX_UID> <FILTER>
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<INDEX_UID>` | Yes | Target index |
| `<FILTER>` | Yes | Filter expression (e.g. `"genre = horror"`) |

#### `document delete-batch`

Delete documents by batch of IDs.

```bash
meilisearch document delete-batch <INDEX_UID> <ID1,ID2,...>
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<INDEX_UID>` | Yes | Target index |
| `<IDS>` | Yes | Comma-separated document IDs |

#### `document edit`

Edit documents by function.

```bash
meilisearch document edit <INDEX_UID> <FUNCTION> [--filter <EXPR>]
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<INDEX_UID>` | Yes | Target index |
| `<FUNCTION>` | Yes | JavaScript function body |

| Option | Description |
|--------|-------------|
| `--filter <EXPR>` | Only edit documents matching this filter |

---

### `settings` - Manage settings

#### `settings get`

Get current settings (all or a specific sub-resource).

```bash
meilisearch settings get <INDEX_UID> [SUB_RESOURCE]
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<INDEX_UID>` | Yes | Target index |
| `[SUB_RESOURCE]` | No | Specific sub-resource to retrieve (see [sub-resources](#settings-sub-resources)) |

#### `settings update`

Update settings from JSON file or stdin.

```bash
meilisearch settings update <INDEX_UID> [SUB_RESOURCE] [--file <PATH>]
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<INDEX_UID>` | Yes | Target index |
| `[SUB_RESOURCE]` | No | Specific sub-resource to update |

| Option | Description |
|--------|-------------|
| `--file <PATH>` | JSON file (reads from stdin if omitted) |

#### `settings reset`

Reset settings to defaults.

```bash
meilisearch settings reset <INDEX_UID> [SUB_RESOURCE]
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<INDEX_UID>` | Yes | Target index |
| `[SUB_RESOURCE]` | No | Specific sub-resource to reset (resets all if omitted) |

#### `settings edit`

Edit settings in `$EDITOR` or interactive TUI.

```bash
meilisearch settings edit <INDEX_UID> [SUB_RESOURCE] [-i]
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<INDEX_UID>` | Yes | Target index |
| `[SUB_RESOURCE]` | No | Specific sub-resource to edit |

| Option | Description |
|--------|-------------|
| `-i, --interactive` | Open interactive TUI instead of `$EDITOR` |

**Interactive TUI controls:**

| Key | Action |
|-----|--------|
| Up/Down | Navigate resources or items |
| Enter | Edit selected resource |
| Space | Toggle checkbox / select option |
| Shift+Up/Down | Reorder items (ordered settings) |
| `*` | Toggle wildcard mode (displayed/searchable attributes) |
| `a` | Add item (string lists) |
| `d` | Delete item (string lists) |
| `s` | Save all changes |
| `q` | Quit (warns on unsaved changes) |
| Esc | Back to resource list / cancel |

#### `settings diff`

Show current settings (diff view).

```bash
meilisearch settings diff <INDEX_UID>
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<INDEX_UID>` | Yes | Target index |

#### Settings Sub-Resources

The following sub-resources can be used with `settings get`, `settings update`, `settings reset`, and `settings edit`:

| Sub-Resource | Type | Description |
|--------------|------|-------------|
| `displayed-attributes` | Array of strings | Fields returned in search results |
| `searchable-attributes` | Array of strings | Fields searched (order = priority) |
| `filterable-attributes` | Array of strings | Fields available for filtering |
| `sortable-attributes` | Array of strings | Fields available for sorting |
| `ranking-rules` | Array of strings | Ranking rule order |
| `stop-words` | Array of strings | Words ignored during search |
| `synonyms` | Object | Synonym definitions |
| `distinct-attribute` | String or null | Field used for deduplication |
| `typo-tolerance` | Object | Typo tolerance configuration |
| `faceting` | Object | Faceting configuration |
| `pagination` | Object | Pagination limits |
| `proximity-precision` | String | `"byWord"` or `"byAttribute"` |
| `prefix-search` | String | `"indexingTime"` or `"disabled"` |
| `search-cutoff-ms` | Number or null | Search timeout in milliseconds |
| `dictionary` | Array of strings | Custom dictionary words |
| `separator-tokens` | Array of strings | Custom separator tokens |
| `non-separator-tokens` | Array of strings | Custom non-separator tokens |
| `localized-attributes` | Array of objects | Localization rules |
| `embedders` | Object | Embedding model configuration |
| `facet-search` | Boolean | Enable/disable facet search |
| `chat` | Object | Chat configuration |

---

### `import` - Import documents from file

```bash
meilisearch import <INDEX_UID> [--file <PATH>] [--primary-key <KEY>] [--batch-size <BYTES>]
```

Imports documents with a progress bar. Automatically batches NDJSON files. Reads from stdin if `--file` is not provided.

| Argument | Required | Description |
|----------|----------|-------------|
| `<INDEX_UID>` | Yes | Target index |

| Option | Default | Description |
|--------|---------|-------------|
| `--file <PATH>` | stdin | Path to the file (JSON, NDJSON, or CSV) |
| `--primary-key <KEY>` | | Primary key field name |
| `--batch-size <BYTES>` | `20971520` (20 MiB) | Batch size in bytes for NDJSON splitting |

---

## Search

### `search` - Search an index

```bash
meilisearch search <INDEX_UID> [QUERY] [OPTIONS]
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<INDEX_UID>` | Yes | Index to search |
| `[QUERY]` | No | Search query string |

| Option | Description |
|--------|-------------|
| `--filter <EXPR>` | Filter expression |
| `--facets <F1,F2>` | Comma-separated facets to return |
| `--limit <N>` | Maximum number of results |
| `--offset <N>` | Number of results to skip |
| `--sort <RULES>` | Comma-separated sort rules (e.g. `price:asc`) |
| `--attributes-to-retrieve <A1,A2>` | Comma-separated attributes to return |
| `--attributes-to-highlight <A1,A2>` | Comma-separated attributes to highlight |
| `-i, --interactive` | Open interactive search TUI |

**Interactive TUI controls:**

| Key | Action |
|-----|--------|
| Type | Search query (debounced) |
| Up/Down | Navigate results |
| Enter | Expand selected document |
| Esc | Close expanded view / quit |
| `q` | Quit |

---

### `multi-search` - Search across multiple indexes

```bash
meilisearch multi-search [--file <PATH>]
```

| Option | Description |
|--------|-------------|
| `--file <PATH>` | JSON file containing the queries array (reads from stdin if omitted) |

Example input:

```json
[
  { "indexUid": "movies", "q": "action" },
  { "indexUid": "books", "q": "fiction" }
]
```

---

### `facet-search` - Perform a facet search

```bash
meilisearch facet-search <INDEX_UID> <FACET_NAME> [--facet-query <QUERY>] [--filter <EXPR>]
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<INDEX_UID>` | Yes | Target index |
| `<FACET_NAME>` | Yes | Facet name to search |

| Option | Description |
|--------|-------------|
| `--facet-query <QUERY>` | Facet query string |
| `--filter <EXPR>` | Filter expression |

---

### `similar` - Find similar documents

```bash
meilisearch similar <INDEX_UID> <DOCUMENT_ID> [--limit <N>] [--filter <EXPR>]
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<INDEX_UID>` | Yes | Target index |
| `<DOCUMENT_ID>` | Yes | Document ID to find similar documents for |

| Option | Description |
|--------|-------------|
| `--limit <N>` | Maximum number of results |
| `--filter <EXPR>` | Filter expression |

---

### `chat` - Interactive chat with your data

```bash
meilisearch chat [MESSAGE] [--workspace <UID>] [--model <MODEL>] [-i]
```

| Argument | Required | Description |
|----------|----------|-------------|
| `[MESSAGE]` | No | Chat message (opens TUI if omitted with `-i`) |

| Option | Default | Description |
|--------|---------|-------------|
| `--workspace <UID>` | `cloud` | Workspace UID |
| `--model <MODEL>` | `gpt-4o-mini` | Model to use (passed through to LLM provider) |
| `-i, --interactive` | | Open interactive chat TUI |

**Interactive TUI controls:**

| Key | Action |
|-----|--------|
| Enter | Send message |
| Up/Down | Scroll chat history |
| Ctrl+C | Quit |
| Ctrl+D | Show log |
| Ctrl+S | Show sources |
| `/clear` | Clear conversation |
| `/compact` | Summarize and compact conversation |
| `/system <msg>` | Set system prompt |

---

## Operations

### `clone` - Clone an index

```bash
meilisearch clone <SOURCE_UID> <DEST_UID> [--from <PROJECT>] [--to <PROJECT>]
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<SOURCE_UID>` | Yes | Source index UID |
| `<DEST_UID>` | Yes | Destination index UID |

| Option | Description |
|--------|-------------|
| `--from <PROJECT>` | Source project (uses default if omitted) |
| `--to <PROJECT>` | Destination project (uses default if omitted) |

---

### `promote` - Promote indexes between projects

```bash
meilisearch promote --to <PROJECT> [--from <PROJECT>] [--indexes <I1,I2>] [--dry-run] [--create]
```

| Option | Default | Description |
|--------|---------|-------------|
| `--from <PROJECT>` | `local` | Source project |
| `--to <PROJECT>` | *required* | Destination project |
| `--indexes <I1,I2>` | all | Only promote specific indexes (comma-separated) |
| `--dry-run` | | Show what would be promoted without executing |
| `--create` | | Create destination indexes if they don't exist |

---

### `dump` - Manage dumps

#### `dump create`

Create a dump.

```bash
meilisearch dump create
```

#### `dump snapshot`

Create a snapshot.

```bash
meilisearch dump snapshot
```

---

### `task` - Manage tasks

#### `task list`

List tasks with optional filters.

```bash
meilisearch task list [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--uids <UIDS>` | Filter by task UIDs |
| `--statuses <STATUSES>` | Filter by status (e.g. `enqueued,processing`) |
| `--types <TYPES>` | Filter by type (e.g. `documentAdditionOrUpdate`) |
| `--index-uids <INDEX_UIDS>` | Filter by index UIDs |
| `--canceled-by <UIDS>` | Filter by canceling task UIDs |
| `--before-enqueued-at <DATE>` | Filter tasks enqueued before date |
| `--after-enqueued-at <DATE>` | Filter tasks enqueued after date |
| `--before-started-at <DATE>` | Filter tasks started before date |
| `--after-started-at <DATE>` | Filter tasks started after date |
| `--before-finished-at <DATE>` | Filter tasks finished before date |
| `--after-finished-at <DATE>` | Filter tasks finished after date |
| `--from <N>` | Start from task UID |
| `--limit <N>` | Maximum number of tasks to return |

#### `task get`

Get a task by ID.

```bash
meilisearch task get <TASK_ID>
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<TASK_ID>` | Yes | Task UID |

#### `task cancel`

Cancel tasks matching filters. Takes the same filter options as `task list`.

```bash
meilisearch task cancel [--uids <UIDS>] [--statuses <STATUSES>] [...]
```

#### `task delete`

Delete tasks matching filters. Takes the same filter options as `task list`.

```bash
meilisearch task delete [--uids <UIDS>] [--statuses <STATUSES>] [...]
```

#### `task wait`

Wait for a task to complete.

```bash
meilisearch task wait <TASK_ID> [--timeout <MS>]
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<TASK_ID>` | Yes | Task UID |

| Option | Default | Description |
|--------|---------|-------------|
| `--timeout <MS>` | `60000` | Timeout in milliseconds |

#### `task watch`

Watch a task until completion (polls and displays status).

```bash
meilisearch task watch <TASK_ID>
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<TASK_ID>` | Yes | Task UID |

---

### `batch` - Manage batches

#### `batch list`

List batches.

```bash
meilisearch batch list [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--limit <N>` | Maximum number of batches to return |
| `--from <N>` | Start from batch UID |
| `--uids <UIDS>` | Filter by batch UIDs |
| `--index-uids <INDEX_UIDS>` | Filter by index UIDs |
| `--statuses <STATUSES>` | Filter by status |
| `--types <TYPES>` | Filter by type |

#### `batch get`

Get a batch by UID.

```bash
meilisearch batch get <BATCH_UID>
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<BATCH_UID>` | Yes | Batch UID |

---

## Server

### `health` - Check server health

```bash
meilisearch health
```

### `version` - Show server version

```bash
meilisearch version
```

Shows CLI version, Meilisearch server version, and whether a CLI update is available.

### `stats` - Show server stats

```bash
meilisearch stats
```

### `metrics` - Show Prometheus metrics

```bash
meilisearch metrics
```

Returns raw Prometheus-format metrics from the server.

---

### `key` - Manage API keys

#### `key list`

List all API keys.

```bash
meilisearch key list
```

#### `key get`

Get an API key.

```bash
meilisearch key get <API_KEY>
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<API_KEY>` | Yes | API key or UID |

#### `key create`

Create an API key.

```bash
meilisearch key create --actions <ACTIONS> --indexes <INDEXES> [--description <DESC>] [--expires-at <DATE>]
```

| Option | Description |
|--------|-------------|
| `--actions <ACTIONS>` | Comma-separated actions (e.g. `search,documents.add`) |
| `--indexes <INDEXES>` | Comma-separated index UIDs (e.g. `movies,*`) |
| `--description <DESC>` | Human-readable description |
| `--expires-at <DATE>` | Expiration date (ISO 8601) |

#### `key update`

Update an API key.

```bash
meilisearch key update <API_KEY> [--description <DESC>] [--name <NAME>]
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<API_KEY>` | Yes | API key or UID |

| Option | Description |
|--------|-------------|
| `--description <DESC>` | New description |
| `--name <NAME>` | New name |

#### `key delete`

Delete an API key.

```bash
meilisearch key delete <API_KEY>
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<API_KEY>` | Yes | API key or UID |

---

### `log` - Manage logs

#### `log stderr`

Update the target of stderr logs.

```bash
meilisearch log stderr <TARGET>
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<TARGET>` | Yes | Log target (e.g. `info`, `meilisearch=debug`) |

#### `log stream`

Start streaming logs.

```bash
meilisearch log stream <TARGET> [--mode <MODE>]
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<TARGET>` | Yes | Log target (e.g. `info`, `meilisearch=debug`) |

| Option | Description |
|--------|-------------|
| `--mode <MODE>` | Output mode: `human` or `json` |

#### `log stop`

Stop streaming logs.

```bash
meilisearch log stop
```

---

### `network` - Manage network configuration

#### `network get`

Get current network configuration.

```bash
meilisearch network get
```

#### `network update`

Update network configuration.

```bash
meilisearch network update [--file <PATH>]
```

| Option | Description |
|--------|-------------|
| `--file <PATH>` | JSON file (reads from stdin if omitted) |

---

### `experimental` - Manage experimental features

#### `experimental get`

Get all experimental features.

```bash
meilisearch experimental get
```

#### `experimental update`

Configure experimental features.

```bash
meilisearch experimental update [--file <PATH>]
```

| Option | Description |
|--------|-------------|
| `--file <PATH>` | JSON file (reads from stdin if omitted) |

---

## Configuration

### `project` - Manage project credentials

Projects are stored in `~/.config/meilisearch/config.toml`.

#### `project add`

Add a new project.

```bash
meilisearch project add <NAME> --url <URL> [--api-key <KEY>]
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<NAME>` | Yes | Project name |

| Option | Description |
|--------|-------------|
| `--url <URL>` | Meilisearch instance URL |
| `--api-key <KEY>` | API key for authentication |

#### `project remove`

Remove a project.

```bash
meilisearch project remove <NAME>
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<NAME>` | Yes | Project name to remove |

#### `project list`

List all projects.

```bash
meilisearch project list
```

#### `project use`

Set the default project.

```bash
meilisearch project use <NAME>
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<NAME>` | Yes | Project name to set as default |

#### `project current`

Show the current default project.

```bash
meilisearch project current
```

---

### `local` - Manage local Meilisearch instance

Manages a local Meilisearch instance using Docker (preferred) or a downloaded binary as fallback.

#### `local start`

Start the local Meilisearch instance.

```bash
meilisearch local start
```

#### `local stop`

Stop the local Meilisearch instance.

```bash
meilisearch local stop
```

#### `local restart`

Restart the local Meilisearch instance.

```bash
meilisearch local restart
```

#### `local status`

Show status of the local instance.

```bash
meilisearch local status
```

#### `local logs`

Show logs of the local instance.

```bash
meilisearch local logs [-f]
```

| Option | Description |
|--------|-------------|
| `-f, --follow` | Follow log output (stream continuously) |

#### `local reset`

Reset local data (wipe and restart fresh).

```bash
meilisearch local reset
```

#### `local upgrade`

Upgrade local Meilisearch to latest version.

```bash
meilisearch local upgrade
```

---

### `self-update` - Update the CLI

```bash
meilisearch self-update [--force]
```

| Option | Description |
|--------|-------------|
| `--force` | Force re-download even if already up to date |

---

### `completions` - Generate shell completions

```bash
meilisearch completions <SHELL>
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<SHELL>` | Yes | Shell to generate completions for: `bash`, `zsh`, `fish`, `powershell`, `elvish` |

**Installation:**

```bash
# Bash
meilisearch completions bash > ~/.local/share/bash-completion/completions/meilisearch

# Zsh
meilisearch completions zsh > ~/.zfunc/_meilisearch

# Fish
meilisearch completions fish > ~/.config/fish/completions/meilisearch.fish
```

The install script automatically sets up completions for your detected shell.
