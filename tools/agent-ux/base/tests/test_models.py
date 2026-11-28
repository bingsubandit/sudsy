from shop.models import get_all_users, get_user


def test_users():
    assert get_user(1).name == "Ada"
    assert get_user(42) is None
    assert len(get_all_users()) == 4
