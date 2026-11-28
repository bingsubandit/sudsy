"""Fake http client used by the shop (no network)."""

from __future__ import annotations

from typing import Any

calls: list[tuple[str, str, float | None]] = []

_ORDERS = {
    "A100": {"id": "A100", "total": "120.50", "currency": "USD"},
    "A200": {"id": "A200", "total": "19.99", "currency": "USD"},
}
_RATES = {"EU": "0.21", "US": "0.08", "UK": "0.20"}
_USERS = {"1": {"id": 1, "name": "Ada"}, "2": {"id": 2, "name": "bob"}}


def get(url: str, timeout: float | None = None) -> dict[str, Any]:
    """Pretend to GET url."""
    calls.append(("get", url, timeout))
    parts = url.rstrip("/").split("/")
    kind, key = parts[-2], parts[-1]
    table = {"orders": _ORDERS, "rates": None, "users": _USERS}.get(kind, {})
    if kind == "rates":
        if key in _RATES:
            return {"region": key, "rate": _RATES[key]}
        return {"error": "not found"}
    if table and key in table:
        return dict(table[key])
    return {"error": "not found"}


def post(
    url: str, data: dict[str, Any] | None = None, timeout: float | None = None
) -> dict[str, Any]:
    """Pretend to POST data to url."""
    calls.append(("post", url, timeout))
    return {"ok": True, "url": url, "data": data}
