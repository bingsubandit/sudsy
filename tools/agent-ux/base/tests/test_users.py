from shop import users
from shop.cache import Cache
from shop.models import User, get_user


def test_profile():
    assert users.load_profile(1)["name"] == "Ada"


def test_warm_and_display():
    got = users.warm_users([1, 2, 99])
    assert got[0].name == "Ada" and got[2] is None
    cache = Cache()
    cache.set(3, User(3, "Cleo"))
    assert users.display_name(3, cache) == "Cleo"
    assert users.display_name(2, Cache()) == "bob"
    assert users.display_name(99, Cache()) == "unknown"


def test_summary():
    u = get_user(1)
    assert users.user_summary(u) == "Ada, 1"
    assert users.user_summary(u, True) == "Ada, 1, ada@example.com"
    assert users.email_domain(get_user(4)) == ""
    assert users.email_domain(u) == "example.com"
