"""Plain data models."""

from __future__ import annotations

from dataclasses import dataclass
from decimal import Decimal


@dataclass
class Item:
    sku: str
    name: str
    qty: int
    price: Decimal


@dataclass
class User:
    id: int
    name: str
    active: bool = True
    email: str = ""


_USERS: dict[int, User] = {
    1: User(1, "Ada", True, "ada@example.com"),
    2: User(2, "bob", False, "bob@example.com"),
    3: User(3, "Cleo", True, "cleo@example.com"),
    4: User(4, "dave", True, ""),
}


def get_user(uid: int) -> User | None:
    """Look up a user by id."""
    return _USERS.get(uid)


def get_all_users() -> list[User]:
    """Return every user."""
    return list(_USERS.values())
