---
name: msc-promote
description: Moving Meilisearch indexes between environments with `msc`. Use when promoting indexes from local to staging or production, syncing settings across projects, copying an index to another instance, or replacing a production index without downtime.
---

# Promoting indexes with `msc`

Promotion copies settings and documents from one **project** (a configured instance: `local`, `staging`, `prod`, …) to another. It is done when the destination matches the source: same settings, same document count, same results for sample queries.

## Steps

1. **Check both ends.** `msc whoami --project <from>` and `msc whoami --project <to>` both report `"auth":"ok"`. The destination key needs write access (`msc project list` shows configured projects).
2. **Preview.**
   ```bash
   msc promote --from <from> --to <to> --indexes <a>,<b> --dry-run
   ```
   Check each index's `numberOfDocuments`, and `destinationExists`. Missing destinations need `--create`.
3. **Review the settings change.**
   ```bash
   msc settings get <index> --project <from> > settings.json
   msc settings diff <index> --project <to> --against settings.json
   ```
   Done when the user has seen the `changes` that will hit the destination.
4. **Promote**: the method depends on the destination.
   - **Non-production, or the index doesn't exist yet:**
     ```bash
     msc promote --from <from> --to <to> --indexes <index> --create
     ```
   - **Live production index, with zero downtime:** build a fresh copy next to it, then swap atomically:
     ```bash
     msc clone <index> <index>_next --from <from> --to <to>
     # verify <index>_next (step 5), then:
     msc index swap <index> <index>_next --project <to> --wait
     msc index delete <index>_next --project <to> --wait     # now holds the old version
     ```
5. **Verify.** Done when all of these match between `--project <from>` and `--project <to>`:
   - `msc index stats <index> --select numberOfDocuments`
   - `msc settings get <index>` (compare with `settings diff --against`, which should report no `changes`)
   - `msc search <index> "<query>" --limit 5 --select <pk>` for a few representative queries

## Gotchas

- **Promote only adds documents.** Documents deleted at the source stay at the destination. For an exact replica, use the clone-and-swap path above.
- **Promote tries the server-side export route first**: the *source server* pushes to the destination URL. A cloud source can't reach a `localhost` destination; `msc` then falls back to copying through the client, which is slower but works anywhere.
- **Embedder secrets.** Settings reads hide embedder API keys. After promoting an index with embedders that use API keys, check that semantic or hybrid search works at the destination, and re-set the key with `msc settings update <index> embedders --project <to> --wait` if it doesn't.
- **Large indexes take time.** Settings changes re-index the destination. The default `--wait-timeout` is 5 minutes; pass a larger one (in ms) for big datasets.
