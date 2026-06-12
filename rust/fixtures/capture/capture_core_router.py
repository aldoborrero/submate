"""Capture the core server router contract -> fixtures/server/core_router.json.

Falsifier target: submate-server parity::core_router (GET / and GET /status).

Drives the *Python* `create_core_router()` handlers
(`submate/server/handlers/core/router.py`) and records the static response
contract:

- `GET /` — the full server-info object (`name`, `version`, `docs`, and all five
  `endpoints` keys/values).
- `GET /status` — only the *static* envelope: `status`, `version`, and the
  presence of the `queue` key. The `queue` value is `task_queue.stats`, which is
  live Huey state (`{pending, scheduled}`); the Rust server uses a different
  node-topology shape on purpose, so the live numbers are out of scope here and
  recorded only as a presence sentinel.
"""

from __future__ import annotations

import asyncio
import inspect
from typing import Any

from _common import write_json

from submate.server.handlers.core.router import create_core_router


def _route_handler(router: Any, path: str) -> Any:
    """Find the endpoint coroutine for `path` on the APIRouter."""
    for route in router.routes:
        if getattr(route, "path", None) == path:
            return route.endpoint
    raise KeyError(f"no route registered for {path!r}")


def _call(handler: Any) -> Any:
    """Invoke an endpoint, awaiting if it is a coroutine function."""
    if inspect.iscoroutinefunction(handler):
        return asyncio.run(handler())
    return handler()


def main() -> None:
    router = create_core_router()

    root_body = _call(_route_handler(router, "/"))

    status_body = _call(_route_handler(router, "/status"))
    # Record only the static contract for /status: the scalar envelope plus the
    # mere presence of the `queue` key. The queue value is live Huey state and
    # the Rust port uses a different topology, so it is not pinned.
    status_static = {
        "status": status_body["status"],
        "version": status_body["version"],
        "queue_key_present": "queue" in status_body,
    }

    write_json("server/core_router.json", {"root": root_body, "status": status_static})


if __name__ == "__main__":
    main()
