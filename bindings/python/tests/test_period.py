"""Periods, and the date arithmetic they carry."""

import datetime

import pytest

from fasti import FastiError, Frequency, Period


def test_a_period_is_its_length_and_its_unit() -> None:
    assert Period.months(3).length() == 3
    assert Period.months(3).unit == "months"
    assert Period.days(-7).length() == -7
    assert Period.ZERO.is_zero() and Period.days(0) == Period.ZERO
    assert not Period.months(1).is_zero()


def test_the_unit_is_part_of_the_value() -> None:
    # Three months is not ninety days, and only a calendar could say.
    assert Period.months(3) != Period.days(90)
    assert Period.weeks(1) != Period.days(7)
    assert len({Period.months(12), Period.years(1)}) == 2


def test_normalizing_moves_to_the_largest_exact_unit() -> None:
    assert Period.months(12).normalized() == Period.years(1)
    assert Period.days(14).normalized() == Period.weeks(2)
    assert Period.months(5).normalized() == Period.months(5)


def test_frequencies_round_trip_through_their_canonical_period() -> None:
    for frequency in [Frequency.ANNUAL, Frequency.SEMIANNUAL, Frequency.QUARTERLY,
                      Frequency.MONTHLY, Frequency.WEEKLY, Frequency.DAILY]:
        assert Period.from_frequency(frequency).frequency() == frequency
    assert Period.from_frequency("annual") == Period.months(12)
    with pytest.raises(FastiError, match="does not correspond to a canonical frequency"):
        Period.months(5).frequency()


def test_scaling_and_negation_are_checked_where_the_crate_wraps() -> None:
    assert Period.months(3) * 4 == Period.months(12)
    assert 4 * Period.months(3) == Period.months(12)
    assert -Period.months(3) == Period.months(-3)
    with pytest.raises(FastiError, match="out of range"):
        Period.days(2**30) * 4


def test_a_date_steps_by_a_period() -> None:
    assert datetime.date(2026, 1, 15) + Period.months(6) == datetime.date(2026, 7, 15)
    assert datetime.date(2026, 7, 15) - Period.months(6) == datetime.date(2026, 1, 15)
    assert datetime.date(2026, 1, 15) + Period.weeks(1) == datetime.date(2026, 1, 22)
    assert datetime.date(2026, 1, 15) + (-Period.months(1)) == datetime.date(2025, 12, 15)


def test_stepping_a_date_does_not_snap_to_month_ends() -> None:
    # `date + period` is the crate's Add, which clamps and never snaps;
    # end-of-month preservation is Calendar.advance's business.
    assert datetime.date(2026, 1, 31) + Period.months(1) == datetime.date(2026, 2, 28)
    assert datetime.date(2026, 2, 28) + Period.months(1) == datetime.date(2026, 3, 28)


def test_stepping_out_of_the_supported_range_is_refused() -> None:
    with pytest.raises(FastiError, match="date arithmetic result out of range"):
        datetime.date(2199, 12, 31) + Period.years(1)
    with pytest.raises(FastiError, match="date arithmetic result out of range"):
        datetime.date(1901, 1, 1) - Period.days(1)


def test_a_period_only_steps_dates() -> None:
    with pytest.raises(TypeError):
        _ = "2026-01-15" + Period.days(1)  # type: ignore[operator]
    with pytest.raises(TypeError):
        _ = 1 + Period.days(1)  # type: ignore[operator]
    # A checker cannot catch this one: datetime subclasses date. The
    # runtime refuses it, as it refuses one anywhere else.
    with pytest.raises(TypeError, match="not a datetime.datetime"):
        _ = datetime.datetime(2026, 1, 15, 12) + Period.days(1)
