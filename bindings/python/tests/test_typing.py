"""The stubs, checked the way a user's type checker checks them.

A date position that says `datetime.date` is what lets mypy reject a
string before the code runs, so it is worth a test of its own: the
runtime `TypeError` proves the boundary holds, and this proves the
boundary is *visible*.
"""

import re

import pytest

mypy_api = pytest.importorskip("mypy.api", reason="mypy is a development dependency")

#: Code that must type-check clean.
ACCEPTED = """
from datetime import date, datetime

import fasti

day: date = date(2026, 7, 3)
converted: date = datetime(2026, 7, 3, 9, 30).date()

closed: bool = fasti.calendars.US_NYSE.is_holiday(day)
also_closed: bool = fasti.calendars.US_NYSE.is_holiday(converted)
rolled: date = fasti.calendars.US_NYSE.adjust(day, "modified_following")
schedule = fasti.Schedule(day, date(2030, 1, 15), "6M", fasti.calendars.US_SETTLEMENT)
first: date = schedule[0]
window: list[date] = schedule[0:2]
custom = fasti.Calendar.custom("x", holidays=[day], rules=[fasti.Rule.one_off(day)])
counter = fasti.DayCount("30E/360 ISDA", termination=date(2030, 1, 15))
"""

#: Each line is a date position given something that is not a date.
REJECTED = [
    'fasti.calendars.US_NYSE.is_holiday("2026-07-03")',
    "fasti.calendars.US_NYSE.is_holiday(20260703)",
    'fasti.Schedule("2025-01-15", date(2030, 1, 15), "6M")',
    'fasti.Calendar.custom("x", holidays=["2026-08-14"])',
    'fasti.DayCount("30E/360 ISDA", termination="2030-01-15")',
    'fasti.Rule.one_off("2026-08-14")',
    'fasti.year_fraction("2025-01-01", date(2025, 7, 1), "ACT/360")',
]


def _check(tmp_path, source, name):
    source_file = tmp_path / name
    source_file.write_text(source)
    out, _err, status = mypy_api.run(
        [
            "--strict",
            "--no-error-summary",
            "--cache-dir",
            str(tmp_path / ".mypy_cache"),
            str(source_file),
        ]
    )
    return out, status


def test_dates_and_datetimes_type_check(tmp_path):
    out, status = _check(tmp_path, ACCEPTED, "accepted.py")
    assert status == 0, out


#: The one rule the stubs cannot state. `datetime.datetime` subclasses
#: `datetime.date`, so a checker sees a valid argument where the runtime
#: raises and asks for `.date()`. This is a tripwire: if it ever starts
#: failing, the type system grew a way to say it and the docs saying
#: "a checker cannot help here" are due an update.
UNCATCHABLE = """
from datetime import datetime

import fasti

fasti.calendars.US_NYSE.is_holiday(datetime(2026, 7, 3, 9, 30))
"""


def test_a_datetime_is_the_one_thing_the_stubs_cannot_refuse(tmp_path):
    _, status = _check(tmp_path, UNCATCHABLE, "uncatchable.py")
    assert status == 0, "the stubs can now express date-but-not-datetime; update the docs"


def test_every_date_position_rejects_a_non_date(tmp_path):
    header = "from datetime import date\n\nimport fasti\n\n"
    source = header + "\n".join(REJECTED) + "\n"
    out, status = _check(tmp_path, source, "rejected.py")
    assert status != 0

    first_call = header.count("\n") + 1
    # `path:line: error: …`, where the path may itself contain a colon
    # after a Windows drive letter.
    reported = re.compile(r":(\d+):(?:\d+:)? error:")
    flagged = {int(m.group(1)) for m in map(reported.search, out.splitlines()) if m}
    expected = set(range(first_call, first_call + len(REJECTED)))
    assert flagged == expected, out
    # The message names the type a caller has to produce.
    assert 'expected "date"' in out
