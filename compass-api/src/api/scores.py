"""REST endpoints for score management."""
from datetime import datetime, timezone
from typing import Annotated

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
async def update_score(update: ScoreUpdate, db: Annotated[Database, Depends(get_db)] = None) -> ScoreResponse:
    """Recompute and persist an entity's score from updated interest/strategy/consensus values."""
    entity = await db.get_entity(update.entity_id)
    if not entity:
        raise HTTPException(status_code=404, detail="Entity not found")

    now = datetime.now(tz=timezone.utc).isoformat()
    old_score_row = await db.conn.execute(
        "SELECT interest, strategy, consensus, final_score FROM scores WHERE entity_id = ?",
        (update.entity_id,),
    )
    old_score = await old_score_row.fetchone()

    interest = update.interest if update.interest is not None else float(old_score["interest"]) if old_score else 5.0
    strategy = update.strategy if update.strategy is not None else float(old_score["strategy"]) if old_score else 5.0
    consensus = update.consensus if update.consensus is not None else float(old_score["consensus"]) if old_score else 0.0
    old_final = float(old_score["final_score"]) if old_score else 0.0
    last_boosted = entity.get("last_boosted_at") or now

    score_result = await rust_client.compute_score(
        interest=interest, strategy=strategy, consensus=consensus, last_boosted_at=last_boosted,
    )
    new_final = round(score_result.final_score, 2)

    score_data = {
        "entity_id": update.entity_id,
        "interest": interest, "strategy": strategy, "consensus": consensus,
        "final_score": new_final, "manual_override": update.manual_override, "updated_at": now,
    }

    reason = "manual_override" if update.manual_override else "auto_update"
    await db.begin()
    try:
        await db.upsert_score(score_data)
        # Write score history
        await db.conn.execute(
            """INSERT INTO score_history (entity_id, interest, strategy, consensus, final_score, reason, created_at)
               VALUES (?, ?, ?, ?, ?, ?, ?)""",
            (update.entity_id, interest, strategy, consensus, new_final, reason, now),
        )
        if update.manual_override:
            await db.conn.execute(
                "UPDATE entities SET last_boosted_at = ? WHERE id = ?", (now, update.entity_id),
            )
        await db.log_event(
            update.entity_id, "score_updated",
            trigger="manual" if update.manual_override else "auto",
        )
        await db.commit()
    except Exception:
        await db.rollback()
        raise

    return ScoreResponse(
        entity_id=update.entity_id, final_score=new_final,
        decay_factor=round(score_result.decay_factor, 4),
        days_elapsed=round(score_result.days_elapsed, 1),
    )
