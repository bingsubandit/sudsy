"""Project logger."""

from __future__ import annotations


class _Log:
    def __init__(self) -> None:
        self.records: list[str] = []

    def info(self, *args: object) -> None:
        """Record an info message."""
        self.records.append(" ".join(str(a) for a in args))


log = _Log()
