"""Time fasti against the other Python calendar and day-count libraries.

Run it to reproduce the numbers quoted for this package:

    pip install QuantLib holidays numpy pandas workalendar
    python benches/compare.py

Every single-call timing starts from a ``datetime.date``, because that is
what a caller actually holds. Where a library wants its own date type,
building one is part of the cost — QuantLib is timed both ways so the
split is visible. Libraries that are not installed are skipped, not
faked; a row that cannot be measured says so.

The comparison is only fair within one run of this file on one machine.
Do not read a number here against a number from somewhere else.
"""

from __future__ import annotations

import importlib.util
import platform
import statistics
import sys
import timeit

REPEAT = 9
SETUP = "import datetime as dt\nd = dt.date(2024, 7, 3)\n"


def have(module: str) -> bool:
    try:
        return importlib.util.find_spec(module) is not None
    except (ImportError, ValueError):
        return False


def time_ns(stmt: str, setup: str, number: int) -> tuple[float, float]:
    """Per-call nanoseconds: the minimum, and the median absolute deviation."""
    runs = [t / number * 1e9 for t in timeit.repeat(stmt, setup, number=number, repeat=REPEAT)]
    median = statistics.median(runs)
    spread = statistics.median(abs(r - median) for r in runs)
    return min(runs), spread


def row(
    label: str,
    requires: str | None,
    stmt: str,
    setup: str,
    number: int = 20_000,
    unit: str = "ns",
) -> None:
    """One measured line. A whole section shares a unit, so it reads as a column."""
    if requires and not have(requires):
        print(f"  {label:<36} {'skipped':>10}     ({requires} not installed)")
        return
    best, spread = time_ns(stmt, SETUP + setup, number)
    scale, places = (1e6, 2) if unit == "ms" else (1.0, 0)
    print(f"  {label:<36} {best / scale:>10.{places}f} {unit}   \u00b1{spread / scale:.{places}f}")


def heading(text: str) -> None:
    print()
    print(text)
    print("-" * len(text))


FASTI_CAL = "from fasti.calendars import us as _us; cal = _us.SETTLEMENT"
QL_CAL = "import QuantLib as ql; cal = ql.UnitedStates(ql.UnitedStates.Settlement)"
NP_CAL = (
    "import numpy as np, holidays; "
    "hd = sorted(holidays.US(years=range(1930, 2031))); "
    "cal = np.busdaycalendar(holidays=np.array(hd, dtype='datetime64[D]'))"
)

print(f"{platform.python_implementation()} {platform.python_version()} on {platform.machine()}")
print(f"best of {REPEAT} runs, ± is the median absolute deviation across runs")

heading("Is this a business day? US settlement calendar")
row("fasti", None, "cal.is_business_day(d)", FASTI_CAL)
row("QuantLib, from a datetime.date", "QuantLib", "cal.isBusinessDay(ql.Date(d.day, d.month, d.year))", QL_CAL)
row("QuantLib, ql.Date already in hand", "QuantLib", "cal.isBusinessDay(qd)", QL_CAL + "; qd = ql.Date(3, 7, 2024)")
row("holidays, plus a weekday test", "holidays",
    "d.weekday() < 5 and d not in cal", "import holidays; cal = holidays.US(years=range(2020, 2031))")
row("workalendar", "workalendar", "cal.is_working_day(d)",
    "from workalendar.usa import UnitedStates; cal = UnitedStates()", number=2_000)
row("numpy is_busday, one date", "numpy", "np.is_busday(d, busdaycal=cal)", NP_CAL)

heading("Roll to the next business day (Following)")
row("fasti", None, "cal.adjust(d, 'following')", FASTI_CAL)
row("QuantLib", "QuantLib", "cal.adjust(ql.Date(d.day, d.month, d.year), ql.Following)", QL_CAL)
row("numpy busday_offset", "numpy", "np.busday_offset(d, 0, roll='forward', busdaycal=cal)", NP_CAL)

heading("Advance one month, ModifiedFollowing, end of month preserved")
row("fasti", None, "cal.advance(d, p, 'modifiedfollowing', end_of_month=True)",
    FASTI_CAL + "; from fasti import Period; p = Period.months(1)")
row("QuantLib", "QuantLib", "cal.advance(ql.Date(d.day, d.month, d.year), p, ql.ModifiedFollowing, True)",
    QL_CAL + "; p = ql.Period(1, ql.Months)")

heading("Year fraction, ACT/ACT ISDA, across a year boundary")
DATES = "a = dt.date(2024, 11, 15); b = dt.date(2025, 5, 15)\n"
row("fasti, exact Fraction", None, "dc.year_fraction(a, b)",
    DATES + "from fasti import DayCount; dc = DayCount.act_act_isda()")
row("QuantLib, float", "QuantLib",
    "dc.yearFraction(ql.Date(a.day, a.month, a.year), ql.Date(b.day, b.month, b.year))",
    DATES + "import QuantLib as ql; dc = ql.ActualActual(ql.ActualActual.ISDA)")

heading("Every business day of a century, 1930-01-01 to 2029-12-31 (36,524 days)")
SPAN = "a = dt.date(1930, 1, 1); b = dt.date(2029, 12, 31)\n"
row("fasti business_days()", None, "cal.business_days(a, b)", SPAN + FASTI_CAL, 20, "ms")
row("QuantLib businessDayList()", "QuantLib", "ql.Calendar.businessDayList(cal, qa, qb)",
    QL_CAL + "; qa = ql.Date(1, 1, 1930); qb = ql.Date(31, 12, 2029)", 20, "ms")
row("numpy is_busday over the span", "numpy", "days[np.is_busday(days, busdaycal=cal)]",
    NP_CAL + "; days = np.arange('1930-01-01', '2030-01-01', dtype='datetime64[D]')", 20, "ms")

heading("Every holiday of the same century")
row("fasti holidays()", None, "cal.holidays(a, b)", SPAN + FASTI_CAL, 20, "ms")
row("QuantLib holidayList()", "QuantLib", "ql.Calendar.holidayList(cal, qa, qb, False)",
    QL_CAL + "; qa = ql.Date(1, 1, 1930); qb = ql.Date(31, 12, 2029)", 20, "ms")

heading("What the boundary costs, against the work behind it")
row("a call that carries no date", None, "p.length()", "from fasti import Period; p = Period.months(1)")
row("one date in, weekend mask only", None, "cal.is_weekend(d)", FASTI_CAL)
row("one date in and one out, no rules", None, "cal.adjust(d, 'unadjusted')", FASTI_CAL)
row("the same, with the rules walked", None, "cal.adjust(d, 'following')", FASTI_CAL)

heading("Naming a convention: the string spelling against the member")
row("adjust, convention as a string", None, 'cal.adjust(d, "following")', FASTI_CAL)
row("adjust, convention as the member", None, "cal.adjust(d, c)",
    FASTI_CAL + "; from fasti import BusinessDayConvention as B; c = B.FOLLOWING")
row("advance, convention as a string", None,
    'cal.advance(d, p, "modifiedfollowing", end_of_month=True)',
    FASTI_CAL + "; from fasti import Period; p = Period.months(1)")
row("advance, convention as the member", None,
    "cal.advance(d, p, c, end_of_month=True)",
    FASTI_CAL + "; from fasti import Period, BusinessDayConvention as B; "
    "p = Period.months(1); c = B.MODIFIED_FOLLOWING")

heading("Exactness of the same ACT/ACT ISDA fraction")
if have("QuantLib"):
    import datetime as dt
    from decimal import Decimal, getcontext

    import QuantLib as ql

    from fasti import DayCount

    getcontext().prec = 40
    a, b = dt.date(2024, 11, 15), dt.date(2025, 5, 15)
    exact = DayCount.act_act_isda().year_fraction(a, b)
    true_value = Decimal(exact.numerator) / Decimal(exact.denominator)
    theirs = ql.ActualActual(ql.ActualActual.ISDA).yearFraction(
        ql.Date(15, 11, 2024), ql.Date(15, 5, 2025)
    )
    print(f"  true value      {true_value}")
    print(f"  fasti           {exact!r}, i.e. {float(exact)!r}")
    print(f"    off by        {abs(Decimal(float(exact)) - true_value):.3E}")
    print(f"  QuantLib        {theirs!r}")
    print(f"    off by        {abs(Decimal(theirs) - true_value):.3E}")
else:
    print("  skipped (QuantLib not installed)")

print()
sys.stdout.flush()
