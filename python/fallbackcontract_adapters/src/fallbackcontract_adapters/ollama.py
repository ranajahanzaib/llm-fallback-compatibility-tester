"""Small non-streaming Ollama chat adapter; model downloads are never triggered."""
from __future__ import annotations

import time
import json
from typing import Any


def chat(request: dict[str, Any]) -> dict[str, Any]:
    """Normalize one Ollama chat response. Request has id, model, messages, timeout_ms."""
    import httpx

    request_id = request.get("id")
    model = request.get("model")
    if not isinstance(request_id, str) or not isinstance(model, str):
        raise ValueError("request requires string id and model")
    timeout = max(0.05, min(float(request.get("timeout_ms", 30000)) / 1000, 300))
    base_url = str(request.get("base_url", "http://127.0.0.1:11434")).rstrip("/")
    options = {key: value for key, value in (request.get("options") or {}).items() if value is not None}
    started = time.monotonic()
    with httpx.Client(timeout=timeout) as client:
        with client.stream(
            "POST",
            f"{base_url}/api/chat",
            json={
                "model": model,
                "messages": request.get("messages", []),
                "stream": False,
                "format": request.get("format"),
                "tools": request.get("tools"),
                "options": options,
            },
        ) as response:
            response.raise_for_status()
            if int(response.headers.get("content-length", "0") or 0) > 8 * 1024 * 1024:
                raise ValueError("Ollama response exceeds the 8 MiB limit")
            payload = bytearray()
            for chunk in response.iter_bytes():
                payload.extend(chunk)
                if len(payload) > 8 * 1024 * 1024:
                    raise ValueError("Ollama response exceeds the 8 MiB limit")
            body = json.loads(payload)
    message = body.get("message") or {}
    return {
        "id": request_id,
        "provider": "ollama",
        "model": body.get("model", model),
        "content": message.get("content", ""),
        "tool_calls": message.get("tool_calls", []),
        "finish_reason": body.get("done_reason"),
        "usage": {
            "input_tokens": body.get("prompt_eval_count"),
            "output_tokens": body.get("eval_count"),
        },
        "latency_ms": round((time.monotonic() - started) * 1000),
    }
