"""The built-in market calendars, one module attribute each.

Every name here is also reachable through :func:`load`, which is what
``Calendar(name)`` does — the module attributes exist so that editors and
type checkers can see them.

>>> from datetime import date
>>> from fasti import calendars
>>> calendars.US_SETTLEMENT.is_holiday(date(2026, 7, 3))   # Jul 4 is a Saturday
True
"""

from __future__ import annotations

from ._fasti import Calendar

#: Eurozone TARGET2 settlement calendar.
TARGET = Calendar("TARGET")

#: Generic US settlement calendar.
US_SETTLEMENT = Calendar("US.SETTLEMENT")
#: New York Stock Exchange trading calendar.
US_NYSE = Calendar("US.NYSE")
#: US government bond market (SIFMA) calendar.
US_GOVERNMENT_BOND = Calendar("US.GOVERNMENT_BOND")
#: Federal Reserve Bankwire calendar.
US_FEDERAL_RESERVE = Calendar("US.FEDERAL_RESERVE")
#: SOFR publication calendar.
US_SOFR = Calendar("US.SOFR")
#: NERC off-peak energy calendar.
US_NERC = Calendar("US.NERC")

#: UK settlement calendar.
UK_SETTLEMENT = Calendar("UK.SETTLEMENT")

#: French settlement calendar.
FRANCE_SETTLEMENT = Calendar("FRANCE.SETTLEMENT")
#: Euronext Paris trading calendar.
FRANCE_EXCHANGE = Calendar("FRANCE.EXCHANGE")

#: Saturday/Sunday weekend, no holidays.
WEEKENDS_ONLY = Calendar("WEEKENDS_ONLY")
#: No weekend and no holidays: every day is a business day.
NULL = Calendar("NULL")


def load(name: str) -> Calendar:
    """Load a built-in calendar by name, ignoring case and punctuation.

    ``"US.SETTLEMENT"``, ``"us settlement"`` and ``"us_settlement"`` all
    name the same calendar; ``"nyse"`` and ``"uk"`` are accepted short
    forms.
    """
    return Calendar(name)


def names() -> list[str]:
    """The canonical name of every built-in calendar."""
    return Calendar.names()


__all__ = [
    "FRANCE_EXCHANGE",
    "FRANCE_SETTLEMENT",
    "NULL",
    "TARGET",
    "UK_SETTLEMENT",
    "US_FEDERAL_RESERVE",
    "US_GOVERNMENT_BOND",
    "US_NERC",
    "US_NYSE",
    "US_SETTLEMENT",
    "US_SOFR",
    "WEEKENDS_ONLY",
    "load",
    "names",
]
