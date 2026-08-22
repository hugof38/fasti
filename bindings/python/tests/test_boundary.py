"""The boundary: what crosses it, and what is refused at it."""

import datetime
import fractions
import pickle
import re

import pytest

import fasti
from fasti import Calendar, DayCount, FastiError, Rule
from fasti.calendars import WEEKENDS_ONLY, us

JULY_FOURTH = datetime.date(2024, 7, 4)


def test_dates_come_back_as_exactly_datetime_date() -> None:
    adjusted = us.SETTLEMENT.adjust(JULY_FOURTH, "following")
    assert type(adjusted) is datetime.date
    assert adjusted == datetime.date(2024, 7, 5)


def test_the_supported_range_maps_end_to_end() -> None:
    first, last = datetime.date(1901, 1, 1), datetime.date(2199, 12, 31)
    assert us.SETTLEMENT.adjust(first, "unadjusted") == first
    assert us.SETTLEMENT.adjust(last, "unadjusted") == last


@pytest.mark.parametrize(
    "outside", [datetime.date(1900, 12, 31), datetime.date(2200, 1, 1), datetime.date.min]
)
def test_dates_outside_the_supported_range_are_refused(outside: datetime.date) -> None:
    with pytest.raises(FastiError, match="date arithmetic result out of range"):
        us.SETTLEMENT.is_holiday(outside)


def test_a_str_is_refused_and_told_how_to_parse() -> None:
    with pytest.raises(TypeError) as excinfo:
        us.SETTLEMENT.is_holiday("2024-07-04")  # type: ignore[arg-type]
    assert "datetime.date.fromisoformat" in str(excinfo.value)


def test_a_datetime_is_refused_and_told_to_pick_a_day() -> None:
    moment = datetime.datetime(2024, 7, 4, 23, 30)
    # A checker cannot reject this: datetime subclasses date. The
    # runtime does.
    with pytest.raises(TypeError) as excinfo:
        us.SETTLEMENT.is_holiday(moment)
    message = str(excinfo.value)
    assert ".date()" in message and "time zone" in message


def test_datetime_is_checked_before_date() -> None:
    # datetime subclasses date, so order is the whole test.
    assert isinstance(datetime.datetime(2024, 7, 4), datetime.date)
    with pytest.raises(TypeError, match="not a datetime.datetime"):
        us.SETTLEMENT.is_holiday(datetime.datetime(2024, 7, 4))


@pytest.mark.parametrize("wrong", [42, None, 738000, [2024, 7, 4]])
def test_anything_else_is_refused_by_type(wrong: object) -> None:
    with pytest.raises(TypeError, match="fasti takes a datetime.date"):
        us.SETTLEMENT.is_holiday(wrong)  # type: ignore[arg-type]


def test_a_date_subclass_is_accepted() -> None:
    class Anniversary(datetime.date):
        pass

    assert us.SETTLEMENT.is_holiday(Anniversary(2024, 7, 4))


def test_year_fractions_are_exact_fractions_never_floats() -> None:
    value = DayCount.act_360().year_fraction(
        datetime.date(2025, 1, 1), datetime.date(2025, 4, 1)
    )
    assert type(value) is fractions.Fraction
    assert value == fractions.Fraction(1, 4)
    assert not isinstance(value, float)


def test_errors_are_value_errors_and_type_mistakes_are_type_errors() -> None:
    assert issubclass(FastiError, ValueError)
    with pytest.raises(FastiError):
        Rule.fixed(2, 30)
    with pytest.raises(TypeError):
        Calendar("Acme", ["sat"], [object()])  # type: ignore[list-item]


def test_the_error_carries_the_crate_message() -> None:
    with pytest.raises(FastiError, match=re.escape("year out of range: supported range is 1901..=2199")):
        fasti.easter_sunday(1899)


def test_every_public_value_is_immutable() -> None:
    values: list[object] = [
        us.SETTLEMENT,
        Rule.fixed(7, 4),
        fasti.Period.months(3),
        fasti.Frequency.ANNUAL,
        fasti.Schedule.from_dates([JULY_FOURTH, datetime.date(2025, 7, 4)]),
        DayCount.act_360(),
    ]
    for value in values:
        with pytest.raises(AttributeError):
            setattr(value, "spam", 1)  # noqa: B010


def test_mutators_return_new_values() -> None:
    base = Calendar("Acme", ["sat", "sun"])
    extended = base.with_rule(Rule.fixed(7, 4))
    assert base != extended
    assert not base.is_holiday(JULY_FOURTH)
    assert extended.is_holiday(JULY_FOURTH)


def test_a_calendar_survives_a_pickle_round_trip_by_value() -> None:
    assert pickle.loads(pickle.dumps(WEEKENDS_ONLY)) == WEEKENDS_ONLY
