"""Schedule generation, stubs, and the sequence protocol."""

import datetime

import pytest

import fasti
from fasti import Schedule, calendars


def test_regular_backward_schedule():
    s = Schedule("2025-01-15", "2027-01-15", "6M")
    assert s.dates == [
        datetime.date(2025, 1, 15),
        datetime.date(2025, 7, 15),
        datetime.date(2026, 1, 15),
        datetime.date(2026, 7, 15),
        datetime.date(2027, 1, 15),
    ]
    assert s.tenor == fasti.Period("6M")
    assert s.end_of_month is False


def test_no_calendar_means_no_adjustment():
    """2026-07-04 is a Saturday and stays one without a calendar."""
    s = Schedule("2026-01-04", "2027-01-04", "6M")
    assert datetime.date(2026, 7, 4) in s.dates


def test_a_calendar_rolls_the_grid_onto_business_days():
    s = Schedule("2026-01-05", "2027-01-05", "6M", calendars.US_SETTLEMENT)
    assert datetime.date(2026, 7, 6) in s.dates  # 2026-07-05 is a Sunday


def test_termination_defaults_to_unadjusted():
    s = Schedule("2025-07-06", "2026-07-04", "6M", calendars.WEEKENDS_ONLY)
    assert s.dates[-1] == datetime.date(2026, 7, 4)  # a Saturday, left alone
    s = Schedule(
        "2025-07-06", "2026-07-04", "6M", calendars.WEEKENDS_ONLY,
        termination_convention="preceding",
    )
    assert s.dates[-1] == datetime.date(2026, 7, 3)


def test_forward_generation_puts_the_stub_at_the_back():
    s = Schedule("2025-01-15", "2026-02-15", "6M", rule="forward")
    assert s.dates == [
        datetime.date(2025, 1, 15),
        datetime.date(2025, 7, 15),
        datetime.date(2026, 1, 15),
        datetime.date(2026, 2, 15),
    ]


def test_backward_generation_puts_the_stub_at_the_front():
    s = Schedule("2025-01-15", "2026-07-15", "6M", rule="backward")
    assert s.dates[:2] == [datetime.date(2025, 1, 15), datetime.date(2025, 7, 15)]


def test_zero_rule_has_no_interior_dates():
    s = Schedule("2025-01-15", "2030-01-15", "6M", rule="zero")
    assert len(s) == 2


def test_reference_periods_extend_a_stub():
    s = Schedule("2025-03-15", "2026-01-15", "6M", rule="backward")
    coupons = s.periods()
    references = s.reference_periods()
    assert len(coupons) == len(references)
    # The front stub accrues against a full quasi-coupon period.
    assert references[0][0] < coupons[0][0]


def test_end_of_month_generation():
    s = Schedule("2025-01-31", "2026-01-31", "3M", end_of_month=True)
    assert s.end_of_month is True
    assert s.dates == [
        datetime.date(2025, 1, 31),
        datetime.date(2025, 4, 30),
        datetime.date(2025, 7, 31),
        datetime.date(2025, 10, 31),
        datetime.date(2026, 1, 31),
    ]


def test_sequence_protocol():
    s = Schedule("2025-01-15", "2027-01-15", "6M")
    assert len(s) == 5
    assert s[0] == datetime.date(2025, 1, 15)
    assert s[-1] == datetime.date(2027, 1, 15)
    assert s[1:3] == [datetime.date(2025, 7, 15), datetime.date(2026, 1, 15)]
    assert list(s) == s.dates
    assert [d for d in s][0] == s[0]
    with pytest.raises(IndexError):
        s[99]
    with pytest.raises(TypeError):
        s["first"]


def test_periods_are_consecutive_pairs():
    s = Schedule("2025-01-15", "2026-01-15", "6M")
    assert s.periods() == [
        (datetime.date(2025, 1, 15), datetime.date(2025, 7, 15)),
        (datetime.date(2025, 7, 15), datetime.date(2026, 1, 15)),
    ]


def test_navigation_and_slicing_by_date():
    s = Schedule("2025-01-15", "2027-01-15", "6M")
    assert s.next_date("2025-07-15") == datetime.date(2026, 1, 15)
    assert s.previous_date("2025-07-15") == datetime.date(2025, 1, 15)
    assert s.lower_bound("2025-07-15") == datetime.date(2025, 7, 15)
    assert s.previous_date("2025-01-15") is None
    assert s.after("2026-01-15").dates[0] == datetime.date(2026, 1, 15)
    assert s.until("2026-01-15").dates[-1] == datetime.date(2026, 1, 15)


def test_from_dates_wraps_a_term_sheet_schedule():
    s = Schedule.from_dates(["2025-01-15", "2025-07-15", datetime.date(2026, 1, 15)])
    assert len(s) == 3
    assert s.tenor is None
    assert s.end_of_month is None


def test_from_dates_refuses_dates_that_do_not_increase():
    with pytest.raises(fasti.FastiError, match="monotonic"):
        Schedule.from_dates(["2025-07-15", "2025-01-15"])


def test_effective_must_precede_termination():
    with pytest.raises(fasti.FastiError, match="strictly before"):
        Schedule("2027-01-15", "2025-01-15", "6M")


def test_zero_tenor_is_refused():
    with pytest.raises(fasti.FastiError, match="tenor"):
        Schedule("2025-01-15", "2027-01-15", "0D")


def test_schedules_compare_by_value():
    assert Schedule("2025-01-15", "2026-01-15", "6M") == Schedule(
        "2025-01-15", "2026-01-15", "6M"
    )
    assert Schedule("2025-01-15", "2026-01-15", "6M") != Schedule(
        "2025-01-15", "2026-01-15", "3M"
    )


def test_stub_dates():
    s = Schedule("2025-01-15", "2027-01-15", "6M", first_date="2025-04-15")
    assert s.dates[1] == datetime.date(2025, 4, 15)
    with pytest.raises(fasti.FastiError, match="stub"):
        Schedule("2025-01-15", "2027-01-15", "6M", first_date="2028-04-15")
