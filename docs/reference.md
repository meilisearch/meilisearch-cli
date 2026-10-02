# CLI Reference

Complete reference for every command, subcommand, and option in the Meilisearch CLI.

Using `msc` from a script, CI job, or AI agent? See the [agent guide](agents.md). Run `msc schema` for a machine-readable version of this reference.

## Global Options

These options apply to all commands.

| Option | Env var | Description |
|--------|---------|-------------|
| `--project <NAME>` | `MSC_PROJECT` | Target a project from the config file (overrides default) |
| `--url <URL>` | `MSC_URL` | Meilisearch URL; bypasses the config file entirely |
| `--api-key <KEY>` | `MSC_API_KEY` | API key (overrides the project's key) |
| `-j, --json` | | Output compact JSON (default when stdout is not a terminal). Aliases: `--raw`, `-r` |
| `--pretty` | | Force human-readable output even when piped |
| `-t, --table` | | Render results as a text table: lists (`results`/`hits`) become rows, a single object becomes key/value rows. Applies even when piped; conflicts with `--json` |
| `-q, --quiet` | | No status messages on stderr |
| `--select <PATHS>` | | Keep only these fields in the output (comma-separated dot paths, e.g. `uid,error.code`). Applied to each element of `results`/`hits` lists |
| `--wait` | `MSC_WAIT` | Wait for enqueued tasks to finish and print the final task. Exits with code 7 if the task fails |
| `--wait-timeout <MS>` | | Timeout for `--wait`, in milliseconds (default `300000`) |
| `-h, --help` | | Print help |
| `-V, --version` | | Print version |

Connection resolution: `--url` wins and never reads the config file; otherwise `--project` (or the default project) is used. `--api-key` overrides the key in both cases.

```bash
# One-off command against any instance, no config needed
MSC_URL=http://localhost:7700 MSC_API_KEY=masterKey msc index list

# Write and wait for indexing in one step
msc document add movies --file movies.json --wait

# Keep only what you need
msc index list --select uid,primaryKey
```

---

## Output & Exit Codes

**stdout** carries only the command's result. **stderr** carries status messages and, on failure, the error.

- When stdout is a terminal, results are pretty-printed and colored. When stdout is not a terminal (piped, captured by a script or agent), or with `--json`, results are compact single-line JSON. Use `--pretty` to force human output.
- Status and progress messages (`Waiting for task…`, progress bars) go to stderr. They are silenced with `--quiet` and in JSON mode, where stderr is reserved for the JSON error.
- Streaming commands (`task watch`, `import --events`, `chat --events`) emit NDJSON: one JSON event per line.
- `NO_COLOR` disables colored output.
- `MSC_CONFIG` overrides the config file path (default `~/.config/msc/config.toml`).

In JSON mode, errors are printed to stderr as:

```json
{"error":{"kind":"not_found","code":"index_not_found","message":"Index `movies` not found.","exitCode":4,"type":"invalid_request","link":"https://docs.meilisearch.com/errors#index_not_found","httpStatus":404}}
```

`kind`, `code`, `message` and `exitCode` are always present; `httpStatus`, `type`, `link`, `hint` and `taskUid` are included when relevant. In human mode the same error prints as `Error: <message>` followed by `hint:` and `docs:` lines.

| Exit code | Kind | Meaning |
|-----------|------|---------|
| `0` | | Success |
| `1` | `general` | Unclassified error |
| `2` | `usage` | Invalid arguments, input, or configuration; interactive command without a terminal |
| `3` | `auth` | Missing or invalid API key (HTTP 401/403) |
| `4` | `not_found` | Resource not found (HTTP 404) |
| `5` | `api` | Other Meilisearch API error |
| `6` | `network` | Meilisearch server unreachable |
| `7` | `task_failed` | Task finished as `failed` or `canceled` (the final task is printed on stdout) |
| `8` | `timeout` | Timed out waiting for a task |

Commands that need a terminal (`settings edit`, `search -i`, `chat -i`, prompts in `project add`/`project update`) fail fast with exit code `2` and error code `non_interactive` instead of hanging.

---

## Data

### `index` - Manage indexes

#### `index list`

List all indexes.

```bash
msc index list [--offset <N>] [--limit <N>]
```

| Option | Description |
|--------|-------------|
| `--offset <N>` | Number of indexes to skip |
| `--limit <N>` | Maximum number of indexes to return |

#### `index create`

Create an index.

```bash
msc index create <INDEX_UID> [--primary-key <KEY>] [--if-not-exists]
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<INDEX_UID>` | Yes | Unique identifier for the index |

| Option | Description |
|--------|-------------|
| `--primary-key <KEY>` | Primary key attribute |
| `--if-not-exists` | Succeed without changes if the index already exists. Prints `{"skipped": true, "reason": "already_exists", "index": {...}}` |

#### `index get`

Get index info.

```bash
msc index get <INDEX_UID>
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<INDEX_UID>` | Yes | Index to retrieve |

#### `index delete`

Delete an index.

```bash
msc index delete <INDEX_UID> [--if-exists] [--dry-run]
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<INDEX_UID>` | Yes | Index to delete |

| Option | Description |
|--------|-------------|
| `--if-exists` | Succeed without changes if the index does not exist. Prints `{"skipped": true, "reason": "not_found", ...}` |
| `--dry-run` | Show the index and its document count without deleting it |

#### `index stats`

Show index stats.

```bash
msc index stats <INDEX_UID>
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<INDEX_UID>` | Yes | Index to get stats for |

#### `index update`

Update an index (change primary key).

```bash
msc index update <INDEX_UID> [--primary-key <KEY>]
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
msc index swap <INDEX_A> <INDEX_B>
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<INDEX_A>` | Yes | First index UID |
| `<INDEX_B>` | Yes | Second index UID |

---

### `document` - Manage documents

#### `document add`

Add or replace documents from a file or stdin.

```bash
msc document add <INDEX_UID> [--file <PATH>] [--format <FORMAT>] [--primary-key <KEY>]
```

Reads from stdin if `--file` is not provided (and errors instead of waiting if stdin is a terminal). Supports JSON, NDJSON (`.ndjson`, `.jsonl`), and CSV (`.csv`) formats.

| Argument | Required | Description |
|----------|----------|-------------|
| `<INDEX_UID>` | Yes | Target index |

| Option | Description |
|--------|-------------|
| `--file <PATH>` | Path to the documents file |
| `--format <FORMAT>` | Payload format: `json`, `ndjson` or `csv` (default: from file extension, else `json`). Use it for NDJSON/CSV on stdin |
| `--primary-key <KEY>` | Primary key attribute (only used if the index has none yet) |

```bash
cat movies.ndjson | msc document add movies --format ndjson --wait
```

#### `document update`

Add or update documents (partial update).

```bash
msc document update <INDEX_UID> [--file <PATH>] [--format <FORMAT>] [--primary-key <KEY>]
```

Same options as `document add`. Uses PUT instead of POST, so existing documents are partially updated.

#### `document get`

Get a single document.

```bash
msc document get <INDEX_UID> <DOCUMENT_ID>
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<INDEX_UID>` | Yes | Target index |
| `<DOCUMENT_ID>` | Yes | Document ID to retrieve |

#### `document list`

List documents.

```bash
msc document list <INDEX_UID> [--offset <N>] [--limit <N>] [--fields <FIELDS>]
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
msc document fetch <INDEX_UID> [--filter <EXPR>] [--offset <N>] [--limit <N>] [--fields <F1,F2>]
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
msc document delete <INDEX_UID> <DOCUMENT_ID>
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<INDEX_UID>` | Yes | Target index |
| `<DOCUMENT_ID>` | Yes | Document ID to delete |

#### `document delete-all`

Delete all documents in an index.

```bash
msc document delete-all <INDEX_UID> [--dry-run]
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<INDEX_UID>` | Yes | Target index |

| Option | Description |
|--------|-------------|
| `--dry-run` | Show how many documents would be deleted (`matchedDocuments`) without deleting them |

#### `document delete-by-filter`

Delete documents matching a filter.

```bash
msc document delete-by-filter <INDEX_UID> <FILTER> [--dry-run]
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<INDEX_UID>` | Yes | Target index |
| `<FILTER>` | Yes | Filter expression (e.g. `"genre = horror"`) |

| Option | Description |
|--------|-------------|
| `--dry-run` | Show how many documents match (`matchedDocuments`) without deleting them |

#### `document delete-batch`

Delete documents by batch of IDs.

```bash
msc document delete-batch <INDEX_UID> <ID1,ID2,...>
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<INDEX_UID>` | Yes | Target index |
| `<IDS>` | Yes | Comma-separated document IDs |

#### `document edit`

Edit documents by function.

```bash
msc document edit <INDEX_UID> <FUNCTION> [--filter <EXPR>]
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
msc settings get <INDEX_UID> [SUB_RESOURCE]
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<INDEX_UID>` | Yes | Target index |
| `[SUB_RESOURCE]` | No | Specific sub-resource to retrieve (see [sub-resources](#settings-sub-resources)) |

#### `settings update`

Update settings from JSON file or stdin.

```bash
msc settings update <INDEX_UID> [SUB_RESOURCE] [--file <PATH>] [--dry-run]
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<INDEX_UID>` | Yes | Target index |
| `[SUB_RESOURCE]` | No | Specific sub-resource to update |

| Option | Description |
|--------|-------------|
| `--file <PATH>` | JSON file (reads from stdin if omitted) |
| `--dry-run` | Show the changes that would be applied without applying them |

With `--dry-run`, only the settings whose value would change are listed:

```json
{"dryRun":true,"action":"settings.update","indexUid":"movies","changes":{"searchableAttributes":{"from":["*"],"to":["title"]}}}
```

#### `settings reset`

Reset settings to defaults.

```bash
msc settings reset <INDEX_UID> [SUB_RESOURCE] [--dry-run]
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<INDEX_UID>` | Yes | Target index |
| `[SUB_RESOURCE]` | No | Specific sub-resource to reset (resets all if omitted) |

| Option | Description |
|--------|-------------|
| `--dry-run` | Show the current values that would be reset without resetting them |

#### `settings edit`

Edit settings in `$EDITOR` or interactive TUI. Requires an interactive terminal; without one it exits with code `2` (`non_interactive`). In scripts, use `settings get` + `settings update --file` instead (add `--dry-run` to preview).

```bash
msc settings edit <INDEX_UID> [SUB_RESOURCE] [-i]
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

Show what changes between the current settings and a settings file.

```bash
msc settings diff <INDEX_UID> [--against <PATH>]
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<INDEX_UID>` | Yes | Target index |

| Option | Description |
|--------|-------------|
| `--against <PATH>` | Settings JSON to compare against (reads stdin if omitted) |

Settings updates are partial, so only keys present in the file are compared. Output: `{"indexUid": "...", "changes": {"<setting>": {"from": ..., "to": ...}}}`.

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
msc import <INDEX_UID> [--file <PATH>] [--format <FORMAT>] [--primary-key <KEY>] [--batch-size <BYTES>] [--no-wait] [--events]
```

Imports documents with a progress bar (on stderr, hidden in JSON mode). NDJSON is split on lines and JSON arrays on elements into batches of at most `--batch-size` bytes; CSV is sent as a single batch. All batches are enqueued back to back, then the command waits for every task and fails with exit code `7` if one fails. Reads from stdin if `--file` is not provided.

| Argument | Required | Description |
|----------|----------|-------------|
| `<INDEX_UID>` | Yes | Target index |

| Option | Default | Description |
|--------|---------|-------------|
| `--file <PATH>` | stdin | Path to the file (JSON, NDJSON, or CSV) |
| `--format <FORMAT>` | from extension, else `json` | Payload format: `json`, `ndjson` or `csv` |
| `--primary-key <KEY>` | | Primary key field name |
| `--batch-size <BYTES>` | `20971520` (20 MiB) | Maximum batch size in bytes |
| `--no-wait` | | Return as soon as all batches are enqueued, without waiting for indexing |
| `--events` | | Emit NDJSON progress events on stdout instead of a single summary |

The result is a summary:

```json
{"indexUid":"movies","batches":2,"bytes":20,"taskUids":[3,4],"status":"succeeded","indexedDocuments":2}
```

With `--no-wait`, `status` is `enqueued` and `indexedDocuments` is omitted. With `--events`, each line is one of `batch_enqueued`, `task_finished` or `done` (whose `summary` field holds the summary above).

---

## Search

### `search` - Search an index

```bash
msc search <INDEX_UID> [QUERY] [OPTIONS]
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
| `--body <JSON>` | Extra search parameters as a JSON object, merged over the flags (any [search parameter](https://www.meilisearch.com/docs/reference/api/search), e.g. `hybrid`, `showRankingScore`, `matchingStrategy`) |
| `-i, --interactive` | Open interactive search TUI |

```bash
msc search movies "space opera" --limit 5 \
  --body '{"hybrid":{"embedder":"default","semanticRatio":0.7},"showRankingScore":true}'
```

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
msc multi-search [--file <PATH>]
```

| Option | Description |
|--------|-------------|
| `--file <PATH>` | JSON file with a queries array or a full request body (reads from stdin if omitted) |

Example input, as a bare queries array:

```json
[
  { "indexUid": "movies", "q": "action" },
  { "indexUid": "books", "q": "fiction" }
]
```

Or as a full body, for example a federated search:

```json
{
  "federation": { "limit": 10 },
  "queries": [
    { "indexUid": "movies", "q": "action" },
    { "indexUid": "books", "q": "action" }
  ]
}
```

---

### `facet-search` - Perform a facet search

```bash
msc facet-search <INDEX_UID> <FACET_NAME> [--facet-query <QUERY>] [--filter <EXPR>]
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
msc similar <INDEX_UID> <DOCUMENT_ID> [--limit <N>] [--offset <N>] [--filter <EXPR>] [--embedder <NAME>]
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<INDEX_UID>` | Yes | Target index |
| `<DOCUMENT_ID>` | Yes | Document ID to find similar documents for |

| Option | Description |
|--------|-------------|
| `--limit <N>` | Maximum number of results |
| `--offset <N>` | Number of results to skip |
| `--filter <EXPR>` | Filter expression |
| `--embedder <NAME>` | Embedder to use |

---

### `chat` - Interactive chat with your data

```bash
msc chat [MESSAGE] [--workspace <UID>] [--model <MODEL>] [-i] [--events]
```

| Argument | Required | Description |
|----------|----------|-------------|
| `[MESSAGE]` | Yes, unless `-i` | Chat message |

| Option | Default | Description |
|--------|---------|-------------|
| `--workspace <UID>` | `cloud` | Workspace UID |
| `--model <MODEL>` | `gpt-4o-mini` | Model to use (passed through to LLM provider) |
| `-i, --interactive` | | Open interactive chat TUI (requires a terminal) |
| `--events` | | Emit NDJSON events (`delta`, `sources`, `done`) as the answer streams |

In a terminal, the answer streams as plain text. In JSON mode, the command waits for the full answer and prints one object, including the documents Meilisearch used as sources:

```json
{"answer":"…","sources":[{"id":1,"title":"Dune"}],"model":"gpt-4o-mini","workspace":"cloud"}
```

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
msc clone <SOURCE_UID> <DEST_UID> [--from <PROJECT>] [--to <PROJECT>] [--dry-run]
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<SOURCE_UID>` | Yes | Source index UID |
| `<DEST_UID>` | Yes | Destination index UID |

| Option | Description |
|--------|-------------|
| `--from <PROJECT>` | Source project (uses `--url` or the default project if omitted) |
| `--to <PROJECT>` | Destination project (uses the source project if omitted) |
| `--dry-run` | Show the source document count and settings, and whether the destination exists, without writing anything |

Result: `{"source": {...}, "destination": {...}, "documentsCopied": N, "stats": {...}}`.

---

### `promote` - Promote indexes between projects

```bash
msc promote --to <PROJECT> [--from <PROJECT>] [--indexes <I1,I2>] [--dry-run] [--create]
```

| Option | Default | Description |
|--------|---------|-------------|
| `--from <PROJECT>` | `local` | Source project |
| `--to <PROJECT>` | *required* | Destination project |
| `--indexes <I1,I2>` | all | Only promote specific indexes (comma-separated) |
| `--dry-run` | | Show what would be promoted (document counts, whether each destination index exists) without executing |
| `--create` | | Create destination indexes if they don't exist |

In JSON mode the result lists each promoted index with its document count, the method used (`export` route or paginated `copy`) and the time taken.

---

### `dump` - Manage dumps

#### `dump create`

Create a dump.

```bash
msc dump create
```

#### `dump snapshot`

Create a snapshot.

```bash
msc dump snapshot
```

---

### `task` - Manage tasks

#### Task filters

`task list`, `task cancel` and `task delete` share these filters:

| Option | Description |
|--------|-------------|
| `--uids <UIDS>` | Comma-separated task UIDs |
| `--statuses <STATUSES>` | Comma-separated statuses: `enqueued`, `processing`, `succeeded`, `failed`, `canceled` |
| `--types <TYPES>` | Comma-separated task types (e.g. `documentAdditionOrUpdate`, `indexCreation`, `settingsUpdate`) |
| `--index-uids <INDEX_UIDS>` | Comma-separated index UIDs |
| `--canceled-by <UIDS>` | Comma-separated UIDs of the canceling tasks |
| `--before-enqueued-at <DATE>` | Enqueued before this RFC 3339 date |
| `--after-enqueued-at <DATE>` | Enqueued after this RFC 3339 date |
| `--before-started-at <DATE>` | Started before this RFC 3339 date |
| `--after-started-at <DATE>` | Started after this RFC 3339 date |
| `--before-finished-at <DATE>` | Finished before this RFC 3339 date |
| `--after-finished-at <DATE>` | Finished after this RFC 3339 date |

#### `task list`

List tasks with optional [filters](#task-filters).

```bash
msc task list [FILTERS] [--from <N>] [--limit <N>]
```

| Option | Description |
|--------|-------------|
| `--from <N>` | Start listing from this task UID (pagination cursor) |
| `--limit <N>` | Maximum number of tasks to return |

#### `task get`

Get a task by ID.

```bash
msc task get <TASK_ID>
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<TASK_ID>` | Yes | Task UID |

#### `task cancel`

Cancel tasks matching [filters](#task-filters).

```bash
msc task cancel [FILTERS] [--dry-run]
```

| Option | Description |
|--------|-------------|
| `--dry-run` | Show the matching tasks without canceling them: `{"dryRun": true, "matchedTasks": N, "sampleUids": [...]}`. Without `--statuses`, only `enqueued` and `processing` tasks are counted, since only those can be canceled |

#### `task delete`

Delete tasks matching [filters](#task-filters).

```bash
msc task delete [FILTERS] [--dry-run]
```

| Option | Description |
|--------|-------------|
| `--dry-run` | Show the matching tasks without deleting them |

#### `task wait`

Wait for a task to finish and print it. Exits with code `7` if the task failed or was canceled (the task is still printed on stdout), and `8` on timeout.

```bash
msc task wait <TASK_ID> [--timeout <MS>]
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<TASK_ID>` | Yes | Task UID |

| Option | Default | Description |
|--------|---------|-------------|
| `--timeout <MS>` | `--wait-timeout` (`300000`) | Timeout in milliseconds |

#### `task watch`

Watch a task until completion. In a terminal, a status line updates on stderr and the final task is printed. In JSON mode, NDJSON events are emitted: `{"event": "status", ...}` on each status change, then `{"event": "done", "task": {...}}`. Exits with code `7` if the task did not succeed.

```bash
msc task watch <TASK_ID>
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<TASK_ID>` | Yes | Task UID |

---

### `batch` - Manage batches

#### `batch list`

List batches.

```bash
msc batch list [OPTIONS]
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
msc batch get <BATCH_UID>
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<BATCH_UID>` | Yes | Batch UID |

---

## Server

### `health` - Check server health

```bash
msc health
```

### `version` - Show server version

```bash
msc version
```

Shows CLI version, Meilisearch server version, and (in a terminal) whether a CLI update is available. Printed on stdout; in JSON mode: `{"cli": "0.4.0", "server": {"pkgVersion": "1.x", ...}}`.

### `stats` - Show server stats

```bash
msc stats
```

### `metrics` - Show Prometheus metrics

```bash
msc metrics
```

Returns raw Prometheus-format metrics from the server.

### `whoami` - Show the target and check credentials

```bash
msc whoami
```

Shows which server a command would use and where that came from (`flag --url`, `env MSC_URL`, `flag --project`, `env MSC_PROJECT` or the config default), whether an API key is set and where it came from (the key itself is never printed), and probes the server:

```json
{"project":"prod","url":"https://…","source":"config default project","apiKey":{"set":true,"source":"project 'prod'"},"configFile":"/Users/me/.config/msc/config.toml","reachable":true,"auth":"ok","version":"1.54.2"}
```

`auth` is `ok`, `key_required` or `invalid_key`. The report is always printed; the exit code is `0` when everything works, `6` when the server is unreachable and `3` when authentication fails.

---

### `key` - Manage API keys

#### `key list`

List all API keys.

```bash
msc key list [--offset <N>] [--limit <N>]
```

| Option | Description |
|--------|-------------|
| `--offset <N>` | Number of keys to skip |
| `--limit <N>` | Maximum number of keys to return |

#### `key get`

Get an API key.

```bash
msc key get <API_KEY>
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<API_KEY>` | Yes | API key or UID |

#### `key create`

Create an API key.

```bash
msc key create --actions <ACTIONS> --indexes <INDEXES> [--description <DESC>] [--expires-at <DATE>]
```

| Option | Description |
|--------|-------------|
| `--actions <ACTIONS>` | Comma-separated actions (e.g. `search,documents.add`) |
| `--indexes <INDEXES>` | Comma-separated index UIDs or patterns (`*` for all) |
| `--description <DESC>` | Key description |
| `--expires-at <DATE>` | Expiration date (RFC 3339), omit for no expiry |

#### `key update`

Update an API key.

```bash
msc key update <API_KEY> [--description <DESC>] [--name <NAME>]
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
msc key delete <API_KEY> [--if-exists] [--dry-run]
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<API_KEY>` | Yes | API key or UID |

| Option | Description |
|--------|-------------|
| `--if-exists` | Succeed without changes if the key does not exist |
| `--dry-run` | Show the key that would be deleted without deleting it |

---

### `log` - Manage logs

#### `log stderr`

Update the target of stderr logs.

```bash
msc log stderr <TARGET>
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<TARGET>` | Yes | Log target (e.g. `info`, `meilisearch=debug`) |

#### `log stream`

Stream logs to stdout until interrupted (Ctrl+C). Requires the `logsRoute` experimental feature.

```bash
msc log stream <TARGET> [--mode <MODE>]
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<TARGET>` | Yes | Log target (e.g. `info`, `meilisearch=debug`) |

| Option | Description |
|--------|-------------|
| `--mode <MODE>` | Output mode: `human`, `json` (NDJSON) or `profile`. Defaults to `json` when output is JSON |

#### `log stop`

Stop streaming logs.

```bash
msc log stop
```

---

### `network` - Manage network configuration

#### `network get`

Get current network configuration.

```bash
msc network get
```

#### `network update`

Update network configuration.

```bash
msc network update [--file <PATH>]
```

| Option | Description |
|--------|-------------|
| `--file <PATH>` | JSON file (reads from stdin if omitted) |

---

### `experimental` - Manage experimental features

#### `experimental get`

Get all experimental features.

```bash
msc experimental get
```

#### `experimental update`

Configure experimental features.

```bash
msc experimental update [--file <PATH>]
```

| Option | Description |
|--------|-------------|
| `--file <PATH>` | JSON file (reads from stdin if omitted) |

---

## Configuration

### `project` - Manage project credentials

Projects are stored in `~/.config/msc/config.toml` (override with `MSC_CONFIG`). Project commands never print API keys; JSON output reports `apiKeySet` instead.

#### `project add`

Add a new project. The URL and key come from the global `--url` and `--api-key` options (or `MSC_URL` / `MSC_API_KEY`). In a terminal, missing values are prompted for; without a terminal, a missing `--url` is an error (exit code `2`) and a missing key means no key.

```bash
msc project add <NAME> --url <URL> [--api-key <KEY>] [--if-not-exists | --force]
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<NAME>` | Yes | Project name |

| Option | Description |
|--------|-------------|
| `--url <URL>` | Meilisearch instance URL (global option) |
| `--api-key <KEY>` | API key for authentication (global option) |
| `--if-not-exists` | Succeed without changes if the project already exists |
| `--force` | Overwrite the project if it already exists |

#### `project remove`

Remove a project.

```bash
msc project remove <NAME> [--if-exists]
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<NAME>` | Yes | Project name to remove |

| Option | Description |
|--------|-------------|
| `--if-exists` | Succeed without changes if the project does not exist |

#### `project update`

Update an existing project. With any of the options below, only the given fields change and nothing is prompted; with none, the URL and key are prompted for (requires a terminal).

```bash
msc project update <NAME> [--url <URL>] [--api-key <KEY> | --remove-api-key]
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<NAME>` | Yes | Project name |

| Option | Description |
|--------|-------------|
| `--url <URL>` | New URL (global option) |
| `--api-key <KEY>` | New API key (global option) |
| `--remove-api-key` | Remove the stored API key |

#### `project list`

List all projects. In JSON mode: `{"default": "local", "results": [{"name", "url", "default", "apiKeySet", "type"}]}`.

```bash
msc project list
```

#### `project use`

Set the default project.

```bash
msc project use <NAME>
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<NAME>` | Yes | Project name to set as default |

#### `project current`

Show the current default project.

```bash
msc project current
```

---

### `local` - Manage local Meilisearch instance

Manages a local Meilisearch instance using Docker (preferred) or a downloaded binary as fallback. `start`, `stop`, `restart`, `status`, `reset` and `upgrade` return a JSON object in JSON mode (e.g. `{"status": "running", "mode": "docker", "url": "http://127.0.0.1:7700"}`); Docker's own output never reaches stdout.

#### `local start`

Start the local Meilisearch instance.

```bash
msc local start
```

#### `local stop`

Stop the local Meilisearch instance.

```bash
msc local stop
```

#### `local restart`

Restart the local Meilisearch instance.

```bash
msc local restart
```

#### `local status`

Show status of the local instance.

```bash
msc local status
```

#### `local logs`

Show logs of the local instance.

```bash
msc local logs [-f]
```

| Option | Description |
|--------|-------------|
| `-f, --follow` | Follow log output (stream continuously) |

#### `local reset`

Reset local data (wipe and restart fresh).

```bash
msc local reset [--dry-run]
```

| Option | Description |
|--------|-------------|
| `--dry-run` | Show the data directory and its size without wiping anything |

#### `local upgrade`

Upgrade local Meilisearch to latest version.

```bash
msc local upgrade
```

---

### `self-update` - Update the CLI

```bash
msc self-update [--force]
```

| Option | Description |
|--------|-------------|
| `--force` | Force re-download even if already up to date |

---

### `completions` - Generate shell completions

```bash
msc completions <SHELL>
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<SHELL>` | Yes | Shell to generate completions for: `bash`, `zsh`, `fish`, `powershell`, `elvish` |

**Installation:**

```bash
# Bash
msc completions bash > ~/.local/share/bash-completion/completions/msc

# Zsh
msc completions zsh > ~/.zfunc/_msc

# Fish
msc completions fish > ~/.config/fish/completions/msc.fish
```

The install script automatically sets up completions for your detected shell.

---

## Agents

See the [agent guide](agents.md) for recommended patterns.

### `api` - Call any API route

```bash
msc api <METHOD> <PATH> [-d <JSON> | --file <PATH> | stdin] [-p KEY=VALUE]... [--content-type <TYPE>]
```

The escape hatch for routes and parameters without a dedicated command. It uses the same target, API key, error handling, exit codes, `--wait` and `--select` as every other command.

| Argument / Option | Description |
|-------------------|-------------|
| `<METHOD>` | `GET`, `POST`, `PUT`, `PATCH` or `DELETE` (case-insensitive) |
| `<PATH>` | Route path starting with `/` |
| `-d, --data <JSON>` | Inline request body |
| `--file <PATH>` | Request body file. Without `--data` or `--file`, a piped stdin is used as the body for non-GET requests |
| `-p, --param <KEY=VALUE>` | Query parameter, repeatable (values are URL-encoded) |
| `--content-type <TYPE>` | Body content type (default `application/json`, which is validated before sending) |

```bash
msc api GET /indexes/movies/settings/ranking-rules
msc api GET /indexes/movies/documents -p limit=5 -p fields=id,title
msc api PATCH /experimental-features -d '{"metrics": true}'
msc api POST /indexes/movies/documents --content-type application/x-ndjson --wait < movies.ndjson
```

Writes that enqueue a task return the summarized task, or the finished task with `--wait`. Responses that are not JSON (such as `/metrics`) are printed as text, or as a JSON string in JSON mode.

### `schema` - Describe the CLI as JSON

```bash
msc schema [COMMAND...]
```

Prints the full command tree as JSON: every command, argument (name, flag, type, required, default, allowed values, env var), the global options, the exit codes and the environment variables. Pass a command path to describe a single command.

| Argument | Required | Description |
|----------|----------|-------------|
| `[COMMAND...]` | No | Only describe this command (e.g. `index create`) |

```bash
msc schema index create
```

```json
{"name":"create","usage":"msc index create","description":"Create an index","args":[{"name":"uid","type":"string","required":true,"description":"Index UID","positional":true},{"name":"primary_key","type":"string","required":false,"description":"Primary key attribute","flag":"--primary-key"},{"name":"if_not_exists","type":"boolean","required":false,"description":"Succeed without changes if the index already exists","flag":"--if-not-exists"}]}
```

### `mcp` - Run as an MCP server

```bash
msc mcp [--read-only] [--tools <NAMES>]
```

Runs a [Model Context Protocol](https://modelcontextprotocol.io) server over stdio. Every runnable command becomes a tool named after its path (`index_create`, `document_add`, `search`, `task_wait`, …), generated from the same definitions as `msc schema`. Each tool call runs `msc` itself with `--json`, so results and errors are identical to the CLI. Commands that need a terminal or never finish (`settings edit`, `task watch`, `log stream`, `-i` modes) are not exposed.

Every tool also accepts `project` and `select`; write tools accept `wait`. Tools that read a file (`document_add`, `settings_update`, `import`, `multi_search`, …) accept an `input` argument with the content inline. Read-only tools are annotated with `readOnlyHint`, deletions and resets with `destructiveHint`.

| Option | Description |
|--------|-------------|
| `--read-only` | Only expose read-only tools (list, get, search, …) |
| `--tools <NAMES>` | Only expose these tools (comma-separated names; a trailing `*` matches a prefix, e.g. `index_*,search`) |

Connection options given to `msc mcp` (`--url`, `--api-key`, `--project`) are forwarded to every tool call. `MSC_MCP_TIMEOUT_SECS` sets the per-call timeout (default `600`).

Add it to Claude Code:

```bash
claude mcp add meilisearch -- msc mcp
```

Or in any MCP client configuration:

```json
{
  "mcpServers": {
    "meilisearch": {
      "command": "msc",
      "args": ["mcp"],
      "env": {
        "MSC_URL": "http://localhost:7700",
        "MSC_API_KEY": "your-api-key"
      }
    }
  }
}
```

---

### `skill` - Install the agent skill

`msc` bundles an [agent skill](../skills/msc/SKILL.md) that teaches coding agents how to use it: targeting a server, `msc schema`, `--wait`, `--dry-run`, `--select` and exit codes. The skill is compiled into the binary, so it always matches the installed version.

```bash
msc skill install            # ~/.claude/skills/msc/SKILL.md (all your projects)
msc skill install --local    # ./.claude/skills/msc/SKILL.md (this repository only)
msc skill install --dir DIR  # any skills directory, e.g. for another agent
msc skill list               # bundled skills and where they are installed
msc skill show               # print SKILL.md
msc skill uninstall          # same --local / --dir options
```

`install` reports `installed`, `updated` or `unchanged` for each skill and is safe to re-run. `msc self-update` refreshes a skill installed in `~/.claude/skills`, and `install.sh` installs it automatically when `~/.claude` exists (set `MSC_NO_SKILL=1` to skip).

### Claude Code plugin

The repository is also a Claude Code plugin marketplace. The plugin bundles the skill and the `msc mcp` server, and needs `msc` on your `PATH`:

```
/plugin marketplace add meilisearch/meilisearch-cli
/plugin install meilisearch-cli@meilisearch
```

Set `MSC_URL` / `MSC_API_KEY` (or a default project with `msc project use`) before starting Claude Code so the MCP server knows which instance to use.
