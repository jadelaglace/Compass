"""REST endpoints for Insight management."""
from datetime import datetime, timezone
from typing import Annotated, Optional

from fastapi import APIRouter, HTTPException, Depends, Query
from pydantic import BaseModel, Field

from src.db.database import Database, get_db

router = APIRouter(prefix="/insights", tags=["insights"])


# ---- Request/Response schemas ----

class InsightCreate(BaseModel):
    """Schema for creating a new insight via POST /insights."""

    entity_id: str
    title: str
    content: Optional[str] = None


class InsightResponse(BaseModel):
    """Schema returned after creating or fetching an insight."""

    id: str
    entity_id: str
    title: str
    content: Optional[str]
    maturity: str
    source_type: str
    created_at: str
    updated_at: str


class InsightListItem(BaseModel):
    """Single item in GET /insights response."""

    id: str
    entity_id: str
    title: str
    maturity: str
    source_type: str
    created_at: str
    updated_at: str


class InsightListResponse(BaseModel):
    """Response schema for GET /insights."""

    items: list[InsightListItem]
    total: int
    has_more: bool


# ---- Maturity state machine ----

VALID_MATURITY_TRANSITIONS = {
    "seedling": ["sprout"],
    "sprout": ["mature"],
    "mature": [],
}


def _next_maturity(current: str) -> Optional[str]:
    """Return the next maturity level, or None if fully mature."""
    transitions = VALID_MATURITY_TRANSITIONS.get(current, [])
    return transitions[0] if transitions else None


# ---- Endpoints ----

@router.post("", response_model=InsightResponse)
async def create_insight(
    insight: InsightCreate,
    db: Annotated[Database, Depends(get_db)] = None,
) -> InsightResponse:
    """Create a new insight attached to an entity."""
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


@router.get("", response_model=InsightListResponse)
async def list_insights(
    maturity: Annotated[
        Optional[str],
        Query(description="Filter by maturity: seedling | sprout | mature"),
    ] = None,
    limit: Annotated[int, Query(ge=1, le=200)] = 20,
    offset: Annotated[int, Query(ge=0)] = 0,
    db: Annotated[Database, Depends(get_db)] = None,
) -> InsightListResponse:
    """List all insights with optional maturity filter and pagination."""
    if maturity is not None and maturity not in ("seedling", "sprout", "mature"):
        raise HTTPException(status_code=422, detail="Invalid maturity value")

    items, total = await db.list_insights(maturity=maturity, limit=limit, offset=offset)
    has_more = (offset + limit) < total
    return InsightListResponse(
        items=[InsightListItem(**item) for item in items],
        total=total,
        has_more=has_more,
    )


@router.get("/{insight_id}", response_model=InsightResponse)
async def get_insight(
    insight_id: str,
    db: Annotated[Database, Depends(get_db)] = None,
) -> InsightResponse:
    """Fetch a single insight by ID."""
    insight = await db.get_insight(insight_id)
    if not insight:
        raise HTTPException(status_code=404, detail="Insight not found")
    return InsightResponse(**insight)


@router.patch("/{insight_id}/maturity", response_model=InsightResponse)
async def upgrade_maturity(
    insight_id: str,
    db: Annotated[Database, Depends(get_db)] = None,
) -> InsightResponse:
    """Advance maturity to next level (seedling→sprout→mature).

    Returns 422 if already fully mature.
    """
    insight = await db.get_insight(insight_id)
    if not insight:
        raise HTTPException(status_code=404, detail="Insight not found")

    current = insight["maturity"]
    next_m = _next_maturity(current)
    if next_m is None:
        raise HTTPException(
            status_code=422,
            detail="Already fully mature",
        )

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