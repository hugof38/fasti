"""Period parsing, arithmetic, and the frequency vocabulary."""

import datetime

import pytest

import fasti
from fasti import Frequency, Period


@pytest.mark.parametrize(
    ("text", "length", "unit"),
    [
        ("6M", 6, "months"),
        ("6m", 6, "months"),
        ("6 months", 6, "months"),
        ("1Y", 1, "years"),
        ("2 weeks", 2, "weeks"),
        ("-3D", -3, "days"),
        ("+3d", 3, "days"),
        ("0D", 0, "days"),
        ("quarterly", 3, "months"),
        ("semiannual", 6, "months"),
        ("weekly", 1, "weeks"),
    ],
)
def test_period_spellings(text, length, unit):
    p = Period(text)
    assert (p.length, p.unit) == (length, unit)


def test_keyword_construction_matches_string_construction():
    assert Period(months=6) == Period("6M") == Period.months(6)
    assert Period(days=7) == Period.days(7)
    assert Period(weeks=2) == Period.weeks(2)
    assert Period(years=1) == Period.years(1)


def test_period_from_timedelta():
    assert Period(datetime.timedelta(days=10)) == Period.days(10)


def test_timedelta_with_a_time_component_is_refused():
    with pytest.raises(fasti.FastiError, match="whole number of days"):
        Period(datetime.timedelta(days=1, hours=6))


def test_period_needs_exactly_one_source():
    with pytest.raises(fasti.FastiError):
        Period()
    with pytest.raises(fasti.FastiError):
        Period("6M", months=6)
    with pytest.raises(fasti.FastiError):
        Period(months=6, years=1)


@pytest.mark.parametrize("text", ["", "M", "6", "6 fortnights", "banana"])
def test_unparseable_periods_raise(text):
    with pytest.raises(fasti.FastiError):
        Period(text)


def test_arithmetic_and_normalization():
    assert -Period("3M") == Period("-3M")
    assert Period("3M") * 4 == Period("12M")
    assert 4 * Period("3M") == Period("12M")
    assert Period("12M").normalized() == Period("1Y")
    assert Period("14D").normalized() == Period("2W")
    assert Period("5M").normalized() == Period("5M")


def test_zero_and_repr():
    assert Period("0M").is_zero
    assert repr(Period("6M")) == "Period('6M')"
    assert str(Period("6M")) == "6M"


def test_periods_are_hashable_values():
    assert len({Period("6M"), Period(months=6), Period("1Y")}) == 2


def test_frequency_round_trip():
    assert Period("6M").frequency == Frequency.SEMIANNUAL
    assert Period("5M").frequency is None
    assert Frequency.QUARTERLY.per_year == 4
    assert Frequency.QUARTERLY.period == Period("3M")
    assert Frequency.parse("quarterly") == Frequency.QUARTERLY
    assert Frequency.parse(Period("3M")) == Frequency.QUARTERLY


def test_periods_are_accepted_wherever_a_tenor_is_wanted():
    cal = fasti.calendars.NULL
    assert cal.advance("2026-01-31", Period("1M")) == datetime.date(2026, 2, 28)
    assert cal.advance("2026-01-31", "1M") == datetime.date(2026, 2, 28)
    assert cal.advance("2026-01-31", Frequency.MONTHLY) == datetime.date(2026, 2, 28)
