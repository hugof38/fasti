"""Dates, calendars, business-day conventions and day-count fractions.

``fasti`` answers the questions financial code asks of a calendar — *is
the market open?*, *when is the next coupon?*, *how much has accrued?* —
in a Rust extension module that speaks Python's own types.

Dates in are :class:`datetime.date` — or a :class:`datetime.datetime`,
whose time component is dropped — and dates out are always
:class:`datetime.date`. Nothing else is accepted; a date-shaped string is
a string, and :meth:`datetime.date.fromisoformat` turns it into a date.
Year fractions come back as :class:`fractions.Fraction`, because that is
what they are: the library computes them as reduced integer rationals and
never touches a float.

>>> import fasti
>>> from datetime import date
>>> nyse = fasti.calendars.US_NYSE
>>> nyse.is_business_day(date(2026, 7, 3))
False
>>> nyse.next_business_day(date(2026, 7, 3))
datetime.date(2026, 7, 6)
>>> fasti.year_fraction(date(2025, 1, 1), date(2025, 7, 1), "ACT/360")
Fraction(181, 360)
"""

from __future__ import annotations

from . import calendars
from ._fasti import (
    MAX_DATE,
    MIN_DATE,
    BusinessDayConvention,
    Calendar,
    DateGenerationRule,
    DayCount,
    FastiError,
    Frequency,
    Period,
    Rule,
    Schedule,
    Weekday,
    WeekendShift,
    __version__,
    day_count,
    easter_monday,
    easter_sunday,
    year_fraction,
)

__all__ = [
    "MAX_DATE",
    "MIN_DATE",
    "BusinessDayConvention",
    "Calendar",
    "DateGenerationRule",
    "DayCount",
    "FastiError",
    "Frequency",
    "Period",
    "Rule",
    "Schedule",
    "Weekday",
    "WeekendShift",
    "__version__",
    "calendars",
    "day_count",
    "easter_monday",
    "easter_sunday",
    "year_fraction",
]
