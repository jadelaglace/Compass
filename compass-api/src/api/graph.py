"""REST endpoint: graph neighbor queries."""
from fastapi import APIRouter, Depends, HTTPException
from pydantic import BaseModel
from typing import Optional

from src.db.database import Database, get_db

router = APIRouter(prefix="/graph", tags=["graph"])


class GraphNode(BaseModel):
    id: str
    title: str
    entity_type: str
    score_composite: Optional[float] = None


class GraphEdge(BaseModel):
    source: str
    target: str
    ref_type: str  # always "cites" for now
    strength: float
    direction: str  # "incoming" | "outgoing"


class NeighborsResponse(BaseModel):
    nodes: list[GraphNode]
    edges: list[GraphEdge]
    total_neighbors: int


@router.get("/neighbors/{entity_id}", response_model=NeighborsResponse)
async def get_neighbors(
    entity_id: str,
    db: Database = Depends(get_db),
) -> NeighborsResponse:
    """Return all direct neighbors (incoming + outgoing) of an entity."""
    # Verify entity exists
    entity = await db.get_entity(entity_id)
    if not entity:
        raise HTTPException(status_code=404, detail="Entity not found")

    # OUTGOING: references where source_id = entity_id
    out_cur = await db.conn.execute(
        """
        SELECT r.target_id, r.strength, e.title, e.entity_type, s.final_score
        FROM "references" r
        JOIN entities e ON e.id = r.target_id
        LEFT JOIN scores s ON s.entity_id = r.target_id
        WHERE r.source_id = ?
        """,
        (entity_id,),
    )
    out_rows = await out_cur.fetchall()

    # INCOMING: references where target_id = entity_id
    in_cur = await db.conn.execute(
        """
        SELECT r.source_id, r.strength, e.title, e.entity_type, s.final_score
        FROM "references" r
        JOIN entities e ON e.id = r.source_id
        LEFT JOIN scores s ON s.entity_id = r.source_id
        WHERE r.target_id = ?
        """,
        (entity_id,),
    )
    in_rows = await in_cur.fetchall()

    # Deduplicate nodes
    nodes_map: dict[str, GraphNode] = {}
    edges: list[GraphEdge] = []

    for row in out_rows:
        tid = row["target_id"]
        if tid not in nodes_map:
            nodes_map[tid] = GraphNode(
                id=tid,
                title=row["title"],
                entity_type=row["entity_type"],
                score_composite=row["final_score"],
            )
        edges.append(GraphEdge(
            source=entity_id,
            target=tid,
            ref_type="cites",  # all refs currently are cites
            strength=row["strength"],
            direction="outgoing",
        ))

    for row in in_rows:
        sid = row["source_id"]
        if sid not in nodes_map:
            nodes_map[sid] = GraphNode(
                id=sid,
                title=row["title"],
                entity_type=row["entity_type"],
                score_composite=row["final_score"],
            )
        edges.append(GraphEdge(
            source=sid,
            target=entity_id,
            ref_type="cites",
            strength=row["strength"],
            direction="incoming",
        ))

    return NeighborsResponse(
        nodes=list(nodes_map.values()),
        edges=edges,
        total_neighbors=len(nodes_map),
    )