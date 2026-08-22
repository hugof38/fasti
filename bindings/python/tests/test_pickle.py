"""Pickling, because the first thing a calendar meets at scale is a worker process.

Every type rebuilds by replaying the constructor calls that made it, so
these tests check the rebuilt value *behaves* the same rather than that
some opaque state came back.
"""

import copy
import datetime
import multiprocessing
import pickle
from datetime import date

import pytest

import fasti
from fasti import calendars

PROTOCOLS = [2, pickle.DEFAULT_PROTOCOL, pickle.HIGHEST_PROTOCOL]


def round_trip(obj, protocol):
    return pickle.loads(pickle.dumps(obj, protocol))


@pytest.mark.parametrize("protocol", PROTOCOLS)
@pytest.mark.parametrize(
    "value",
    [
        fasti.Weekday.SAT,
        fasti.BusinessDayConvention.MODIFIED_FOLLOWING,
        fasti.DateGenerationRule.BACKWARD,
        fasti.Frequency.SEMIANNUAL,
        fasti.WeekendShift.SAT_BACK_SUN_FORWARD,
        fasti.Period("6M"),
        fasti.Period("-3D"),
        fasti.Period("2W"),
    ],
    ids=repr,
)
def test_values_round_trip_by_equality(value, protocol):
    assert round_trip(value, protocol) == value


@pytest.mark.parametrize(
    "rule",
    [
        fasti.Rule.fixed("Jul", 4),
        fasti.Rule.fixed("Jul", 4, shift="us", from_year=1971, to_year=2100),
        fasti.Rule.nth_weekday(3, "mon", "Jan"),
        fasti.Rule.last_weekday("mon", "May"),
        fasti.Rule.easter(-2),
        fasti.Rule.easter(1, method="orthodox", from_year=1990),
        fasti.Rule.one_off(date(2026, 8, 14)),
    ],
    ids=repr,
)
def test_rules_round_trip(rule):
    rebuilt = round_trip(rule, pickle.HIGHEST_PROTOCOL)
    assert repr(rebuilt) == repr(rule)
    day, end = date(2020, 1, 1), date(2030, 1, 1)
    while day < end:
        assert rebuilt.is_holiday(day) == rule.is_holiday(day), day
        day += datetime.timedelta(days=1)


def calendars_under_test():
    return {
        # NYSE and the government bond calendar carry `Rule::Custom`
        # predicates — fn pointers, with nothing to serialize — which is
        # why a built-in travels as its name.
        "builtin": calendars.US_NYSE,
        "builtin with fn-pointer rules": calendars.US_GOVERNMENT_BOND,
        "union": calendars.US_SETTLEMENT.union(calendars.FRANCE_SETTLEMENT),
        "custom": fasti.Calendar.custom(
            "Acme",
            weekend=["sat", "sun"],
            rules=[fasti.Rule.fixed("Jun", 19, shift="us", from_year=2022)],
            holidays=[date(2026, 8, 14)],
        ),
        "derived chain": (
            calendars.UK_SETTLEMENT.with_holidays([date(2026, 8, 14)])
            .renamed("UK plus")
            .with_weekend(["sun"])
        ),
        "union of derived": calendars.TARGET.union(
            fasti.Calendar.custom("x", holidays=[date(2026, 3, 3)])
        ),
    }


def assert_same_calendar(a, b):
    assert a.name == b.name
    assert a.weekend == b.weekend
    day, end = date(2020, 1, 1), date(2030, 1, 1)
    while day < end:
        assert a.is_business_day(day) == b.is_business_day(day), day
        assert a.is_holiday(day) == b.is_holiday(day), day
        day += datetime.timedelta(days=1)


@pytest.mark.parametrize("label", list(calendars_under_test()))
@pytest.mark.parametrize("protocol", PROTOCOLS)
def test_calendars_round_trip(label, protocol):
    cal = calendars_under_test()[label]
    assert_same_calendar(cal, round_trip(cal, protocol))


def schedules_under_test():
    return {
        "regular": fasti.Schedule(date(2025, 1, 15), date(2027, 1, 15), "6M", calendars.US_SETTLEMENT),
        "front stub": fasti.Schedule(date(2025, 3, 15), date(2026, 1, 15), "6M", rule="backward"),
        "end of month": fasti.Schedule(date(2025, 1, 31), date(2026, 1, 31), "3M", end_of_month=True),
        "every option": fasti.Schedule(
            date(2025, 1, 15), date(2027, 1, 15), "6M", calendars.US_NYSE,
            convention="following", termination_convention="preceding",
            rule="forward", first_date=date(2025, 4, 15),
        ),
        "from dates": fasti.Schedule.from_dates([date(2025, 1, 15), date(2025, 7, 15)]),
        "sliced": fasti.Schedule(date(2025, 1, 15), date(2028, 1, 15), "6M")
        .after(date(2026, 1, 15))
        .until(date(2027, 1, 15)),
    }


@pytest.mark.parametrize("label", list(schedules_under_test()))
def test_schedules_round_trip_including_their_reference_grid(label):
    schedule = schedules_under_test()[label]
    rebuilt = round_trip(schedule, pickle.HIGHEST_PROTOCOL)
    assert rebuilt == schedule
    assert rebuilt.dates == schedule.dates
    # The stub reference periods are not recoverable from the dates
    # alone, so this is the check that matters.
    assert rebuilt.reference_periods() == schedule.reference_periods()
    assert rebuilt.tenor == schedule.tenor
    assert rebuilt.end_of_month == schedule.end_of_month


@pytest.mark.parametrize(
    "convention",
    [
        fasti.DayCount("ACT/360"),
        fasti.DayCount("30E/360 ISDA", termination=date(2030, 2, 28)),
        fasti.DayCount("ACT/ACT ICMA", frequency="semiannual"),
        fasti.DayCount(
            "ACT/ACT ICMA",
            schedule=fasti.Schedule(date(2025, 3, 15), date(2026, 1, 15), "6M"),
        ),
    ],
    ids=lambda dc: dc.name,
)
def test_day_counts_round_trip(convention):
    rebuilt = round_trip(convention, pickle.HIGHEST_PROTOCOL)
    assert rebuilt == convention
    start, end = date(2025, 4, 1), date(2025, 10, 1)
    assert rebuilt.year_fraction(start, end) == convention.year_fraction(start, end)


@pytest.mark.parametrize(
    "value",
    [
        calendars.US_NYSE,
        fasti.Period("6M"),
        fasti.Schedule(date(2025, 1, 15), date(2026, 1, 15), "6M"),
        fasti.DayCount("ACT/360"),
        fasti.Rule.fixed("Jul", 4),
        fasti.Weekday.MON,
    ],
    ids=lambda v: type(v).__name__,
)
def test_deepcopy(value):
    """`copy.deepcopy` goes through the same protocol."""
    assert copy.deepcopy(value) is not None


def _count(calendar):
    return calendar.count_business_days(date(2026, 1, 1), date(2027, 1, 1))


@pytest.mark.skipif(
    multiprocessing.get_start_method(allow_none=True) == "spawn"
    and __name__ != "__main__",
    reason="spawn start method re-imports the test module",
)
def test_calendars_survive_a_worker_process():
    with multiprocessing.Pool(2) as pool:
        counts = pool.map(_count, [calendars.US_NYSE, calendars.TARGET])
    assert counts == [_count(calendars.US_NYSE), _count(calendars.TARGET)]


def test_an_unpicklable_payload_is_rejected_clearly():
    with pytest.raises(fasti.FastiError, match="unknown calendar spec"):
        fasti._fasti._rebuild_calendar(("nonsense", "x"))
