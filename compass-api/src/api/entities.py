"""REST endpoints for entity management."""
from datetime import datetime, timezone
from typing import Optional

from fastapi import APIRouter, HTTPException, Depends
from pydantic import BaseModel, Field

from src import config
from src.db.database import Database, get_db
from src.core.rust_client import rust_client

router = APIRouter(prefix="/entities", tags=["entities"])


class EntityCreate(BaseModel):
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
    id: str
    title: str
    category: str
    vault_path: str
    final_score: float
    created_at: str
    updated_at: str


@router.post("", response_model=EntityResponse)
async def create_entity(entity: EntityCreate, db: Database = Depends(get_db)) -> EntityResponse:
    now = datetime.utcnow().replace(tzinfo=timezone.utc).isoformat()
    vault_path = entity.vault_path
    file_path = entity.file_path or str(config.VAULT_PATH / vault_path)

    # Parse refs via Rust
    refs: list[str] = []
    if entity.content:
        refs_result = rust_client.parse_refs(entity.content, current_id=entity.id)
        refs = refs_result.refs

    # Score via Rust
    score_result = rust_client.compute_score(
        interest=entity.interest,
        strategy=entity.strategy,
        consensus=entity.consensus,
        last_boosted_at=now,
    )

    # Persist entity
    await db.upsert_entity({
        "id": entity.id,
        "file_path": file_path,
        "vault_path": vault_path,
        "title": entity.title,
        "category": entity.category,
        "created_at": now,
        "updated_at": now,
        "metadata": entity.metadata,
    })

    # Persist score
    await db.upsert_score({
        "entity_id": entity.id,
        "interest": entity.interest,
        "strategy": entity.strategy,
        "consensus": entity.consensus,
        "final_score": round(score_result.final_score, 2),
        "updated_at": now,
    })

    # Persist references
    for ref_id in refs:
        await db.upsert_reference(entity.id, ref_id)

    await db.log_event(entity.id, "created", trigger="api")

    return EntityResponse(
        id=entity.id,
        title=entity.title,
        category=entity.category,
        vault_path=vault_path,
        final_score=round(score_result.final_score, 2),
        created_at=now,
        updated_at=now,
    )


@router.get("/search")
async def search_entities(q: str, limit: int = 20, db: Database = Depends(get_db)) -> dict:
    results = await db.search_entities(q, limit=limit)
    return {"results": results, "count": len(results)}


@router.get("/top")
async def top_entities(
    limit: int = 20, category: Optional[str] = None, db: Database = Depends(get_db)
) -> dict:
    results = await db.get_top_entities(limit=limit, category=category)
    return {"results": results, "count": len(results)}


@router.get("/{entity_id}")
async def get_entity(entity_id: str, db: Database = Depends(get_db)) -> dict:
    entity = await db.get_entity(entity_id)
    if not entity:
        raise HTTPException(status_code=404, detail="Entity not found")
    out_refs, in_refs = await db.get_references(entity_id)
    return {**entity, "outgoing_refs": out_refs, "incoming_refs": in_refs}
