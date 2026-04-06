"""REST endpoints for score management."""
from datetime import datetime, timezone

from fastapi import APIRouter, Depends, HTTPException
from pydantic import BaseModel

from src.db.database import Database, get_db
from src.core.rust_client import rust_client

router = APIRouter(prefix="/scores", tags=["scores"])


class ScoreUpdate(BaseModel):
    entity_id: str
    interest: float | None = None
    strategy: float | None = None
    consensus: float | None = None
    manual_override: bool = False


class ScoreResponse(BaseModel):
    entity_id: str
    final_score: float
    decay_factor: float
    days_elapsed: float


@router.post("/update", response_model=ScoreResponse)
async def update_score(update: ScoreUpdate, db: Database = Depends(get_db)) -> ScoreResponse:
    entity = await db.get_entity(update.entity_id)
    if not entity:
        raise HTTPException(status_code=404, detail="Entity not found")

    now = datetime.utcnow().replace(tzinfo=timezone.utc).isoformat()
    interest = update.interest if update.interest is not None else float(entity.get("interest", 5.0))
    strategy = update.strategy if update.strategy is not None else float(entity.get("strategy", 5.0))
    consensus = update.consensus if update.consensus is not None else float(entity.get("consensus", 0.0))
    last_boosted = entity.get("last_boosted_at") or now

    score_result = rust_client.compute_score(
        interest=interest,
        strategy=strategy,
        consensus=consensus,
        last_boosted_at=last_boosted,
    )

    await db.upsert_score({
        "entity_id": update.entity_id,
        "interest": interest,
        "strategy": strategy,
        "consensus": consensus,
        "final_score": round(score_result.final_score, 2),
        "manual_override": update.manual_override,
        "updated_at": now,
    })

    if update.manual_override:
        await db.conn.execute(
            "UPDATE entities SET last_boosted_at = ? WHERE id = ?",
            (now, update.entity_id),
        )
        await db.conn.commit()

    await db.log_event(update.entity_id, "score_updated", trigger="manual" if update.manual_override else "auto")

    return ScoreResponse(
        entity_id=update.entity_id,
        final_score=round(score_result.final_score, 2),
        decay_factor=round(score_result.decay_factor, 4),
        days_elapsed=round(score_result.days_elapsed, 1),
    )
