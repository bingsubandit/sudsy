from __future__ import annotations

import datetime as dt

from freezegun import freeze_time

import humanize


@freeze_time("2026-06-15")
def test_naturalage() -> None:
    assert humanize.naturalage(dt.date(2000, 6, 15)) == "26 years"
    assert humanize.naturalage(dt.date(2000, 6, 16)) == "25 years"
    assert humanize.naturalage(dt.date(2025, 6, 15)) == "1 year"
    assert humanize.naturalage(dt.date(2026, 6, 15)) == "0 years"
