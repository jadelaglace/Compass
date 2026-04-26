"""REST endpoints for entity management."""
from datetime import datetime, timezone
from typing import Annotated, Optional

from fastapi import APIRouter, HTTPException, Depends, Query
from pydantic import BaseModel, Field

from src import config
from src.db.database import Database, get_db
from src.core.rust_client import rust_client

# ---- entity ID normalization (mirrors FileWatcher's vault_path_to_entity_id) ----

import re

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
) -> tuple[dict, list[str]]:
    """Compute score via Rust and extract refs if content is provided.

    Returns (score_data, ref_ids).
    """
    now = datetime.now(tz=timezone.utc).isoformat()
    refs: list[str] = []
    if content:
        refs_result = await rust_client.parse_refs(content, current_id=entity_id)
        raw_refs = refs_result.refs
        # Normalize refs so self-reference filtering works correctly:
        # Rust extracts e.g. "Projects/compass-v2"; entity_id is "projects-compass-v2"
        normalized_entity_id = normalize_entity_id(entity_id)
        refs = [
            r for r in (normalize_entity_id(r) for r in raw_refs)
            if r != normalized_entity_id
        ]

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
    return score_data, refs


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
        ref_ids=refs,
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


@router.get("")
async def list_entities(
    limit: Annotated[int, Query(ge=1, le=1000)] = 100,
    offset: Annotated[int, Query(ge=0)] = 0,
    category: Optional[str] = None,
    db: Annotated[Database, Depends(get_db)] = None,
) -> dict:
    """List all entities without requiring a query, supports pagination and category filter."""
    results = await db.get_all_entities(limit=limit, offset=offset, category=category)
    return {"results": results, "count": len(results)}


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
        # Replace outgoing refs: delete all then re-insert
        await db.conn.execute('DELETE FROM "references" WHERE source_id = ?', (entity_id,))
        for ref_id in refs:
            await db.upsert_reference(entity_id, ref_id)
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
