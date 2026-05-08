"""MCP Server — 15 tools exposing Compass API to AI agents via Model Context Protocol."""
import logging
from typing import Annotated, Any
from fastapi import APIRouter, Depends, HTTPException
from pydantic import BaseModel, Field
from src.db.database import Database, get_db
from src.core.rust_client import rust_client

logger = logging.getLogger(__name__)
router = APIRouter(prefix="/mcp", tags=["mcp"])


# ─── Input / Output models ────────────────────────────────────────────────────

class ToolInput(BaseModel):
    """Base schema shared by all tool inputs (reserved for future common fields)."""
    pass


# ── READ tools ────────────────────────────────────────────────────────────────

class ListEntitiesInput(ToolInput):
    entity_type: str | None = Field(None, description="knowledge | case | log | insight")
    min_score: float = Field(0.0, ge=0, le=100)
    tags: list[str] | None = Field(None, description="AND filter")
    limit: int = Field(20, ge=1, le=100)
    offset: int = Field(0, ge=0)


class ListEntitiesOutput(BaseModel):
    items: list[dict]
    total: int
    has_more: bool


@router.get("/list_entities", response_model=ListEntitiesOutput)
async def mcp_list_entities(
    entity_type: Annotated[str | None, Field(description="knowledge | case | log | insight")] = None,
    min_score: float = 0.0,
    tags: Annotated[str | None, Field(description="comma-separated tags")] = None,
    limit: int = 20,
    offset: int = 0,
    db: Annotated[Database, Depends(get_db)] = None,
) -> ListEntitiesOutput:
    """List entities with optional type/score/tag filtering and pagination."""
    tag_list: list[str] | None = tags.split(",") if tags else None
    items, total = await db.list_entities(
        entity_type=entity_type, min_score=min_score, tags=tag_list, limit=limit, offset=offset
    )
    has_more = (offset + limit) < total
    return ListEntitiesOutput(items=items, total=total, has_more=has_more)


class GetEntityInput(ToolInput):
    entity_id: str


class GetEntityOutput(BaseModel):
    entity: dict


@router.get("/get_entity/{entity_id}", response_model=GetEntityOutput)
async def mcp_get_entity(
    entity_id: str,
    db: Annotated[Database, Depends(get_db)] = None,
) -> GetEntityOutput:
    """Fetch a single entity with its outgoing/incoming references and tags."""
    entity = await db.get_entity(entity_id)
    if not entity:
        raise HTTPException(status_code=404, detail="Entity not found")
    out_refs, in_refs = await db.get_references(entity_id)
    async with db.conn.execute(
        "SELECT tag FROM taggings WHERE entity_id = ?", (entity_id,)
    ) as cur:
        tags = [row[0] for row in await cur.fetchall()]
    return GetEntityOutput(entity={**entity, "outgoing_refs": out_refs, "incoming_refs": in_refs, "tags": tags})


class GetGraphNeighborsInput(ToolInput):
    entity_id: str
    depth: int = Field(1, ge=1, le=3)
    min_strength: float = Field(0.0, ge=0.0, le=2.0)


class GetGraphNeighborsOutput(BaseModel):
    entity_id: str
    neighbors: list[dict]


@router.get("/get_graph_neighbors/{entity_id}", response_model=GetGraphNeighborsOutput)
async def mcp_get_graph_neighbors(
    entity_id: str,
    depth: int = 1,
    min_strength: float = 0.0,
    db: Annotated[Database, Depends(get_db)] = None,
) -> GetGraphNeighborsOutput:
    """Get graph neighbors of an entity up to N hops with optional min_strength filter."""
    entity = await db.get_entity(entity_id)
    if not entity:
        raise HTTPException(status_code=404, detail="Entity not found")
    rows = await db.get_neighbors(entity_id, depth=depth, min_strength=min_strength)
    return GetGraphNeighborsOutput(entity_id=entity_id, neighbors=rows)


class GetTimelineInput(ToolInput):
    entity_id: str
    limit: int = Field(50, ge=1, le=200)
    offset: int = 0


class GetTimelineOutput(BaseModel):
    entity_id: str
    items: list[dict]
    total: int


@router.get("/get_timeline/{entity_id}", response_model=GetTimelineOutput)
async def mcp_get_timeline(
    entity_id: str,
    limit: int = 50,
    offset: int = 0,
    db: Annotated[Database, Depends(get_db)] = None,
) -> GetTimelineOutput:
    """Fetch the event timeline for a specific entity."""
    entity = await db.get_entity(entity_id)
    if not entity:
        raise HTTPException(status_code=404, detail="Entity not found")
    async with db.conn.execute(
        """SELECT event_type, trigger, created_at, metadata
           FROM timeline_events
           WHERE entity_id = ?
           ORDER BY created_at DESC
           LIMIT ? OFFSET ?""",
        (entity_id, limit, offset),
    ) as cur:
        rows = await cur.fetchall()
    async with db.conn.execute(
        "SELECT COUNT(*) FROM timeline_events WHERE entity_id = ?", (entity_id,)
    ) as cur:
        total = (await cur.fetchone())[0]
    items = [
        {
            "event_type": r["event_type"],
            "trigger": r["trigger"],
            "created_at": r["created_at"],
            "metadata": __import__("json").loads(r["metadata"]) if r["metadata"] else None,
        }
        for r in rows
    ]
    return GetTimelineOutput(entity_id=entity_id, items=items, total=total)


class GetInsightsInput(ToolInput):
    maturity: str | None = Field(None, description="seedling | growing | mature")
    limit: int = Field(20, ge=1, le=100)
    offset: int = 0


class GetInsightsOutput(BaseModel):
    items: list[dict]
    total: int


@router.get("/get_insights", response_model=GetInsightsOutput)
async def mcp_get_insights(
    maturity: str | None = None,
    limit: int = 20,
    offset: int = 0,
    db: Annotated[Database, Depends(get_db)] = None,
) -> GetInsightsOutput:
    """List all insights, optionally filtered by maturity."""
    async with db.conn.execute(
        """SELECT e.id, e.title, e.category, e.created_at, e.updated_at,
                  i.maturity, i.evolved_from, i.refined_at, e.final_score
           FROM insights i
           JOIN entities e ON e.id = i.entity_id
           WHERE (? IS NULL OR i.maturity = ?)
           ORDER BY e.updated_at DESC
           LIMIT ? OFFSET ?""",
        (maturity, maturity, limit, offset),
    ) as cur:
        rows = await cur.fetchall()
    async with db.conn.execute(
        "SELECT COUNT(*) FROM insights WHERE (? IS NULL OR maturity = ?)",
        (maturity, maturity),
    ) as cur:
        total = (await cur.fetchone())[0]
    return GetInsightsOutput(items=list(rows), total=total)


# ── WRITE tools ────────────────────────────────────────────────────────────────

class CreateEntityInput(ToolInput):
    id: str
    title: str
    category: str = "Inbox"
    vault_path: str
    interest: float = 5.0
    strategy: float = 5.0
    consensus: float = 0.0
    content: str | None = None
    metadata: dict = Field(default_factory=dict)


class CreateEntityOutput(BaseModel):
    entity: dict


@router.post("/create_entity", response_model=CreateEntityOutput)
async def mcp_create_entity(
    body: CreateEntityInput,
    db: Annotated[Database, Depends(get_db)] = None,
) -> CreateEntityOutput:
    """Create a new entity — computes score, extracts refs, derives auto-tags."""
    from datetime import datetime, timezone
    from src.api.entities import _compute_score_and_refs, normalize_entity_id, _extract_tags

    now = datetime.now(tz=timezone.utc).isoformat()
    score_data, refs = await _compute_score_and_refs(
        body.interest, body.strategy, body.consensus, body.content, body.id
    )
    entity_data = {
        "id": body.id,
        "file_path": str(db._vault_path / body.vault_path) if hasattr(db, '_vault_path') else body.vault_path,
        "vault_path": body.vault_path,
        "title": body.title,
        "category": body.category,
        "created_at": now,
        "updated_at": now,
        "last_boosted_at": now,
        "metadata": body.metadata,
    }
    await db.create_entity_full(
        entity_data=entity_data,
        score_data=score_data,
        ref_entries=refs,
        event_type="created",
        event_trigger="mcp",
    )
    tags = _extract_tags(body.title)
    if tags:
        await db.begin()
        try:
            for tag in tags:
                await db.upsert_tagging(body.id, tag)
            await db.commit()
        except Exception:
            await db.rollback()
            raise
    return CreateEntityOutput(entity={**entity_data, **score_data, "tags": tags})


class UpdateEntityInput(ToolInput):
    entity_id: str
    title: str | None = None
    category: str | None = None
    interest: float | None = None
    strategy: float | None = None
    consensus: float | None = None
    content: str | None = None
    metadata: dict | None = None


class UpdateEntityOutput(BaseModel):
    entity: dict


@router.put("/update_entity/{entity_id}", response_model=UpdateEntityOutput)
async def mcp_update_entity(
    entity_id: str,
    body: UpdateEntityInput,
    db: Annotated[Database, Depends(get_db)] = None,
) -> UpdateEntityOutput:
    """Update an entity's fields and recompute its score."""
    from datetime import datetime, timezone
    from src.api.entities import _compute_score_and_refs, normalize_entity_id

    existing = await db.get_entity(entity_id)
    if not existing:
        raise HTTPException(status_code=404, detail="Entity not found")

    now = datetime.now(tz=timezone.utc).isoformat()
    interest = body.interest if body.interest is not None else float(existing.get("interest", 5.0))
    strategy = body.strategy if body.strategy is not None else float(existing.get("strategy", 5.0))
    consensus = body.consensus if body.consensus is not None else float(existing.get("consensus", 0.0))
    content = body.content if body.content is not None else ""

    score_data, refs = await _compute_score_and_refs(interest, strategy, consensus, content, entity_id)
    update_data = {
        "id": entity_id,
        "title": body.title if body.title is not None else existing["title"],
        "category": body.category if body.category is not None else existing["category"],
        "file_path": existing["file_path"],
        "vault_path": existing["vault_path"],
        "created_at": existing["created_at"],
        "updated_at": now,
        "last_boosted_at": score_data["last_boosted_at"],
        "metadata": body.metadata if body.metadata is not None else existing.get("metadata", {}),
    }
    await db.begin()
    try:
        await db.upsert_entity(update_data)
        await db.upsert_score(score_data)
        await db.conn.execute('DELETE FROM "references" WHERE source_id = ?', (entity_id,))
        for target_id, strength in refs:
            await db.upsert_reference(entity_id, target_id, strength, bidirectional=True)
        await db.log_event(entity_id, "updated", trigger="mcp")
        await db.commit()
    except Exception:
        await db.rollback()
        raise

    return UpdateEntityOutput(entity={**update_data, **score_data})


class DeleteEntityInput(ToolInput):
    entity_id: str


class DeleteEntityOutput(BaseModel):
    deleted: str


@router.delete("/delete_entity/{entity_id}", response_model=DeleteEntityOutput)
async def mcp_delete_entity(
    entity_id: str,
    db: Annotated[Database, Depends(get_db)] = None,
) -> DeleteEntityOutput:
    """Delete an entity and all associated data (scores, refs, events)."""
    existing = await db.get_entity(entity_id)
    if not existing:
        raise HTTPException(status_code=404, detail="Entity not found")
    await db.begin()
    try:
        await db.conn.execute('DELETE FROM "references" WHERE source_id = ? OR target_id = ?', (entity_id, entity_id))
        await db.conn.execute("DELETE FROM timeline_events WHERE entity_id = ?", (entity_id,))
        await db.conn.execute("DELETE FROM scores WHERE entity_id = ?", (entity_id,))
        await db.conn.execute("DELETE FROM taggings WHERE entity_id = ?", (entity_id,))
        await db.conn.execute("DELETE FROM score_history WHERE entity_id = ?", (entity_id,))
        await db.conn.execute("DELETE FROM insights WHERE entity_id = ?", (entity_id,))
        await db.conn.execute("DELETE FROM entities WHERE id = ?", (entity_id,))
        await db.commit()
    except Exception:
        await db.rollback()
        raise
    return DeleteEntityOutput(deleted=entity_id)


class CreateReferenceInput(ToolInput):
    source_id: str
    target_id: str
    strength: float = Field(1.0, ge=0.0, le=2.0)


class CreateReferenceOutput(BaseModel):
    forward: dict
    reverse: dict | None = None


@router.post("/create_reference", response_model=CreateReferenceOutput)
async def mcp_create_reference(
    body: CreateReferenceInput,
    db: Annotated[Database, Depends(get_db)] = None,
) -> CreateReferenceOutput:
    """Create a bidirectional reference edge between two entities."""
    src = await db.get_entity(body.source_id)
    tgt = await db.get_entity(body.target_id)
    if not src or not tgt:
        raise HTTPException(status_code=404, detail="Source or target entity not found")
    await db.upsert_reference(body.source_id, body.target_id, body.strength, bidirectional=True)
    return CreateReferenceOutput(
        forward={"source_id": body.source_id, "target_id": body.target_id, "strength": body.strength},
        reverse={"source_id": body.target_id, "target_id": body.source_id, "strength": round(body.strength * 0.5, 2)},
    )


class UpdateScoresInput(ToolInput):
    entity_id: str
    interest: float | None = None
    strategy: float | None = None
    consensus: float | None = None


class UpdateScoresOutput(BaseModel):
    entity_id: str
    final_score: float


@router.patch("/update_scores", response_model=UpdateScoresOutput)
async def mcp_update_scores(
    body: UpdateScoresInput,
    db: Annotated[Database, Depends(get_db)] = None,
) -> UpdateScoresOutput:
    """Update one or more score dimensions for an entity and recompute composite score."""
    entity = await db.get_entity(body.entity_id)
    if not entity:
        raise HTTPException(status_code=404, detail="Entity not found")
    from datetime import datetime, timezone
    now = datetime.now(tz=timezone.utc).isoformat()
    interest = body.interest if body.interest is not None else float(entity.get("interest", 5.0))
    strategy = body.strategy if body.strategy is not None else float(entity.get("strategy", 5.0))
    consensus = body.consensus if body.consensus is not None else float(entity.get("consensus", 0.0))
    result = await rust_client.compute_score(interest, strategy, consensus, last_boosted_at=now)
    score_data = {
        "entity_id": body.entity_id,
        "interest": interest, "strategy": strategy, "consensus": consensus,
        "final_score": round(result.final_score, 2),
        "updated_at": now, "last_boosted_at": now,
    }
    await db.upsert_score(score_data)
    await db.log_event(body.entity_id, "score_updated", trigger="mcp")
    return UpdateScoresOutput(entity_id=body.entity_id, final_score=score_data["final_score"])


# ── ADVANCED tools ────────────────────────────────────────────────────────────

class GetFeedInput(ToolInput):
    category: str = "all"
    limit: int = Field(10, ge=1, le=50)


class GetFeedOutput(BaseModel):
    items: list[dict]
    count: int


@router.get("/get_feed", response_model=GetFeedOutput)
async def mcp_get_feed(
    category: str = "all",
    limit: int = 10,
    db: Annotated[Database, Depends(get_db)] = None,
) -> GetFeedOutput:
    """Return top-scoring entities as a personalised feed, optionally filtered by category."""
    items, total = await db.list_entities(entity_type=None, min_score=0.0, tags=None, limit=limit, offset=0)
    scored = []
    for e in items:
        last_boosted = e.get("last_boosted_at") or ""
        r = await rust_client.compute_score(
            interest=float(e.get("interest") or 5.0),
            strategy=float(e.get("strategy") or 5.0),
            consensus=float(e.get("consensus") or 0.0),
            last_boosted_at=last_boosted,
        )
        e["final_score"] = round(r.final_score, 2)
        scored.append(e)
    scored.sort(key=lambda x: x["final_score"], reverse=True)
    return GetFeedOutput(items=scored[:limit], count=len(scored[:limit]))


class GetPathInput(ToolInput):
    from_entity_id: str
    to_entity_id: str
    max_hops: int = Field(5, ge=1, le=10)


class GetPathOutput(BaseModel):
    path: list[str]
    hops: int


@router.get("/get_path/{from_entity_id}/{to_entity_id}", response_model=GetPathOutput)
async def mcp_get_path(
    from_entity_id: str,
    to_entity_id: str,
    max_hops: int = 5,
    db: Annotated[Database, Depends(get_db)] = None,
) -> GetPathOutput:
    """Find the shortest path (entity IDs) between two entities via graph traversal."""
    path = await db.get_shortest_path(from_entity_id, to_entity_id, max_hops=max_hops)
    if not path:
        return GetPathOutput(path=[], hops=-1)
    return GetPathOutput(path=path, hops=len(path) - 1)


class SimulateDecayInput(ToolInput):
    interest: float
    strategy: float
    consensus: float
    days: int = Field(30, ge=1, le=3650)


class SimulateDecayOutput(BaseModel):
    days: int
    interest: float
    strategy: float
    consensus: float
    composite: float


@router.get("/simulate_decay", response_model=SimulateDecayOutput)
async def mcp_simulate_decay(
    interest: float = 5.0,
    strategy: float = 5.0,
    consensus: float = 0.0,
    days: int = 30,
    db: Annotated[Database, Depends(get_db)] = None,
) -> SimulateDecayOutput:
    """Return decayed scores after N days using each dimension's half-life."""
    decay_row = await db.conn.execute(
        "SELECT interest_hl, strategy_hl, consensus_hl FROM decay_config LIMIT 1"
    )
    row = await decay_row.fetchone()
    int_hl = float(row["interest_hl"]) if row else 30.0
    str_hl = float(row["strategy_hl"]) if row else 60.0
    con_hl = float(row["consensus_hl"]) if row else 90.0

    def decay(init, d, hl): return init * (0.5 ** (d / hl))
    int_d = decay(interest, days, int_hl)
    str_d = decay(strategy, days, str_hl)
    con_d = decay(consensus, days, con_hl)
    composite = int_d * 0.4 + str_d * 0.35 + con_d * 0.25

    return SimulateDecayOutput(
        days=days,
        interest=round(int_d, 4),
        strategy=round(str_d, 4),
        consensus=round(con_d, 4),
        composite=round(composite, 4),
    )


class GetConfigOutput(BaseModel):
    interest_hl: float
    strategy_hl: float
    consensus_hl: float
    weight_interest: float
    weight_strategy: float
    weight_consensus: float


@router.get("/get_config", response_model=GetConfigOutput)
async def mcp_get_config(
    db: Annotated[Database, Depends(get_db)] = None,
) -> GetConfigOutput:
    """Return current decay half-lives and score weights."""
    decay_row = await db.conn.execute("SELECT interest_hl, strategy_hl, consensus_hl FROM decay_config LIMIT 1")
    decay = await decay_row.fetchone()
    return GetConfigOutput(
        interest_hl=float(decay["interest_hl"]) if decay else 30.0,
        strategy_hl=float(decay["strategy_hl"]) if decay else 60.0,
        consensus_hl=float(decay["consensus_hl"]) if decay else 90.0,
        weight_interest=0.40,
        weight_strategy=0.35,
        weight_consensus=0.25,
    )