# fasti-py

[![PyPI](https://img.shields.io/pypi/v/fasti-py.svg)](https://pypi.org/project/fasti-py/)

Python bindings for [fasti](https://github.com/hugof38/fasti) — dates,
calendars, business-day conventions and day-count fractions for
financial code, in native Rust with no floating-point arithmetic
anywhere.

The distribution is `fasti-py` because `fasti` on PyPI is an unrelated
project. The import name is `fasti`.

```console
$ pip install fasti-py
```

Wheels are `abi3` from CPython 3.10 up, so one wheel per platform covers
every version, and a new CPython needs no new wheel.

## The boundary

This is a translation layer, not a second library. Every name maps to
one in the crate, with the same method names, the same semantics and the
same argument order.

- **Dates in and out are `datetime.date`, and nothing else.** A `str` is
  refused, naming `datetime.date.fromisoformat`. A `datetime.datetime`
  is refused too: which day a moment falls on is a time-zone decision,
  and this library has no time-zone concept — call `.date()` yourself.
- **Year fractions come back as `fractions.Fraction`.** Never a float.
  The crate is float-free, and that has to survive the boundary.
- **Errors are `FastiError`, a `ValueError`.** Type mistakes are
  `TypeError`.

```python
>>> import datetime
>>> from fasti.calendars import us
>>> us.SETTLEMENT.is_holiday("2024-07-04")
Traceback (most recent call last):
TypeError: fasti takes a datetime.date, not a str: ...
>>> us.SETTLEMENT.is_holiday(datetime.datetime(2024, 7, 4, 23, 30))
Traceback (most recent call last):
TypeError: fasti takes a datetime.date, not a datetime.datetime: ...

```

- **Values are immutable.** Every mutator returns a new value; every
  value compares, hashes and pickles.

The operators the crate defines are spelled the Python way, and only
those: a date steps by a period, and the two vocabularies the crate
orders are the two that sort.

```python
>>> import datetime
>>> from fasti import Frequency, Period, Weekday
>>> datetime.date(2026, 1, 15) + Period.months(6)
datetime.date(2026, 7, 15)
>>> datetime.date(2026, 1, 31) + Period.months(1)   # Add clamps; it never snaps
datetime.date(2026, 2, 28)
>>> sorted([Weekday.SUN, Weekday.MON]), Frequency.ANNUAL < Frequency.MONTHLY
([Weekday.MON, Weekday.SUN], True)

```

End-of-month preservation is `Calendar.advance`'s business, not `+`'s —
the same split the crate makes between `Add` and `Date::advance`. A
`BusinessDayConvention` refuses `<` because the crate gives it no order.

## Quickstart

```python
>>> import datetime
>>> from fasti import DayCount, Period, Rule, Schedule
>>> from fasti.calendars import us

>>> us.SETTLEMENT.is_business_day(datetime.date(2024, 7, 4))
False
>>> us.SETTLEMENT.adjust(datetime.date(2024, 7, 4), "following")
datetime.date(2024, 7, 5)
>>> us.SETTLEMENT.advance(datetime.date(2025, 1, 31), Period.months(1),
...                       "modified following", True)
datetime.date(2025, 2, 28)

>>> schedule = Schedule(datetime.date(2025, 1, 15), datetime.date(2026, 1, 15),
...                     "semiannual", us.GOVERNMENT_BOND)
>>> schedule.dates()
[datetime.date(2025, 1, 15), datetime.date(2025, 7, 15), datetime.date(2026, 1, 15)]

>>> DayCount.act_act_icma("semiannual").bind(schedule).year_fraction(
...     datetime.date(2025, 1, 15), datetime.date(2025, 4, 15))
Fraction(45, 181)

```

A `Schedule` is its coupon dates: it has a length, indexes, slices,
iterates and reverses, because the crate's schedule derefs to `[Date]`.
Slicing gives you the dates; `after` and `until` are what give you back a
schedule, since a bare run of dates names no lattice.

```python
>>> schedule[1:], schedule[-1], len(schedule)
([datetime.date(2025, 7, 15), datetime.date(2026, 1, 15)], datetime.date(2026, 1, 15), 3)

```

Calendars compose:

```python
>>> from fasti.calendars import france, us
>>> joint = us.SETTLEMENT.union(france.SETTLEMENT)
>>> joint.is_holiday(datetime.date(2026, 7, 14)), joint.is_holiday(datetime.date(2026, 11, 26))
(True, True)
>>> quiet = joint.with_rule(Rule.one_off(datetime.date(2026, 8, 3))).with_name("Acme")
>>> quiet.name
'Acme'

```

## Reading the API

One rule settles what is a property and what is a method: **a field in the
crate is a property here, a method there is a method here.** So
`calendar.name` and `period.unit` are properties, while
`day_count.name()`, `period.length()` and `frequency.per_year()` are
calls. It is not the usual Python instinct — the usual instinct is that
anything cheap is a property — but it means you can read the Rust docs and
know what to type without a second table.

## Vocabularies

Conventions, weekdays, frequencies, weekend shifts, Easter methods and
generation rules are classes with constants — and every argument that
takes one also takes a string. Matching ignores case and punctuation, so
`"ModifiedFollowing"`, `"modified_following"` and `"modified following"`
are one spelling. The canonical spelling is what prints, pickles and
appears in errors.

```python
>>> from fasti import BusinessDayConvention, WeekendShift
>>> BusinessDayConvention("mod_following") is BusinessDayConvention.MODIFIED_FOLLOWING
False
>>> BusinessDayConvention("mod_following") == BusinessDayConvention.MODIFIED_FOLLOWING
True
>>> WeekendShift("fed")
WeekendShift.SUN_FORWARD

```

`"fed"` is a spelling of `SUN_FORWARD`, the Federal Reserve and SIFMA
convention. `"federal"` is a spelling of nothing: the US federal
convention is `SAT_BACK_SUN_FORWARD`, and one word standing for two
different answers would be a trap.

## Equality and pickling

`Calendar` and `Rule` compare **structurally** — two values are equal
when they were built the same way. That is not "these two agree on every
date": settling that means walking 1901 through 2199. (`Rule` cannot
derive equality in the crate at all, since its escape hatch holds a
function pointer.)

Everything pickles. A built-in calendar travels as its registry name,
because it holds those function-pointer rules; a derived one travels as
the operations applied to it; a generated `Schedule` replays its
generation arguments, because its stub reference grid is not recoverable
from the dates alone.

## Typing

The package ships type stubs and `py.typed`. Date positions are spelled
`datetime.date`.

A type checker cannot reject a `datetime.datetime` where a
`datetime.date` is wanted — `datetime` subclasses `date`, so it type-checks.
The runtime rejects it.

## What is not here

- `Rule.Custom`. The crate's escape hatch is a bare `fn(Date) -> bool`
  pointer, which no Python callable can be.
- `Date`, `Year`, `Month`, `Ordinal`, `Weekend`, `Fraction`. Python
  already has `datetime.date` and `fractions.Fraction`; months, years
  and ordinals are `int`s; a weekend is a sequence of `Weekday`.

## Development

See [CONTRIBUTING.md](../../CONTRIBUTING.md). In short:

```console
$ uv sync            # or: pip install maturin pytest mypy
$ maturin develop
$ pytest && mypy
```

## License

`Apache-2.0 OR MIT`, the same as the crate.
