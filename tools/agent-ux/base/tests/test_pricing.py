from decimal import Decimal

from shop import http, pricing
from shop.cache import Cache


def test_prices():
    assert pricing.price_for(Decimal("2.50"), 10) == Decimal("22.50")
    assert pricing.price_for(Decimal("2.50"), 2) == Decimal("5.00")
    assert pricing.with_markup(Decimal("10.00")) == Decimal("11.50")
    assert pricing.shipping_cost(10) == Decimal("4.50")


def test_rates():
    assert pricing.load_rate("US") == Decimal("0.08")
    cache = Cache()
    assert pricing.cached_rate("EU", cache) == Decimal("0.21")
    assert dict(cache.items())[("rate", "EU")] == Decimal("0.21")
    p = pricing.Pricer()
    assert p.taxed(Decimal("100"), "UK") == Decimal("120.00")
    assert p.rate("UK") == Decimal("0.20")


def test_quote():
    out = pricing.submit_quote("a", 1, Decimal("2"))
    assert out["ok"] is True
    assert http.calls[-1][0] == "post"
