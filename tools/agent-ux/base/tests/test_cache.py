from shop.cache import Cache


def test_set_len_stats():
    c = Cache()
    c.set("a", 1)
    c.set("b", 2)
    assert len(c) == 2
    assert c.get_stats()["size"] == 2
    assert sorted(c.items()) == [("a", 1), ("b", 2)]
    c.clear()
    assert len(c) == 0
