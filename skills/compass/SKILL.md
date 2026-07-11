---
name: compass
description: Access your personal knowledge graph ? search, score, and navigate entities in your Compass PKM system. Translates natural-language intent to Compass HTTP API calls and renders JSON output to human-readable text.
---

## Two-Step Call Pattern

Every compass interaction follows this exact sequence:

1. **Call the action** ? get raw JSON from the Compass HTTP API?Rust ??????? 8080?
2. **Call `compass render`** ? translate JSON to human-readable text
3. **Send via Feishu** ? deliver the formatted text to the user

**Never return raw JSON to the user. Never skip the render step.**

## Intent Detection

| User says... | Step 1 calls | render action= |
|---|---|---|
| "???..." / "???..." | `compass create` | `create` |
| "??..." / "???..." / "????..." | `compass search` | `search` |
| "...???" / "...??" | `compass get` | `get` |
| "?????" / "????" | `compass feed` | `feed` |
| "Top 5" / ?? | `compass top` | `top` |
| "????..." / "???..." / "????..." | `compass access` | `access` |
| Deep research / ????? | `compass context` | `context` |
| Tag suggestions | `compass tags` | `tags` |
| Related notes | `compass related` | `related` |
| Weekly report | `compass weekly` | `weekly` |

## compass Actions

```bash
# FTS5 ??
compass search q=<natural language query> [limit=20]

# Top ?????? layer ???
compass top [limit=20] [layer=knowledge|direction|case|log|insight]

# ??????
compass get id=<entity_id>

# ?????????
compass feed [limit=10] [mode=explore|consolidate|strategic]

# Agent ?????
compass context task=<research question> [top_k=5]

# ?????id ????????
compass create title=<title> [layer=knowledge] [content=] [interest=5.0] [strategy=5.0] [consensus=0.0]

# ?????AI ????????
compass score id=<entity_id> [interest=<float>] [strategy=<float>] [consensus=<float>]

# ??????? boost + ???
compass access id=<entity_id> [depth=glance|read|study|apply]

# Phase 4 suggestions are deterministic API calls. Only accept writes to Vault.
compass tags id=<entity_id> [candidates='<JSON array>']
compass accept_tag suggestion_id=<suggestion_id>
compass reject_tag suggestion_id=<suggestion_id>

compass related id=<entity_id> [limit=10]
compass accept_related suggestion_id=<suggestion_id>
compass reject_related suggestion_id=<suggestion_id>

# Weekly reports require an explicit IANA timezone.
compass weekly from=YYYY-MM-DD to=YYYY-MM-DD tz=Asia/Shanghai
```

## compass render ? JSON to Human Text

After any API call, pass the raw JSON output to `compass render`??? `raw=` ??? stdin ????

```
compass render raw=<JSON string> action=<action_name>
```

| action | ???? |
|--------|----------|
| `search` | ?????`1. [??](id) - ?X.X? [??]` |
| `top` | ?????`1. [??](id) - ?X.X?` |
| `get` | ??? + ???? + ???? |
| `feed` | ???**????** / **????** / **????** |
| `context` | ???? + ???? + ?? |
| `create` | `? ????[??](id)` |
| `score` | `? ??????[id](id) - ?X.X` |
| `access` | `? ??????[id](id) - ?X.X` |
| `tags` | tag suggestions or an empty-state response |
| `related` | explainable related-note suggestions or an empty-state response |
| `accept_tag` / `reject_tag` | accepted, rejected, or expired tag result |
| `accept_related` / `reject_related` | accepted, rejected, or expired link result |
| `weekly` | structured weekly report with data-quality notice |

## Feishu Coordination

- `compass render` ??? `feishu.send_message` ??
- ????? ? ?????????????
- ??? ? ?????????????
- **??**??? JSON ????

## Core Concepts

- **Entity**?Obsidian vault ????????? `id`???+?????create ? id ????????
- **????**?`interest`???????/ `strategy`???????/ `consensus`????????
  `composite = interest?0.40 + strategy?0.35 + consensus?0.25`??????? `id` / `composite`?
- **??**?**?? `interest`** ?? `new = max(interest?0.5, interest?0.98^days_inactive)`?`strategy` / `consensus` ????
- **???**???? ? consensus +2??????? ? interest +1??? boost ????`glance` +0 / `read` +1 / `study` +3 / `apply` +2(interest)+5(strategy)?
- **??**????? `[[wiki-links]]` ???
- **Phase 4 suggestions**: suggestions and related notes are read-only until an explicit `accept_*`; `reject_*` never writes Vault. An `expired` response means the content changed and no write occurred.
- **Weekly report**: pass `from`, `to`, and an IANA `tz`; Compass returns data and the skill renders it, without sending messages itself.

## Error Handling

- **404**?????? ? ?? `id`
- **Connection refused**?Compass ????? ? `cd compass-core && cargo run --release`
- **Rate limit**?????

## Environment

`score.composite` is the stable base score. Compass derives a read-time effective score from
optional freshness metadata; default `evergreen` notes are not time-discounted and no scheduler
mutates scores.

| ?? | ??? |
|------|--------|
| `COMPASS_API_URL` | `http://localhost:8080` |
| `COMPASS_API_TOKEN` | Optional Bearer token sent on every HTTP request |

## Compass Server?Rust ?????

```bash
cd compass-core
cargo run --release
```

??????? vault ?????? ? FileWatcher ?? vault ?? ? HTTP API ?? `http://localhost:8080`?? Python?? subprocess?
