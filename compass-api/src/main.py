"""Compass API — FastAPI entry point."""
from contextlib import asynccontextmanager

import uvicorn
from fastapi import FastAPI

from src import config
from src.db.database import Database, set_db
from src.api import entities, scores, feed, agent

# Global DB — initialized once at startup
db = Database()


@asynccontextmanager
async def lifespan(app: FastAPI):
    await db.connect()
    set_db(db)  # register the shared instance for Depends()
    print(f"[Compass API] DB ready at {config.DB_PATH}")
    print(f"[Compass API] Vault at {config.VAULT_PATH}")
    print(f"[Compass API] Rust binary at {config.RUST_BINARY_PATH}")
    yield
    await db.close()
    print("[Compass API] Shutdown complete")


app = FastAPI(
    title="Compass API",
    description="Python glue layer — compass-core (Rust) + FastAPI + SQLite",
    version="0.1.0",
    lifespan=lifespan,
)

app.include_router(entities.router)
app.include_router(scores.router)
app.include_router(feed.router)
app.include_router(agent.router)


@app.get("/health")
async def health():
    return {"status": "ok", "vault": str(config.VAULT_PATH)}


if __name__ == "__main__":
    uvicorn.run("main:app", host=config.HOST, port=config.PORT, reload=False)
