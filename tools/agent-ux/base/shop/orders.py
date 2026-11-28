"""Order handling: line items, orders, order books and reports."""

from __future__ import annotations

from dataclasses import dataclass, field
from decimal import ROUND_HALF_EVEN, Decimal

from shop.cache import Cache

CENT = Decimal("0.01")
MAX_LINES = 50
MAX_CODE_LEN = 16
MAX_WEIGHT = 5000
_memo: dict[str, int] = {}


def _round_money(value: Decimal) -> Decimal:
    """Round a Decimal to cents (half even)."""
    return value.quantize(CENT, rounding=ROUND_HALF_EVEN)


_WAREHOUSE_TABLE: dict[str, str] = {
    "std": "standard",
    "exp": "express",
    "warehouse": "warehouse special",
}


def _warehouse_code(value: str) -> str:
    """Return the canonical warehouse code for value."""
    cleaned = value.strip().upper().replace(" ", "-")
    if len(cleaned) > MAX_CODE_LEN:
        cleaned = cleaned[:MAX_CODE_LEN]
    return f"WAR-{cleaned}"


def _warehouse_label(code: str, count: int) -> str:
    """Format a human label for a warehouse code."""
    suffix = "item" if count == 1 else "items"
    return f"{code}: {count} {suffix}"


def _is_valid_warehouse(code: str) -> bool:
    """Check that a warehouse code has the expected shape."""
    prefix, _, rest = code.partition("-")
    return prefix == "WAR" and bool(rest)


def _warehouse_lookup(key: str, default: str = "") -> str:
    """Look up a warehouse name in its table."""
    return _WAREHOUSE_TABLE.get(key, default)


def _clamp_warehouse_qty(qty: int, limit: int = MAX_LINES) -> int:
    """Clamp a quantity for warehouse handling."""
    return max(0, min(qty, limit))


def _warehouse_weight(sku: str) -> int:
    """Memoized weight of a sku in grams for warehouse shipments."""
    cached = _memo.get(sku)
    if cached is not None:
        return cached
    weight = sum(ord(ch) for ch in sku) % MAX_WEIGHT
    _memo[sku] = weight
    return weight


_CARRIER_TABLE: dict[str, str] = {
    "std": "standard",
    "exp": "express",
    "carrier": "carrier special",
}


def _carrier_code(value: str) -> str:
    """Return the canonical carrier code for value."""
    cleaned = value.strip().upper().replace(" ", "-")
    if len(cleaned) > MAX_CODE_LEN:
        cleaned = cleaned[:MAX_CODE_LEN]
    return f"CAR-{cleaned}"


def _carrier_label(code: str, count: int) -> str:
    """Format a human label for a carrier code."""
    suffix = "item" if count == 1 else "items"
    return f"{code}: {count} {suffix}"


def _is_valid_carrier(code: str) -> bool:
    """Check that a carrier code has the expected shape."""
    prefix, _, rest = code.partition("-")
    return prefix == "CAR" and bool(rest)


def _carrier_lookup(key: str, default: str = "") -> str:
    """Look up a carrier name in its table."""
    return _CARRIER_TABLE.get(key, default)


def _clamp_carrier_qty(qty: int, limit: int = MAX_LINES) -> int:
    """Clamp a quantity for carrier handling."""
    return max(0, min(qty, limit))


def _carrier_weight(sku: str) -> int:
    """Memoized weight of a sku in grams for carrier shipments."""
    cached = _memo.get(sku)
    if cached is not None:
        return cached
    weight = sum(ord(ch) for ch in sku) % MAX_WEIGHT
    _memo[sku] = weight
    return weight


_CHANNEL_TABLE: dict[str, str] = {
    "std": "standard",
    "exp": "express",
    "channel": "channel special",
}


def _channel_code(value: str) -> str:
    """Return the canonical channel code for value."""
    cleaned = value.strip().upper().replace(" ", "-")
    if len(cleaned) > MAX_CODE_LEN:
        cleaned = cleaned[:MAX_CODE_LEN]
    return f"CHA-{cleaned}"


def _channel_label(code: str, count: int) -> str:
    """Format a human label for a channel code."""
    suffix = "item" if count == 1 else "items"
    return f"{code}: {count} {suffix}"


def _is_valid_channel(code: str) -> bool:
    """Check that a channel code has the expected shape."""
    prefix, _, rest = code.partition("-")
    return prefix == "CHA" and bool(rest)


def _channel_lookup(key: str, default: str = "") -> str:
    """Look up a channel name in its table."""
    return _CHANNEL_TABLE.get(key, default)


def _clamp_channel_qty(qty: int, limit: int = MAX_LINES) -> int:
    """Clamp a quantity for channel handling."""
    return max(0, min(qty, limit))


def _channel_weight(sku: str) -> int:
    """Memoized weight of a sku in grams for channel shipments."""
    cached = _memo.get(sku)
    if cached is not None:
        return cached
    weight = sum(ord(ch) for ch in sku) % MAX_WEIGHT
    _memo[sku] = weight
    return weight


_PROMO_TABLE: dict[str, str] = {
    "std": "standard",
    "exp": "express",
    "promo": "promo special",
}


def _promo_code(value: str) -> str:
    """Return the canonical promo code for value."""
    cleaned = value.strip().upper().replace(" ", "-")
    if len(cleaned) > MAX_CODE_LEN:
        cleaned = cleaned[:MAX_CODE_LEN]
    return f"PRO-{cleaned}"


def _promo_label(code: str, count: int) -> str:
    """Format a human label for a promo code."""
    suffix = "item" if count == 1 else "items"
    return f"{code}: {count} {suffix}"


def _is_valid_promo(code: str) -> bool:
    """Check that a promo code has the expected shape."""
    prefix, _, rest = code.partition("-")
    return prefix == "PRO" and bool(rest)


def _promo_lookup(key: str, default: str = "") -> str:
    """Look up a promo name in its table."""
    return _PROMO_TABLE.get(key, default)


def _clamp_promo_qty(qty: int, limit: int = MAX_LINES) -> int:
    """Clamp a quantity for promo handling."""
    return max(0, min(qty, limit))


def _promo_weight(sku: str) -> int:
    """Memoized weight of a sku in grams for promo shipments."""
    cached = _memo.get(sku)
    if cached is not None:
        return cached
    weight = sum(ord(ch) for ch in sku) % MAX_WEIGHT
    _memo[sku] = weight
    return weight


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
        return f"{self.sku} x{self.qty}"


_CUSTOMER_TABLE: dict[str, str] = {
    "std": "standard",
    "exp": "express",
    "customer": "customer special",
}


def _customer_code(value: str) -> str:
    """Return the canonical customer code for value."""
    cleaned = value.strip().upper().replace(" ", "-")
    if len(cleaned) > MAX_CODE_LEN:
        cleaned = cleaned[:MAX_CODE_LEN]
    return f"CUS-{cleaned}"


def _customer_label(code: str, count: int) -> str:
    """Format a human label for a customer code."""
    suffix = "item" if count == 1 else "items"
    return f"{code}: {count} {suffix}"


def _is_valid_customer(code: str) -> bool:
    """Check that a customer code has the expected shape."""
    prefix, _, rest = code.partition("-")
    return prefix == "CUS" and bool(rest)


def _customer_lookup(key: str, default: str = "") -> str:
    """Look up a customer name in its table."""
    return _CUSTOMER_TABLE.get(key, default)


def _clamp_customer_qty(qty: int, limit: int = MAX_LINES) -> int:
    """Clamp a quantity for customer handling."""
    return max(0, min(qty, limit))


def _customer_weight(sku: str) -> int:
    """Memoized weight of a sku in grams for customer shipments."""
    cached = _memo.get(sku)
    if cached is not None:
        return cached
    weight = sum(ord(ch) for ch in sku) % MAX_WEIGHT
    _memo[sku] = weight
    return weight


_GIFT_TABLE: dict[str, str] = {
    "std": "standard",
    "exp": "express",
    "gift": "gift special",
}


def _gift_code(value: str) -> str:
    """Return the canonical gift code for value."""
    cleaned = value.strip().upper().replace(" ", "-")
    if len(cleaned) > MAX_CODE_LEN:
        cleaned = cleaned[:MAX_CODE_LEN]
    return f"GIF-{cleaned}"


def _gift_label(code: str, count: int) -> str:
    """Format a human label for a gift code."""
    suffix = "item" if count == 1 else "items"
    return f"{code}: {count} {suffix}"


def _is_valid_gift(code: str) -> bool:
    """Check that a gift code has the expected shape."""
    prefix, _, rest = code.partition("-")
    return prefix == "GIF" and bool(rest)


def _gift_lookup(key: str, default: str = "") -> str:
    """Look up a gift name in its table."""
    return _GIFT_TABLE.get(key, default)


def _clamp_gift_qty(qty: int, limit: int = MAX_LINES) -> int:
    """Clamp a quantity for gift handling."""
    return max(0, min(qty, limit))


def _gift_weight(sku: str) -> int:
    """Memoized weight of a sku in grams for gift shipments."""
    cached = _memo.get(sku)
    if cached is not None:
        return cached
    weight = sum(ord(ch) for ch in sku) % MAX_WEIGHT
    _memo[sku] = weight
    return weight


_REGION_TABLE: dict[str, str] = {
    "std": "standard",
    "exp": "express",
    "region": "region special",
}


def _region_code(value: str) -> str:
    """Return the canonical region code for value."""
    cleaned = value.strip().upper().replace(" ", "-")
    if len(cleaned) > MAX_CODE_LEN:
        cleaned = cleaned[:MAX_CODE_LEN]
    return f"REG-{cleaned}"


def _region_label(code: str, count: int) -> str:
    """Format a human label for a region code."""
    suffix = "item" if count == 1 else "items"
    return f"{code}: {count} {suffix}"


def _is_valid_region(code: str) -> bool:
    """Check that a region code has the expected shape."""
    prefix, _, rest = code.partition("-")
    return prefix == "REG" and bool(rest)


def _region_lookup(key: str, default: str = "") -> str:
    """Look up a region name in its table."""
    return _REGION_TABLE.get(key, default)


def _clamp_region_qty(qty: int, limit: int = MAX_LINES) -> int:
    """Clamp a quantity for region handling."""
    return max(0, min(qty, limit))


def _region_weight(sku: str) -> int:
    """Memoized weight of a sku in grams for region shipments."""
    cached = _memo.get(sku)
    if cached is not None:
        return cached
    weight = sum(ord(ch) for ch in sku) % MAX_WEIGHT
    _memo[sku] = weight
    return weight


_SUPPLIER_TABLE: dict[str, str] = {
    "std": "standard",
    "exp": "express",
    "supplier": "supplier special",
}


def _supplier_code(value: str) -> str:
    """Return the canonical supplier code for value."""
    cleaned = value.strip().upper().replace(" ", "-")
    if len(cleaned) > MAX_CODE_LEN:
        cleaned = cleaned[:MAX_CODE_LEN]
    return f"SUP-{cleaned}"


def _supplier_label(code: str, count: int) -> str:
    """Format a human label for a supplier code."""
    suffix = "item" if count == 1 else "items"
    return f"{code}: {count} {suffix}"


def _is_valid_supplier(code: str) -> bool:
    """Check that a supplier code has the expected shape."""
    prefix, _, rest = code.partition("-")
    return prefix == "SUP" and bool(rest)


def _supplier_lookup(key: str, default: str = "") -> str:
    """Look up a supplier name in its table."""
    return _SUPPLIER_TABLE.get(key, default)


def _clamp_supplier_qty(qty: int, limit: int = MAX_LINES) -> int:
    """Clamp a quantity for supplier handling."""
    return max(0, min(qty, limit))


def _supplier_weight(sku: str) -> int:
    """Memoized weight of a sku in grams for supplier shipments."""
    cached = _memo.get(sku)
    if cached is not None:
        return cached
    weight = sum(ord(ch) for ch in sku) % MAX_WEIGHT
    _memo[sku] = weight
    return weight


def _apply_discount(amount: Decimal, pct: Decimal) -> Decimal:
    """Reduce amount by pct percent (pct=10 means 10 percent off); not rounded."""
    return amount * (Decimal(100) - pct) / Decimal(100)


_BATCH_TABLE: dict[str, str] = {
    "std": "standard",
    "exp": "express",
    "batch": "batch special",
}


def _batch_code(value: str) -> str:
    """Return the canonical batch code for value."""
    cleaned = value.strip().upper().replace(" ", "-")
    if len(cleaned) > MAX_CODE_LEN:
        cleaned = cleaned[:MAX_CODE_LEN]
    return f"BAT-{cleaned}"


def _batch_label(code: str, count: int) -> str:
    """Format a human label for a batch code."""
    suffix = "item" if count == 1 else "items"
    return f"{code}: {count} {suffix}"


def _is_valid_batch(code: str) -> bool:
    """Check that a batch code has the expected shape."""
    prefix, _, rest = code.partition("-")
    return prefix == "BAT" and bool(rest)


def _batch_lookup(key: str, default: str = "") -> str:
    """Look up a batch name in its table."""
    return _BATCH_TABLE.get(key, default)


def _clamp_batch_qty(qty: int, limit: int = MAX_LINES) -> int:
    """Clamp a quantity for batch handling."""
    return max(0, min(qty, limit))


def _batch_weight(sku: str) -> int:
    """Memoized weight of a sku in grams for batch shipments."""
    cached = _memo.get(sku)
    if cached is not None:
        return cached
    weight = sum(ord(ch) for ch in sku) % MAX_WEIGHT
    _memo[sku] = weight
    return weight


_RETURN_TABLE: dict[str, str] = {
    "std": "standard",
    "exp": "express",
    "return": "return special",
}


def _return_code(value: str) -> str:
    """Return the canonical return code for value."""
    cleaned = value.strip().upper().replace(" ", "-")
    if len(cleaned) > MAX_CODE_LEN:
        cleaned = cleaned[:MAX_CODE_LEN]
    return f"RET-{cleaned}"


def _return_label(code: str, count: int) -> str:
    """Format a human label for a return code."""
    suffix = "item" if count == 1 else "items"
    return f"{code}: {count} {suffix}"


def _is_valid_return(code: str) -> bool:
    """Check that a return code has the expected shape."""
    prefix, _, rest = code.partition("-")
    return prefix == "RET" and bool(rest)


def _return_lookup(key: str, default: str = "") -> str:
    """Look up a return name in its table."""
    return _RETURN_TABLE.get(key, default)


def _clamp_return_qty(qty: int, limit: int = MAX_LINES) -> int:
    """Clamp a quantity for return handling."""
    return max(0, min(qty, limit))


def _return_weight(sku: str) -> int:
    """Memoized weight of a sku in grams for return shipments."""
    cached = _memo.get(sku)
    if cached is not None:
        return cached
    weight = sum(ord(ch) for ch in sku) % MAX_WEIGHT
    _memo[sku] = weight
    return weight


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

    def warehouse_total(self) -> Decimal:
        """Total of lines whose sku starts with WAR."""
        return sum(
            (line.total() for line in self.lines if line.sku.startswith("WAR")),
            Decimal(0),
        )

    def warehouse_count(self) -> int:
        """Number of units whose sku starts with WAR."""
        return sum(line.qty for line in self.lines if line.sku.startswith("WAR"))

    def has_warehouse(self) -> bool:
        """True when any line is a warehouse line."""
        return any(line.sku.startswith("WAR") for line in self.lines)

    def carrier_total(self) -> Decimal:
        """Total of lines whose sku starts with CAR."""
        return sum(
            (line.total() for line in self.lines if line.sku.startswith("CAR")),
            Decimal(0),
        )

    def carrier_count(self) -> int:
        """Number of units whose sku starts with CAR."""
        return sum(line.qty for line in self.lines if line.sku.startswith("CAR"))

    def has_carrier(self) -> bool:
        """True when any line is a carrier line."""
        return any(line.sku.startswith("CAR") for line in self.lines)

    def channel_total(self) -> Decimal:
        """Total of lines whose sku starts with CHA."""
        return sum(
            (line.total() for line in self.lines if line.sku.startswith("CHA")),
            Decimal(0),
        )

    def channel_count(self) -> int:
        """Number of units whose sku starts with CHA."""
        return sum(line.qty for line in self.lines if line.sku.startswith("CHA"))

    def has_channel(self) -> bool:
        """True when any line is a channel line."""
        return any(line.sku.startswith("CHA") for line in self.lines)

    def promo_total(self) -> Decimal:
        """Total of lines whose sku starts with PRO."""
        return sum(
            (line.total() for line in self.lines if line.sku.startswith("PRO")),
            Decimal(0),
        )

    def promo_count(self) -> int:
        """Number of units whose sku starts with PRO."""
        return sum(line.qty for line in self.lines if line.sku.startswith("PRO"))

    def has_promo(self) -> bool:
        """True when any line is a promo line."""
        return any(line.sku.startswith("PRO") for line in self.lines)

    def customer_total(self) -> Decimal:
        """Total of lines whose sku starts with CUS."""
        return sum(
            (line.total() for line in self.lines if line.sku.startswith("CUS")),
            Decimal(0),
        )

    def customer_count(self) -> int:
        """Number of units whose sku starts with CUS."""
        return sum(line.qty for line in self.lines if line.sku.startswith("CUS"))

    def has_customer(self) -> bool:
        """True when any line is a customer line."""
        return any(line.sku.startswith("CUS") for line in self.lines)

    def gift_total(self) -> Decimal:
        """Total of lines whose sku starts with GIF."""
        return sum(
            (line.total() for line in self.lines if line.sku.startswith("GIF")),
            Decimal(0),
        )

    def gift_count(self) -> int:
        """Number of units whose sku starts with GIF."""
        return sum(line.qty for line in self.lines if line.sku.startswith("GIF"))

    def has_gift(self) -> bool:
        """True when any line is a gift line."""
        return any(line.sku.startswith("GIF") for line in self.lines)

    def region_total(self) -> Decimal:
        """Total of lines whose sku starts with REG."""
        return sum(
            (line.total() for line in self.lines if line.sku.startswith("REG")),
            Decimal(0),
        )

    def region_count(self) -> int:
        """Number of units whose sku starts with REG."""
        return sum(line.qty for line in self.lines if line.sku.startswith("REG"))

    def has_region(self) -> bool:
        """True when any line is a region line."""
        return any(line.sku.startswith("REG") for line in self.lines)

    def supplier_total(self) -> Decimal:
        """Total of lines whose sku starts with SUP."""
        return sum(
            (line.total() for line in self.lines if line.sku.startswith("SUP")),
            Decimal(0),
        )

    def supplier_count(self) -> int:
        """Number of units whose sku starts with SUP."""
        return sum(line.qty for line in self.lines if line.sku.startswith("SUP"))

    def has_supplier(self) -> bool:
        """True when any line is a supplier line."""
        return any(line.sku.startswith("SUP") for line in self.lines)

    def batch_total(self) -> Decimal:
        """Total of lines whose sku starts with BAT."""
        return sum(
            (line.total() for line in self.lines if line.sku.startswith("BAT")),
            Decimal(0),
        )

    def batch_count(self) -> int:
        """Number of units whose sku starts with BAT."""
        return sum(line.qty for line in self.lines if line.sku.startswith("BAT"))

    def has_batch(self) -> bool:
        """True when any line is a batch line."""
        return any(line.sku.startswith("BAT") for line in self.lines)

    def return_total(self) -> Decimal:
        """Total of lines whose sku starts with RET."""
        return sum(
            (line.total() for line in self.lines if line.sku.startswith("RET")),
            Decimal(0),
        )

    def return_count(self) -> int:
        """Number of units whose sku starts with RET."""
        return sum(line.qty for line in self.lines if line.sku.startswith("RET"))

    def has_return(self) -> bool:
        """True when any line is a return line."""
        return any(line.sku.startswith("RET") for line in self.lines)

    def pickup_total(self) -> Decimal:
        """Total of lines whose sku starts with PIC."""
        return sum(
            (line.total() for line in self.lines if line.sku.startswith("PIC")),
            Decimal(0),
        )

    def pickup_count(self) -> int:
        """Number of units whose sku starts with PIC."""
        return sum(line.qty for line in self.lines if line.sku.startswith("PIC"))

    def has_pickup(self) -> bool:
        """True when any line is a pickup line."""
        return any(line.sku.startswith("PIC") for line in self.lines)

    def bundle_total(self) -> Decimal:
        """Total of lines whose sku starts with BUN."""
        return sum(
            (line.total() for line in self.lines if line.sku.startswith("BUN")),
            Decimal(0),
        )

    def bundle_count(self) -> int:
        """Number of units whose sku starts with BUN."""
        return sum(line.qty for line in self.lines if line.sku.startswith("BUN"))

    def has_bundle(self) -> bool:
        """True when any line is a bundle line."""
        return any(line.sku.startswith("BUN") for line in self.lines)


_PICKUP_TABLE: dict[str, str] = {
    "std": "standard",
    "exp": "express",
    "pickup": "pickup special",
}


def _pickup_code(value: str) -> str:
    """Return the canonical pickup code for value."""
    cleaned = value.strip().upper().replace(" ", "-")
    if len(cleaned) > MAX_CODE_LEN:
        cleaned = cleaned[:MAX_CODE_LEN]
    return f"PIC-{cleaned}"


def _pickup_label(code: str, count: int) -> str:
    """Format a human label for a pickup code."""
    suffix = "item" if count == 1 else "items"
    return f"{code}: {count} {suffix}"


def _is_valid_pickup(code: str) -> bool:
    """Check that a pickup code has the expected shape."""
    prefix, _, rest = code.partition("-")
    return prefix == "PIC" and bool(rest)


def _pickup_lookup(key: str, default: str = "") -> str:
    """Look up a pickup name in its table."""
    return _PICKUP_TABLE.get(key, default)


def _clamp_pickup_qty(qty: int, limit: int = MAX_LINES) -> int:
    """Clamp a quantity for pickup handling."""
    return max(0, min(qty, limit))


def _pickup_weight(sku: str) -> int:
    """Memoized weight of a sku in grams for pickup shipments."""
    cached = _memo.get(sku)
    if cached is not None:
        return cached
    weight = sum(ord(ch) for ch in sku) % MAX_WEIGHT
    _memo[sku] = weight
    return weight


_BUNDLE_TABLE: dict[str, str] = {
    "std": "standard",
    "exp": "express",
    "bundle": "bundle special",
}


def _bundle_code(value: str) -> str:
    """Return the canonical bundle code for value."""
    cleaned = value.strip().upper().replace(" ", "-")
    if len(cleaned) > MAX_CODE_LEN:
        cleaned = cleaned[:MAX_CODE_LEN]
    return f"BUN-{cleaned}"


def _bundle_label(code: str, count: int) -> str:
    """Format a human label for a bundle code."""
    suffix = "item" if count == 1 else "items"
    return f"{code}: {count} {suffix}"


def _is_valid_bundle(code: str) -> bool:
    """Check that a bundle code has the expected shape."""
    prefix, _, rest = code.partition("-")
    return prefix == "BUN" and bool(rest)


def _bundle_lookup(key: str, default: str = "") -> str:
    """Look up a bundle name in its table."""
    return _BUNDLE_TABLE.get(key, default)


def _clamp_bundle_qty(qty: int, limit: int = MAX_LINES) -> int:
    """Clamp a quantity for bundle handling."""
    return max(0, min(qty, limit))


def _bundle_weight(sku: str) -> int:
    """Memoized weight of a sku in grams for bundle shipments."""
    cached = _memo.get(sku)
    if cached is not None:
        return cached
    weight = sum(ord(ch) for ch in sku) % MAX_WEIGHT
    _memo[sku] = weight
    return weight


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

    def warehouse_total(self) -> Decimal:
        """Total of lines whose sku starts with WAR."""
        return sum(
            (line.total() for line in self._all_lines() if line.sku.startswith("WAR")),
            Decimal(0),
        )

    def warehouse_count(self) -> int:
        """Number of units whose sku starts with WAR."""
        return sum(line.qty for line in self._all_lines() if line.sku.startswith("WAR"))

    def has_warehouse(self) -> bool:
        """True when any line is a warehouse line."""
        return any(line.sku.startswith("WAR") for line in self._all_lines())

    def carrier_total(self) -> Decimal:
        """Total of lines whose sku starts with CAR."""
        return sum(
            (line.total() for line in self._all_lines() if line.sku.startswith("CAR")),
            Decimal(0),
        )

    def carrier_count(self) -> int:
        """Number of units whose sku starts with CAR."""
        return sum(line.qty for line in self._all_lines() if line.sku.startswith("CAR"))

    def has_carrier(self) -> bool:
        """True when any line is a carrier line."""
        return any(line.sku.startswith("CAR") for line in self._all_lines())

    def channel_total(self) -> Decimal:
        """Total of lines whose sku starts with CHA."""
        return sum(
            (line.total() for line in self._all_lines() if line.sku.startswith("CHA")),
            Decimal(0),
        )

    def channel_count(self) -> int:
        """Number of units whose sku starts with CHA."""
        return sum(line.qty for line in self._all_lines() if line.sku.startswith("CHA"))

    def has_channel(self) -> bool:
        """True when any line is a channel line."""
        return any(line.sku.startswith("CHA") for line in self._all_lines())

    def promo_total(self) -> Decimal:
        """Total of lines whose sku starts with PRO."""
        return sum(
            (line.total() for line in self._all_lines() if line.sku.startswith("PRO")),
            Decimal(0),
        )

    def promo_count(self) -> int:
        """Number of units whose sku starts with PRO."""
        return sum(line.qty for line in self._all_lines() if line.sku.startswith("PRO"))

    def has_promo(self) -> bool:
        """True when any line is a promo line."""
        return any(line.sku.startswith("PRO") for line in self._all_lines())

    def customer_total(self) -> Decimal:
        """Total of lines whose sku starts with CUS."""
        return sum(
            (line.total() for line in self._all_lines() if line.sku.startswith("CUS")),
            Decimal(0),
        )

    def customer_count(self) -> int:
        """Number of units whose sku starts with CUS."""
        return sum(line.qty for line in self._all_lines() if line.sku.startswith("CUS"))

    def has_customer(self) -> bool:
        """True when any line is a customer line."""
        return any(line.sku.startswith("CUS") for line in self._all_lines())

    def gift_total(self) -> Decimal:
        """Total of lines whose sku starts with GIF."""
        return sum(
            (line.total() for line in self._all_lines() if line.sku.startswith("GIF")),
            Decimal(0),
        )

    def gift_count(self) -> int:
        """Number of units whose sku starts with GIF."""
        return sum(line.qty for line in self._all_lines() if line.sku.startswith("GIF"))

    def has_gift(self) -> bool:
        """True when any line is a gift line."""
        return any(line.sku.startswith("GIF") for line in self._all_lines())

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

    def warehouse_total(self) -> Decimal:
        """Total of lines whose sku starts with WAR."""
        return sum(
            (line.total() for line in self.lines() if line.sku.startswith("WAR")),
            Decimal(0),
        )

    def warehouse_count(self) -> int:
        """Number of units whose sku starts with WAR."""
        return sum(line.qty for line in self.lines() if line.sku.startswith("WAR"))

    def has_warehouse(self) -> bool:
        """True when any line is a warehouse line."""
        return any(line.sku.startswith("WAR") for line in self.lines())

    def carrier_total(self) -> Decimal:
        """Total of lines whose sku starts with CAR."""
        return sum(
            (line.total() for line in self.lines() if line.sku.startswith("CAR")),
            Decimal(0),
        )

    def carrier_count(self) -> int:
        """Number of units whose sku starts with CAR."""
        return sum(line.qty for line in self.lines() if line.sku.startswith("CAR"))

    def has_carrier(self) -> bool:
        """True when any line is a carrier line."""
        return any(line.sku.startswith("CAR") for line in self.lines())

    def channel_total(self) -> Decimal:
        """Total of lines whose sku starts with CHA."""
        return sum(
            (line.total() for line in self.lines() if line.sku.startswith("CHA")),
            Decimal(0),
        )

    def channel_count(self) -> int:
        """Number of units whose sku starts with CHA."""
        return sum(line.qty for line in self.lines() if line.sku.startswith("CHA"))

    def has_channel(self) -> bool:
        """True when any line is a channel line."""
        return any(line.sku.startswith("CHA") for line in self.lines())

    def promo_total(self) -> Decimal:
        """Total of lines whose sku starts with PRO."""
        return sum(
            (line.total() for line in self.lines() if line.sku.startswith("PRO")),
            Decimal(0),
        )

    def promo_count(self) -> int:
        """Number of units whose sku starts with PRO."""
        return sum(line.qty for line in self.lines() if line.sku.startswith("PRO"))

    def has_promo(self) -> bool:
        """True when any line is a promo line."""
        return any(line.sku.startswith("PRO") for line in self.lines())

    def customer_total(self) -> Decimal:
        """Total of lines whose sku starts with CUS."""
        return sum(
            (line.total() for line in self.lines() if line.sku.startswith("CUS")),
            Decimal(0),
        )

    def customer_count(self) -> int:
        """Number of units whose sku starts with CUS."""
        return sum(line.qty for line in self.lines() if line.sku.startswith("CUS"))

    def has_customer(self) -> bool:
        """True when any line is a customer line."""
        return any(line.sku.startswith("CUS") for line in self.lines())

    def gift_total(self) -> Decimal:
        """Total of lines whose sku starts with GIF."""
        return sum(
            (line.total() for line in self.lines() if line.sku.startswith("GIF")),
            Decimal(0),
        )

    def gift_count(self) -> int:
        """Number of units whose sku starts with GIF."""
        return sum(line.qty for line in self.lines() if line.sku.startswith("GIF"))

    def has_gift(self) -> bool:
        """True when any line is a gift line."""
        return any(line.sku.startswith("GIF") for line in self.lines())

    def region_total(self) -> Decimal:
        """Total of lines whose sku starts with REG."""
        return sum(
            (line.total() for line in self.lines() if line.sku.startswith("REG")),
            Decimal(0),
        )

    def region_count(self) -> int:
        """Number of units whose sku starts with REG."""
        return sum(line.qty for line in self.lines() if line.sku.startswith("REG"))

    def has_region(self) -> bool:
        """True when any line is a region line."""
        return any(line.sku.startswith("REG") for line in self.lines())

    def supplier_total(self) -> Decimal:
        """Total of lines whose sku starts with SUP."""
        return sum(
            (line.total() for line in self.lines() if line.sku.startswith("SUP")),
            Decimal(0),
        )

    def supplier_count(self) -> int:
        """Number of units whose sku starts with SUP."""
        return sum(line.qty for line in self.lines() if line.sku.startswith("SUP"))

    def has_supplier(self) -> bool:
        """True when any line is a supplier line."""
        return any(line.sku.startswith("SUP") for line in self.lines())

    def batch_total(self) -> Decimal:
        """Total of lines whose sku starts with BAT."""
        return sum(
            (line.total() for line in self.lines() if line.sku.startswith("BAT")),
            Decimal(0),
        )

    def batch_count(self) -> int:
        """Number of units whose sku starts with BAT."""
        return sum(line.qty for line in self.lines() if line.sku.startswith("BAT"))

    def has_batch(self) -> bool:
        """True when any line is a batch line."""
        return any(line.sku.startswith("BAT") for line in self.lines())

    def return_total(self) -> Decimal:
        """Total of lines whose sku starts with RET."""
        return sum(
            (line.total() for line in self.lines() if line.sku.startswith("RET")),
            Decimal(0),
        )

    def return_count(self) -> int:
        """Number of units whose sku starts with RET."""
        return sum(line.qty for line in self.lines() if line.sku.startswith("RET"))

    def has_return(self) -> bool:
        """True when any line is a return line."""
        return any(line.sku.startswith("RET") for line in self.lines())

    def pickup_total(self) -> Decimal:
        """Total of lines whose sku starts with PIC."""
        return sum(
            (line.total() for line in self.lines() if line.sku.startswith("PIC")),
            Decimal(0),
        )

    def pickup_count(self) -> int:
        """Number of units whose sku starts with PIC."""
        return sum(line.qty for line in self.lines() if line.sku.startswith("PIC"))

    def has_pickup(self) -> bool:
        """True when any line is a pickup line."""
        return any(line.sku.startswith("PIC") for line in self.lines())

    def bundle_total(self) -> Decimal:
        """Total of lines whose sku starts with BUN."""
        return sum(
            (line.total() for line in self.lines() if line.sku.startswith("BUN")),
            Decimal(0),
        )

    def bundle_count(self) -> int:
        """Number of units whose sku starts with BUN."""
        return sum(line.qty for line in self.lines() if line.sku.startswith("BUN"))

    def has_bundle(self) -> bool:
        """True when any line is a bundle line."""
        return any(line.sku.startswith("BUN") for line in self.lines())


@dataclass
class OrderReport:
    """Aggregated numbers for a set of orders."""

    orders: list[Order] = field(default_factory=list)

    @property
    def lines(self) -> list[LineItem]:
        """All lines of all orders."""
        return [line for order in self.orders for line in order.lines]

    def warehouse_total(self) -> Decimal:
        """Total of lines whose sku starts with WAR."""
        return sum(
            (line.total() for line in self.lines if line.sku.startswith("WAR")),
            Decimal(0),
        )

    def warehouse_count(self) -> int:
        """Number of units whose sku starts with WAR."""
        return sum(line.qty for line in self.lines if line.sku.startswith("WAR"))

    def has_warehouse(self) -> bool:
        """True when any line is a warehouse line."""
        return any(line.sku.startswith("WAR") for line in self.lines)

    def carrier_total(self) -> Decimal:
        """Total of lines whose sku starts with CAR."""
        return sum(
            (line.total() for line in self.lines if line.sku.startswith("CAR")),
            Decimal(0),
        )

    def carrier_count(self) -> int:
        """Number of units whose sku starts with CAR."""
        return sum(line.qty for line in self.lines if line.sku.startswith("CAR"))

    def has_carrier(self) -> bool:
        """True when any line is a carrier line."""
        return any(line.sku.startswith("CAR") for line in self.lines)

    def channel_total(self) -> Decimal:
        """Total of lines whose sku starts with CHA."""
        return sum(
            (line.total() for line in self.lines if line.sku.startswith("CHA")),
            Decimal(0),
        )

    def channel_count(self) -> int:
        """Number of units whose sku starts with CHA."""
        return sum(line.qty for line in self.lines if line.sku.startswith("CHA"))

    def has_channel(self) -> bool:
        """True when any line is a channel line."""
        return any(line.sku.startswith("CHA") for line in self.lines)

    def promo_total(self) -> Decimal:
        """Total of lines whose sku starts with PRO."""
        return sum(
            (line.total() for line in self.lines if line.sku.startswith("PRO")),
            Decimal(0),
        )

    def promo_count(self) -> int:
        """Number of units whose sku starts with PRO."""
        return sum(line.qty for line in self.lines if line.sku.startswith("PRO"))

    def has_promo(self) -> bool:
        """True when any line is a promo line."""
        return any(line.sku.startswith("PRO") for line in self.lines)

    def customer_total(self) -> Decimal:
        """Total of lines whose sku starts with CUS."""
        return sum(
            (line.total() for line in self.lines if line.sku.startswith("CUS")),
            Decimal(0),
        )

    def customer_count(self) -> int:
        """Number of units whose sku starts with CUS."""
        return sum(line.qty for line in self.lines if line.sku.startswith("CUS"))

    def has_customer(self) -> bool:
        """True when any line is a customer line."""
        return any(line.sku.startswith("CUS") for line in self.lines)

    def gift_total(self) -> Decimal:
        """Total of lines whose sku starts with GIF."""
        return sum(
            (line.total() for line in self.lines if line.sku.startswith("GIF")),
            Decimal(0),
        )

    def gift_count(self) -> int:
        """Number of units whose sku starts with GIF."""
        return sum(line.qty for line in self.lines if line.sku.startswith("GIF"))

    def has_gift(self) -> bool:
        """True when any line is a gift line."""
        return any(line.sku.startswith("GIF") for line in self.lines)

    def region_total(self) -> Decimal:
        """Total of lines whose sku starts with REG."""
        return sum(
            (line.total() for line in self.lines if line.sku.startswith("REG")),
            Decimal(0),
        )

    def region_count(self) -> int:
        """Number of units whose sku starts with REG."""
        return sum(line.qty for line in self.lines if line.sku.startswith("REG"))

    def has_region(self) -> bool:
        """True when any line is a region line."""
        return any(line.sku.startswith("REG") for line in self.lines)

    def supplier_total(self) -> Decimal:
        """Total of lines whose sku starts with SUP."""
        return sum(
            (line.total() for line in self.lines if line.sku.startswith("SUP")),
            Decimal(0),
        )

    def supplier_count(self) -> int:
        """Number of units whose sku starts with SUP."""
        return sum(line.qty for line in self.lines if line.sku.startswith("SUP"))

    def has_supplier(self) -> bool:
        """True when any line is a supplier line."""
        return any(line.sku.startswith("SUP") for line in self.lines)

    def batch_total(self) -> Decimal:
        """Total of lines whose sku starts with BAT."""
        return sum(
            (line.total() for line in self.lines if line.sku.startswith("BAT")),
            Decimal(0),
        )

    def batch_count(self) -> int:
        """Number of units whose sku starts with BAT."""
        return sum(line.qty for line in self.lines if line.sku.startswith("BAT"))

    def has_batch(self) -> bool:
        """True when any line is a batch line."""
        return any(line.sku.startswith("BAT") for line in self.lines)

    def return_total(self) -> Decimal:
        """Total of lines whose sku starts with RET."""
        return sum(
            (line.total() for line in self.lines if line.sku.startswith("RET")),
            Decimal(0),
        )

    def return_count(self) -> int:
        """Number of units whose sku starts with RET."""
        return sum(line.qty for line in self.lines if line.sku.startswith("RET"))

    def has_return(self) -> bool:
        """True when any line is a return line."""
        return any(line.sku.startswith("RET") for line in self.lines)

    def pickup_total(self) -> Decimal:
        """Total of lines whose sku starts with PIC."""
        return sum(
            (line.total() for line in self.lines if line.sku.startswith("PIC")),
            Decimal(0),
        )

    def pickup_count(self) -> int:
        """Number of units whose sku starts with PIC."""
        return sum(line.qty for line in self.lines if line.sku.startswith("PIC"))

    def has_pickup(self) -> bool:
        """True when any line is a pickup line."""
        return any(line.sku.startswith("PIC") for line in self.lines)

    def bundle_total(self) -> Decimal:
        """Total of lines whose sku starts with BUN."""
        return sum(
            (line.total() for line in self.lines if line.sku.startswith("BUN")),
            Decimal(0),
        )

    def bundle_count(self) -> int:
        """Number of units whose sku starts with BUN."""
        return sum(line.qty for line in self.lines if line.sku.startswith("BUN"))

    def has_bundle(self) -> bool:
        """True when any line is a bundle line."""
        return any(line.sku.startswith("BUN") for line in self.lines)


_AUDIT_TABLE: dict[str, str] = {
    "std": "standard",
    "exp": "express",
    "audit": "audit special",
}


def _audit_code(value: str) -> str:
    """Return the canonical audit code for value."""
    cleaned = value.strip().upper().replace(" ", "-")
    if len(cleaned) > MAX_CODE_LEN:
        cleaned = cleaned[:MAX_CODE_LEN]
    return f"AUD-{cleaned}"


def _audit_label(code: str, count: int) -> str:
    """Format a human label for a audit code."""
    suffix = "item" if count == 1 else "items"
    return f"{code}: {count} {suffix}"


def _is_valid_audit(code: str) -> bool:
    """Check that a audit code has the expected shape."""
    prefix, _, rest = code.partition("-")
    return prefix == "AUD" and bool(rest)


def _audit_lookup(key: str, default: str = "") -> str:
    """Look up a audit name in its table."""
    return _AUDIT_TABLE.get(key, default)


def _clamp_audit_qty(qty: int, limit: int = MAX_LINES) -> int:
    """Clamp a quantity for audit handling."""
    return max(0, min(qty, limit))


def _audit_weight(sku: str) -> int:
    """Memoized weight of a sku in grams for audit shipments."""
    cached = _memo.get(sku)
    if cached is not None:
        return cached
    weight = sum(ord(ch) for ch in sku) % MAX_WEIGHT
    _memo[sku] = weight
    return weight


_LEDGER_TABLE: dict[str, str] = {
    "std": "standard",
    "exp": "express",
    "ledger": "ledger special",
}


def _ledger_code(value: str) -> str:
    """Return the canonical ledger code for value."""
    cleaned = value.strip().upper().replace(" ", "-")
    if len(cleaned) > MAX_CODE_LEN:
        cleaned = cleaned[:MAX_CODE_LEN]
    return f"LED-{cleaned}"


def _ledger_label(code: str, count: int) -> str:
    """Format a human label for a ledger code."""
    suffix = "item" if count == 1 else "items"
    return f"{code}: {count} {suffix}"


def _is_valid_ledger(code: str) -> bool:
    """Check that a ledger code has the expected shape."""
    prefix, _, rest = code.partition("-")
    return prefix == "LED" and bool(rest)


def _ledger_lookup(key: str, default: str = "") -> str:
    """Look up a ledger name in its table."""
    return _LEDGER_TABLE.get(key, default)


def _clamp_ledger_qty(qty: int, limit: int = MAX_LINES) -> int:
    """Clamp a quantity for ledger handling."""
    return max(0, min(qty, limit))


def _ledger_weight(sku: str) -> int:
    """Memoized weight of a sku in grams for ledger shipments."""
    cached = _memo.get(sku)
    if cached is not None:
        return cached
    weight = sum(ord(ch) for ch in sku) % MAX_WEIGHT
    _memo[sku] = weight
    return weight


_TAX_RATES: dict[str, Decimal] = {
    "US": Decimal("0.08"),
    "EU": Decimal("0.21"),
    "UK": Decimal("0.20"),
}


def _tax_for(region: str) -> Decimal:
    """Sales tax rate for region as a fraction (0.08 means 8 percent)."""
    try:
        return _TAX_RATES[region]
    except KeyError:
        msg = f"unknown region: {region}"
        raise ValueError(msg) from None


def cached_total(order_id: str, cache: Cache) -> Decimal | None:
    """Return the cached total of an order, if known."""
    total = cache.get(order_id)
    if total is None:
        return None
    return Decimal(total)
