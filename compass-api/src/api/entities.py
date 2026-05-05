"""REST endpoints for entity management."""
import re
from datetime import datetime, timedelta, timezone
from typing import Annotated, Optional

from fastapi import APIRouter, HTTPException, Depends, Query
from pydantic import BaseModel, Field

from src import config
from src.db.database import Database, get_db
from src.core.rust_client import rust_client

# Reference strength patterns — computed from link text / context
# Ordered: first match wins
_REF_STRENGTH_PATTERNS = [
    (re.compile(r"(?:see|referenced?|linked|cited)\s+:?\s*\[\[([^\]]+)\]\]", re.IGNORECASE), 0.9),
    (re.compile(r"\[\[([^\]]+)\]\](?:\s*\([^)]+\))?"), 1.0),
]


def _extract_refs_with_strength(content: str) -> list[tuple[str, float]]:
    """Extract (entity_id, strength) from all [[wikilinks]] in content.

    Strengths:
    - Explicit citation language: see [[ref]], refer [[ref]] → 0.9
    - Standard [[link]] → 1.0
    """
    wikilink_pattern = re.compile(r"\[\[([^\]]+)\]\]")
    citation_pattern = re.compile(
        r"(?:see|referenced?|linked|cited)\s+:?\s*\[\[([^\]]+)\]\]",
        re.IGNORECASE,
    )
    # Find all citation-style first
    citation_ids = {
        m.group(1) for m in citation_pattern.finditer(content)
    }
    results: list[tuple[str, float]] = []
    seen: set[str] = set()
    for m in wikilink_pattern.finditer(content):
        entity_id = normalize_entity_id(m.group(1))
        if entity_id in seen:
            continue
        seen.add(entity_id)
        strength = 0.9 if m.group(0) in citation_ids or entity_id in citation_ids else 1.0
        results.append((entity_id, strength))
    return results


def _calc_ref_strength(link_text: str) -> float:
    """Infer reference strength from wikilink context.

    Strengths:
    - Explicit citation language (see [[ref]]) → 0.9
    - Standard [[link]] → 1.0
    """
    for pattern, strength in _REF_STRENGTH_PATTERNS:
        if pattern.search(link_text):
            return strength
    return 1.0

# ---- entity ID normalization (mirrors FileWatcher's vault_path_to_entity_id) ----

_STRIP_EXT_RE = re.compile(r"\.(md|MD|markdown|MARKDOWN)$")
_MULTI_DASH_RE = re.compile(r"-+")


def normalize_entity_id(raw: str) -> str:
    """Normalize a raw wiki-link or vault-path to a canonical entity ID.

    Mirrors FileWatcher's vault_path_to_entity_id so that Rust-extracted
    refs and the current_entity_id are on the same baseline for self-filtering.
    """
    # Strip file extension
    stem = _STRIP_EXT_RE.sub("", raw)
    # Replace path separators with dash
    stem = stem.replace("/", "-").replace("\\", "-")
    # Collapse multiple dashes
    stem = _MULTI_DASH_RE.sub("-", stem)
    # Strip leading/trailing dashes
    return stem.strip("-").lower()


router = APIRouter(prefix="/entities", tags=["entities"])


# Reusable score computation shared by create and update.
async def _compute_score_and_refs(
    interest: float,
    strategy: float,
    consensus: float,
    content: Optional[str],
    entity_id: str,
) -> tuple[dict, list[tuple[str, float]]]:
    """Compute score via Rust and extract refs if content is provided.

    Returns (score_data, ref_entries) where ref_entries is list of (target_id, strength).
    """
    now = datetime.now(tz=timezone.utc).isoformat()
    ref_entries: list[tuple[str, float]] = []
    if content:
        # Extract refs with per-link strength inference
        ref_entries = _extract_refs_with_strength(content)
        normalized_entity_id = normalize_entity_id(entity_id)
        # Filter out self-references
        ref_entries = [(tid, s) for tid, s in ref_entries if tid != normalized_entity_id]

    score_result = await rust_client.compute_score(
        interest=interest,
        strategy=strategy,
        consensus=consensus,
        last_boosted_at=now,
    )
    score_data = {
        "entity_id": entity_id,
        "interest": interest,
        "strategy": strategy,
        "consensus": consensus,
        "final_score": round(score_result.final_score, 2),
        "updated_at": now,
        "last_boosted_at": now,  # used by decay formula — must be kept current
    }
    return score_data, ref_entries


class EntityCreate(BaseModel):
    """Schema for creating a new entity via POST /entities."""

    id: str
    title: str
    category: str = "Inbox"
    vault_path: str = Field(description="Path inside vault, e.g. Inbox/note.md")
    file_path: Optional[str] = None
    interest: float = 5.0
    strategy: float = 5.0
    consensus: float = 0.0
    content: Optional[str] = None  # Markdown body — used for ref extraction
    metadata: dict = Field(default_factory=dict)


class EntityResponse(BaseModel):
    """Schema returned after creating or fetching an entity."""

    id: str
    title: str
    category: str
    vault_path: str
    final_score: float
    created_at: str
    updated_at: str


class EntityListItem(BaseModel):
    """Single item in GET /entities response."""

    id: str
    title: str
    entity_type: str
    category: str
    vault_path: str
    final_score: float
    tags: list[str] = Field(default_factory=list)
    created_at: str
    updated_at: str


class EntityListResponse(BaseModel):
    """Response schema for GET /entities."""

    items: list[EntityListItem]
    total: int
    has_more: bool


@router.post("", response_model=EntityResponse)
async def create_entity(entity: EntityCreate, db: Annotated[Database, Depends(get_db)] = None) -> EntityResponse:
    """Create a new entity with computed score and extracted refs."""
    now = datetime.now(tz=timezone.utc).isoformat()
    vault_path = entity.vault_path
    file_path = entity.file_path or str(config.VAULT_PATH / vault_path)

    score_data, refs = await _compute_score_and_refs(
        entity.interest, entity.strategy, entity.consensus, entity.content, entity.id
    )

    entity_data = {
        "id": entity.id,
        "file_path": file_path,
        "vault_path": vault_path,
        "title": entity.title,
        "category": entity.category,
        "created_at": now,
        "updated_at": now,
        "last_boosted_at": now,
        "metadata": entity.metadata,
    }

    # Atomic write: entity + score + refs + event all succeed or all rollback
    await db.create_entity_full(
        entity_data=entity_data,
        score_data=score_data,
        ref_entries=refs,
        event_type="created",
        event_trigger="api",
    )

    return EntityResponse(
        id=entity.id,
        title=entity.title,
        category=entity.category,
        vault_path=vault_path,
        final_score=score_data["final_score"],
        created_at=now,
        updated_at=now,
    )


@router.get("/search")
async def search_entities(q: str, limit: int = 20, db: Annotated[Database, Depends(get_db)] = None) -> dict:
    """Full-text search across entity titles and categories via FTS5."""
    results = await db.search_entities(q, limit=limit)
    return {"results": results, "count": len(results)}


@router.get("/top")
async def top_entities(
    limit: int = 20, category: Optional[str] = None, db: Annotated[Database, Depends(get_db)] = None
) -> dict:
    """Return top-scoring entities, optionally filtered by category."""
    results = await db.get_top_entities(limit=limit, category=category)
    return {"results": results, "count": len(results)}


@router.get("", response_model=EntityListResponse)
async def list_entities(
    type: Annotated[
        Optional[str],
        Query(description="实体类型：knowledge | case | log | insight"),
    ] = None,
    min_score: Annotated[
        float,
        Query(ge=0, le=100, description="最低综合分数"),
    ] = 0.0,
    tags: Annotated[
        Optional[list[str]],
        Query(description="标签过滤（AND 逻辑）"),
    ] = None,
    limit: Annotated[
        int,
        Query(ge=1, le=100, description="每页条数"),
    ] = 20,
    offset: Annotated[
        int,
        Query(ge=0, description="偏移量"),
    ] = 0,
    db: Annotated[Database, Depends(get_db)] = None,
) -> EntityListResponse:
    """List all entities with optional filters and pagination."""
    if type is not None and type not in ("knowledge", "case", "log", "insight"):
        raise HTTPException(status_code=422, detail="Invalid entity_type")

    items, total = await db.list_entities(
        entity_type=type,
        min_score=min_score,
        tags=tags,
        limit=limit,
        offset=offset,
    )
    has_more = (offset + limit) < total
    return EntityListResponse(
        items=[EntityListItem(**item) for item in items],
        total=total,
        has_more=has_more,
    )


@router.put("/{entity_id}", response_model=EntityResponse)
async def update_entity(
    entity_id: str,
    update: EntityCreate,
    db: Annotated[Database, Depends(get_db)] = None
) -> EntityResponse:
    """Full update: re-parses content for refs, recomputes scores.

    Used by FileWatcher when a vault file changes.
    """
    existing = await db.get_entity(entity_id)
    if not existing:
        raise HTTPException(status_code=404, detail="Entity not found")

    # Guard: body id must match URL path id
    if update.id != entity_id:
        raise HTTPException(
            status_code=400,
            detail=f"ID mismatch: body has '{update.id}', URL has '{entity_id}'",
        )

    now = datetime.now(tz=timezone.utc).isoformat()
    vault_path = update.vault_path
    # Preserve existing file_path when not explicitly provided in the update.
    # update.file_path=None means "don't change" (use stored value).
    file_path = (
        update.file_path if update.file_path is not None else existing["file_path"]
    )

    score_data, refs = await _compute_score_and_refs(
        update.interest, update.strategy, update.consensus, update.content, entity_id
    )

    entity_data = {
        "id": entity_id,
        "file_path": file_path,
        "vault_path": vault_path,
        "title": update.title,
        "category": update.category,
        "created_at": existing["created_at"],  # preserve original
        "updated_at": now,
        "last_boosted_at": score_data["last_boosted_at"],  # refresh on every update
        "metadata": update.metadata,
    }

    # Atomic: upsert entity + score + replace refs + event
    await db.begin()
    try:
        await db.upsert_entity(entity_data)
        await db.upsert_score(score_data)
        # Replace outgoing refs: delete all then re-insert with computed strength
        # Also remove orphaned reverse refs pointing to this entity as source
        await db.conn.execute('DELETE FROM "references" WHERE source_id = ?', (entity_id,))
        # Clean up reverse refs: any ref where this entity is the target and source
        # has no corresponding forward ref back to this entity (i.e., the source has
        # been updated and no longer references this entity)
        await db.conn.execute(
            """
            DELETE FROM "references"
            WHERE target_id = ?
              AND source_id IN (
                  SELECT source_id FROM "references"
                  WHERE target_id = ? AND source_id != ?
                  GROUP BY source_id
                  HAVING COUNT(*) = 1
              )
            """,
            (entity_id, entity_id, entity_id),
        )
        for target_id, strength in refs:
            await db.upsert_reference(entity_id, target_id, strength, bidirectional=True)
        await db.log_event(entity_id, "updated", trigger="filewatcher")
        await db.commit()
    except Exception:
        await db.rollback()
        raise

    return EntityResponse(
        id=entity_id,
        title=update.title,
        category=update.category,
        vault_path=vault_path,
        final_score=score_data["final_score"],
        created_at=entity_data["created_at"],
        updated_at=now,
    )


@router.delete("/{entity_id}")
async def delete_entity(entity_id: str, db: Annotated[Database, Depends(get_db)] = None) -> dict:
    """Delete entity and all associated data (scores, refs, events).

    Used by FileWatcher when a vault file is removed.
    """
    existing = await db.get_entity(entity_id)
    if not existing:
        raise HTTPException(status_code=404, detail="Entity not found")

    await db.begin()
    try:
        await db.conn.execute('DELETE FROM "references" WHERE source_id = ? OR target_id = ?', (entity_id, entity_id))
        await db.conn.execute("DELETE FROM timeline_events WHERE entity_id = ?", (entity_id,))
        await db.conn.execute("DELETE FROM scores WHERE entity_id = ?", (entity_id,))
        await db.conn.execute("DELETE FROM entities WHERE id = ?", (entity_id,))
        await db.commit()
    except Exception:
        await db.rollback()
        raise

    return {"deleted": entity_id}


@router.get("/{entity_id}")
async def get_entity(entity_id: str, db: Annotated[Database, Depends(get_db)] = None) -> dict:
    """Fetch a single entity by ID with its incoming and outgoing references."""
    entity = await db.get_entity(entity_id)
    if not entity:
        raise HTTPException(status_code=404, detail="Entity not found")
    out_refs, in_refs = await db.get_references(entity_id)
    return {**entity, "outgoing_refs": out_refs, "incoming_refs": in_refs}


# ---- Score History endpoint (History-2) ----

@router.get("/{entity_id}/score/history")
async def get_score_history(
    entity_id: str,
    dimension: str = "composite",
    days: int = 90,
    db: Annotated[Database, Depends(get_db)] = None,
) -> dict:
    """Return score history trend for an entity."""
    entity = await db.get_entity(entity_id)
    if not entity:
        raise HTTPException(status_code=404, detail="Entity not found")

    # Map dimension to column
    dim_col = {
        "composite": "final_score",
        "interest": "interest",
        "strategy": "strategy",
        "consensus": "consensus",
    }.get(dimension, "final_score")

    cutoff = datetime.now(tz=timezone.utc) - timedelta(days=days)
    cutoff_str = cutoff.isoformat()

    async with db.conn.execute(
        f"""SELECT created_at as timestamp, {dim_col} as value
            FROM score_history
            WHERE entity_id = ? AND created_at >= ?
            ORDER BY created_at DESC
            LIMIT 50""",
        (entity_id, cutoff_str),
    ) as cur:
        rows = await cur.fetchall()

    records = [{"timestamp": row["timestamp"], "value": float(row["value"])} for row in rows]

    # Trend calculation: compare recent 3 avg vs older 3 avg
    trend = "stable"
    change_pct = 0.0
    if len(records) >= 3:
        recent = sum(r["value"] for r in records[:3]) / 3
        older = sum(r["value"] for r in records[3:6]) / 3 if len(records) >= 6 else recent
        if older != 0:
            change_pct = round((recent - older) / older * 100, 1)
            if change_pct > 5:
                trend = "rising"
            elif change_pct < -5:
                trend = "declining"

    values = [r["value"] for r in records]
    return {
        "entity_id": entity_id,
        "dimension": dimension,
        "records": records,
        "trend": trend,
        "change_pct": change_pct,
        "min_value": min(values) if values else 0.0,
        "max_value": max(values) if values else 0.0,
    }


# ---- Timeline-1: PATCH /entities/{id}/access ----

ACCESS_DEBOUNCE_SECONDS = 300  # 5 minutes


class AccessResponse(BaseModel):
    entity_id: str
    access_count: int
    accessed_at: str
    decay_updated: bool


@router.patch("/{entity_id}/access", response_model=AccessResponse)
async def record_access(
    entity_id: str,
    db: Annotated[Database, Depends(get_db)] = None,
) -> AccessResponse:
    """Record an entity access with 5-min debounce and decay recalculation."""
    entity = await db.get_entity(entity_id)
    if not entity:
        raise HTTPException(status_code=404, detail="Entity not found")

    now = datetime.now(tz=timezone.utc)
    now_str = now.isoformat()
    last_boosted_str = entity.get("last_boosted_at")

    # access_count lives in metadata JSON, not as a direct entity column
    import json
    meta = json.loads(entity.get("metadata") or "{}")
    current_access_count = meta.get("access_count", 0)

    # Debounce: skip count bump if last access within 5 minutes
    if last_boosted_str:
        try:
            from datetime import timedelta
            last_boosted = datetime.fromisoformat(last_boosted_str.replace("Z", "+00:00"))
            if (now - last_boosted).total_seconds() < ACCESS_DEBOUNCE_SECONDS:
                return AccessResponse(
                    entity_id=entity_id,
                    access_count=current_access_count,
                    accessed_at=last_boosted.isoformat(),
                    decay_updated=False,
                )
        except Exception:
            pass

    # Get current scores for decay recalculation
    score_row = await db.conn.execute(
        "SELECT interest, strategy, consensus FROM scores WHERE entity_id = ?",
        (entity_id,),
    )
    score = await score_row.fetchone()
    interest = float(score["interest"]) if score else 5.0
    strategy = float(score["strategy"]) if score else 5.0
    consensus = float(score["consensus"]) if score else 0.0

    # Recalculate decay with fresh timestamp
    result = await rust_client.compute_score(
        interest=interest, strategy=strategy, consensus=consensus, last_boosted_at=now_str,
    )
    new_final = round(result.final_score, 2)

    # Update metadata access_count
    new_count = current_access_count + 1
    meta["access_count"] = new_count
    meta["access_count"] = new_count

    await db.begin()
    try:
        await db.conn.execute(
            "UPDATE entities SET last_boosted_at = ?, metadata = ? WHERE id = ?",
            (now_str, json.dumps(meta), entity_id),
        )
        await db.conn.execute(
            "UPDATE scores SET final_score = ?, updated_at = ? WHERE entity_id = ?",
            (new_final, now_str, entity_id),
        )
        await db.log_event(entity_id, "accessed", trigger="api")
        await db.commit()
    except Exception:
        await db.rollback()
        raise

    return AccessResponse(
        entity_id=entity_id,
        access_count=new_count,
        accessed_at=now_str,
        decay_updated=True,
    )


# ---- Timeline-2: GET /entities/timeline ----

class TimelineItem(BaseModel):
    entity_id: str
    title: str
    category: str
    event_type: str
    event_trigger: Optional[str]
    created_at: str


class TimelineResponse(BaseModel):
    items: list[TimelineItem]
    total: int
    has_more: bool


@router.get("/timeline", response_model=TimelineResponse)
async def get_entities_timeline(
    start: Annotated[str, Query(description="ISO datetime start of time window")],
    end: Annotated[
        Optional[str],
        Query(description="ISO datetime end of time window (defaults to now)"),
    ] = None,
    event_type: Annotated[
        Optional[str],
        Query(description="Filter by event type"),
    ] = None,
    limit: Annotated[int, Query(ge=1, le=200)] = 50,
    offset: Annotated[int, Query(ge=0)] = 0,
    db: Annotated[Database, Depends(get_db)] = None,
) -> TimelineResponse:
    """Return entities with timeline events in a time range.

    Shows the most recent event per entity within the window.
    """
    # Parse datetimes
    try:
        start_dt = datetime.fromisoformat(start.replace("Z", "+00:00"))
    except ValueError:
        raise HTTPException(status_code=400, detail="Invalid start datetime format")

    if end:
        try:
            end_dt = datetime.fromisoformat(end.replace("Z", "+00:00"))
        except ValueError:
            raise HTTPException(status_code=400, detail="Invalid end datetime format")
    else:
        end_dt = datetime.now(tz=timezone.utc)

    if end_dt <= start_dt:
        raise HTTPException(status_code=400, detail="end must be after start")

    # Build query — latest event per entity
    params: list[Any] = [start_dt.isoformat(), end_dt.isoformat()]

    event_filter = ""
    if event_type:
        event_filter = " AND te.event_type = ?"
        params.append(event_type)

    # Count total unique entities with matching events
    count_sql = f"""
        SELECT COUNT(DISTINCT te.entity_id)
        FROM timeline_events te
        JOIN entities e ON e.id = te.entity_id
        WHERE te.created_at >= ? AND te.created_at <= ?{event_filter}
    """
    async with db.conn.execute(count_sql, params) as cur:
        total = (await cur.fetchone())[0]

    # Paginated items — latest event per entity
    paginated_sql = f"""
        WITH ranked AS (
            SELECT
                te.entity_id,
                te.event_type,
                te.trigger,
                te.created_at,
                e.title,
                e.category,
                ROW_NUMBER() OVER (
                    PARTITION BY te.entity_id
                    ORDER BY te.created_at DESC
                ) AS rn
            FROM timeline_events te
            JOIN entities e ON e.id = te.entity_id
            WHERE te.created_at >= ? AND te.created_at <= ?{event_filter}
        )
        SELECT entity_id, title, category, event_type, trigger, created_at
        FROM ranked
        WHERE rn = 1
        ORDER BY created_at DESC
        LIMIT ? OFFSET ?
    """
    params.extend([limit, offset])
    async with db.conn.execute(paginated_sql, params) as cur:
        rows = await cur.fetchall()

    items = [
        TimelineItem(
            entity_id=row["entity_id"],
            title=row["title"],
            category=row["category"],
            event_type=row["event_type"],
            event_trigger=row["trigger"],
            created_at=row["created_at"],
        )
        for row in rows
    ]
    has_more = (offset + limit) < total
    return TimelineResponse(items=items, total=total, has_more=has_more)


# ---- Per-entity timeline: GET /entities/{entity_id}/timeline ----

class EntityTimelineItem(BaseModel):
    event_type: str
    event_trigger: Optional[str]
    created_at: str
    metadata: Optional[dict] = None


class EntityTimelineResponse(BaseModel):
    entity_id: str
    items: list[EntityTimelineItem]
    total: int


@router.get("/{entity_id}/timeline", response_model=EntityTimelineResponse)
async def get_entity_timeline(
    entity_id: str,
    limit: Annotated[int, Query(ge=1, le=200)] = 50,
    offset: Annotated[int, Query(ge=0)] = 0,
    db: Annotated[Database, Depends(get_db)] = None,
) -> EntityTimelineResponse:
    """Return all timeline events for a specific entity."""
    entity = await db.get_entity(entity_id)
    if not entity:
        raise HTTPException(status_code=404, detail="Entity not found")

    # Count total events for this entity
    count_sql = "SELECT COUNT(*) FROM timeline_events WHERE entity_id = ?"
    async with db.conn.execute(count_sql, (entity_id,)) as cur:
        total = (await cur.fetchone())[0]

    # Fetch paginated events
    sql = """
        SELECT event_type, trigger, created_at, metadata
        FROM timeline_events
        WHERE entity_id = ?
        ORDER BY created_at DESC
        LIMIT ? OFFSET ?
    """
    async with db.conn.execute(sql, (entity_id, limit, offset)) as cur:
        rows = await cur.fetchall()

    import json as _json
    items = [
        EntityTimelineItem(
            event_type=row["event_type"],
            event_trigger=row["trigger"],
            created_at=row["created_at"],
            metadata=_json.loads(row["metadata"]) if row["metadata"] else None,
        )
        for row in rows
    ]
    return EntityTimelineResponse(entity_id=entity_id, items=items, total=total)
