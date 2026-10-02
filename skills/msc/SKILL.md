---
name: msc
description: Meilisearch via the `msc` CLI. Use when creating or inspecting indexes, adding or importing documents, searching, changing index settings, debugging failed tasks, or moving data between Meilisearch projects.
---

# Meilisearch with `msc`

`msc` speaks JSON to programs: when stdout is piped, every result is one line of compact JSON. Its exit code tells you the outcome. `msc schema` is the source of truth for every command and flag, so look things up there instead of guessing.

## Workflow

1. **Target**: set `MSC_URL` (and `MSC_API_KEY` if the server has keys), or pass `--project <name>` to use a configured project (`msc project list` lists them). Confirm with `msc whoami`. Done when it reports `"reachable":true,"auth":"ok"`.
2. **Discover**: run `msc schema <command…>` for any command you haven't used yet (for example `msc schema settings update`). Done when you know the command's required and positional arguments.
3. **Act**: every write gets `--wait`, so it returns the *finished* task. Make steps re-runnable with `--if-not-exists` / `--if-exists`. Preview destructive or bulk changes with `--dry-run` and read the result before running for real.
4. **Verify**: read back what you changed (`index stats`, `settings get`, `search`) using `--select` to keep only the fields you need. Done when the read-back matches what you intended.

## Exit codes

| Code | Meaning | Next move |
|------|---------|-----------|
| 0 | ok | — |
| 2 | usage | read `error.hint`, then fix the arguments (check `msc schema`) |
| 3 | auth | supply an API key |
| 4 | not found | create the resource or fix the name |
| 5 | API error | read `error.code` and `error.message`; `error.link` explains the code |
| 6 | network | the server is down or the URL is wrong |
| 7 | task failed | the final task is on stdout; its `error.code` says why |
| 8 | timeout | the task is still running: `msc task wait <uid>` |

On failure, stderr holds `{"error": {"kind", "code", "message", "hint"?, …}}`.

## Patterns

```bash
msc index create movies --primary-key id --if-not-exists --wait
msc import movies --file movies.ndjson               # waits; prints {"status","indexedDocuments",…}
echo '[{"id":1,"title":"Dune"}]' | msc document add movies --wait
msc settings update movies --file s.json --dry-run   # {"changes":{key:{from,to}}}
msc settings update movies --file s.json --wait
msc search movies "dune" --filter 'year > 1960' --limit 5 --select id,title
msc task list --statuses failed --limit 5 --select uid,type,error.code,error.message
```

- Filtering and sorting only work on attributes listed in the `filterableAttributes` / `sortableAttributes` settings. Set those first, with `--wait`.
- Data goes in through `--file` or a stdin pipe. Pass `--format ndjson|csv` when piping non-JSON data.
- For a parameter or route without a flag, use `msc search … --body '<json>'` (any search parameter) or `msc api <METHOD> <PATH> -d '<json>'` (any route, same auth, errors and `--wait`).
- Each interactive mode (`-i`, `settings edit`, prompts) has a scriptable form: `settings get` + `settings update --file`, plain `search`, `chat "<question>"`, and `project add <name> --url … --api-key …`.

The full contract (output, NDJSON events, MCP server, recipes): https://github.com/meilisearch/meilisearch-cli/blob/main/docs/agents.md
