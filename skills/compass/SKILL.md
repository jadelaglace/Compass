---
name: compass
description: Access your personal knowledge graph — search, score, and navigate entities in your Compass PKM system. Handles natural language input and translates API output to human-readable text.

## Two-Step Call Pattern

Every compass interaction follows this exact sequence:

1. **Call the action** → get raw JSON from the API
2. **Call `compass render`** → translate JSON to human-readable text
3. **Send via Feishu** → deliver the formatted text to the user

**Never return raw JSON to the user. Never skip the render step.**

## Intent Detection

| User says... | Step 1 calls | Then calls render with... |
|---|---|---|
| "记一下..." / "存一下..." | `compass create` | `action=create` |
| "说说..." / "是什么..." / "介绍一下..." | `compass search` | `action=search` |
| "...多少分" / "...评分" | `compass get` | `action=get` |
| "今天有什么" / "今日焦点" | `compass feed` | `action=feed` |
| Deep research / needs context | `compass context` | `action=context` |
| "Top 5" / ranking request | `compass top` | `action=top` |

## compass-api Actions

```bash
# Search entities
compass search q=<natural language query> [limit=20]

# Top-scored entities
compass top [limit=20] [category=]

# Single entity by ID
compass get id=<entity_id>

# Daily digest
compass feed [limit=10]

# Agent context injection
compass context task=<research question> [top_k=5]

# Create new entity
compass create id=<id> title=<title> category=<category> vault_path=<path> [content=] [interest=5.0] [strategy=5.0] [consensus=0.0]

# Update scores
compass score id=<entity_id> [interest=<float>] [strategy=<float>] [consensus=<float>]
```

## compass render — JSON to Human Text

After any API call, pass the raw JSON output to `compass render`:

```
compass render raw=<JSON string> action=<action_name>
```

**Render output by action:**

| action | Output format |
|--------|--------------|
| `search` | Numbered list: `1. [标题](id) — ⭐X.X分 [分类]` |
| `top` | Numbered list: `1. [标题](id) — ⭐X.X分` |
| `get` | Title block with scores and links |
| `feed` | Three-section digest: **今日焦点** / **最近更新** / **战略焦点** |
| `context` | Bulleted list with content snippets + reasoning |
| `create` | `✅ 已创建：[标题](id)` |
| `score` | `✅ 评分已更新：[标题](id) — ⭐X.X` |

## Feishu Coordination

- After `compass render` → use `feishu.send_message` to deliver
- Needs user confirmation → return formatted text, let user decide
- Uncertain → return formatted text, do not auto-send
- **Never** send raw JSON to Feishu

## Core Concepts

- **Entity**: A note in your Obsidian vault, identified by a unique `id` (lowercase, dashes)
- **Scoring**: Three dimensions — `interest` (curiosity), `strategy` (importance), `consensus` (peer validation). Final score decays with a 30-day half-life
- **References**: Entities link via `[[wiki-links]]`, tracked as outgoing and incoming

## Error Handling

- **404**: Entity not found — check the `id`
- **Connection refused**: compass-api not running → start with `cd compass-api && .venv/bin/uvicorn src.main:app --reload`
- **Rate limit**: wait and retry with exponential backoff

## Environment

| Variable | Default |
|----------|---------|
| `COMPASS_API_URL` | `http://localhost:8080` |

## compass-api Server

```bash
cd /path/to/Compass/compass-api
source .venv/bin/activate
uvicorn src.main:app --reload --port 8080
```
