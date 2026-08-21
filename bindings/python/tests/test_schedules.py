"""Schedule generation, stubs, and the sequence protocol."""

from datetime import date

import pytest

import fasti
from fasti import Schedule, calendars


def test_regular_backward_schedule():
    s = Schedule(date(2025, 1, 15), date(2027, 1, 15), "6M")
    assert s.dates == [
        date(2025, 1, 15),
        date(2025, 7, 15),
        date(2026, 1, 15),
        date(2026, 7, 15),
        date(2027, 1, 15),
    ]
    assert s.tenor == fasti.Period("6M")
    assert s.end_of_month is False


def test_no_calendar_means_no_adjustment():
    """2026-07-04 is a Saturday and stays one without a calendar."""
    s = Schedule(date(2026, 1, 4), date(2027, 1, 4), "6M")
    assert date(2026, 7, 4) in s.dates


def test_a_calendar_rolls_the_grid_onto_business_days():
    s = Schedule(date(2026, 1, 5), date(2027, 1, 5), "6M", calendars.US_SETTLEMENT)
    assert date(2026, 7, 6) in s.dates  # 2026-07-05 is a Sunday


def test_termination_defaults_to_unadjusted():
    s = Schedule(date(2025, 7, 6), date(2026, 7, 4), "6M", calendars.WEEKENDS_ONLY)
    assert s.dates[-1] == date(2026, 7, 4)  # a Saturday, left alone
    s = Schedule(
        date(2025, 7, 6), date(2026, 7, 4), "6M", calendars.WEEKENDS_ONLY,
        termination_convention="preceding",
    )
    assert s.dates[-1] == date(2026, 7, 3)


def test_forward_generation_puts_the_stub_at_the_back():
    s = Schedule(date(2025, 1, 15), date(2026, 2, 15), "6M", rule="forward")
    assert s.dates == [
        date(2025, 1, 15),
        date(2025, 7, 15),
        date(2026, 1, 15),
        date(2026, 2, 15),
    ]


def test_backward_generation_puts_the_stub_at_the_front():
    s = Schedule(date(2025, 1, 15), date(2026, 7, 15), "6M", rule="backward")
    assert s.dates[:2] == [date(2025, 1, 15), date(2025, 7, 15)]


def test_zero_rule_has_no_interior_dates():
    s = Schedule(date(2025, 1, 15), date(2030, 1, 15), "6M", rule="zero")
    assert len(s) == 2


def test_reference_periods_extend_a_stub():
    s = Schedule(date(2025, 3, 15), date(2026, 1, 15), "6M", rule="backward")
    coupons = s.periods()
    references = s.reference_periods()
    assert len(coupons) == len(references)
    # The front stub accrues against a full quasi-coupon period.
    assert references[0][0] < coupons[0][0]


def test_end_of_month_generation():
    s = Schedule(date(2025, 1, 31), date(2026, 1, 31), "3M", end_of_month=True)
    assert s.end_of_month is True
    assert s.dates == [
        date(2025, 1, 31),
        date(2025, 4, 30),
        date(2025, 7, 31),
        date(2025, 10, 31),
        date(2026, 1, 31),
    ]


def test_sequence_protocol():
    s = Schedule(date(2025, 1, 15), date(2027, 1, 15), "6M")
    assert len(s) == 5
    assert s[0] == date(2025, 1, 15)
    assert s[-1] == date(2027, 1, 15)
    assert s[1:3] == [date(2025, 7, 15), date(2026, 1, 15)]
    assert list(s) == s.dates
    assert [d for d in s][0] == s[0]
    with pytest.raises(IndexError):
        s[99]
    with pytest.raises(TypeError):
        s["first"]


def test_periods_are_consecutive_pairs():
    s = Schedule(date(2025, 1, 15), date(2026, 1, 15), "6M")
    assert s.periods() == [
        (date(2025, 1, 15), date(2025, 7, 15)),
        (date(2025, 7, 15), date(2026, 1, 15)),
    ]


def test_navigation_and_slicing_by_date():
    s = Schedule(date(2025, 1, 15), date(2027, 1, 15), "6M")
    assert s.next_date(date(2025, 7, 15)) == date(2026, 1, 15)
    assert s.previous_date(date(2025, 7, 15)) == date(2025, 1, 15)
    assert s.lower_bound(date(2025, 7, 15)) == date(2025, 7, 15)
    assert s.previous_date(date(2025, 1, 15)) is None
    assert s.after(date(2026, 1, 15)).dates[0] == date(2026, 1, 15)
    assert s.until(date(2026, 1, 15)).dates[-1] == date(2026, 1, 15)


def test_from_dates_wraps_a_term_sheet_schedule():
    s = Schedule.from_dates([date(2025, 1, 15), date(2025, 7, 15), date(2026, 1, 15)])
    assert len(s) == 3
    assert s.tenor is None
    assert s.end_of_month is None


def test_from_dates_refuses_dates_that_do_not_increase():
    with pytest.raises(fasti.FastiError, match="monotonic"):
        Schedule.from_dates([date(2025, 7, 15), date(2025, 1, 15)])


def test_effective_must_precede_termination():
    with pytest.raises(fasti.FastiError, match="strictly before"):
        Schedule(date(2027, 1, 15), date(2025, 1, 15), "6M")


def test_zero_tenor_is_refused():
    with pytest.raises(fasti.FastiError, match="tenor"):
        Schedule(date(2025, 1, 15), date(2027, 1, 15), "0D")


def test_schedules_compare_by_value():
    assert Schedule(date(2025, 1, 15), date(2026, 1, 15), "6M") == Schedule(
        date(2025, 1, 15), date(2026, 1, 15), "6M"
    )
    assert Schedule(date(2025, 1, 15), date(2026, 1, 15), "6M") != Schedule(
        date(2025, 1, 15), date(2026, 1, 15), "3M"
    )


def test_stub_dates():
    s = Schedule(date(2025, 1, 15), date(2027, 1, 15), "6M", first_date=date(2025, 4, 15))
    assert s.dates[1] == date(2025, 4, 15)
    with pytest.raises(fasti.FastiError, match="stub"):
        Schedule(date(2025, 1, 15), date(2027, 1, 15), "6M", first_date=date(2028, 4, 15))
