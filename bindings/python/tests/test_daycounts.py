"""Day-count conventions, as exact fractions."""

from fractions import Fraction

import pytest

import fasti
from fasti import DayCount


def test_act_360_and_365_over_a_leap_year():
    assert DayCount("ACT/360").year_fraction("2024-01-01", "2025-01-01") == Fraction(366, 360)
    assert DayCount("ACT/365F").year_fraction("2024-01-01", "2025-01-01") == Fraction(366, 365)
    assert DayCount("ACT/ACT ISDA").year_fraction("2024-01-01", "2025-01-01") == 1


def test_direction_is_signed_and_equal_dates_are_zero():
    dc = DayCount("ACT/360")
    assert dc.year_fraction("2025-07-01", "2025-01-01") == -dc.year_fraction(
        "2025-01-01", "2025-07-01"
    )
    assert dc.year_fraction("2025-01-01", "2025-01-01") == 0


def test_act_family_is_additive_across_a_split():
    dc = DayCount("ACT/ACT ISDA")
    whole = dc.year_fraction("2023-11-01", "2024-05-01")
    parts = dc.year_fraction("2023-11-01", "2024-01-01") + dc.year_fraction(
        "2024-01-01", "2024-05-01"
    )
    assert whole == parts


def test_thirty_360_counts_its_own_days():
    assert DayCount("30/360").day_count("2025-01-31", "2025-02-28") == 28
    assert DayCount("30E/360").day_count("2025-01-31", "2025-02-28") == 28
    assert DayCount("ACT/360").day_count("2025-01-31", "2025-02-28") == 28
    assert DayCount("30/360").year_fraction("2025-01-01", "2025-07-01") == Fraction(1, 2)


@pytest.mark.parametrize(
    ("alias", "canonical"),
    [
        ("act/360", "ACT/360"),
        ("Actual/360", "ACT/360"),
        ("ACT_360", "ACT/360"),
        ("actual 365 fixed", "ACT/365F"),
        ("Bond Basis", "30/360"),
        ("30E/360", "30E/360"),
        ("Actual/Actual (ISDA)", "ACT/ACT ISDA"),
    ],
)
def test_name_aliases_resolve_to_the_same_convention(alias, canonical):
    assert DayCount(alias) == DayCount(canonical)
    assert DayCount(alias).name == DayCount(canonical).name


def test_unknown_convention_raises():
    with pytest.raises(fasti.FastiError, match="unknown day-count convention"):
        DayCount("ACT/366")


def test_thirty_e_360_isda_needs_a_termination_date():
    with pytest.raises(fasti.FastiError, match="termination"):
        DayCount("30E/360 ISDA")
    dc = DayCount("30E/360 ISDA", termination="2030-02-28")
    assert dc.year_fraction("2025-01-01", "2025-07-01") == Fraction(1, 2)


def test_act_act_icma_needs_a_frequency():
    with pytest.raises(fasti.FastiError, match="frequency"):
        DayCount("ACT/ACT ICMA")
    dc = DayCount("ACT/ACT ICMA", frequency="semiannual")
    assert dc.frequency == fasti.Frequency.SEMIANNUAL
    # Unbound, an accrual counts as one whole coupon period.
    assert dc.year_fraction("2025-01-15", "2025-07-15") == Fraction(1, 2)


def test_act_act_icma_bound_to_a_schedule_accrues_by_reference_period():
    schedule = fasti.Schedule("2025-01-15", "2027-01-15", "6M")
    dc = DayCount("ACT/ACT ICMA", schedule=schedule)
    assert dc.frequency == fasti.Frequency.SEMIANNUAL
    assert [dc.year_fraction(a, b) for a, b in schedule.periods()] == [Fraction(1, 2)] * 4
    assert sum(dc.year_fraction(a, b) for a, b in schedule.periods()) == 2


def test_bind_attaches_a_schedule_after_the_fact():
    schedule = fasti.Schedule("2025-01-15", "2026-01-15", "6M")
    bound = DayCount("ACT/ACT ICMA", frequency="semiannual").bind(schedule)
    # 90 days into a 181-day semiannual coupon: 90 / (2 * 181).
    assert bound.year_fraction("2025-01-15", "2025-04-15") == Fraction(45, 181)


def test_module_level_helpers_match_the_class():
    assert fasti.year_fraction("2025-01-01", "2025-07-01", "ACT/360") == DayCount(
        "ACT/360"
    ).year_fraction("2025-01-01", "2025-07-01")
    assert fasti.day_count("2025-01-31", "2025-02-28", "30/360") == 28
    assert fasti.year_fraction(
        "2025-01-01", "2025-07-01", DayCount("ACT/360")
    ) == Fraction(181, 360)
    assert fasti.year_fraction(
        "2025-01-15", "2025-07-15", "ACT/ACT ICMA", frequency="semiannual"
    ) == Fraction(1, 2)
