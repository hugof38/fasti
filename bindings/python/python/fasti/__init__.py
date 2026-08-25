"""Dates, calendars, business-day conventions and day-count fractions.

Python bindings for the `fasti <https://github.com/hugof38/fasti>`_ Rust
crate. Every name here maps to one in the crate, with the same method
names, the same semantics and the same argument order.

Dates cross the boundary as :class:`datetime.date` and nothing else, and
year fractions come back as :class:`fractions.Fraction` — the crate is
float-free, and that survives the boundary.
"""

from ._fasti import (
    __version__ as __version__,
    BusinessDayConvention,
    Calendar,
    DateGenerationRule,
    DayCount,
    EasterMethod,
    FastiError,
    Frequency,
    Generation,
    Period,
    Rule,
    Schedule,
    Weekday,
    WeekendShift,
    easter_monday,
    easter_sunday,
)
from . import calendars

__all__ = [
    "BusinessDayConvention",
    "Calendar",
    "DateGenerationRule",
    "DayCount",
    "EasterMethod",
    "FastiError",
    "Frequency",
    "Generation",
    "Period",
    "Rule",
    "Schedule",
    "Weekday",
    "WeekendShift",
    "calendars",
    "easter_monday",
    "easter_sunday",
]
