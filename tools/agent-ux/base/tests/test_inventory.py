from decimal import Decimal

from shop.inventory import Inventory, is_in_stock, stock_level, store, total_value
from shop.models import Item


def test_stock():
    store.clear()
    assert stock_level("widget") == 5
    assert stock_level("widget") == 5
    assert is_in_stock("widget")
    assert not is_in_stock("gadget")
    assert stock_level("nothing", 7) == 7


def test_inventory():
    items = [Item("a", "Apple", 2, Decimal("1.50")), Item("b", "Bean", 1, Decimal("2"))]
    assert total_value(items) == Decimal("5.00")
    inv = Inventory(items)
    assert inv.find("b").name == "Bean"
    assert inv.find("zz") is None
    assert inv.summary().startswith("2 items")
    assert Inventory().summary() == "empty"
