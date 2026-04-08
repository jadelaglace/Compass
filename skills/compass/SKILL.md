---
name: compass
description: Access your personal knowledge graph — search, score, and navigate entities in your Compass PKM system. Use when user asks about notes, wants to find information, check scores, or manage knowledge.
---

# Compass — Personal Knowledge Graph Skill

Compass is your cognitive operating system. It indexes your Obsidian vault, maintains a scoring engine (interest × strategy × consensus with decay), and surfaces the most relevant knowledge at any moment.

## Quick Reference

```bash
# Search entities
compass search q=<query> [limit=20]

# Get top-scored entities
compass top [limit=20] [category=]

# Get single entity with refs
compass get id=<entity_id>

# Daily feed (top Inbox + recent + strategic)
compass feed [limit=10]

# Agent context preparation (returns scored entities for a task)
compass context task=<task description> [top_k=5]

# Create a new entity
compass create id=<id> title=<title> category=<category> vault_path=<path> [interest=5.0] [strategy=5.0] [consensus=0.0] [content=]

# Update entity score
compass score id=<entity_id> [interest=<float>] [strategy=<float>] [consensus=<float>]
```

## Core Concepts

- **Entity**: A note/card in your Obsidian vault. Each has a unique `id` (normalized from vault path, e.g. `projects-compass-v2` from `Projects/compass-v2.md`)
- **Scoring**: Three dimensions — `interest` (0-10, personal curiosity), `strategy` (0-10, strategic importance), `consensus` (0-10, peer validation). Final score decays over time with a 30-day half-life.
- **References**: Entities can link to each other via `[[wiki-links]]`. Both outgoing and incoming refs are tracked.
- **Feed**: Your daily digest — top Inbox items, recently updated high-scorers, and strategic (Direction category) items.

## Action Details

### search — Full-text search
Search entities by keyword across title, category, and content.
```
compass search q=compass limit=10
```
Returns: `{"results": [...], "count": N}`

### top — Top-scored entities
Returns highest-scoring entities, optionally filtered by category.
```
compass top limit=5 category=Inbox
compass top limit=10
```
Categories: `Inbox`, `Direction`, `Knowledge`, `Logs`, `Insights`

### get — Single entity
Get full entity data including outgoing and incoming references.
```
compass get id=projects-compass-v2
```
Returns: entity object with `outgoing_refs` and `incoming_refs` arrays.

### feed — Daily digest
Returns three sections: `top_inbox`, `recently_updated`, `strategic`.
```
compass feed limit=5
```

### context — Agent context injection
For AI agent use — given a task/question, returns the most relevant scored entities to use as context.
```
compass context task="Explain the Compass scoring engine" top_k=5
```
Returns: `{"context": [...], "suggested_entities": [...], "reasoning": "..."}`

### create — New entity
Create a new vault entry. The `id` is the canonical entity ID (use lowercase, dashes for spaces/paths).
```
compass create id=my-quick-note title="Quick Note" category=Inbox vault_path=Inbox/quick-note.md content="Some initial content"
```
Vault path is relative to vault root (e.g. `Inbox/`, `Knowledge/`, `Direction/`).

### score — Update scoring dimensions
Update one or more scoring dimensions for an entity. Triggers score recomputation.
```
compass score id=projects-compass-v2 interest=8 strategy=6
compass score id=projects-compass-v2 consensus=7
```

## Error Handling

- **404 Not Found**: Entity does not exist. Check the `id` parameter.
- **Connection refused**: compass-api is not running. Start it with `cd compass-api && .venv/bin/uvicorn src.main:app --reload`.
- **Rate limit / server error**: Wait and retry with exponential backoff.

## Environment

| Variable | Default | Description |
|----------|---------|-------------|
| `COMPASS_API_URL` | `http://localhost:8080` | compass-api server address |

Set `COMPASS_API_URL` if the API is running on a different host/port.

## compass-api Server

The API must be running for this skill to work:

```bash
cd /path/to/Compass/compass-api
source .venv/bin/activate
uvicorn src.main:app --reload --port 8080
```

For file watching (auto-sync vault changes):
```bash
python -m src.services.filewatcher
```
