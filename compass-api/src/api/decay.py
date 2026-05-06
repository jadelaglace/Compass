"""REST endpoints for decay configuration and simulation."""
from datetime import datetime, timedelta, timezone
from typing import Annotated, Optional

from fastapi import APIRouter, HTTPException, Depends, Query
from pydantic import BaseModel, Field

from src.db.database import Database, get_db
from src.core.rust_client import rust_client

router = APIRouter(prefix="/decay", tags=["decay"])


# ---- Decay-1: Half-life Configuration ----

class DecayConfig(BaseModel):
    """Decay half-life configuration for one entity."""

    entity_id: str
    interest_half_life_days: float = Field(ge=1, le=3650, default=30.0)
    strategy_half_life_days: float = Field(ge=1, le=3650, default=365.0)
    consensus_half_life_days: float = Field(ge=1, le=3650, default=60.0)


class DecayConfigResponse(BaseModel):
    entity_id: str
    interest_half_life_days: float
    strategy_half_life_days: float
    consensus_half_life_days: float
    current_scores: dict


class DecayConfigUpdate(BaseModel):
    interest_half_life_days: Optional[float] = Field(None, ge=1, le=3650)
    strategy_half_life_days: Optional[float] = Field(None, ge=1, le=3650)
    consensus_half_life_days: Optional[float] = Field(None, ge=1, le=3650)


@router.get("/{entity_id}/config", response_model=DecayConfigResponse)
async def get_decay_config(
    entity_id: str,
    db: Annotated[Database, Depends(get_db)] = None,
) -> DecayConfigResponse:
    """Get decay half-life configuration for an entity."""
    entity = await db.get_entity(entity_id)
    if not entity:
        raise HTTPException(status_code=404, detail="Entity not found")

    score = await db.get_score(entity_id)
    return DecayConfigResponse(
        entity_id=entity_id,
        interest_half_life_days=score.get("interest_half_life_days", 30.0),
        strategy_half_life_days=score.get("strategy_half_life_days", 365.0),
        consensus_half_life_days=score.get("consensus_half_life_days", 60.0),
        current_scores={
            "interest": score.get("interest", 5.0) if score else 5.0,
            "strategy": score.get("strategy", 5.0) if score else 5.0,
            "consensus": score.get("consensus", 0.0) if score else 0.0,
            "final_score": score.get("final_score", 0.0) if score else 0.0,
        },
    )


@router.patch("/{entity_id}/config", response_model=DecayConfigResponse)
async def update_decay_config(
    entity_id: str,
    config: DecayConfigUpdate,
    db: Annotated[Database, Depends(get_db)] = None,
) -> DecayConfigResponse:
    """Update decay half-life configuration for an entity."""
    entity = await db.get_entity(entity_id)
    if not entity:
        raise HTTPException(status_code=404, detail="Entity not found")

    score = await db.get_score(entity_id)
    if not score:
        raise HTTPException(status_code=404, detail="Score not found for entity")

    # Get current config
    interest_hl = config.interest_half_life_days or score.get("interest_half_life_days", 30.0)
    strategy_hl = config.strategy_half_life_days or score.get("strategy_half_life_days", 365.0)
    consensus_hl = config.consensus_half_life_days or score.get("consensus_half_life_days", 60.0)

    now = datetime.now(tz=timezone.utc).isoformat()

    await db.begin()
    try:
        await db.conn.execute(
            """
            UPDATE scores
            SET interest_half_life_days = ?, strategy_half_life_days = ?, consensus_half_life_days = ?, updated_at = ?
            WHERE entity_id = ?
            """,
            (interest_hl, strategy_hl, consensus_hl, now, entity_id),
        )
        # Recompute score with new half-life config
        score_result = await rust_client.compute_score(
            interest=float(score.get("interest", 5.0)),
            strategy=float(score.get("strategy", 5.0)),
            consensus=float(score.get("consensus", 0.0)),
            last_boosted_at=score.get("last_boosted_at", now),
            interest_half_life_days=interest_hl,
            strategy_half_life_days=strategy_hl,
            consensus_half_life_days=consensus_hl,
        )
        await db.conn.execute(
            "UPDATE scores SET final_score = ?, updated_at = ? WHERE entity_id = ?",
            (round(score_result.final_score, 2), now, entity_id),
        )
        await db.commit()
    except Exception:
        await db.rollback()
        raise

    return DecayConfigResponse(
        entity_id=entity_id,
        interest_half_life_days=interest_hl,
        strategy_half_life_days=strategy_hl,
        consensus_half_life_days=consensus_hl,
        current_scores={
            "interest": score.get("interest", 5.0),
            "strategy": score.get("strategy", 5.0),
            "consensus": score.get("consensus", 0.0),
            "final_score": round(score_result.final_score, 2),
        },
    )


# ---- Decay-2: Decay Preview ----

class DecayPreviewResponse(BaseModel):
    entity_id: str
    current_score: float
    future_score: float
    days_elapsed: int
    days_remaining: float
    decayed_components: dict


@router.get("/{entity_id}/preview")
async def preview_decay(
    entity_id: str,
    days: Annotated[int, Query(ge=1, le=3650, description="Days into the future")] = 30,
    db: Annotated[Database, Depends(get_db)] = None,
) -> DecayPreviewResponse:
    """Preview what a score will be after N days with current decay config."""
    entity = await db.get_entity(entity_id)
    if not entity:
        raise HTTPException(status_code=404, detail="Entity not found")

    score = await db.get_score(entity_id)
    if not score:
        raise HTTPException(status_code=404, detail="Score not found for entity")

    now = datetime.now(tz=timezone.utc)
    future_dt = now + timedelta(days=days)

    current_score = score.get("final_score", 0.0)
    last_boosted_at = score.get("last_boosted_at", now.isoformat())

    # Decay from last_boosted_at to the future point (days ahead of now)
    interest_hl = float(score.get("interest_half_life_days", 30.0))
    strategy_hl = float(score.get("strategy_half_life_days", 365.0))
    consensus_hl = float(score.get("consensus_half_life_days", 60.0))
    interest_init = float(score.get("interest", 5.0))
    strategy_init = float(score.get("strategy", 5.0))
    consensus_init = float(score.get("consensus", 0.0))

    try:
        lb_dt = datetime.fromisoformat(last_boosted_at.replace("Z", "+00:00"))
        future_days = (future_dt - lb_dt).total_seconds() / 86400.0
        future_days = max(0.0, future_days)
    except Exception:
        future_days = float(days)

    interest_decayed = interest_init * (0.5 ** (future_days / interest_hl))
    strategy_decayed = strategy_init * (0.5 ** (future_days / strategy_hl))
    consensus_decayed = consensus_init * (0.5 ** (future_days / consensus_hl))
    future_score = round(interest_decayed * 0.4 + strategy_decayed * 0.4 + consensus_decayed * 0.2, 2)
    days_remaining = round(interest_hl * 3.32, 1) if interest_hl > 0 else 0.0

    return DecayPreviewResponse(
        entity_id=entity_id,
        current_score=round(current_score, 2),
        future_score=future_score,
        days_elapsed=round(future_days, 1),
        days_remaining=days_remaining,
        decayed_components={
            "interest": round(interest_decayed, 4),
            "strategy": round(strategy_decayed, 4),
            "consensus": round(consensus_decayed, 4),
        },
    )


# ---- Decay-3: Decay Simulator ----

class SimulatorEntry(BaseModel):
    day: int
    date: str
    final_score: float
    interest: float
    strategy: float
    consensus: float


class SimulatorResponse(BaseModel):
    entity_id: str
    start_date: str
    end_date: str
    start_score: float
    end_score: float
    total_decay_pct: float
    trajectory: list[SimulatorEntry]


@router.get("/{entity_id}/simulate")
async def simulate_decay(
    entity_id: str,
    days: Annotated[int, Query(ge=1, le=3650, description="Number of days to simulate")] = 90,
    step_days: Annotated[int, Query(ge=1, le=365, description="Sampling interval in days")] = 7,
    db: Annotated[Database, Depends(get_db)] = None,
) -> SimulatorResponse:
    """Simulate score decay over N days with current config.

    Returns score trajectory at each step_days interval.
    """
    entity = await db.get_entity(entity_id)
    if not entity:
        raise HTTPException(status_code=404, detail="Entity not found")

    score = await db.get_score(entity_id)
    if not score:
        raise HTTPException(status_code=404, detail="Score not found for entity")

    now = datetime.now(tz=timezone.utc)
    start_iso = score.get("last_boosted_at", now.isoformat())

    interest_hl = float(score.get("interest_half_life_days", 30.0))
    strategy_hl = float(score.get("strategy_half_life_days", 365.0))
    consensus_hl = float(score.get("consensus_half_life_days", 60.0))

    interest_val = float(score.get("interest", 5.0))
    strategy_val = float(score.get("strategy", 5.0))
    consensus_val = float(score.get("consensus", 0.0))

    trajectory: list[SimulatorEntry] = []
    current_score = score.get("final_score", 0.0)

    # Day 0 — baseline
    trajectory.append(SimulatorEntry(
        day=0,
        date=now.strftime("%Y-%m-%d"),
        final_score=round(current_score, 2),
        interest=interest_val,
        strategy=strategy_val,
        consensus=consensus_val,
    ))

    # Parse last_boosted_at for trajectory computation
    try:
        lb_dt = datetime.fromisoformat(start_iso.replace("Z", "+00:00"))
    except Exception:
        lb_dt = now

    for step in range(step_days, days + 1, step_days):
        future_dt = now + timedelta(days=step)
        # days from last_boosted_at to this future point
        days_from_boost = (future_dt - lb_dt).total_seconds() / 86400.0
        days_from_boost = max(0.0, days_from_boost)
        trajectory.append(SimulatorEntry(
            day=step,
            date=future_dt.strftime("%Y-%m-%d"),
            final_score=round(
                interest_val * (0.5 ** (days_from_boost / interest_hl)) * 0.4 +
                strategy_val * (0.5 ** (days_from_boost / strategy_hl)) * 0.4 +
                consensus_val * (0.5 ** (days_from_boost / consensus_hl)) * 0.2,
                2,
            ),
            interest=round(interest_val * (0.5 ** (days_from_boost / interest_hl)), 4),
            strategy=round(strategy_val * (0.5 ** (days_from_boost / strategy_hl)), 4),
            consensus=round(consensus_val * (0.5 ** (days_from_boost / consensus_hl)), 4),
        ))

    start_score = trajectory[0].final_score
    end_score = trajectory[-1].final_score
    total_decay_pct = round((1 - end_score / start_score) * 100, 2) if start_score > 0 else 0.0

    return SimulatorResponse(
        entity_id=entity_id,
        start_date=now.strftime("%Y-%m-%d"),
        end_date=(now + timedelta(days=days)).strftime("%Y-%m-%d"),
        start_score=start_score,
        end_score=end_score,
        total_decay_pct=total_decay_pct,
        trajectory=trajectory,
    )