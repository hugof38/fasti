"""Schedules: generation, stubs, the reference grid, and slicing."""

import datetime

import pytest

from fasti import DateGenerationRule, FastiError, Frequency, Period, Schedule
from fasti.calendars import WEEKENDS_ONLY, us


def days(*iso: str) -> list[datetime.date]:
    return [datetime.date.fromisoformat(text) for text in iso]


def unadjusted(effective: str, termination: str, tenor: object, **kwargs: object) -> Schedule:
    return Schedule(
        datetime.date.fromisoformat(effective),
        datetime.date.fromisoformat(termination),
        tenor,  # type: ignore[arg-type]
        WEEKENDS_ONLY,
        convention="unadjusted",
        **kwargs,  # type: ignore[arg-type]
    )


def ranges(schedule: Schedule) -> list[tuple[datetime.date, datetime.date]]:
    return [(p.start, p.end) for p in schedule.periods()]


def refs(schedule: Schedule) -> list[tuple[datetime.date, datetime.date]]:
    return [p.reference for p in schedule.periods()]


def test_a_regular_schedule_has_no_stub_and_its_own_grid() -> None:
    schedule = unadjusted("2025-01-15", "2026-01-15", "quarterly")
    assert schedule.dates() == days(
        "2025-01-15", "2025-04-15", "2025-07-15", "2025-10-15", "2026-01-15"
    )
    assert ranges(schedule) == list(zip(schedule.dates(), schedule.dates()[1:]))
    assert refs(schedule) == ranges(schedule)


def test_backward_generation_puts_the_stub_at_the_front() -> None:
    schedule = unadjusted("2025-02-01", "2026-01-15", "semiannual", rule="backward")
    assert schedule.dates() == days("2025-02-01", "2025-07-15", "2026-01-15")
    # The stub accrues against the notional coupon one tenor back.
    assert refs(schedule)[0] == (
        datetime.date(2025, 1, 15),
        datetime.date(2025, 7, 15),
    )


def test_forward_generation_puts_the_stub_at_the_back() -> None:
    schedule = unadjusted("2025-01-15", "2026-02-01", "semiannual", rule="forward")
    assert schedule.dates() == days("2025-01-15", "2025-07-15", "2026-01-15", "2026-02-01")
    assert refs(schedule)[-1] == (
        datetime.date(2026, 1, 15),
        datetime.date(2026, 7, 15),
    )


def test_a_zero_schedule_is_just_its_two_ends() -> None:
    schedule = unadjusted("2025-01-15", "2026-01-15", "annual", rule=DateGenerationRule.ZERO)
    assert schedule.dates() == days("2025-01-15", "2026-01-15")


def test_explicit_stub_anchors() -> None:
    front = unadjusted(
        "2025-01-01", "2026-01-15", "semiannual", first_date=datetime.date(2025, 1, 15)
    )
    assert front.dates() == days("2025-01-01", "2025-01-15", "2025-07-15", "2026-01-15")
    back = unadjusted(
        "2025-01-15",
        "2026-02-01",
        "semiannual",
        rule="forward",
        next_to_last_date=datetime.date(2026, 1, 15),
    )
    assert back.dates() == days("2025-01-15", "2025-07-15", "2026-01-15", "2026-02-01")


def test_end_of_month_generation_keeps_month_ends() -> None:
    schedule = unadjusted("2025-01-31", "2025-07-31", "quarterly", end_of_month=True)
    assert schedule.dates() == days("2025-01-31", "2025-04-30", "2025-07-31")
    assert schedule.generation() is not None
    assert schedule.generation().end_of_month is True  # type: ignore[union-attr]


def test_the_tenor_may_be_a_period_a_frequency_or_a_spelling() -> None:
    by_period = unadjusted("2025-01-15", "2026-01-15", Period.months(6))
    by_enum = unadjusted("2025-01-15", "2026-01-15", Frequency.SEMIANNUAL)
    by_name = unadjusted("2025-01-15", "2026-01-15", "semiannual")
    assert by_period == by_enum == by_name


def test_the_conventions_apply_where_the_crate_applies_them() -> None:
    # The termination date defaults to Unadjusted even when the interior
    # dates roll.
    schedule = Schedule(
        datetime.date(2025, 1, 31),
        datetime.date(2026, 1, 31),
        "quarterly",
        us.GOVERNMENT_BOND,
    )
    assert schedule.dates() == days(
        "2025-01-31", "2025-04-30", "2025-07-31", "2025-10-31", "2026-01-31"
    )
    rolled = Schedule(
        datetime.date(2025, 1, 31),
        datetime.date(2026, 1, 31),
        "quarterly",
        us.GOVERNMENT_BOND,
        termination_convention="following",
    )
    assert rolled.dates()[-1] == datetime.date(2026, 2, 2)


def test_a_schedule_from_a_bare_date_list_names_no_lattice() -> None:
    dates = days("2025-01-15", "2025-07-15", "2026-01-15")
    schedule = Schedule.from_dates(dates)
    assert schedule.dates() == dates
    assert schedule.generation() is None
    assert refs(schedule) == ranges(schedule)


def test_a_schedule_indexes_iterates_and_measures() -> None:
    schedule = unadjusted("2025-01-15", "2026-01-15", "quarterly")
    assert len(schedule) == 5
    assert schedule[0] == datetime.date(2025, 1, 15)
    assert schedule[-1] == datetime.date(2026, 1, 15)
    assert list(schedule) == schedule.dates()
    with pytest.raises(IndexError):
        schedule[5]


def test_the_neighbour_queries_differ_only_on_an_exact_match() -> None:
    schedule = unadjusted("2025-01-15", "2026-01-15", "quarterly")
    on_a_date = datetime.date(2025, 4, 15)
    assert schedule.previous_date(on_a_date) == datetime.date(2025, 1, 15)
    assert schedule.next_date(on_a_date) == datetime.date(2025, 7, 15)
    assert schedule.lower_bound(on_a_date) == on_a_date
    assert schedule.previous_date(datetime.date(2024, 1, 1)) is None
    assert schedule.next_date(datetime.date(2027, 1, 1)) is None


def test_slicing_keeps_the_lattice_and_drops_the_stub_it_cuts_off() -> None:
    schedule = unadjusted("2025-02-01", "2026-01-15", "semiannual")
    tail = schedule.after(datetime.date(2025, 7, 15))
    assert tail.dates() == days("2025-07-15", "2026-01-15")
    assert tail.generation() == schedule.generation()
    # The front stub went with the dates it belonged to.
    assert refs(tail) == ranges(tail)
    head = schedule.until(datetime.date(2025, 7, 15))
    assert head.dates() == days("2025-02-01", "2025-07-15")


@pytest.mark.parametrize(
    ("effective", "termination", "tenor", "message"),
    [
        ("2026-01-15", "2025-01-15", "annual", "strictly before termination"),
        ("2025-01-15", "2026-01-15", Period.days(0), "tenor must be non-zero"),
    ],
)
def test_inconsistent_inputs_are_refused(
    effective: str, termination: str, tenor: object, message: str
) -> None:
    with pytest.raises(FastiError, match=message):
        unadjusted(effective, termination, tenor)


def test_a_stub_anchor_outside_the_schedule_is_refused() -> None:
    with pytest.raises(FastiError, match="stub date is out of"):
        unadjusted(
            "2025-01-15", "2026-01-15", "semiannual", first_date=datetime.date(2027, 1, 1)
        )


def test_a_schedule_slices_as_the_crate_slices_its_dates() -> None:
    schedule = unadjusted("2025-01-15", "2026-01-15", "quarterly")
    dates = schedule.dates()
    assert schedule[1:3] == dates[1:3]
    assert schedule[:2] == dates[:2]
    assert schedule[-2:] == dates[-2:]
    assert schedule[::2] == dates[::2]
    assert schedule[::-1] == dates[::-1] == list(reversed(schedule))
    assert schedule[9:] == []
    assert schedule[0:0] == []


def test_slicing_a_schedule_yields_dates_not_a_schedule() -> None:
    # A slice is a run of coupon dates; it names no lattice, so it is not
    # a schedule. `after`/`until` are what keep one.
    schedule = unadjusted("2025-01-15", "2026-01-15", "quarterly")
    assert isinstance(schedule[1:3], list)
    assert all(isinstance(d, datetime.date) for d in schedule[1:3])
    assert isinstance(schedule.after(datetime.date(2025, 7, 15)), Schedule)


def test_a_schedule_is_iterable_in_both_directions() -> None:
    schedule = unadjusted("2025-01-15", "2026-01-15", "quarterly")
    assert list(reversed(schedule)) == schedule.dates()[::-1]
    assert next(iter(schedule)) == schedule[0]
    assert schedule[0] in schedule
    assert datetime.date(2025, 1, 16) not in schedule


def test_coupons_pay_on_the_next_business_day_after_accrual() -> None:
    schedule = Schedule(
        datetime.date(2025, 1, 15), datetime.date(2030, 1, 15), "semiannual", us.GOVERNMENT_BOND
    )
    coupon = schedule.coupons()[5]
    assert (coupon.accrual.start, coupon.accrual.end) == (
        datetime.date(2027, 7, 15),
        datetime.date(2028, 1, 15),
    )
    assert coupon.payment == datetime.date(2028, 1, 18)
    assert len(schedule.payment_dates()) == len(schedule.periods())
    lagged = Schedule(
        datetime.date(2025, 1, 15),
        datetime.date(2030, 1, 15),
        "semiannual",
        us.GOVERNMENT_BOND,
        payment_lag=1,
    )
    # A lag counts business days from the accrual end, Saturday the 15th:
    # Monday is MLK Day, so one business day on is Tuesday.
    assert lagged.payment_dates()[5] == datetime.date(2028, 1, 18)
    assert lagged.dates() == schedule.dates()


def test_accrual_period_is_half_open() -> None:
    schedule = unadjusted("2025-01-15", "2026-01-15", "semiannual")
    coupon_date = datetime.date(2025, 7, 15)
    period = schedule.accrual_period(coupon_date)
    assert period is not None and period.start == coupon_date
    assert coupon_date in period
    truncated = period.until(coupon_date)
    assert truncated is not None and truncated.start == truncated.end
    assert schedule.accrual_period(datetime.date(2024, 1, 1)) is None
