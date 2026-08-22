"""UK market calendars."""

from .._fasti import _builtin

#: The UK settlement calendar, with Commonwealth substitute days.
SETTLEMENT = _builtin("uk::SETTLEMENT")

__all__ = ["SETTLEMENT"]
