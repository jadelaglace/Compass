"""REST endpoints for evolution rules."""
from datetime import datetime, timezone
from typing import Annotated, Optional

from fastapi import APIRouter, HTTPException, Depends, Query
from pydantic import BaseModel

from src.db.database import Database, get_db

router = APIRouter(prefix="/evolution-rules", tags=["evolution"])


# ---- Schemas ----

class EvolutionRuleCreate(BaseModel):
    id: str
    category: str
    upgrade_conditions: dict  # e.g. {"access_count": 5, "min_score": 5.0}
    downgrade_conditions: dict  # e.g. {"days": 30}
    locked: bool = False


class EvolutionRuleResponse(BaseModel):
    id: str
    category: str
    upgrade_conditions: dict
    downgrade_conditions: dict
    locked: bool
    created_at: str
    updated_at: str


# ---- Endpoints ----

@router.post("", response_model=EvolutionRuleResponse)
async def create_evolution_rule(
    rule: EvolutionRuleCreate,
    db: Annotated[Database, Depends(get_db)] = None,
) -> EvolutionRuleResponse:
    """Create or update an evolution rule for a category."""
    now = datetime.now(tz=timezone.utc).isoformat()
    data = {
        "id": rule.id,
        "category": rule.category,
        "upgrade_conditions": rule.upgrade_conditions,
        "downgrade_conditions": rule.downgrade_conditions,
        "locked": rule.locked,
        "created_at": now,
        "updated_at": now,
    }
    await db.begin()
    try:
        await db.upsert_evolution_rule(data)
        await db.commit()
    except Exception:
        await db.rollback()
        raise

    return EvolutionRuleResponse(
        id=rule.id,
        category=rule.category,
        upgrade_conditions=rule.upgrade_conditions,
        downgrade_conditions=rule.downgrade_conditions,
        locked=rule.locked,
        created_at=now,
        updated_at=now,
    )


@router.get("", response_model=list[EvolutionRuleResponse])
async def list_evolution_rules(
    db: Annotated[Database, Depends(get_db)] = None,
) -> list[EvolutionRuleResponse]:
    """List all evolution rules."""
    rules = await db.get_all_evolution_rules()
    return [
        EvolutionRuleResponse(
            id=r["id"],
            category=r["category"],
            upgrade_conditions=r["upgrade_conditions"],
            downgrade_conditions=r["downgrade_conditions"],
            locked=r["locked"],
            created_at=r["created_at"],
            updated_at=r["updated_at"],
        )
        for r in rules
    ]


@router.get("/{category}", response_model=EvolutionRuleResponse)
async def get_evolution_rule(
    category: str,
    db: Annotated[Database, Depends(get_db)] = None,
) -> EvolutionRuleResponse:
    """Get the evolution rule for a specific category."""
    rule = await db.get_evolution_rule(category)
    if not rule:
        raise HTTPException(status_code=404, detail=f"No evolution rule found for category '{category}'")
    return EvolutionRuleResponse(
        id=rule["id"],
        category=rule["category"],
        upgrade_conditions=rule["upgrade_conditions"],
        downgrade_conditions=rule["downgrade_conditions"],
        locked=rule["locked"],
        created_at=rule["created_at"],
        updated_at=rule["updated_at"],
    )


@router.delete("/{category}")
async def delete_evolution_rule(
    category: str,
    db: Annotated[Database, Depends(get_db)] = None,
) -> dict:
    """Delete an evolution rule for a category."""
    await db.begin()
    try:
        deleted = await db.delete_evolution_rule(category)
        await db.commit()
    except Exception:
        await db.rollback()
        raise

    if not deleted:
        raise HTTPException(status_code=404, detail=f"No evolution rule found for category '{category}'")

    return {"deleted": category}