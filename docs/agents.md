# Using msc from scripts and AI agents

`msc` is built to be driven by programs as well as people. This page is the
contract: what goes to stdout and stderr, which exit codes mean what, and the
flags that make multi-step workflows reliable.

If you are an agent, the short version:

```bash
export MSC_URL=http://localhost:7700 MSC_API_KEY=...   # or use --project
msc schema                                              # full command spec as JSON
msc index create movies --primary-key id --if-not-exists --wait
msc import movies --file movies.ndjson                  # waits, prints a JSON summary
msc search movies "dune" --limit 5 --select id,title
```

Check the exit code. On failure, read the JSON error on stderr.

## Output contract

| Stream | Content |
|--------|---------|
| stdout | The command's result, and nothing else. Compact JSON (one line) when stdout is not a terminal, or with `--json`. |
| stderr | On failure, one JSON error object. In JSON mode, status and progress messages are suppressed. |

- JSON output is automatic when piped. Use `--pretty` to force human output, or `--json` (`-j`; `--raw`/`-r` also work) to force JSON in a terminal.
- Streaming commands (`import --events`, `chat --events`, `task watch`) print **NDJSON**: one JSON event per line, flushed immediately.
- `--quiet` silences status messages in human mode too.
- `--table` (`-t`) renders lists as text tables for people. Agents should stay on JSON.
- `NO_COLOR` disables colors.

### Errors

```json
{"error":{"kind":"not_found","code":"index_not_found","message":"Index `movies` not found.","exitCode":4,"httpStatus":404,"type":"invalid_request","link":"https://docs.meilisearch.com/errors#index_not_found"}}
```

| Field | Notes |
|-------|-------|
| `kind` | One of the exit-code categories below. Branch on this. |
| `code` | Meilisearch error code (`index_not_found`, `invalid_search_filter`, …) or a CLI code (`missing_argument`, `non_interactive`, `connection_failed`, `task_timeout`, …). |
| `message` | Human-readable explanation. |
| `hint` | Optional. What to do next (e.g. which flag to pass). |
| `httpStatus`, `type`, `link` | Present for API errors. |
| `taskUid` | Present when an awaited task failed. The full task is on stdout. |

### Exit codes

| Code | Kind | Meaning |
|------|------|---------|
| 0 | — | Success |
| 1 | `general` | Unclassified error |
| 2 | `usage` | Invalid arguments or input, unknown project, or a prompt/TUI requested without a terminal |
| 3 | `auth` | Missing or invalid API key (HTTP 401/403) |
| 4 | `not_found` | Resource not found (HTTP 404) |
| 5 | `api` | Any other Meilisearch API error |
| 6 | `network` | Server unreachable |
| 7 | `task_failed` | An awaited task finished as `failed` or `canceled` |
| 8 | `timeout` | `--wait` / `task wait` timed out; the task may still finish |

## Connecting

Pick one of these. The first match wins:

1. `--url <URL>` / `MSC_URL`: talks to that server directly and never reads or writes the config file.
2. `--project <NAME>` / `MSC_PROJECT`: a project from `~/.config/msc/config.toml`.
3. The default project in the config file.

`--api-key` / `MSC_API_KEY` overrides the key in every case. Prefer the environment variable or a configured project over `--api-key`, since command-line arguments are visible to other processes.

`msc whoami` shows the resolved target, where each setting came from, and whether the server is reachable and the key works. It exits 0, 6 (unreachable) or 3 (auth), so it doubles as a preflight check. `MSC_CONFIG` points at another config file, which is useful in sandboxes and tests.

## Asynchronous writes: `--wait`

Meilisearch writes (documents, settings, index creation and deletion) are asynchronous. They return a summarized task:

```json
{"taskUid":42,"indexUid":"movies","status":"enqueued","type":"documentAdditionOrUpdate","enqueuedAt":"…"}
```

Add `--wait` (or set `MSC_WAIT=true`) to any write and `msc` polls until the task finishes. It then prints the **final task** and:

- exits 0 if the task `succeeded`
- exits 7 if it `failed` or was `canceled`, with the task on stdout and the error (including the task's error `code`) on stderr
- exits 8 if `--wait-timeout` (milliseconds, default 300000) elapses

You can also wait later with `msc task wait <uid>`, which uses the same exit codes.

`import` always waits, unless you pass `--no-wait`.

## Safe retries: idempotency flags

| Command | Flag | Result when there's nothing to do |
|---------|------|-----------------------------------|
| `index create` | `--if-not-exists` | `{"skipped":true,"reason":"already_exists","index":{…}}` |
| `index delete` | `--if-exists` | `{"skipped":true,"reason":"not_found",…}` |
| `key delete` | `--if-exists` | same |
| `project add` | `--if-not-exists` (or `--force` to overwrite) | same |
| `project remove` | `--if-exists` | same |

All of these exit 0. Adding documents is already an upsert, and settings updates are already idempotent.

## Preview before acting: `--dry-run`

Destructive and bulk commands can describe what they would do without changing anything:

| Command | Dry-run output |
|---------|----------------|
| `index delete --dry-run` | the index and its document count |
| `document delete-all --dry-run` | `matchedDocuments` |
| `document delete-by-filter <F> --dry-run` | `matchedDocuments` for the filter |
| `settings update --dry-run` | `changes: {key: {from, to}}`, listing only the keys that would change |
| `settings reset --dry-run` | current values that would be reset |
| `task cancel/delete --dry-run` | `matchedTasks` and up to 20 sample UIDs |
| `key delete --dry-run` | the key |
| `clone --dry-run` | source stats and settings, and whether the destination exists |
| `promote --dry-run` | per-index document counts, and whether each destination exists |
| `local reset --dry-run` | data directory and its size |

Every dry-run result has `"dryRun": true`.

`msc settings diff <index> --against new.json` (or with JSON on stdin) shows the same `changes` object as `settings update --dry-run`.

## Keeping output small: `--select` and pagination

`--select` keeps only the listed fields (comma-separated, dot paths for nested fields). On list and search results, it is applied to each element of `results` or `hits`, and keeps pagination metadata such as `total` and `estimatedTotalHits`:

```bash
msc search movies "dune" --select id,title
msc task list --statuses failed --limit 5 --select uid,type,error.code
msc index list --select uid
```

List commands support `--limit` and `--offset`. `task list` and `batch list` use `--from <uid>` as a cursor instead of an offset.

## Anything without a command: `msc api` and `search --body`

Every route is reachable through `msc api`, with the same auth, errors, exit codes, `--wait` and `--select`:

```bash
msc api GET /indexes/movies/settings/embedders
msc api PATCH /experimental-features -d '{"metrics": true}'
msc api GET /indexes/movies/documents -p limit=5 -p fields=id,title
```

`msc search --body '<json>'` merges any search parameter over the flags: `hybrid`, `showRankingScore`, `matchingStrategy`, `attributesToCrop` and so on.

## Never blocking on input

`msc` never waits for input that a program can't provide:

- Prompts (`project add` / `project update` without flags), `$EDITOR` (`settings edit`) and TUIs (`-i`) exit 2 with `code: "non_interactive"` and a hint when there is no terminal.
- Commands that read data (`document add`, `import`, `settings update`, `multi-search`, …) take `--file`, or read stdin when it is piped. With neither, they exit 2 instead of waiting.
- Use `--format json|ndjson|csv` when piping non-JSON data to `document add` / `document update` / `import`.

Non-interactive equivalents:

| Interactive | Non-interactive |
|-------------|-----------------|
| `msc project add prod` (prompts) | `msc project add prod --url https://… --api-key …` |
| `msc settings edit movies` | `msc settings get movies > s.json`, edit, then `msc settings update movies --file s.json --dry-run` and run again without `--dry-run` |
| `msc search movies -i` | `msc search movies "query"` |
| `msc chat -i` | `msc chat "question"`, which returns `{"answer","sources","model","workspace"}` |

## Streaming events (NDJSON)

```bash
msc import movies --file big.ndjson --events
# {"event":"batch_enqueued","batch":1,"batches":3,"bytes":20971380,"taskUid":51}
# …
# {"event":"task_finished","taskUid":51,"status":"succeeded","indexedDocuments":48211}
# {"event":"done","summary":{"indexUid":"movies","batches":3,"taskUids":[51,52,53],"status":"succeeded","indexedDocuments":131000,…}}

msc chat "best sci-fi movies?" --events
# {"event":"delta","content":"Here"} … {"event":"sources","documents":[…]} … {"event":"done","result":{…}}

msc task watch 51
# {"event":"status","uid":51,"type":"documentAdditionOrUpdate","status":"processing"}
# {"event":"done","task":{…}}
```

`msc log stream <target>` streams the server's logs as NDJSON when output is JSON (it requires the `logsRoute` experimental feature).

## Discovering commands: `msc schema`

`msc schema` prints the whole command tree as JSON: every command, argument, type (`string`, `integer`, `boolean`), whether it's required or positional, defaults, allowed values, global options, environment variables and exit codes. `msc schema index create` describes a single command.

Load it once instead of running `--help` for each command.

## Agent skill and Claude Code plugin

The `msc` skill is a short playbook that agents load when a task involves Meilisearch. It is bundled in the binary:

```bash
msc skill install          # ~/.claude/skills/msc (or --local, or --dir <skills dir>)
```

Or install the skill and the MCP server below in one step as a Claude Code plugin:

```
/plugin marketplace add meilisearch/meilisearch-cli
/plugin install meilisearch-cli@meilisearch
```

## MCP server: `msc mcp`

`msc mcp` runs a [Model Context Protocol](https://modelcontextprotocol.io) server over stdio. Every command becomes a tool (`index_create`, `search`, `document_add`, `settings_update`, `task_wait`, …), with an input schema generated from the same definitions as `msc schema`. Each tool call runs `msc` with `--json`, so results and errors match the CLI exactly.

Extra tool arguments:

- `input`: inline data for commands that take `--file` (documents, settings, multi-search queries). Pass a JSON value, or a string for NDJSON or CSV.
- `wait`: on write tools, return the finished task.
- `select`: same as `--select`.
- `project`: target another configured project.

Tools carry `readOnlyHint` and `destructiveHint` annotations. Terminal-only commands (`settings edit`, TUIs, `task watch`, `log stream`) are not exposed.

```bash
# Claude Code
claude mcp add meilisearch -- msc mcp
claude mcp add meilisearch-prod -- msc --project prod mcp --read-only

# Only expose a few tools
msc mcp --tools 'search,index_*,document_*'
```

Generic `mcpServers` configuration:

```json
{
  "mcpServers": {
    "meilisearch": {
      "command": "msc",
      "args": ["mcp"],
      "env": { "MSC_URL": "http://localhost:7700", "MSC_API_KEY": "…" }
    }
  }
}
```

`MSC_MCP_TIMEOUT_SECS` (default 600) caps how long a single tool call may run.

## Recipes

Set up an index from scratch, safely re-runnable:

```bash
set -e
msc index create products --primary-key sku --if-not-exists --wait
msc settings update products --file settings.json --wait
msc import products --file products.ndjson
msc index stats products --select numberOfDocuments
```

Change settings only after reviewing the diff:

```bash
msc settings update products --file settings.json --dry-run --select changes
msc settings update products --file settings.json --wait
```

Find out why recent tasks failed:

```bash
msc task list --statuses failed --limit 10 --select uid,type,indexUid,error.code,error.message
```

Branch on errors in a shell script:

```bash
msc index get products >/dev/null 2>err.json
case $? in
  0) ;;                                        # exists
  4) msc index create products --wait ;;       # not found
  6) echo "server down" >&2; exit 1 ;;
  *) jq -r .error.message err.json >&2; exit 1 ;;
esac
```
