from decimal import Decimal

from shop.cache import Cache
from shop.orders import LineItem, Order, OrderBook, cached_total


def make_book():
    book = OrderBook()
    o1 = Order(
        "o1",
        [LineItem("WAR-1", 2, Decimal("10.01")), LineItem("X", 1, Decimal("5.005"))],
    )
    o2 = Order("o2", [LineItem("Y", 3, Decimal("99.99"))])
    book.add(o1)
    book.add(o2)
    return book


def test_totals():
    book = make_book()
    assert book.orders[0].total() == Decimal("25.025")
    assert book.subtotal() == Decimal("324.995")
    assert book.rounded_subtotal() == Decimal("325.00")
    assert book.find("o2").unit_count() == 3
    assert book.find("zz") is None
    assert book.order_ids() == ["o1", "o2"]
    assert book.largest().order_id == "o2"


def test_discount():
    book = make_book()
    book.discount_pct = Decimal("10")
    assert book.discounted_total() == Decimal("292.50")


def test_cached_total():
    cache = Cache()
    cache.set("o1", "12.5")
    assert cached_total("o1", cache) == Decimal("12.5")
    assert cached_total("zz", cache) is None
