"""REST endpoint: URL content fetching."""
import re
from datetime import datetime, timezone
from typing import Optional

import httpx
from fastapi import APIRouter, HTTPException
from pydantic import BaseModel, HttpUrl

router = APIRouter(prefix="/fetch", tags=["fetch"])


class FetchRequest(BaseModel):
    """Request body for POST /fetch."""
    url: HttpUrl


class FetchResponse(BaseModel):
    """Response from POST /fetch."""
    url: str
    title: Optional[str]
    raw_content: str
    content_type: str
    status_code: int
    fetched_at: str


# User-Agent header as required by spec
_USER_AGENT = "Compass-Fetch/2.1"
_TIMEOUT = 10.0


def _extract_html_title(html: str) -> Optional[str]:
    """Extract <title>...</title> from HTML content."""
    match = re.search(r"<title[^>]*>([^<]+)</title>", html, re.IGNORECASE)
    if match:
        return match.group(1).strip()
    return None


@router.post("", response_model=FetchResponse)
async def fetch_url(req: FetchRequest) -> FetchResponse:
    """Fetch raw HTML/content from a URL.

    - HTTP/HTTPS only
    - 10s timeout → 408
    - Non-2xx response → 502
    - Follows up to 3 redirects
    """
    try:
        async with httpx.AsyncClient(
            timeout=httpx.Timeout(_TIMEOUT),
            follow_redirects=True,
            max_redirects=3,
            headers={"User-Agent": _USER_AGENT},
        ) as client:
            resp = await client.get(str(req.url))
    except httpx.TimeoutException:
        raise HTTPException(status_code=408, detail="Fetch timeout (10s)")
    except httpx.RequestError:
        raise HTTPException(status_code=400, detail="Request failed — check URL")

    if not (200 <= resp.status_code < 300):
        raise HTTPException(
            status_code=502,
            detail=f"Upstream returned {resp.status_code}",
        )

    content_type = resp.headers.get("content-type", "text/plain")
    title = None
    if "text/html" in content_type.lower():
        title = _extract_html_title(resp.text)

    return FetchResponse(
        url=str(req.url),
        title=title,
        raw_content=resp.text,
        content_type=content_type,
        status_code=resp.status_code,
        fetched_at=datetime.now(tz=timezone.utc).isoformat(),
    )