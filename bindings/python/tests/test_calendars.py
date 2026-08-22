"""Built-in calendars against published holiday lists, and the calendar
methods that read them."""

import datetime
from collections.abc import Iterable

import pytest

from fasti import Calendar, FastiError, Period, Rule, Weekday
from fasti.calendars import NULL_CALENDAR, TARGET, WEEKENDS_ONLY, france, uk, us


def days(*iso: str) -> list[datetime.date]:
    return [datetime.date.fromisoformat(text) for text in iso]


def year_of(calendar: Calendar, year: int) -> list[datetime.date]:
    return calendar.holidays(datetime.date(year, 1, 1), datetime.date(year + 1, 1, 1))


# Sources: opm.gov (US federal), nyse.com (2024 holidays), SIFMA's 2024
# recommended close list, gov.uk bank holidays, ECB TARGET closing days,
# service-public.fr (jours fériés).
PUBLISHED = {
    "us.SETTLEMENT": (
        us.SETTLEMENT,
        days(
            "2024-01-01", "2024-01-15", "2024-02-19", "2024-05-27", "2024-06-19",
            "2024-07-04", "2024-09-02", "2024-10-14", "2024-11-11", "2024-11-28",
            "2024-12-25",
        ),
    ),
    "us.NYSE": (
        us.NYSE,
        days(
            "2024-01-01", "2024-01-15", "2024-02-19", "2024-03-29", "2024-05-27",
            "2024-06-19", "2024-07-04", "2024-09-02", "2024-11-28", "2024-12-25",
        ),
    ),
    "us.GOVERNMENT_BOND": (
        us.GOVERNMENT_BOND,
        days(
            "2024-01-01", "2024-01-15", "2024-02-19", "2024-03-29", "2024-05-27",
            "2024-06-19", "2024-07-04", "2024-09-02", "2024-10-14", "2024-11-11",
            "2024-11-28", "2024-12-25",
        ),
    ),
    "us.NERC": (
        us.NERC,
        days("2024-01-01", "2024-05-27", "2024-07-04", "2024-09-02", "2024-11-28",
             "2024-12-25"),
    ),
    "uk.SETTLEMENT": (
        uk.SETTLEMENT,
        days(
            "2024-01-01", "2024-03-29", "2024-04-01", "2024-05-06", "2024-05-27",
            "2024-08-26", "2024-12-25", "2024-12-26",
        ),
    ),
    "france.SETTLEMENT": (
        france.SETTLEMENT,
        days(
            "2024-01-01", "2024-04-01", "2024-05-01", "2024-05-08", "2024-05-09",
            "2024-05-20", "2024-07-14", "2024-08-15", "2024-11-01", "2024-11-11",
            "2024-12-25",
        ),
    ),
    "TARGET": (
        TARGET,
        days("2024-01-01", "2024-03-29", "2024-04-01", "2024-05-01", "2024-12-25",
             "2024-12-26"),
    ),
}


@pytest.mark.parametrize("name", sorted(PUBLISHED))
def test_a_full_year_matches_the_published_list(name: str) -> None:
    calendar, published = PUBLISHED[name]
    assert year_of(calendar, 2024) == published


def test_uk_2025_matches_gov_uk() -> None:
    assert year_of(uk.SETTLEMENT, 2025) == days(
        "2025-01-01", "2025-04-18", "2025-04-21", "2025-05-05", "2025-05-26",
        "2025-08-25", "2025-12-25", "2025-12-26",
    )


def test_the_weekend_shift_decides_which_substitute_a_market_takes() -> None:
    # July 4 2026 is a Saturday. The federal convention steps back to the
    # Friday; the Fed's own does not move a Saturday at all.
    saturday, friday = datetime.date(2026, 7, 4), datetime.date(2026, 7, 3)
    assert us.SETTLEMENT.is_holiday(saturday)
    assert us.SETTLEMENT.is_holiday(friday)
    assert us.FEDERAL_RESERVE.is_holiday(saturday)
    assert not us.FEDERAL_RESERVE.is_holiday(friday)
    # France grants no substitute at all: a weekend holiday is lost.
    assert not france.SETTLEMENT.is_holiday(datetime.date(2026, 5, 2))


def test_christmas_on_a_saturday_gives_the_uk_two_substitutes() -> None:
    assert uk.SETTLEMENT.holidays(
        datetime.date(2021, 12, 24), datetime.date(2022, 1, 1)
    ) == days("2021-12-25", "2021-12-26", "2021-12-27", "2021-12-28")


def test_sofr_observes_good_friday_where_the_bond_market_may_not() -> None:
    # 2015: Good Friday fell on a payrolls day, so the bond market opened.
    good_friday = datetime.date(2015, 4, 3)
    assert us.SOFR.is_holiday(good_friday)
    assert not us.GOVERNMENT_BOND.is_holiday(good_friday)


def test_business_days_and_holidays_partition_the_weekdays() -> None:
    start, end = datetime.date(2024, 7, 1), datetime.date(2024, 8, 1)
    business = us.SETTLEMENT.business_days(start, end)
    holidays = us.SETTLEMENT.holidays(start, end)
    assert len(business) == 22
    assert holidays == [datetime.date(2024, 7, 4)]
    weekdays = [
        start + datetime.timedelta(days=n)
        for n in range((end - start).days)
        if (start + datetime.timedelta(days=n)).weekday() < 5
    ]
    assert sorted(business + holidays) == weekdays


def test_the_range_is_half_open() -> None:
    fourth = datetime.date(2024, 7, 4)
    assert us.SETTLEMENT.holidays(fourth, fourth + datetime.timedelta(days=1)) == [fourth]
    assert us.SETTLEMENT.holidays(fourth, fourth) == []


def test_a_century_of_business_days_is_walked_in_rust() -> None:
    start, end = datetime.date(1926, 1, 1), datetime.date(2026, 1, 1)
    walked = us.NYSE.business_days(start, end)
    # The detached walk has to agree with the per-call predicate, day
    # for day, over the whole century.
    one_at_a_time = [
        day
        for n in range((end - start).days)
        if us.NYSE.is_business_day(day := start + datetime.timedelta(days=n))
    ]
    assert walked == one_at_a_time
    assert walked[0] == datetime.date(1926, 1, 4)


@pytest.mark.parametrize(
    ("convention", "expected"),
    [
        ("unadjusted", "2024-07-04"),
        ("following", "2024-07-05"),
        ("preceding", "2024-07-03"),
        ("modified following", "2024-07-05"),
        ("modified preceding", "2024-07-03"),
    ],
)
def test_adjust_covers_every_convention(convention: str, expected: str) -> None:
    assert us.SETTLEMENT.adjust(datetime.date(2024, 7, 4), convention) == (
        datetime.date.fromisoformat(expected)
    )


def test_modified_conventions_do_not_cross_a_month_boundary() -> None:
    sunday = datetime.date(2025, 8, 31)
    assert WEEKENDS_ONLY.adjust(sunday, "following") == datetime.date(2025, 9, 1)
    assert WEEKENDS_ONLY.adjust(sunday, "modified following") == datetime.date(2025, 8, 29)
    saturday = datetime.date(2025, 3, 1)
    assert WEEKENDS_ONLY.adjust(saturday, "preceding") == datetime.date(2025, 2, 28)
    assert WEEKENDS_ONLY.adjust(saturday, "modified preceding") == datetime.date(2025, 3, 3)


def test_advance_steps_then_rolls_and_can_keep_month_ends() -> None:
    end_of_april = datetime.date(2025, 4, 30)
    assert WEEKENDS_ONLY.advance(end_of_april, Period.months(1), "modified following") == (
        datetime.date(2025, 5, 30)
    )
    assert WEEKENDS_ONLY.advance(
        end_of_april, Period.months(1), "modified following", True
    ) == datetime.date(2025, 5, 30)
    assert WEEKENDS_ONLY.advance(
        datetime.date(2025, 1, 31), Period.months(1), "modified following", True
    ) == datetime.date(2025, 2, 28)
    # A frequency names its period, as the crate's with_frequency does.
    assert WEEKENDS_ONLY.advance(
        datetime.date(2025, 1, 6), "weekly", "following"
    ) == datetime.date(2025, 1, 13)


def test_month_edges_and_neighbours() -> None:
    march = datetime.date(2026, 3, 18)
    assert us.SETTLEMENT.first_business_day_of_month(march) == datetime.date(2026, 3, 2)
    assert us.SETTLEMENT.last_business_day_of_month(march) == datetime.date(2026, 3, 31)
    fourth = datetime.date(2024, 7, 4)
    assert us.SETTLEMENT.next_business_day(fourth) == datetime.date(2024, 7, 5)
    assert us.SETTLEMENT.prev_business_day(fourth) == datetime.date(2024, 7, 3)
    assert us.SETTLEMENT.prev_business_day(datetime.date(1901, 1, 1)) is None


def test_the_baseline_calendars_are_the_two_extremes() -> None:
    sunday = datetime.date(2024, 7, 7)
    assert not WEEKENDS_ONLY.is_business_day(sunday)
    assert WEEKENDS_ONLY.is_business_day(datetime.date(2024, 7, 4))
    assert NULL_CALENDAR.is_business_day(sunday)
    assert NULL_CALENDAR.adjust(sunday, "modified following") == sunday
    assert NULL_CALENDAR.weekend == ()


def test_a_calendar_written_in_python() -> None:
    acme = Calendar(
        "Acme",
        ["sat", "sun"],
        [
            Rule.fixed(7, 4, shift="sat_back_sun_forward"),
            Rule.nth_weekday(4, "thu", 11),
            Rule.last_weekday("mon", 5),
            Rule.easter(-2),
            Rule.one_off(datetime.date(2026, 8, 3)),
        ],
    )
    assert acme.name == "Acme"
    assert acme.weekend == (Weekday.SAT, Weekday.SUN)
    assert year_of(acme, 2026) == days(
        "2026-04-03", "2026-05-25", "2026-07-03", "2026-07-04", "2026-08-03", "2026-11-26"
    )


def test_a_rule_can_be_restricted_to_a_span_of_years() -> None:
    juneteenth = Calendar("Acme", ["sat", "sun"], [Rule.fixed(6, 19, years=(2021, None))])
    assert not juneteenth.is_holiday(datetime.date(2020, 6, 19))
    assert juneteenth.is_holiday(datetime.date(2021, 6, 19))
    ended = Calendar("Acme", ["sat", "sun"], [Rule.fixed(6, 19, years=(None, 2022))])
    assert ended.is_holiday(datetime.date(2022, 6, 19))
    assert not ended.is_holiday(datetime.date(2023, 6, 19))


def test_orthodox_easter_is_its_own_computus() -> None:
    western = Calendar("W", (), [Rule.easter(0)])
    orthodox = Calendar("O", (), [Rule.easter(0, method="orthodox")])
    assert western.is_holiday(datetime.date(2024, 3, 31))
    assert orthodox.is_holiday(datetime.date(2024, 5, 5))


def test_derivations_compose() -> None:
    joint = us.SETTLEMENT.union(france.SETTLEMENT)
    assert joint.is_holiday(datetime.date(2026, 7, 14))
    assert joint.is_holiday(datetime.date(2026, 11, 26))
    assert joint.name == "US settlement + France settlement"
    gulf = joint.with_weekend(["fri", "sat"]).with_name("Acme")
    assert gulf.name == "Acme"
    assert gulf.weekend == (Weekday.FRI, Weekday.SAT)
    assert not gulf.is_business_day(datetime.date(2026, 7, 3))
    assert gulf.is_business_day(datetime.date(2026, 7, 5))


def test_a_weekend_accepts_any_spelling_of_a_weekday() -> None:
    spellings: Iterable[object] = ["sunday", Weekday.SAT]
    assert Calendar("Acme", spellings).weekend == (Weekday.SAT, Weekday.SUN)  # type: ignore[arg-type]
    with pytest.raises(FastiError, match="unknown Weekday"):
        Calendar("Acme", ["someday"])
    with pytest.raises(TypeError, match="expected a Weekday"):
        Calendar("Acme", [6])  # type: ignore[list-item]
