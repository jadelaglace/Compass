"""Compass API — FastAPI entry point."""
import logging
from contextlib import asynccontextmanager

logging.basicConfig(level=logging.INFO, format="%(message)s")

import uvicorn
from fastapi import FastAPI

from src import config
from src.db.database import Database, set_db
from src.api import entities, scores, feed, agent

# Global DB — initialized once at startup
db = Database()


@asynccontextmanager
async def lifespan(app: FastAPI):
    """FastAPI lifespan manager — opens DB connection on startup, closes on shutdown."""
    await db.connect()
    set_db(db)  # register the shared instance for Depends()
    logging.info(f"[Compass API] DB ready at {config.DB_PATH}")
    logging.info(f"[Compass API] Vault at {config.VAULT_PATH}")
    logging.info(f"[Compass API] Rust binary at {config.RUST_BINARY_PATH}")
    yield
    await db.close()
    logging.info("[Compass API] Shutdown complete")


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
    """Basic health check endpoint."""
    return {"status": "ok", "vault": str(config.VAULT_PATH)}


if __name__ == "__main__":
    uvicorn.run("main:app", host=config.HOST, port=config.PORT, reload=False)
