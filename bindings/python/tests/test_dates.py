"""The datetime boundary: what goes in, what comes out, and where it stops."""

from datetime import date, datetime
from fractions import Fraction

import pytest

import fasti
from fasti import calendars


def test_ordinal_epoch_matches_python():
    """The serial-to-ordinal offset the extension compiles in."""
    assert fasti.MIN_DATE == date(1901, 1, 1)
    assert fasti.MAX_DATE == date(2199, 12, 31)


def test_dates_come_back_as_datetime_dates():
    result = calendars.WEEKENDS_ONLY.next_business_day(date(2026, 7, 3))
    assert type(result) is date
    assert result == date(2026, 7, 6)


def test_a_datetime_is_a_date():
    """`datetime.datetime` derives from `date`, so it is accepted."""
    cal = calendars.US_SETTLEMENT
    assert cal.is_holiday(date(2026, 7, 3)) is cal.is_holiday(datetime(2026, 7, 3, 15, 30)) is True


def test_datetime_time_component_is_dropped():
    cal = calendars.NULL
    assert cal.adjust(datetime(2026, 7, 3, 23, 59, 59)) == date(2026, 7, 3)


@pytest.mark.parametrize("value", [date(1900, 12, 31), date(2200, 1, 1)])
def test_dates_outside_the_supported_range_raise(value):
    with pytest.raises(fasti.FastiError, match="1901-01-01"):
        calendars.NULL.is_business_day(value)


def test_fasti_error_is_a_value_error():
    assert issubclass(fasti.FastiError, ValueError)
    with pytest.raises(ValueError):
        calendars.NULL.is_business_day(date(1800, 1, 1))


@pytest.mark.parametrize("value", ["2026-07-04", "2026-7-4", "04/07/2026", "not a date"])
def test_date_shaped_strings_are_not_dates(value):
    """Parsing is datetime's job, and the error says so."""
    with pytest.raises(TypeError, match="fromisoformat"):
        calendars.NULL.is_business_day(value)


def test_the_rejection_message_quotes_what_was_passed():
    with pytest.raises(TypeError, match="'2026-07-04'"):
        calendars.NULL.is_business_day("2026-07-04")


def test_a_parsed_string_is_accepted():
    assert calendars.NULL.is_business_day(date.fromisoformat("2026-07-04"))


@pytest.mark.parametrize("value", [42, None, [2026, 7, 4], 1.5])
def test_non_dates_raise_type_error(value):
    with pytest.raises(TypeError, match="datetime.date"):
        calendars.NULL.is_business_day(value)


def test_dates_are_wanted_everywhere_a_date_is_wanted():
    """Every entry point that takes a date takes a date, and only one."""
    day = date(2026, 7, 4)
    assert fasti.Rule.one_off(day).is_holiday(day)
    assert fasti.Calendar.custom("x", holidays=[day]).is_holiday(day)
    assert fasti.Schedule.from_dates([day, date(2026, 8, 4)])[0] == day
    assert fasti.DayCount("30E/360 ISDA", termination=day).name
    for bad in ("2026-07-04",):
        with pytest.raises(TypeError):
            fasti.Rule.one_off(bad)
        with pytest.raises(TypeError):
            fasti.Calendar.custom("x", holidays=[bad])
        with pytest.raises(TypeError):
            fasti.Schedule.from_dates([bad, day])
        with pytest.raises(TypeError):
            fasti.DayCount("30E/360 ISDA", termination=bad)


def test_year_fractions_are_exact_fractions():
    yf = fasti.year_fraction(date(2025, 1, 1), date(2025, 7, 1), "ACT/360")
    assert isinstance(yf, Fraction)
    assert yf == Fraction(181, 360)
    assert float(yf) == pytest.approx(181 / 360)


def test_easter_western_and_orthodox():
    assert fasti.easter_sunday(2024) == date(2024, 3, 31)
    assert fasti.easter_monday(2024) == date(2024, 4, 1)
    assert fasti.easter_sunday(2024, method="orthodox") == date(2024, 5, 5)
    assert fasti.easter_sunday(2025) == fasti.easter_sunday(2025, method="orthodox")


def test_easter_year_outside_the_tables_raises():
    with pytest.raises(fasti.FastiError):
        fasti.easter_sunday(1899)
