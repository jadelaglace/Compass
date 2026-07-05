"""REST endpoint: hybrid semantic + BM25 search."""
import re as _re
from typing import Annotated, Optional

from fastapi import APIRouter, Depends, HTTPException
from pydantic import BaseModel, Field

from src.db.database import Database, get_db

router = APIRouter(prefix="/search", tags=["search"])


class SearchFilters(BaseModel):
    """Optional filters for search."""
    tags: Optional[list[str]] = None
    entity_type: Optional[str] = None
    date_range: Optional[dict] = None  # not yet implemented


class SearchRequest(BaseModel):
    """Request body for POST /search."""
    query: str = Field(..., min_length=1, max_length=500)
    semantic_weight: float = Field(default=0.6, ge=0.0, le=1.0)
    score_weight: float = Field(default=0.4, ge=0.0, le=1.0)
    filters: Optional[SearchFilters] = None
    limit: int = Field(default=20, ge=1, le=100)


class SearchMatch(BaseModel):
    """Single search result item."""
    entity: dict
    match_score: float
    highlights: list[str]


class SearchResponse(BaseModel):
    """Response from POST /search."""
    items: list[SearchMatch]
    total: int
    query_vector_dim: int = 0  # 0 = FTS5 fallback mode (FAISS not yet active)


# FTS5 escape table (same as database.py)
_ESCAPE_TBL = str.maketrans({
    '"': "", "'": "", "(": "", ")": "",
    "*": " ", "-": " ", "+": " ", "^": " ",
    ":": " ", "{": " ", "}": " ", "~": " ",
    "[": " ", "]": " ", "!": " ",
})


def _escape_fts_query(raw: str) -> str:
    """Escape FTS5 special characters."""
    token = " ".join(raw.split())
    if not token:
        return '""'
    escaped = token.translate(_ESCAPE_TBL)
    for kw in ("AND", "OR", "NOT"):
        escaped = _re.sub(rf"\b{kw}\b", " ", escaped, flags=_re.IGNORECASE)
    return " ".join(escaped.split())


def _highlight_match(text: str, query_terms: list[str], radius: int = 40) -> str:
    """Extract a snippet of `text` around the first matching term."""
    text_lower = text.lower()
    for term in query_terms:
        pos = text_lower.find(term.lower())
        if pos >= 0:
            start = max(0, pos - radius)
            end = min(len(text), pos + radius + len(term))
            snippet = text[start:end]
            prefix = "..." if start > 0 else ""
            suffix = "..." if end < len(text) else ""
            return f"{prefix}{snippet}{suffix}"
    return (text[:radius] + "...") if len(text) > radius else text


@router.post("", response_model=SearchResponse)
async def search_entities(
    req: SearchRequest,
    db: Annotated[Database, Depends(get_db)],
) -> SearchResponse:
    """Hybrid search: FTS5 BM25 baseline.

    semantic_weight + score_weight must sum to 1.0.
    FAISS vector search is not yet active — always returns query_vector_dim=0.
    """
    # Validate weight sum
    total_weight = req.semantic_weight + req.score_weight
    if abs(total_weight - 1.0) > 0.001:
        raise HTTPException(
            status_code=422,
            detail=f"semantic_weight ({req.semantic_weight}) + score_weight ({req.score_weight}) must sum to 1.0",
        )

    safe_q = _escape_fts_query(req.query)
    query_terms = req.query.split()

    # Build SQL with optional entity_type filter
    sql_parts = [
        "SELECT e.id, e.title, e.entity_type, e.vault_path, e.category,",
        "s.final_score, rank",
        "FROM entities_fts f",
        "JOIN entities e ON e.id = f.id",
        "LEFT JOIN scores s ON s.entity_id = e.id",
        f"WHERE entities_fts MATCH '{safe_q}'",
    ]
    params: list = []
    if req.filters and req.filters.entity_type:
        sql_parts.append("AND e.entity_type = ?")
        params.append(req.filters.entity_type)

    sql_parts.append("ORDER BY rank LIMIT ?")
    params.append(req.limit)
    sql = " ".join(sql_parts)

    async with db.conn.execute(sql, params) as cur:
        rows = await cur.fetchall()

    # Tag filter (AND logic) in Python
    if req.filters and req.filters.tags:
        filtered = []
        for row in rows:
            entity_id = dict(row)["id"]
            tag_cur = await db.conn.execute(
                "SELECT tag FROM taggings WHERE entity_id = ?",
                (entity_id,),
            )
            entity_tags = {t[0] for t in await tag_cur.fetchall()}
            if all(t in entity_tags for t in req.filters.tags):
                filtered.append(row)
        rows = filtered

    total = len(rows)
    items = []
    for row in rows:
        d = dict(row)
        bm25_score = 1.0 / (1.0 + (d.get("rank") or 0))
        final_score = d.get("final_score") or 0.0
        match_score = round(
            req.semantic_weight * bm25_score + req.score_weight * (final_score / 100.0),
            4,
        )
        entity_dict = {
            "id": d["id"],
            "title": d["title"],
            "entity_type": d["entity_type"],
            "vault_path": d["vault_path"],
            "category": d["category"],
            "score_composite": final_score,
        }
        highlight = _highlight_match(d["title"], query_terms)
        items.append(SearchMatch(
            entity=entity_dict,
            match_score=match_score,
            highlights=[highlight],
        ))

    return SearchResponse(
        items=items,
        total=total,
        query_vector_dim=0,
    )