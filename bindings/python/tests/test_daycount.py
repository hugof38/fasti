"""Day counts as exact fractions."""

import datetime
from fractions import Fraction

import pytest

from fasti import DayCount, FastiError, Schedule
from fasti.calendars import WEEKENDS_ONLY, us

JAN = datetime.date(2025, 1, 1)
APR = datetime.date(2025, 4, 1)
JUL = datetime.date(2025, 7, 1)


@pytest.mark.parametrize(
    ("convention", "name"),
    [
        (DayCount.act_360(), "Actual/360"),
        (DayCount.act_365_fixed(), "Actual/365 (Fixed)"),
        (DayCount.act_act_isda(), "Actual/Actual (ISDA)"),
        (DayCount.act_act_icma(), "Actual/Actual (ICMA)"),
        (DayCount.thirty_360_bond(), "30/360 (Bond Basis)"),
        (DayCount.thirty_360_us(), "30/360 (US)"),
        (DayCount.thirty_360_european(), "30E/360 (Eurobond Basis)"),
        (DayCount.thirty_360_isda(datetime.date(2025, 12, 31)), "30E/360 (ISDA)"),
    ],
)
def test_every_convention_names_itself(convention: DayCount, name: str) -> None:
    assert convention.name() == name


def test_the_simple_bases_are_exact() -> None:
    assert DayCount.act_360().year_fraction(JAN, APR) == Fraction(90, 360)
    assert DayCount.act_365_fixed().year_fraction(JAN, APR) == Fraction(90, 365)
    assert DayCount.act_360().day_count(JAN, APR) == 90


def test_thirty_360_counts_its_own_days() -> None:
    end_of_february = datetime.date(2025, 2, 28)
    assert DayCount.thirty_360_bond().day_count(datetime.date(2025, 1, 31), end_of_february) == 28
    assert DayCount.thirty_360_european().day_count(
        datetime.date(2025, 1, 31), end_of_february
    ) == 28
    assert DayCount.thirty_360_bond().year_fraction(JAN, JUL) == Fraction(1, 2)


def test_act_act_isda_splits_a_year_boundary_the_way_the_paper_does() -> None:
    # ISDA's Actual/Actual paper, the semi-annual example.
    value = DayCount.act_act_isda().year_fraction(
        datetime.date(2003, 11, 1), datetime.date(2004, 5, 1)
    )
    assert value == Fraction(61, 365) + Fraction(121, 366)


def test_act_act_icma_accrues_a_stub_against_its_notional_grid() -> None:
    # The ICMA worked example: a long first coupon from 2002-08-15, whose
    # reference period starts one tenor before the first regular date.
    schedule = Schedule(
        datetime.date(2002, 8, 15), datetime.date(2004, 1, 15), "semiannual", WEEKENDS_ONLY
    )
    stub = schedule.periods()[0]
    assert stub.reference == (datetime.date(2002, 7, 15), datetime.date(2003, 1, 15))
    icma = DayCount.act_act_icma()
    assert icma.accrual_fraction(stub) == Fraction(153, 368)
    assert icma.year_fraction_with_reference(
        stub.start, stub.end, *stub.reference, "semiannual"
    ) == Fraction(153, 368)


def test_icma_ignores_the_accrual_adjustment() -> None:
    # The same bond accrued modified-following or unadjusted: ICMA reads
    # only the unadjusted lattice, so it gives identical fractions.
    def build(convention: str) -> Schedule:
        return Schedule(
            datetime.date(2024, 5, 2),
            datetime.date(2029, 9, 15),
            "semiannual",
            us.SETTLEMENT,
            convention=convention,
        )

    icma = DayCount.act_act_icma()
    unadjusted, adjusted = build("unadjusted"), build("modified following")
    assert unadjusted.dates() != adjusted.dates()
    assert [icma.accrual_fraction(p) for p in unadjusted.periods()] == [
        icma.accrual_fraction(p) for p in adjusted.periods()
    ]


def test_every_convention_measures_a_period_but_icma_only_a_period() -> None:
    schedule = Schedule.from_dates([JAN, JUL])
    period = schedule.periods()[0]
    assert DayCount.act_360().accrual_fraction(period) == DayCount.act_360().year_fraction(
        JAN, JUL
    )
    with pytest.raises(FastiError, match="frequency"):
        DayCount.act_act_icma().accrual_fraction(period)
    framed = Schedule.from_dates([JAN, JUL], "semiannual").periods()[0]
    assert DayCount.act_act_icma().accrual_fraction(framed) == Fraction(1, 2)
    with pytest.raises(FastiError, match="accrual_fraction"):
        DayCount.act_act_icma().year_fraction(JAN, JUL)
    with pytest.raises(FastiError, match="escape hatch"):
        DayCount.act_360().year_fraction_with_reference(JAN, APR, JAN, JUL, "semiannual")


@pytest.mark.parametrize(
    "convention",
    [
        DayCount.act_360(),
        DayCount.act_365_fixed(),
        DayCount.act_act_isda(),
        DayCount.thirty_360_bond(),
        DayCount.thirty_360_european(),
    ],
)
def test_a_day_to_itself_is_zero_and_reversal_negates(convention: DayCount) -> None:
    assert convention.year_fraction(JAN, JAN) == 0
    assert convention.year_fraction(APR, JAN) == -convention.year_fraction(JAN, APR)
    assert convention.day_count(APR, JAN) == -convention.day_count(JAN, APR)


@pytest.mark.parametrize(
    "convention", [DayCount.act_360(), DayCount.act_365_fixed(), DayCount.act_act_isda()]
)
def test_the_act_family_is_additive_across_a_split(convention: DayCount) -> None:
    assert convention.year_fraction(JAN, APR) + convention.year_fraction(APR, JUL) == (
        convention.year_fraction(JAN, JUL)
    )


def test_thirty_360_is_deliberately_not_additive() -> None:
    convention = DayCount.thirty_360_bond()
    split = datetime.date(2025, 1, 31)
    assert convention.year_fraction(JAN, split) + convention.year_fraction(split, APR) != (
        convention.year_fraction(JAN, APR)
    )
