"""REST endpoint: graph neighbor queries."""
from typing import Annotated, Optional

from fastapi import APIRouter, Depends, HTTPException, Query
from pydantic import BaseModel

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
    depth: Annotated[int, Query(ge=1, le=3, description="BFS depth, 1-3")] = 1,
    min_strength: Annotated[
        float, Query(ge=0.0, le=1.0, description="Minimum edge strength filter")
    ] = 0.0,
    db: Database = Depends(get_db),
) -> NeighborsResponse:
    """Return neighbors of an entity with optional depth and strength filtering.

    - depth=1: direct neighbors only (default)
    - depth=2: neighbors + 2nd-degree
    - depth=3: neighbors + 2nd + 3rd-degree (max)
    - min_strength: filter edges below threshold (0 = no filter)
    """
    # Verify entity exists
    entity = await db.get_entity(entity_id)
    if not entity:
        raise HTTPException(status_code=404, detail="Entity not found")

    if depth < 1 or depth > 3:
        raise HTTPException(status_code=400, detail="depth must be 1-3")

    if min_strength > 1.0:
        # Return empty rather than error
        return NeighborsResponse(nodes=[], edges=[], total_neighbors=0)

    visited: set[str] = {entity_id}
    queue: list[tuple[str, int]] = [(entity_id, 0)]
    nodes_map: dict[str, GraphNode] = {}
    edges: list[GraphEdge] = []
    MAX_NODES = 200

    while queue:
        node_id, current_depth = queue.pop(0)
        if current_depth >= depth:
            continue

        # Query outgoing edges
        out_params: list = [node_id]
        strength_clause = ""
        if min_strength > 0:
            strength_clause = "AND r.strength >= ?"
            out_params.append(str(min_strength))

        out_cur = await db.conn.execute(
            f"""
            SELECT r.target_id, r.strength, e.title, e.entity_type, s.final_score
            FROM "references" r
            JOIN entities e ON e.id = r.target_id
            LEFT JOIN scores s ON s.entity_id = r.target_id
            WHERE r.source_id = ? {strength_clause}
            """,
            out_params,
        )
        for row in out_cur:
            tid = row["target_id"]
            if tid not in visited and len(visited) < MAX_NODES:
                visited.add(tid)
                nodes_map[tid] = GraphNode(
                    id=tid,
                    title=row["title"],
                    entity_type=row["entity_type"],
                    score_composite=row["final_score"],
                )
                queue.append((tid, current_depth + 1))
            edges.append(GraphEdge(
                source=node_id,
                target=tid,
                ref_type="cites",
                strength=row["strength"],
                direction="outgoing",
            ))

        # Query incoming edges
        in_params: list = [node_id]
        if min_strength > 0:
            strength_clause_in = "AND r.strength >= ?"
            in_params.append(str(min_strength))
        else:
            strength_clause_in = ""

        in_cur = await db.conn.execute(
            f"""
            SELECT r.source_id, r.strength, e.title, e.entity_type, s.final_score
            FROM "references" r
            JOIN entities e ON e.id = r.source_id
            LEFT JOIN scores s ON s.entity_id = r.source_id
            WHERE r.target_id = ? {strength_clause_in}
            """,
            in_params,
        )
        for row in in_cur:
            sid = row["source_id"]
            if sid not in visited and len(visited) < MAX_NODES:
                visited.add(sid)
                nodes_map[sid] = GraphNode(
                    id=sid,
                    title=row["title"],
                    entity_type=row["entity_type"],
                    score_composite=row["final_score"],
                )
                queue.append((sid, current_depth + 1))
            edges.append(GraphEdge(
                source=sid,
                target=node_id,
                ref_type="cites",
                strength=row["strength"],
                direction="incoming",
            ))

    return NeighborsResponse(
        nodes=list(nodes_map.values()),
        edges=edges,
        total_neighbors=len(nodes_map),
    )


class PathEdge(BaseModel):
    source: str
    target: str
    ref_type: str = "cites"


class PathResponse(BaseModel):
    path: list[str]
    edges: list[PathEdge]
    distance: int


@router.get("/path", response_model=PathResponse)
async def get_path(
    from_entity: str = Query(..., alias="from"),
    to_entity: str = Query(..., alias="to"),
    db: Database = Depends(get_db),
) -> PathResponse:
    """Find shortest path between two entities via BFS."""
    if from_entity == to_entity:
        return PathResponse(
            path=[from_entity],
            edges=[],
            distance=0,
        )

    visited: set[str] = {from_entity}
    queue: list[tuple[str, list[str]]] = [(from_entity, [from_entity])]
    MAX_VISIT = 500

    while queue:
        node_id, path = queue.pop(0)
        if len(visited) >= MAX_VISIT:
            break

        # Get all neighbors of current node
        cur = await db.conn.execute(
            """
            SELECT 'outgoing' AS direction, r.source_id, r.target_id
            FROM "references" r WHERE r.source_id = ?
            UNION ALL
            SELECT 'incoming' AS direction, r.source_id, r.target_id
            FROM "references" r WHERE r.target_id = ?
            """,
            (node_id, node_id),
        )
        for row in cur:
            neighbor = row["target_id"] if row["direction"] == "outgoing" else row["source_id"]
            if neighbor == to_entity:
                full_path = path + [neighbor]
                edge_list = [
                    PathEdge(source=full_path[i], target=full_path[i + 1])
                    for i in range(len(full_path) - 1)
                ]
                return PathResponse(
                    path=full_path,
                    edges=edge_list,
                    distance=len(full_path) - 1,
                )
            if neighbor not in visited and len(visited) < MAX_VISIT:
                visited.add(neighbor)
                queue.append((neighbor, path + [neighbor]))

    raise HTTPException(
        status_code=404,
        detail="No path found between entities",
    )