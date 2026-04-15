# Interactive Mode

The CLI includes full-screen TUI modes for search and chat, built with ratatui.

## Interactive Search

```bash
msc search <uid> -i
```

Opens a full-screen search interface with:

- **Live search input** with 150ms debounce
- **Results list** showing the first few fields of each document
- **Document expansion** — press Enter to view full JSON
- **Keyboard navigation**

### Controls

| Key | Action |
|-----|--------|
| Type | Update search query |
| Backspace | Delete character |
| ↑ / ↓ | Navigate results |
| Enter | Expand/collapse selected document |
| Esc | Close expanded view, or exit |
| q | Quit (when not expanded) |
| Ctrl+C | Force quit |

### Status Bar

Shows hit count and processing time in milliseconds.

## Interactive Chat

```bash
msc chat -i
```

Opens a full-screen chat interface for conversational search using the Meilisearch `/chat/completions` endpoint.

### Features

- Scrollable message history
- Bottom input area
- Full conversation context sent with each message

### Controls

| Key | Action |
|-----|--------|
| Type | Compose message |
| Enter | Send message |
| ↑ / ↓ | Scroll history |
| Ctrl+L | Clear conversation |
| Ctrl+C / Esc | Quit |

### Requirements

The chat endpoint must be configured on the Meilisearch instance. See the [Meilisearch chat documentation](https://www.meilisearch.com/docs/reference/api/chat) for setup instructions.
