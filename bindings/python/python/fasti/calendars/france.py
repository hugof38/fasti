"""France market calendars."""

from .._fasti import _builtin

#: Euronext Paris.
EXCHANGE = _builtin("france::EXCHANGE")
#: The France settlement calendar.
SETTLEMENT = _builtin("france::SETTLEMENT")

__all__ = ["EXCHANGE", "SETTLEMENT"]
