"""A tiny in-memory cache."""

from __future__ import annotations

from typing import Any


class Cache:
    """Key/value cache with a hit counter."""

    def __init__(self) -> None:
        self._data: dict[Any, Any] = {}
        self.hits = 0

    def get(self, key: Any) -> Any:
        """Return the cached value for key, or None."""
        value = self._data.get(key)
        if value is not None:
            self.hits += 1
        return value

    def set(self, key: Any, value: Any) -> None:
        """Store value under key."""
        self._data[key] = value

    def items(self) -> list[tuple[Any, Any]]:
        """Return all (key, value) pairs."""
        return list(self._data.items())

    def get_stats(self) -> dict[str, int]:
        """Return size and hit count."""
        return {"size": len(self._data), "hits": self.hits}

    def clear(self) -> None:
        """Drop everything."""
        self._data.clear()

    def __len__(self) -> int:
        return len(self._data)
