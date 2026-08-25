"""US market calendars."""

from .._fasti import _builtin

#: Fed Bankwire: settlement with no Saturday-back shift.
FEDERAL_RESERVE = _builtin("us::FEDERAL_RESERVE")
#: Settlement plus Good Friday, with the post-1996 payrolls exception.
GOVERNMENT_BOND = _builtin("us::GOVERNMENT_BOND")
#: North American Energy Reliability Council: six holidays.
NERC = _builtin("us::NERC")
#: New York Stock Exchange, historic closings included.
NYSE = _builtin("us::NYSE")
#: The generic US settlement calendar.
SETTLEMENT = _builtin("us::SETTLEMENT")
#: SOFR fixings: government bond with Good Friday always observed.
SOFR = _builtin("us::SOFR")

__all__ = ["FEDERAL_RESERVE", "GOVERNMENT_BOND", "NERC", "NYSE", "SETTLEMENT", "SOFR"]
