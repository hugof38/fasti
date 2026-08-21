"""Holiday rules, and the calendars built out of them."""

import datetime

import pytest

import fasti
from fasti import Calendar, Rule, Weekday


def only(rule):
    """A calendar with no weekend, so only the rule decides."""
    return Calendar.custom("test", weekend="none", rules=[rule])


def test_fixed_date_rule():
    rule = Rule.fixed("Jul", 4)
    assert rule.is_holiday("2026-07-04")
    assert not rule.is_holiday("2026-07-03")
    assert Rule.fixed(7, 4).is_holiday("2026-07-04")


def test_shift_is_applied_by_the_calendar_not_the_rule():
    rule = Rule.fixed("Jul", 4, shift="us")
    assert not rule.is_holiday("2026-07-03")
    assert Calendar.custom("us-ish", rules=[rule]).is_holiday("2026-07-03")


@pytest.mark.parametrize(
    ("shift", "observed"),
    [
        ("none", None),
        ("forward", datetime.date(2026, 7, 6)),  # Saturday moves to Monday
        ("sun_forward", None),  # Saturday stays put
        ("sat_back_sun_forward", datetime.date(2026, 7, 3)),
    ],
)
def test_weekend_shift_policies(shift, observed):
    cal = Calendar.custom("t", rules=[Rule.fixed("Jul", 4, shift=shift)])
    for day in (datetime.date(2026, 7, 3), datetime.date(2026, 7, 6)):
        assert cal.is_holiday(day) is (day == observed)


def test_nth_and_last_weekday_rules():
    mlk = Rule.nth_weekday(3, "mon", "Jan")
    assert mlk.is_holiday("2026-01-19")
    memorial = Rule.last_weekday(Weekday.MON, "May")
    assert memorial.is_holiday("2026-05-25")
    assert not memorial.is_holiday("2026-05-18")


def test_year_ranges_bound_a_rule():
    rule = Rule.fixed("Jun", 19, from_year=2022)
    assert rule.is_holiday("2022-06-19")
    assert not rule.is_holiday("2021-06-19")
    windowed = Rule.fixed("Jun", 19, from_year=2022, to_year=2024)
    assert not windowed.is_holiday("2025-06-19")
    assert Rule.fixed("Jun", 19, to_year=2024).is_holiday("1999-06-19")


def test_invalid_year_range_is_refused():
    with pytest.raises(fasti.FastiError):
        Rule.fixed("Jun", 19, from_year=2024, to_year=2022)


def test_easter_offsets_count_from_easter_sunday():
    easter = fasti.easter_sunday(2026)
    assert Rule.easter(0).is_holiday(easter)
    assert Rule.good_friday().is_holiday(easter - datetime.timedelta(days=2))
    assert Rule.easter_monday().is_holiday(easter + datetime.timedelta(days=1))
    assert Rule.ascension().is_holiday(easter + datetime.timedelta(days=39))
    assert Rule.whit_monday().is_holiday(easter + datetime.timedelta(days=50))
    assert Rule.corpus_christi().is_holiday(easter + datetime.timedelta(days=60))


def test_orthodox_easter_rules():
    orthodox = fasti.easter_sunday(2026, method="orthodox")
    assert Rule.easter(0, method="orthodox").is_holiday(orthodox)
    assert not Rule.easter(0).is_holiday(orthodox)


def test_one_off_rule():
    rule = Rule.one_off(datetime.date(2026, 8, 14))
    assert rule.is_holiday("2026-08-14")
    assert not rule.is_holiday("2027-08-14")


@pytest.mark.parametrize(
    "rule",
    [
        Rule.fixed("Jul", 4),
        Rule.fixed("Jul", 4, shift="us", from_year=1971),
        Rule.nth_weekday(3, "mon", "Jan"),
        Rule.last_weekday("mon", "May"),
        Rule.easter(-2),
        Rule.one_off("2026-08-14"),
    ],
    ids=repr,
)
def test_rules_describe_themselves_as_the_call_that_built_them(rule):
    rebuilt = eval(repr(rule), {"Rule": Rule})  # noqa: S307 - our own repr
    assert repr(rebuilt) == repr(rule)


def test_invalid_rule_arguments():
    with pytest.raises(fasti.FastiError):
        Rule.fixed("Feb", 30)
    with pytest.raises(fasti.FastiError):
        Rule.nth_weekday(6, "mon", "Jan")
    with pytest.raises(fasti.FastiError, match="month"):
        Rule.fixed("Smarch", 1)
    with pytest.raises(fasti.FastiError, match="weekday"):
        Rule.last_weekday("Funday", "May")


def test_rules_can_extend_a_built_in_calendar():
    cal = fasti.calendars.US_SETTLEMENT.with_rules([Rule.fixed("Oct", 31)]).renamed("US + Halloween")
    assert cal.is_holiday("2026-10-31")
    assert cal.is_holiday("2026-07-03")
    assert not fasti.calendars.US_SETTLEMENT.is_holiday("2026-10-31")
