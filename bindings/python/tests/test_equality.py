"""Equality and hashing.

Calendars and rules compare by how they were built — the same name,
weekend and rules, reached the same way. That is a structural comparison,
not a claim that two calendars agree on every date; settling *that* means
walking three centuries, which is not what ``==`` should cost.
"""

import pickle
from datetime import date

import pytest

import fasti
from fasti import calendars

BLACKOUT = date(2026, 8, 14)
OTHER = date(2026, 9, 15)


@pytest.mark.parametrize(
    ("left", "right"),
    [
        (fasti.Calendar("nyse"), fasti.Calendar("US.NYSE")),
        (fasti.Calendar("us settlement"), fasti.Calendar("US.SETTLEMENT")),
        (
            calendars.US_SETTLEMENT.union(calendars.TARGET),
            calendars.US_SETTLEMENT.union(calendars.TARGET),
        ),
        (
            calendars.US_NYSE.with_holidays([BLACKOUT]),
            calendars.US_NYSE.with_holidays([BLACKOUT]),
        ),
        # Two additions or one: the same calendar either way.
        (
            calendars.US_NYSE.with_holidays([BLACKOUT]).with_holidays([OTHER]),
            calendars.US_NYSE.with_holidays([BLACKOUT, OTHER]),
        ),
        # A one-off holiday is a rule; saying it either way is the same.
        (
            fasti.Calendar.custom("A", holidays=[BLACKOUT]),
            fasti.Calendar.custom("A", rules=[fasti.Rule.one_off(BLACKOUT)]),
        ),
    ],
)
def test_calendars_that_were_built_the_same_way_are_equal(left, right):
    assert left == right
    assert hash(left) == hash(right)


@pytest.mark.parametrize(
    ("left", "right"),
    [
        (calendars.US_NYSE, calendars.TARGET),
        (calendars.US_NYSE, calendars.US_NYSE.with_holidays([BLACKOUT])),
        (calendars.US_NYSE, calendars.US_NYSE.renamed("NYSE")),
        (calendars.US_NYSE, calendars.US_NYSE.with_weekend(["sun"])),
        (
            calendars.US_SETTLEMENT.union(calendars.TARGET),
            calendars.TARGET.union(calendars.US_SETTLEMENT),
        ),
    ],
)
def test_calendars_built_differently_are_not_equal(left, right):
    assert left != right


def test_a_calendar_is_not_equal_to_other_things():
    assert calendars.US_NYSE != "US.NYSE"
    assert calendars.US_NYSE is not None


def test_equality_survives_pickling():
    """The property that makes a calendar usable across processes."""
    for calendar in (
        calendars.US_NYSE,
        calendars.US_NYSE.with_holidays([BLACKOUT]),
        fasti.Calendar.custom("A", weekend="fri_sat", holidays=[BLACKOUT]),
    ):
        assert pickle.loads(pickle.dumps(calendar)) == calendar


def test_calendars_work_as_dict_keys():
    accrued = {calendars.US_NYSE: 1, calendars.TARGET: 2}
    assert accrued[fasti.Calendar("nyse")] == 1
    assert len({calendars.US_NYSE, fasti.Calendar("US.NYSE"), calendars.TARGET}) == 2


@pytest.mark.parametrize(
    ("left", "right", "equal"),
    [
        (fasti.Rule.fixed("Jul", 4), fasti.Rule.fixed(7, 4), True),
        (fasti.Rule.fixed("Jul", 4), fasti.Rule.fixed("Jul", 4, shift="us"), False),
        (fasti.Rule.fixed("Jul", 4), fasti.Rule.fixed("Jul", 4, from_year=1971), False),
        (fasti.Rule.easter(-2), fasti.Rule.good_friday(), True),
        (fasti.Rule.easter(-2), fasti.Rule.easter(-2, method="orthodox"), False),
        (fasti.Rule.one_off(BLACKOUT), fasti.Rule.one_off(BLACKOUT), True),
        (fasti.Rule.one_off(BLACKOUT), fasti.Rule.one_off(OTHER), False),
        (fasti.Rule.nth_weekday(3, "mon", "Jan"), fasti.Rule.nth_weekday(3, 1, 1), True),
    ],
)
def test_rules_compare_by_what_they_say(left, right, equal):
    assert (left == right) is equal
    if equal:
        assert hash(left) == hash(right)


def test_every_value_type_is_hashable():
    """Immutable values belong in sets and dict keys."""
    schedule = fasti.Schedule(date(2025, 1, 15), date(2026, 1, 15), "6M")
    values = [
        calendars.US_NYSE,
        fasti.Rule.fixed("Jul", 4),
        fasti.Period("6M"),
        fasti.Frequency.SEMIANNUAL,
        fasti.Weekday.MON,
        fasti.BusinessDayConvention.FOLLOWING,
        schedule,
        fasti.DayCount("ACT/360"),
    ]
    assert len({v: None for v in values}) == len(values)


def test_equal_schedules_and_day_counts_hash_alike():
    make = lambda: fasti.Schedule(date(2025, 1, 15), date(2026, 1, 15), "6M")  # noqa: E731
    assert make() == make() and hash(make()) == hash(make())
    assert fasti.DayCount("ACT/360") == fasti.DayCount("act 360")
    assert hash(fasti.DayCount("ACT/360")) == hash(fasti.DayCount("Actual/360"))
