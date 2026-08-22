"""Built-in calendars, checked against published holiday dates."""

from datetime import date, timedelta

import pytest

import fasti
from fasti import calendars


def test_july_fourth_on_a_saturday_is_observed_on_the_friday():
    us = calendars.US_SETTLEMENT
    assert us.is_holiday(date(2026, 7, 3))
    assert not us.is_business_day(date(2026, 7, 3))
    assert us.is_business_day(date(2026, 7, 6))


def test_uk_christmas_on_a_saturday_pushes_boxing_day_to_the_tuesday():
    uk = calendars.UK_SETTLEMENT
    for day in (25, 27, 28):
        assert uk.is_holiday(date(2021, 12, day)), day


def test_target_keeps_good_friday_and_loses_weekend_holidays():
    target = calendars.TARGET
    assert target.is_holiday(date(2026, 4, 3))  # Good Friday
    assert target.is_holiday(date(2026, 12, 25))


def test_nyse_closes_on_good_friday_but_us_settlement_does_not():
    good_friday = date(2026, 4, 3)
    assert calendars.US_NYSE.is_holiday(good_friday)
    assert not calendars.US_SETTLEMENT.is_holiday(good_friday)


def test_weekends_are_not_holidays():
    """A plain weekend day is closed, but it is not a holiday."""
    saturday = date(2026, 7, 11)
    assert calendars.US_SETTLEMENT.is_weekend(saturday)
    assert not calendars.US_SETTLEMENT.is_holiday(saturday)
    assert not calendars.US_SETTLEMENT.is_business_day(saturday)


def test_a_holiday_keeps_its_natural_date_as_well_as_its_substitute():
    us = calendars.US_SETTLEMENT
    assert us.is_holiday(date(2026, 7, 4))  # the Saturday itself
    assert us.is_holiday(date(2026, 7, 3))  # and the observed Friday


def test_null_calendar_is_always_open():
    assert calendars.NULL.is_business_day(date(2026, 12, 25))
    assert calendars.NULL.weekend == []


def test_business_day_range_is_half_open():
    cal = calendars.WEEKENDS_ONLY
    days = cal.business_days(date(2026, 7, 6), date(2026, 7, 13))
    assert days == [date(2026, 7, d) for d in (6, 7, 8, 9, 10)]
    assert cal.count_business_days(date(2026, 7, 6), date(2026, 7, 13)) == 5
    assert cal.business_days(date(2026, 7, 6), date(2026, 7, 6)) == []


def test_holidays_lists_observed_days_only():
    holidays = calendars.US_SETTLEMENT.holidays(date(2026, 1, 1), date(2026, 3, 1))
    assert holidays == [date(2026, 1, 1), date(2026, 1, 19), date(2026, 2, 16)]


@pytest.mark.parametrize(
    ("convention", "expected"),
    [
        ("following", date(2025, 9, 1)),
        ("modified_following", date(2025, 8, 29)),
        ("preceding", date(2025, 8, 29)),
        ("unadjusted", date(2025, 8, 31)),
        (fasti.BusinessDayConvention.FOLLOWING, date(2025, 9, 1)),
    ],
)
def test_adjust_conventions(convention, expected):
    # Sunday 2025-08-31.
    assert calendars.WEEKENDS_ONLY.adjust(date(2025, 8, 31), convention) == expected


def test_adjust_defaults_to_following():
    assert calendars.WEEKENDS_ONLY.adjust(date(2025, 8, 30)) == date(2025, 9, 1)


def test_advance_respects_end_of_month():
    cal = calendars.WEEKENDS_ONLY
    # April 30 is a month end, so the step lands on May 31 — a Saturday,
    # which ModifiedFollowing pulls back to the Friday rather than let it
    # cross into June.
    assert cal.advance(
        date(2025, 4, 30), "1M", "modified_following", end_of_month=True
    ) == date(2025, 5, 30)
    assert cal.advance(
        date(2025, 4, 30), "1M", "following", end_of_month=True
    ) == date(2025, 6, 2)
    # Without the flag the step is a plain calendar month.
    assert cal.advance(date(2025, 4, 30), "1M") == date(2025, 5, 30)


def test_advance_accepts_a_timedelta():
    cal = calendars.NULL
    assert cal.advance(date(2026, 1, 1), timedelta(days=10)) == date(2026, 1, 11)


def test_month_edges():
    cal = calendars.US_SETTLEMENT
    # 2026-03-01 is a Sunday.
    assert cal.first_business_day_of_month(date(2026, 3, 18)) == date(2026, 3, 2)
    assert cal.last_business_day_of_month(date(2026, 5, 4)) == date(2026, 5, 29)


def test_next_and_prev_business_day_are_strict():
    cal = calendars.WEEKENDS_ONLY
    monday = date(2026, 7, 6)
    assert cal.next_business_day(monday) == date(2026, 7, 7)
    assert cal.prev_business_day(monday) == date(2026, 7, 3)


def test_search_past_the_supported_range_returns_none():
    assert calendars.WEEKENDS_ONLY.next_business_day(fasti.MAX_DATE) is None
    assert calendars.WEEKENDS_ONLY.prev_business_day(fasti.MIN_DATE) is None


def test_names_and_aliases_resolve_to_the_same_calendar():
    for alias in ("US.SETTLEMENT", "us settlement", "us_settlement", "us"):
        assert fasti.Calendar(alias).name == calendars.US_SETTLEMENT.name
    assert set(fasti.Calendar.names()) >= {"TARGET", "US.NYSE", "UK.SETTLEMENT", "NULL"}


def test_unknown_calendar_name_raises():
    with pytest.raises(fasti.FastiError, match="unknown calendar"):
        fasti.Calendar("Atlantis")


def test_union_closes_when_either_side_closes():
    joint = calendars.US_SETTLEMENT.union(calendars.FRANCE_SETTLEMENT)
    assert joint.is_holiday(date(2026, 11, 26))  # Thanksgiving
    assert joint.is_holiday(date(2026, 7, 14))  # Bastille Day


def test_custom_calendar_from_rules_and_one_offs():
    cal = fasti.Calendar.custom(
        "Acme",
        weekend=["sat", "sun"],
        rules=[fasti.Rule.fixed("Jun", 19, shift="us", from_year=2022)],
        holidays=[date(2026, 8, 14)],
    )
    assert cal.name == "Acme"
    assert cal.is_holiday(date(2026, 8, 14))
    assert cal.is_holiday(date(2027, 6, 18))  # Jun 19 2027 is a Saturday
    assert not cal.is_holiday(date(2021, 6, 18))  # rule starts in 2022


@pytest.mark.parametrize("wrap", [lambda x: x, lambda x: [x]], ids=["bare", "in a list"])
def test_a_single_holiday_or_rule_needs_no_list(wrap):
    """Adding one blackout day is the common case; it reads like one."""
    day = date(2026, 8, 14)
    assert calendars.US_NYSE.with_holidays(wrap(day)).is_holiday(day)
    halloween = fasti.Rule.fixed("Oct", 31)
    extended = calendars.US_NYSE.with_rules(wrap(halloween))
    assert extended.is_holiday(date(2026, 10, 31))
    assert fasti.Calendar.custom("A", rules=wrap(halloween), holidays=wrap(day)).is_holiday(day)


def test_the_scalar_and_list_forms_build_the_same_calendar():
    day = date(2026, 8, 14)
    assert calendars.US_NYSE.with_holidays(day) == calendars.US_NYSE.with_holidays([day])


@pytest.mark.parametrize(
    ("value", "message"),
    [
        ("2026-08-14", "fromisoformat"),
        (42, "iterable of them"),
        ([date(2026, 8, 14), "2026-09-15"], "fromisoformat"),
    ],
    ids=["a string", "a number", "a list with a string in it"],
)
def test_holidays_that_are_not_dates_say_so(value, message):
    with pytest.raises(TypeError, match=message):
        calendars.US_NYSE.with_holidays(value)


def test_with_holidays_and_renamed_do_not_mutate_the_original():
    base = calendars.WEEKENDS_ONLY
    extended = base.with_holidays([date(2026, 8, 14)]).renamed("Acme")
    assert extended.is_holiday(date(2026, 8, 14))
    assert not base.is_holiday(date(2026, 8, 14))
    assert extended.name == "Acme"


def test_custom_weekend():
    cal = fasti.Calendar.custom("Gulf", weekend="fri_sat")
    assert cal.weekend == [fasti.Weekday.FRI, fasti.Weekday.SAT]
    assert not cal.is_business_day(date(2026, 7, 3))  # Friday
    assert cal.is_business_day(date(2026, 7, 5))  # Sunday


def test_weekend_accepts_weekday_members_and_iso_numbers():
    assert fasti.Calendar.custom("a", weekend=[fasti.Weekday.SUN]).weekend == [fasti.Weekday.SUN]
    assert fasti.Calendar.custom("b", weekend=[7]).weekend == [fasti.Weekday.SUN]


def test_weekday_zero_is_rejected_with_a_pointer_to_isoweekday():
    with pytest.raises(fasti.FastiError, match="isoweekday"):
        fasti.Calendar.custom("c", weekend=[0])
