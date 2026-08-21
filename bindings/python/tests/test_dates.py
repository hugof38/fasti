"""The datetime boundary: what goes in, what comes out, and where it stops."""

import datetime
from fractions import Fraction

import pytest

import fasti
from fasti import calendars


def test_ordinal_epoch_matches_python():
    """The serial-to-ordinal offset the extension compiles in."""
    assert fasti.MIN_DATE == datetime.date(1901, 1, 1)
    assert fasti.MAX_DATE == datetime.date(2199, 12, 31)


def test_dates_come_back_as_datetime_dates():
    result = calendars.WEEKENDS_ONLY.next_business_day(datetime.date(2026, 7, 3))
    assert type(result) is datetime.date
    assert result == datetime.date(2026, 7, 6)


def test_accepts_date_datetime_and_iso_string():
    cal = calendars.US_SETTLEMENT
    day = datetime.date(2026, 7, 3)
    assert (
        cal.is_holiday(day)
        == cal.is_holiday(datetime.datetime(2026, 7, 3, 15, 30))
        == cal.is_holiday("2026-07-03")
        is True
    )


def test_datetime_time_component_is_dropped():
    cal = calendars.NULL
    assert cal.adjust(datetime.datetime(2026, 7, 3, 23, 59, 59)) == datetime.date(2026, 7, 3)


@pytest.mark.parametrize("value", [datetime.date(1900, 12, 31), datetime.date(2200, 1, 1)])
def test_dates_outside_the_supported_range_raise(value):
    with pytest.raises(fasti.FastiError, match="1901-01-01"):
        calendars.NULL.is_business_day(value)


def test_fasti_error_is_a_value_error():
    assert issubclass(fasti.FastiError, ValueError)
    with pytest.raises(ValueError):
        calendars.NULL.is_business_day(datetime.date(1800, 1, 1))


@pytest.mark.parametrize("value", ["2026-7-4", "04/07/2026", "not a date"])
def test_malformed_date_strings_raise(value):
    with pytest.raises(fasti.FastiError):
        calendars.NULL.is_business_day(value)


@pytest.mark.parametrize("value", [42, None, [2026, 7, 4], 1.5])
def test_non_dates_raise_type_error(value):
    with pytest.raises(TypeError, match="datetime.date"):
        calendars.NULL.is_business_day(value)


def test_year_fractions_are_exact_fractions():
    yf = fasti.year_fraction("2025-01-01", "2025-07-01", "ACT/360")
    assert isinstance(yf, Fraction)
    assert yf == Fraction(181, 360)
    assert float(yf) == pytest.approx(181 / 360)


def test_easter_western_and_orthodox():
    assert fasti.easter_sunday(2024) == datetime.date(2024, 3, 31)
    assert fasti.easter_monday(2024) == datetime.date(2024, 4, 1)
    assert fasti.easter_sunday(2024, method="orthodox") == datetime.date(2024, 5, 5)
    assert fasti.easter_sunday(2025) == fasti.easter_sunday(2025, method="orthodox")


def test_easter_year_outside_the_tables_raises():
    with pytest.raises(fasti.FastiError):
        fasti.easter_sunday(1899)
