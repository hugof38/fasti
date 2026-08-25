"""Built-in calendars, under the names they have in the crate.

Each is a `pub const Calendar<'static>` there, so it holds rules that
are bare function pointers; a built-in travels through pickle as its
name in this registry rather than as its contents.
"""

from .._fasti import _builtin
from . import france, uk, us

#: No weekend, no holidays — every day is a business day.
NULL_CALENDAR = _builtin("NULL_CALENDAR")
#: The TARGET (euro-area) settlement calendar.
TARGET = _builtin("TARGET")
#: Saturday/Sunday weekend, no holidays.
WEEKENDS_ONLY = _builtin("WEEKENDS_ONLY")

__all__ = ["NULL_CALENDAR", "TARGET", "WEEKENDS_ONLY", "france", "uk", "us"]
