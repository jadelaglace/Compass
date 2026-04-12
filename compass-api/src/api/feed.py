"""Feed — daily digest of top entities."""
from typing import Annotated
from fastapi import APIRouter, Depends
from pydantic import BaseModel

from src.db.database import Database, get_db

router = APIRouter(prefix="/feed", tags=["feed"])


class FeedResponse(BaseModel):
    """Daily digest response — top Inbox items, recently updated, and strategic items."""

    top_inbox: list[dict]
    recently_updated: list[dict]
    strategic: list[dict]


@router.get("/today", response_model=FeedResponse)
async def daily_feed(limit: int = 10, db: Annotated[Database, Depends(get_db)] = Depends(get_db)) -> FeedResponse:
    """Return the daily digest: top Inbox items, recently updated, and strategic items."""
    top_inbox = await db.get_top_entities(limit=limit, category="Inbox")
    recently = await db.get_top_entities(limit=limit)
    strategic = await db.get_top_entities(limit=limit, category="Direction")

    return FeedResponse(
        top_inbox=top_inbox,
        recently_updated=recently,
        strategic=strategic,
    )
