---
name: msc-relevance
description: Tuning Meilisearch search relevance with `msc`. Use when search results are wrong, badly ordered, too loose or too strict, when adding synonyms or typo rules, or when setting up hybrid/semantic search.
---

# Tuning relevance with `msc`

Relevance work is measured, not guessed: a **judgment set** of real queries with the results they should return is the yardstick for every change. Change one setting at a time, re-run the whole set, and keep a change only if nothing regresses.

## Steps

1. **Target.** `msc whoami` reports `"auth":"ok"`. For anything beyond a toy index, experiment on a copy: `msc clone <index> <index>_tuning`, then tune `<index>_tuning`.
2. **Judgment set.** Collect 5–15 representative queries (from the user, logs, or the complaint at hand). For each one, note the IDs of the documents that should rank in the top results. Done when every query has its expected IDs written down.
3. **Baseline.** For each query, record the current top 5:
   ```bash
   msc search <index> "<query>" --limit 5 --select <pk>,<title-field> --body '{"showRankingScore":true}'
   ```
   Done when you have a table of query → expected vs actual, with the failing queries marked.
4. **Diagnose.** For each failing query, see which ranking rule decided the order:
   ```bash
   msc search <index> "<query>" --limit 5 --body '{"showRankingScoreDetails":true}' --select <pk>,_rankingScoreDetails
   ```
   Read `msc settings get <index>` alongside it. Match the symptom to a lever in the table below. Done when each failure has a named cause.
5. **Change one lever.** Preview, then apply and wait (settings changes re-index, which can take a while on large indexes):
   ```bash
   echo '{"searchableAttributes":["title","tags","description"]}' | msc settings update <index> --dry-run
   echo '{"searchableAttributes":["title","tags","description"]}' | msc settings update <index> --wait
   ```
6. **Re-run the whole judgment set.** Keep the change only if failing queries improved and **no** passing query regressed; otherwise revert it (`settings update` with the previous value from `--dry-run`'s `from`). Repeat steps 4–6.
7. **Finish.** Done when every judged query has its expected documents in the top results with no regressions. If you tuned a copy, apply the final settings to the real index (`msc settings diff <index> --against final.json`, then `settings update --wait`) and delete the copy.

## Symptom → lever

| Symptom | Lever |
|---------|-------|
| A match in a minor field (description) outranks a match in the title | `searchableAttributes` order: earlier attributes weigh more (the `attribute` ranking rule) |
| Irrelevant fields match at all | Remove them from `searchableAttributes` (keep them in `displayedAttributes` if they are shown) |
| Codes, SKUs or short words match the wrong thing via typos | `typoTolerance`: `disableOnAttributes`, `disableOnWords`, or raise `minWordSizeForTypos` |
| Users' vocabulary differs from the documents' ("tv" vs "television") | `synonyms` (one-way or mutual) |
| Very common words drown out the meaningful ones | `stopWords` |
| Near-duplicate documents fill the results | `distinctAttribute` |
| A business signal (popularity, date, stock) should break ties | Custom ranking rule like `"popularity:desc"` in `rankingRules`, usually placed after the built-in rules (it needs to be in `sortableAttributes` only for `sort`, not for ranking rules) |
| Too many weak partial matches, or too few results | Search parameter `matchingStrategy` (`last`, `all`, `frequency`) via `--body` |
| Queries describe intent rather than keywords | Hybrid search: configure an `embedder` setting, then `--body '{"hybrid":{"embedder":"<name>","semanticRatio":0.5}}'` and tune `semanticRatio` on the judgment set |

The default `rankingRules` are `words, typo, proximity, attribute, sort, exactness`. Reordering them is a big hammer, so try the levers above first.

Search parameters (`matchingStrategy`, `hybrid`, `rankingScoreThreshold`, `attributesToSearchOn`) change the query, not the index: they apply to the application's search requests, so report them to the user as query changes to make in their code.
