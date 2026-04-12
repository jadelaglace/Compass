"""Agent-facing endpoints — context injection and tool interfaces."""
from typing import Annotated
from fastapi import APIRouter, Depends
from pydantic import BaseModel

from src.db.database import Database, get_db
from src.core.rust_client import rust_client

router = APIRouter(prefix="/agent", tags=["agent"])


class ContextRequest(BaseModel):
    """Request schema for agent context injection."""

    task: str
    top_k: int = 5


class ContextResponse(BaseModel):
    """Response schema for agent context injection — top candidates + suggestions."""

    context: list[dict]
    suggested_entities: list[str]
    reasoning: str


@router.post("/context", response_model=ContextResponse)
async def get_context(req: ContextRequest, db: Annotated[Database, Depends(get_db)] = Depends(get_db)) -> ContextResponse:
    """Search and score entities as context for an agent task."""
    candidates = await db.search_entities(req.task, limit=req.top_k * 2)
    if not candidates:
        return ContextResponse(context=[], suggested_entities=[], reasoning="No entities found for query.")

    scored = []
    for e in candidates:
        last_boosted = e.get("last_boosted_at") or ""
        score_result = rust_client.compute_score(
            interest=float(e.get("interest") or 5.0),
            strategy=float(e.get("strategy") or 5.0),
            consensus=float(e.get("consensus") or 0.0),
            last_boosted_at=last_boosted,
        )
        e["final_score"] = round(score_result.final_score, 2)
        scored.append(e)

    scored.sort(key=lambda x: x["final_score"], reverse=True)
    top = scored[: req.top_k]
    suggested = [s["id"] for s in scored[req.top_k : req.top_k * 2]]

    return ContextResponse(
        context=top,
        suggested_entities=suggested,
        reasoning=f"Selected top {len(top)} by weighted score (strategy×0.35 + interest×0.4 + consensus×0.25) × decay.",
    )
