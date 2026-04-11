# Promote

`meilisearch promote` replicates indexes from one project to another. The primary use case is promoting a local development instance to a remote production or staging environment.

## Usage

```bash
# Promote all indexes from local to production
meilisearch promote --from local --to production

# Promote specific indexes only
meilisearch promote --from local --to staging --indexes products,categories

# Dry run — see what would be promoted
meilisearch promote --from local --to production --dry-run

# Create destination indexes if they don't exist
meilisearch promote --from local --to production --create
```

## How It Works

For each index being promoted:

1. **Settings sync**: Fetches settings from source, applies to destination
2. **Data transfer**: Uses the Meilisearch export route (`POST /export`) if available, otherwise falls back to paginated document copy
3. **Verification**: Reports document count and timing

## Example Output

```
Promoting local → production
  ✓ products             12,430 docs  settings synced  4.2s
  ✓ categories              284 docs  settings synced  0.3s
  ✓ blog_posts            1,891 docs  settings synced  1.1s

Done. 3 indexes promoted in 5.6s.
```

## Options

| Flag | Description |
|------|------------|
| `--from <project>` | Source project (default: `local`) |
| `--to <project>` | Destination project (required) |
| `--indexes <list>` | Comma-separated list of indexes to promote |
| `--dry-run` | Show what would be promoted without making changes |
| `--create` | Create destination indexes if they don't exist |

## Reverse Direction

To copy from remote to local, use `meilisearch clone` instead:

```bash
meilisearch clone products products --from production --to local
```
