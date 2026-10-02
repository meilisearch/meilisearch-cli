---
name: msc-import
description: Importing data into Meilisearch with `msc`. Use when loading a JSON, NDJSON or CSV dataset into an index, choosing a primary key, re-importing or updating documents, or debugging documents that did not get indexed.
---

# Importing data with `msc`

An import is finished when the index holds **every** source document, has **zero** failed tasks, and answers a sample search. Settings come before data: changing filterable, sortable or searchable attributes after an import re-indexes everything.

## Steps

1. **Target.** `msc whoami` reports `"auth":"ok"`.
2. **Inspect the data.** Find the format (JSON array, NDJSON, CSV), the document count, and one sample record. Choose the **primary key**: a field present in every document, unique, and an integer or a string of only `a-z A-Z 0-9 - _`. Done when you have the count and the primary key.
3. **Create the index and settings first.**
   ```bash
   msc index create <index> --primary-key <pk> --if-not-exists --wait
   msc settings update <index> --wait --file settings.json   # filterableAttributes, sortableAttributes, searchableAttributes, …
   ```
   Always pass `--primary-key`: inference fails when several fields end in `id`, and the key can't change once documents exist.
4. **Import.**
   ```bash
   msc import <index> --file data.ndjson          # waits; prints {"status","indexedDocuments","taskUids",…}
   ```
   NDJSON is best for large files (it streams in batches). JSON arrays are batched too. CSV is sent in one request.
5. **Verify.** Done when all three hold:
   - `indexedDocuments` in the summary, and `msc index stats <index> --select numberOfDocuments`, equal the source count
   - `msc task list --index-uids <index> --statuses failed --select uid,error.code,error.message` returns no results
   - `msc search <index> "<word from the sample record>" --select <pk>` returns the sample document

   When counts don't match, a duplicate primary key value is the usual cause: later documents replace earlier ones with the same key.

## Failed tasks

| `error.code` | Fix |
|--------------|-----|
| `missing_document_id` | Some documents lack the primary key field; fix the data or choose another key |
| `invalid_document_id` | Key values contain characters outside `a-z A-Z 0-9 - _`; normalize them or pick another field |
| `index_primary_key_multiple_candidates_found`, `index_primary_key_no_candidate_found` | Pass `--primary-key` explicitly |
| `index_primary_key_already_exists` | The index already has a different key; import into a new index, or use the existing key |
| `malformed_payload` | Invalid JSON/NDJSON/CSV; check `--format` matches the content |
| `payload_too_large` (HTTP 413) | Lower `--batch-size` (default 20 MiB) |

## Updating later

- `msc import` / `msc document add` **replace** whole documents with the same primary key.
- `msc document update` **merges** fields into existing documents (partial update).
- Removing documents: `msc document delete-by-filter <index> "<filter>" --dry-run` first, then without `--dry-run` and with `--wait` (the filter's fields must be in `filterableAttributes`).

## CSV

All CSV values are strings unless the header declares a type: `id,title,price:number,in_stock:boolean`. Numbers that stay strings can't be sorted or range-filtered.
