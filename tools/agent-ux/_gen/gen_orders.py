"""Generate base/shop/orders.py (run once; output committed in base/)."""
import sys

WORDS = ["warehouse", "carrier", "channel", "promo", "customer", "gift",
         "region", "supplier", "batch", "return", "pickup", "bundle"]


def tmpl_funcs(w: str) -> str:
    p = w[:3].upper()
    return f'''
_{w.upper()}_TABLE: dict[str, str] = {{"std": "standard", "exp": "express", "{w}": "{w} special"}}


def _{w}_code(value: str) -> str:
    """Return the canonical {w} code for value."""
    cleaned = value.strip().upper().replace(" ", "-")
    if len(cleaned) > MAX_CODE_LEN:
        cleaned = cleaned[:MAX_CODE_LEN]
    return f"{p}-{{cleaned}}"


def _{w}_label(code: str, count: int) -> str:
    """Format a human label for a {w} code."""
    suffix = "item" if count == 1 else "items"
    return f"{{code}}: {{count}} {{suffix}}"


def _is_valid_{w}(code: str) -> bool:
    """Check that a {w} code has the expected shape."""
    prefix, _, rest = code.partition("-")
    return prefix == "{p}" and bool(rest)


def _{w}_lookup(key: str, default: str = "") -> str:
    """Look up a {w} name in its table."""
    return _{w.upper()}_TABLE.get(key, default)


def _clamp_{w}_qty(qty: int, limit: int = MAX_LINES) -> int:
    """Clamp a quantity for {w} handling."""
    return max(0, min(qty, limit))


def _{w}_weight(sku: str) -> int:
    """Memoized weight of a sku in grams for {w} shipments."""
    cached = _memo.get(sku)
    if cached is not None:
        return cached
    weight = sum(ord(ch) for ch in sku) % MAX_WEIGHT
    _memo[sku] = weight
    return weight
'''


def tmpl_methods(w: str) -> str:
    p = w[:3].upper()
    return f'''
    def {w}_total(self) -> Decimal:
        """Total of lines whose sku starts with {p}."""
        return sum(
            (line.total() for line in self.lines if line.sku.startswith("{p}")),
            Decimal(0),
        )

    def {w}_count(self) -> int:
        """Number of units whose sku starts with {p}."""
        return sum(line.qty for line in self.lines if line.sku.startswith("{p}"))

    def has_{w}(self) -> bool:
        """True when any line is a {w} line."""
        return any(line.sku.startswith("{p}") for line in self.lines)
'''


def funcs(ws):
    return "".join(tmpl_funcs(w) for w in ws)


def methods(ws):
    return "".join(tmpl_methods(w) for w in ws)


out = f'''"""Order handling: line items, orders, order books and reports."""

from __future__ import annotations

from dataclasses import dataclass, field
from decimal import ROUND_HALF_EVEN, Decimal

from shop.cache import Cache

CENT = Decimal("0.01")
MAX_LINES = 50
MAX_CODE_LEN = 16
MAX_WEIGHT = 5000
_memo: dict[str, int] = {{}}


def _round_money(value: Decimal) -> Decimal:
    """Round a Decimal to cents (half even)."""
    return value.quantize(CENT, rounding=ROUND_HALF_EVEN)

{funcs(WORDS[:4])}

@dataclass
class LineItem:
    """One sku with a quantity and a unit price."""

    sku: str
    qty: int
    unit_price: Decimal

    def total(self) -> Decimal:
        """Unit price times quantity, unrounded."""
        return self.unit_price * self.qty

    def is_empty(self) -> bool:
        """True for zero-quantity lines."""
        return self.qty == 0

    def label(self) -> str:
        """Short text for the line."""
        return f"{{self.sku}} x{{self.qty}}"

{funcs(WORDS[4:8])}

def _apply_discount(amount: Decimal, pct: Decimal) -> Decimal:
    """Reduce amount by pct percent (pct=10 means 10 percent off); not rounded."""
    return amount * (Decimal(100) - pct) / Decimal(100)

{funcs(WORDS[8:10])}

@dataclass
class Order:
    """An order made of line items."""

    order_id: str
    lines: list[LineItem] = field(default_factory=list)

    def add_line(self, line: LineItem) -> None:
        """Append a line."""
        self.lines.append(line)

    def total(self) -> Decimal:
        """Sum of line totals, unrounded."""
        return sum((line.total() for line in self.lines), Decimal(0))

    def unit_count(self) -> int:
        """Total number of units."""
        return sum(line.qty for line in self.lines)
{methods(WORDS)}
{funcs(WORDS[10:])}

class OrderBook:
    """A collection of orders with a book-level discount."""

    def __init__(self) -> None:
        self.orders: list[Order] = []
        self.discount_pct: Decimal = Decimal(0)
        self._cache = Cache()

    def add(self, order: Order) -> None:
        """Add an order and index it."""
        self.orders.append(order)
        self._cache.set(order.order_id, order)

    def find(self, order_id: str) -> Order | None:
        """Find an order by id."""
        hit = self._cache.get(order_id)
        if hit is not None:
            return hit
        for order in self.orders:
            if order.order_id == order_id:
                return order
        return None

    def subtotal(self) -> Decimal:
        """Sum of all order totals, unrounded."""
        return sum((order.total() for order in self.orders), Decimal(0))

    def rounded_subtotal(self) -> Decimal:
        """Subtotal rounded to cents."""
        return _round_money(self.subtotal())

    def discounted_total(self) -> Decimal:
        """Subtotal after the book discount, rounded to cents."""
        return _round_money(_apply_discount(self.subtotal(), self.discount_pct))

    def order_ids(self) -> list[str]:
        """Ids of all orders in insertion order."""
        return [order.order_id for order in self.orders]

    def unit_count(self) -> int:
        """Total units across orders."""
        return sum(order.unit_count() for order in self.orders)

    def largest(self) -> Order | None:
        """The order with the highest total."""
        if not self.orders:
            return None
        return max(self.orders, key=lambda order: order.total())
{methods(WORDS[:6]).replace("self.lines", "self._all_lines()")}
    def _all_lines(self) -> list[LineItem]:
        """Flatten the lines of every order."""
        return [line for order in self.orders for line in order.lines]


@dataclass
class Shipment:
    """A shipment of orders."""

    shipment_id: str
    orders: list[Order] = field(default_factory=list)

    def lines(self) -> list[LineItem]:
        """All lines in the shipment."""
        return [line for order in self.orders for line in order.lines]
{methods(WORDS).replace("self.lines", "self.lines()")}

@dataclass
class OrderReport:
    """Aggregated numbers for a set of orders."""

    orders: list[Order] = field(default_factory=list)

    @property
    def lines(self) -> list[LineItem]:
        """All lines of all orders."""
        return [line for order in self.orders for line in order.lines]
{methods(WORDS)}
{funcs(["audit", "ledger"])}

_TAX_RATES: dict[str, Decimal] = {{
    "US": Decimal("0.08"),
    "EU": Decimal("0.21"),
    "UK": Decimal("0.20"),
}}


def _tax_for(region: str) -> Decimal:
    """Sales tax rate for region as a fraction (0.08 means 8 percent)."""
    try:
        return _TAX_RATES[region]
    except KeyError:
        msg = f"unknown region: {{region}}"
        raise ValueError(msg) from None


def cached_total(order_id: str, cache: Cache) -> Decimal | None:
    """Return the cached total of an order, if known."""
    total = cache.get(order_id)
    if total is None:
        return None
    return Decimal(total)
'''
open(sys.argv[1], "w", newline="\n").write(out)
