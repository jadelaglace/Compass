"""REST endpoints for Insight management."""
import json as _json
from datetime import datetime, timezone
from typing import Annotated, Optional

from fastapi import APIRouter, HTTPException, Depends, Query
from pydantic import BaseModel

from src.db.database import Database, get_db

router = APIRouter(prefix="/insights", tags=["insights"])


# ---- Maturity state machines ----

VALID_INSIGHT_MATURITY = {
    "seedling": ["sprout"],
    "sprout": ["mature"],
    "mature": [],
}

VALID_ENTITY_MATURITY = {
    "seedling": ["sprout"],
    "sprout": ["mature"],
    "mature": [],
}


def _next_insight_maturity(current: str) -> Optional[str]:
    transitions = VALID_INSIGHT_MATURITY.get(current, [])
    return transitions[0] if transitions else None


def _next_entity_maturity(current: str) -> Optional[str]:
    transitions = VALID_ENTITY_MATURITY.get(current, [])
    return transitions[0] if transitions else None


# ---- Schemas ----

class InsightCreate(BaseModel):
    entity_id: str
    title: str
    content: Optional[str] = None


class InsightResponse(BaseModel):
    id: str
    entity_id: str
    title: str
    content: Optional[str]
    maturity: str
    source_type: str
    created_at: str
    updated_at: str


class InsightListItem(BaseModel):
    id: str
    entity_id: str
    title: str
    maturity: str
    source_type: str
    created_at: str
    updated_at: str


class InsightListResponse(BaseModel):
    items: list[InsightListItem]
    total: int
    has_more: bool


class EvolveResponse(BaseModel):
    entity_id: str
    entity_maturity: str
    insight_maturity: str
    evolved: bool
    detail: str


# ---- Insight-1: CRUD ----

@router.post("", response_model=InsightResponse)
async def create_insight(
    insight: InsightCreate,
    db: Annotated[Database, Depends(get_db)] = None,
) -> InsightResponse:
    entity = await db.get_entity(insight.entity_id)
    if not entity:
        raise HTTPException(status_code=404, detail="Entity not found")

    now = datetime.now(tz=timezone.utc).isoformat()
    insight_id = f"insight-{insight.entity_id}-{now[:10]}"

    data = {
        "id": insight_id,
        "entity_id": insight.entity_id,
        "title": insight.title,
        "content": insight.content,
        "maturity": "seedling",
        "source_type": "auto",
        "created_at": now,
        "updated_at": now,
    }

    await db.begin()
    try:
        await db.upsert_insight(data)
        await db.log_event(insight.entity_id, "insight_created", trigger="api")
        await db.commit()
    except Exception:
        await db.rollback()
        raise

    return InsightResponse(**data)


@router.get("")
async def list_insights(
    maturity: Annotated[Optional[str], Query(description="Filter by maturity")] = None,
    limit: Annotated[int, Query(ge=1, le=200)] = 20,
    offset: Annotated[int, Query(ge=0)] = 0,
    format: Annotated[Optional[str], Query(description="Format for export: export|markdown")] = None,
    db: Annotated[Database, Depends(get_db)] = None,
) -> InsightListResponse | dict:
    if maturity is not None and maturity not in ("seedling", "sprout", "mature"):
        raise HTTPException(status_code=422, detail="Invalid maturity value")
    if format is not None and format not in ("export", "markdown"):
        raise HTTPException(status_code=422, detail="format must be export or markdown")

    items, total = await db.list_insights(maturity=maturity, limit=limit, offset=offset)

    if format == "export":
        return {"format": "json", "total": total, "items": [
            {
                "id": item["id"],
                "entity_id": item["entity_id"],
                "title": item["title"],
                "content": item.get("content"),
                "maturity": item["maturity"],
                "source_type": item["source_type"],
                "created_at": item["created_at"],
                "updated_at": item["updated_at"],
            }
            for item in items
        ]}

    if format == "markdown":
        lines = ["# Insights Export\n\n"]
        for item in items:
            lines.append(f"## {item['title']}\n")
            lines.append(f"**ID:** `{item['id']}`  **Maturity:** {item['maturity']}\n")
            lines.append(f"**Entity:** `{item['entity_id']}`  **Source:** {item['source_type']}\n")
            lines.append(f"Created: {item['created_at']}  Updated: {item['updated_at']}\n")
            if item.get("content"):
                lines.append(f"\n{item['content']}\n")
            lines.append("---\n")
        return {"format": "markdown", "content": "".join(lines)}

    has_more = (offset + limit) < total
    return InsightListResponse(
        items=[InsightListItem(**item) for item in items],
        total=total,
        has_more=has_more,
    )




# ---- Insight-3: Export ----

@router.get("/export")
async def export_insights(
    maturity: Annotated[Optional[str], Query(description="Filter by maturity")] = None,
    format: Annotated[str, Query(description="json | markdown")] = "json",
    db: Annotated[Database, Depends(get_db)] = None,
) -> dict:
    if format not in ("json", "markdown"):
        raise HTTPException(status_code=422, detail="format must be json or markdown")

    items, _total = await db.list_insights(maturity=maturity, limit=10000, offset=0)

    if format == "markdown":
        lines = ["# Insights Export\n\n"]
        for item in items:
            lines.append(f"## {item['title']}\n")
            lines.append(f"**ID:** `{item['id']}`  **Maturity:** {item['maturity']}\n")
            lines.append(f"**Entity:** `{item['entity_id']}`  **Source:** {item['source_type']}\n")
            lines.append(f"Created: {item['created_at']}  Updated: {item['updated_at']}\n")
            if item.get("content"):
                lines.append(f"\n{item['content']}\n")
            lines.append("---\n")
        return {"format": "markdown", "content": "".join(lines)}

    return {
        "format": "json",
        "total": len(items),
        "items": [
            {
                "id": item["id"],
                "entity_id": item["entity_id"],
                "title": item["title"],
                "content": item.get("content"),
                "maturity": item["maturity"],
                "source_type": item["source_type"],
                "created_at": item["created_at"],
                "updated_at": item["updated_at"],
            }
            for item in items
        ],
    }




@router.get("/entity/{entity_id}/export")
async def export_entity_insights(
    entity_id: str,
    format: Annotated[str, Query(description="json | markdown")] = "json",
    db: Annotated[Database, Depends(get_db)] = None,
) -> dict:
    if format not in ("json", "markdown"):
        raise HTTPException(status_code=422, detail="format must be json or markdown")

    entity = await db.get_entity(entity_id)
    if not entity:
        raise HTTPException(status_code=404, detail="Entity not found")

    items, _total = await db.list_insights(maturity=None, limit=10000, offset=0)
    entity_insights = [item for item in items if item["entity_id"] == entity_id]

    if format == "markdown":
        lines = [f"# Insights for: {entity['title']}\n\n"]
        for item in entity_insights:
            lines.append(f"## {item['title']}\n")
            lines.append(f"**Maturity:** {item['maturity']}  **Source:** {item['source_type']}\n")
            lines.append(f"Created: {item['created_at']}\n")
            if item.get("content"):
                lines.append(f"\n{item['content']}\n")
            lines.append("---\n")
        return {"format": "markdown", "content": "".join(lines)}

    return {"format": "json", "entity_id": entity_id, "total": len(entity_insights), "items": entity_insights}


@router.get("/{insight_id}", response_model=InsightResponse)
async def get_insight(
    insight_id: str,
    db: Annotated[Database, Depends(get_db)] = None,
) -> InsightResponse:
    insight = await db.get_insight(insight_id)
    if not insight:
        raise HTTPException(status_code=404, detail="Insight not found")
    return InsightResponse(**insight)


@router.patch("/{insight_id}/maturity", response_model=InsightResponse)
async def upgrade_insight_maturity(
    insight_id: str,
    db: Annotated[Database, Depends(get_db)] = None,
) -> InsightResponse:
    insight = await db.get_insight(insight_id)
    if not insight:
        raise HTTPException(status_code=404, detail="Insight not found")

    current = insight["maturity"]
    next_m = _next_insight_maturity(current)
    if next_m is None:
        raise HTTPException(status_code=422, detail="Already fully mature")

    now = datetime.now(tz=timezone.utc).isoformat()
    updated = {**insight, "maturity": next_m, "updated_at": now}

    await db.begin()
    try:
        await db.upsert_insight(updated)
        await db.log_event(
            insight["entity_id"],
            "maturity_upgraded",
            trigger="api",
            extra={"from": current, "to": next_m},
        )
        await db.commit()
    except Exception:
        await db.rollback()
        raise

    return InsightResponse(**updated)


# ---- Insight-2: Entity Evolution Trigger ----

@router.get("/{insight_id}/evolve", response_model=EvolveResponse)
async def evolve_entity_from_insight(
    insight_id: str,
    db: Annotated[Database, Depends(get_db)] = None,
) -> EvolveResponse:
    insight = await db.get_insight(insight_id)
    if not insight:
        raise HTTPException(status_code=404, detail="Insight not found")

    if insight["maturity"] != "mature":
        return EvolveResponse(
            entity_id=insight["entity_id"],
            entity_maturity="",
            insight_maturity=insight["maturity"],
            evolved=False,
            detail="Insight not yet mature",
        )

    entity = await db.get_entity(insight["entity_id"])
    if not entity:
        raise HTTPException(status_code=404, detail="Entity not found")

    current = entity.get("maturity", "seedling")
    next_m = _next_entity_maturity(current)

    if next_m is None:
        return EvolveResponse(
            entity_id=insight["entity_id"],
            entity_maturity=current,
            insight_maturity="mature",
            evolved=False,
            detail="Entity already fully mature",
        )

    now = datetime.now(tz=timezone.utc).isoformat()
    history_entry = _json.dumps({
        "from": current,
        "to": next_m,
        "reason": f"insight_matured:{insight_id}",
        "at": now,
    })

    await db.begin()
    try:
        await db.conn.execute(
            "UPDATE entities SET maturity = ?, maturity_history = ? WHERE id = ?",
            (next_m, history_entry, insight["entity_id"]),
        )
        await db.log_event(
            insight["entity_id"],
            "maturity_upgraded",
            trigger="insight_evolve",
            extra={"from": current, "to": next_m, "insight_id": insight_id},
        )
        await db.commit()
    except Exception:
        await db.rollback()
        raise

    return EvolveResponse(
        entity_id=insight["entity_id"],
        entity_maturity=next_m,
        insight_maturity="mature",
        evolved=True,
        detail=f"Entity evolved from {current} to {next_m}",
    )

