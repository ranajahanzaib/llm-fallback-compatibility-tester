"""JSON Lines process protocol for optional Python provider adapters."""
from __future__ import annotations

import json
import sys
from typing import Any

from . import __version__
from .ollama import chat

PROTOCOL_VERSION = 1


def classify_exception(exc: Exception) -> str:
    try:
        import httpx
        if isinstance(exc, httpx.TimeoutException):
            return "timeout"
        if isinstance(exc, httpx.ConnectError):
            return "connection_failure"
        if isinstance(exc, httpx.HTTPStatusError):
            return {429: "rate_limit", 500: "internal_server_error", 502: "provider_unavailable", 503: "provider_unavailable", 504: "timeout", 401: "authentication_failure", 403: "permission_denied", 400: "invalid_request"}.get(exc.response.status_code, "unknown_provider_error")
    except ImportError:
        pass
    return "unknown_provider_error"


def handle(request: dict[str, Any]) -> dict[str, Any]:
    request_id = request.get("id")
    if request.get("type") == "hello":
        return {"id": request_id, "type": "hello", "protocol_version": PROTOCOL_VERSION, "adapter_version": __version__}
    if request.get("type") == "shutdown":
        return {"id": request_id, "ok": True, "shutdown": True}
    if request.get("provider") == "ollama":
        return chat(request)
    raise ValueError(f"unsupported Python provider: {request.get('provider')!r}")


def main() -> None:
    for line in sys.stdin:
        request: dict[str, Any] = {}
        try:
            request = json.loads(line)
            if not isinstance(request, dict):
                raise ValueError("each JSONL request must be an object")
            result = handle(request)
            print(json.dumps(result, separators=(",", ":")), flush=True)
            if result.get("shutdown"):
                return
        except Exception as exc:  # keep diagnostics off protocol stdout
            error = {"id": request.get("id"), "error": str(exc), "error_classification": classify_exception(exc)}
            print(json.dumps(error, separators=(",", ":")), flush=True)
            print(f"adapter request failed: {exc}", file=sys.stderr, flush=True)


if __name__ == "__main__":
    main()
