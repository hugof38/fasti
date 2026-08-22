"""Values: equality, hashing, repr and pickling."""

import datetime
import multiprocessing
import pickle
from typing import Any

import pytest

import fasti
from fasti import (
    BusinessDayConvention,
    Calendar,
    DateGenerationRule,
    DayCount,
    EasterMethod,
    FastiError,
    Frequency,
    Period,
    Rule,
    Schedule,
    Weekday,
    WeekendShift,
)
from fasti.calendars import TARGET, WEEKENDS_ONLY, france, uk, us

SCHEDULE = Schedule(
    datetime.date(2002, 8, 15),
    datetime.date(2004, 1, 15),
    "semiannual",
    us.GOVERNMENT_BOND,
    convention="unadjusted",
)

VALUES: list[Any] = [
    BusinessDayConvention.MODIFIED_FOLLOWING,
    DateGenerationRule.ZERO,
    EasterMethod.ORTHODOX,
    Frequency.EVERY_FOURTH_WEEK,
    Weekday.SUN,
    WeekendShift.SAT_BACK_SUN_FORWARD,
    Period.ZERO,
    Period.months(-3),
    Period.years(2),
    Rule.fixed(7, 4, shift="fed", years=(1971, 2100)),
    Rule.nth_weekday(3, "mon", 1),
    Rule.last_weekday("mon", 5, years=(None, 1970)),
    Rule.easter(-2, method="orthodox"),
    Rule.one_off(datetime.date(2026, 8, 3)),
    TARGET,
    WEEKENDS_ONLY,
    uk.SETTLEMENT,
    us.NYSE,
    Calendar("Acme", ["sat", "sun"], [Rule.fixed(1, 1), Rule.easter(1)]),
    us.SETTLEMENT.union(france.SETTLEMENT)
    .with_rule(Rule.one_off(datetime.date(2026, 8, 3)))
    .with_weekend(["fri", "sat"])
    .with_name("Acme"),
    SCHEDULE,
    SCHEDULE.after(datetime.date(2003, 1, 15)).until(datetime.date(2003, 7, 15)),
    Schedule.from_dates([datetime.date(2025, 1, 15), datetime.date(2026, 1, 15)]),
    SCHEDULE.generation(),
    DayCount.act_360(),
    DayCount.act_act_icma("semiannual"),
    DayCount.act_act_icma("semiannual").bind(SCHEDULE),
    DayCount.thirty_360_isda(datetime.date(2025, 12, 31)),
]


@pytest.mark.parametrize("value", VALUES, ids=repr)
def test_every_value_pickles_back_to_itself(value: Any) -> None:
    restored = pickle.loads(pickle.dumps(value))
    assert restored == value
    assert hash(restored) == hash(value)
    assert repr(restored) == repr(value)


@pytest.mark.parametrize("protocol", [2, pickle.HIGHEST_PROTOCOL])
def test_pickling_works_at_older_protocols(protocol: int) -> None:
    for value in VALUES:
        assert pickle.loads(pickle.dumps(value, protocol)) == value


def test_every_value_survives_a_real_process_boundary() -> None:
    # A separate interpreter has to rebuild these from the pickle alone;
    # equality in one process would not prove that.
    blobs = [pickle.dumps(value) for value in VALUES]
    with multiprocessing.Pool(2) as pool:
        restored = pool.map(pickle.loads, blobs)
    assert restored == VALUES


def test_a_built_in_calendar_travels_as_its_registry_name() -> None:
    # Its rules include bare function pointers, which cannot travel.
    hook, (origin, ops) = us.NYSE.__reduce__()
    assert (origin, ops) == ("us::NYSE", [])
    assert hook is fasti._fasti._rebuild_calendar


def test_a_derived_calendar_travels_as_the_operations_applied() -> None:
    derived = us.NYSE.with_rule(Rule.fixed(1, 2)).with_name("Acme")
    _, (origin, ops) = derived.__reduce__()
    assert origin == "us::NYSE"
    assert [name for name, _ in ops] == ["with_rule", "with_name"]


def test_a_generated_schedule_replays_its_generation_not_its_dates() -> None:
    # The stub reference grid is not recoverable from the dates alone.
    restored = pickle.loads(pickle.dumps(SCHEDULE))
    assert restored.reference_periods() == SCHEDULE.reference_periods()
    assert restored.reference_periods()[0] != restored.periods()[0]
    assert restored.generation() == SCHEDULE.generation()


def test_equality_is_structural_for_calendars_and_rules() -> None:
    assert Calendar("Acme", ["sat"]) == Calendar("Acme", ["sat"])
    assert Calendar("Acme", ["sat"]) != Calendar("Acme", ["sun"])
    # Same dates, different construction: not equal, and it does not
    # claim to be. Settling that means walking 1901 through 2199.
    spelled_out = Calendar("Weekends only", ["sat", "sun"])
    assert spelled_out != WEEKENDS_ONLY
    span = (datetime.date(2024, 1, 1), datetime.date(2025, 1, 1))
    assert spelled_out.business_days(*span) == WEEKENDS_ONLY.business_days(*span)


def test_a_schedule_is_its_own_value_not_its_construction() -> None:
    generated = Schedule(
        datetime.date(2025, 1, 15), datetime.date(2026, 1, 15), "annual", WEEKENDS_ONLY
    )
    listed = Schedule.from_dates(generated.dates())
    # Same dates, but one names a lattice and the other does not.
    assert generated != listed
    assert Schedule.from_dates(generated.dates()) == listed


@pytest.mark.parametrize("value", VALUES, ids=repr)
def test_every_value_hashes_and_can_key_a_dict(value: Any) -> None:
    assert {value: "kept"}[value] == "kept"
    assert len({value, pickle.loads(pickle.dumps(value))}) == 1


@pytest.mark.parametrize("value", VALUES, ids=repr)
def test_comparing_against_another_type_is_false_not_an_error(value: Any) -> None:
    assert value != object()
    assert not value == 42


def test_repr_of_the_small_values_evaluates_back() -> None:
    namespace = {"datetime": datetime, **{name: getattr(fasti, name) for name in fasti.__all__}}
    for value in VALUES:
        text = repr(value)
        # A calendar reprs as its registry path and a schedule as its
        # span; neither is a constructor call.
        if text.startswith("calendars.") or "Schedule(" in text:
            continue
        assert eval(text, namespace) == value  # noqa: S307


def test_the_reprs_that_are_not_constructor_calls_say_what_they_are() -> None:
    assert repr(SCHEDULE) == (
        "Schedule([datetime.date(2002, 8, 15) .. datetime.date(2004, 1, 15)], 4 dates)"
    )
    assert repr(DayCount.act_act_icma("semiannual").bind(SCHEDULE)) == (
        "DayCount.act_act_icma(Frequency.SEMIANNUAL).bind(" + repr(SCHEDULE) + ")"
    )


def test_a_built_in_calendar_reprs_as_its_path() -> None:
    assert repr(us.NYSE) == "calendars.us.NYSE"
    assert repr(TARGET) == "calendars.TARGET"
    assert repr(us.NYSE.with_name("Acme")) == 'calendars.us.NYSE.with_name("Acme")'
