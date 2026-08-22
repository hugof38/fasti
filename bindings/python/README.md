# fasti

[![PyPI](https://img.shields.io/pypi/v/fasti-py.svg)](https://pypi.org/project/fasti-py/)
[![CI](https://github.com/hugof38/fasti/actions/workflows/ci.yml/badge.svg)](https://github.com/hugof38/fasti/actions/workflows/ci.yml)
[![license](https://img.shields.io/badge/license-Apache--2.0%20OR%20MIT-blue.svg)](#license)

Dates, calendars, business-day conventions, and day-count fractions for
financial code — a Rust library with Python bindings, designed after
[QuantLib]'s `ql/time`.

Named for the *fasti*, the ancient Roman calendar that marked the *dies
fasti* — the days on which business could lawfully be conducted.

[QuantLib]: https://github.com/lballabio/QuantLib

```console
pip install fasti-py
```

The distribution is `fasti-py` — the name `fasti` was taken on PyPI by
an unrelated project — but the import is plain `fasti`:

```python
from datetime import date

import fasti
from fasti import calendars

nyse = calendars.US_NYSE
nyse.is_business_day(date(2026, 7, 3))   # False — July 4 observed
nyse.next_business_day(date(2026, 7, 3)) # datetime.date(2026, 7, 6)

schedule = fasti.Schedule(
    date(2025, 1, 15), date(2030, 1, 15), "6M", calendars.US_SETTLEMENT
)
for start, end in schedule.periods():
    print(start, end, fasti.year_fraction(start, end, "ACT/ACT ISDA"))
```

## It speaks datetime

Dates in are `datetime.date`. Dates out are always `datetime.date`.
There is no date class of ours to learn, convert through, or serialize.

Nothing else is a date, and the two near-misses are refused by name
rather than quietly converted:

```python
>>> import datetime, fasti
>>> fasti.calendars.US_NYSE.is_holiday("2026-07-03")
Traceback (most recent call last):
TypeError: expected a datetime.date, got a str; parse it first, ...
>>> fasti.calendars.US_NYSE.is_holiday(datetime.datetime(2026, 7, 3, 15, 30))
Traceback (most recent call last):
TypeError: expected a datetime.date, got a datetime; call .date() on it, ...
```

Both refusals are the same principle. A string has a format, and
`datetime.date.fromisoformat` is where formats belong — at the edge of
*your* program, where you know what arrived. A `datetime.datetime` has a
time, and often a timezone, so *which day it falls on* is precisely the
question being asked; truncating it here would answer it silently, under
UTC, in a library that has no concept of a zone. Write `.date()` and the
decision stays yours.

One caveat worth knowing: a type checker cannot help with the second
one. `datetime.datetime` is a subclass of `datetime.date`, so mypy sees
a valid argument where the runtime raises.

Year fractions come back as `fractions.Fraction`, because that is what
they are. `fasti` computes day-count fractions as reduced integer
rationals and never touches a float, so scaling a notional by an accrual
fraction stays exact:

```python
>>> from datetime import date
>>> fasti.year_fraction(date(2025, 1, 1), date(2025, 7, 1), "ACT/360")
Fraction(181, 360)
>>> 10_000_000 * fasti.year_fraction(date(2025, 1, 1), date(2025, 7, 1), "ACT/360")
Fraction(45250000, 9)
>>> float(fasti.year_fraction(date(2025, 1, 1), date(2025, 7, 1), "ACT/360"))
0.5027777777777778
```

Conventions, frequencies, weekdays and tenors are named with plain
strings, matched without regard to case or punctuation — `"ACT/360"`,
`"act 360"` and `"Actual_360"` are the same convention; `"6M"`,
`"6 months"` and `"semiannual"` are the same tenor. Where a name is not
recognized, the error says what was expected. The typed enums
(`fasti.BusinessDayConvention.MODIFIED_FOLLOWING`) are accepted
everywhere a string is, and are what the library hands back.

## What's in the box

| Area | API |
|---|---|
| Calendars | `Calendar`, `fasti.calendars.*` — TARGET, UK, US (settlement, NYSE, government bond, Fed, SOFR, NERC), France (settlement, exchange), plus `WEEKENDS_ONLY` and `NULL` |
| Open/closed | `is_business_day`, `is_holiday`, `is_weekend`, `business_days`, `count_business_days`, `holidays` |
| Rolling | `adjust`, `advance`, `next_business_day`, `prev_business_day`, `first_business_day_of_month`, `last_business_day_of_month` |
| Conventions | `BusinessDayConvention`: following, modified following, preceding, modified preceding, unadjusted |
| Day counts | `DayCount`, `year_fraction`, `day_count`: ACT/360, ACT/365F, 30/360 (bond basis, US, 30E/360, 30E/360 ISDA), ACT/ACT (ISDA, and schedule-aware ICMA) |
| Schedules | `Schedule`: forward/backward/zero generation, stubs, end-of-month, `periods()`, `reference_periods()` |
| Tenors | `Period`, `Frequency` |
| Custom calendars | `Calendar.custom`, `Calendar.union`, `Calendar.with_holidays`, `Calendar.with_rules`, and `Rule`: fixed dates with weekend-substitution policies, nth/last weekday, Easter offsets (Western and Orthodox), one-offs |

### Calendars

```python
>>> from datetime import date
>>> from fasti import calendars
>>> calendars.UK_SETTLEMENT.is_holiday(date(2021, 12, 28))  # Boxing Day's substitute
True
>>> joint = calendars.US_SETTLEMENT.union(calendars.FRANCE_SETTLEMENT)
>>> joint.is_holiday(date(2026, 7, 14)), joint.is_holiday(date(2026, 11, 26))
(True, True)
>>> calendars.names()
['TARGET', 'US.SETTLEMENT', 'US.NYSE', ...]
```

Weekend substitution is a calendar's decision, not a rule's: a holiday
rule names its natural date, and the calendar resolves which weekday it
is observed on — which is why UK Christmas on a Saturday sends Boxing
Day's substitute to the Tuesday.

```python
>>> import fasti
>>> from datetime import date
>>> acme = fasti.Calendar.custom(
...     "Acme",
...     weekend=["sat", "sun"],
...     rules=[fasti.Rule.fixed("Jun", 19, shift="us", from_year=2022)],
...     holidays=[date(2026, 8, 14)],
... )
>>> acme.is_holiday(date(2027, 6, 18))   # Juneteenth 2027 falls on a Saturday
True
```

Extending a built-in is a one-liner, and leaves the original alone —
every calendar is immutable, so each of these returns a new one:

```python
>>> from datetime import date
>>> from fasti import calendars
>>> nyse = calendars.US_NYSE
>>> closed = nyse.with_holidays(date(2026, 8, 14))          # one date, or a list
>>> closed.is_holiday(date(2026, 8, 14)), nyse.is_holiday(date(2026, 8, 14))
(True, False)
>>> import fasti
>>> nyse.with_rules(fasti.Rule.fixed("Oct", 31, shift="forward")).is_holiday(date(2026, 10, 31))
True
```

### Schedules and accrual

```python
>>> import fasti
>>> from datetime import date
>>> from fasti import calendars
>>> schedule = fasti.Schedule(
...     date(2025, 1, 15), date(2027, 1, 15), "6M", calendars.US_SETTLEMENT,
...     convention="modified_following",
...     termination_convention="unadjusted",
... )
>>> len(schedule), schedule[0], schedule[-1]
(5, datetime.date(2025, 1, 15), datetime.date(2027, 1, 15))
>>> icma = fasti.DayCount("ACT/ACT ICMA", schedule=schedule)
>>> sum(icma.year_fraction(a, b) for a, b in schedule.periods())
Fraction(2, 1)
```

A `Schedule` is a sequence: iterate it, index it, slice it. Bound to
`ACT/ACT ICMA`, it also supplies the reference periods a stub accrues
against.

## Notes

- **Supported range: 1901-01-01 to 2199-12-31.** A date outside it
  raises `fasti.FastiError`, which subclasses `ValueError`, so
  `except ValueError` catches everything this library refuses.
- **Date ranges are half-open**, as in `range()` and slicing:
  `business_days(start, end)` excludes `end`.
- **No clock, no timezones.** The library has no concept of "now"; you
  tell it the dates.
- **Holiday data is not a promise about the future.** Calendars encode
  published rules, not announcements. Rules that changed have effective
  years attached; one-off closures are one-offs.
- **Values compare and hash by what they are.** Two calendars are equal
  when they were built the same way — same built-in, same rules added in
  the same order — so `Calendar("nyse") == Calendar("US.NYSE")`, an
  unpickled calendar equals the one it came from, and any of these can
  be a dict key. It is not a comparison of the days two calendars call
  holidays: settling that means walking three centuries, which is not
  what `==` should cost.
- **Everything pickles**, so calendars, schedules and conventions can be
  handed to a `multiprocessing` worker, cached, or sent through `joblib`
  or `dask` like any other value. A value rebuilds by replaying the
  calls that made it, so a built-in calendar travels as its name and an
  unpickled schedule keeps the reference grid its stubs accrue against.
- **Immutable, and free-thread-safe.** Every object is frozen: methods
  return new values rather than mutating. The test suite runs under
  free-threaded CPython 3.14 in CI, and a range walk releases the
  interpreter while it runs, so threads are not blocked behind a century
  of holiday rules.
- Wheels are built against the stable ABI (`abi3`, CPython 3.10+), so
  one wheel per platform serves every supported Python version, with
  separate wheels for free-threaded 3.14 — the stable ABI does not cover
  free-threaded builds until `abi3t` in CPython 3.15. Linux gets both
  `manylinux` and `musllinux` builds; anything else falls back to the
  sdist, which needs a Rust toolchain.

## The Rust crate

This package wraps [`fasti`](https://crates.io/crates/fasti), a `no_std`
Rust library with no floating-point arithmetic anywhere and no panics in
library code. The Python layer adds the `datetime` boundary and nothing
else — the calendar data, the day-count math, and the invariants
(ACT-family additivity, adjustment idempotence, schedule monotonicity,
all property-tested) live in the crate.

## License

Dual-licensed under [Apache-2.0](licenses/LICENSE-APACHE) or
[MIT](licenses/LICENSE-MIT), at your option. The Easter-Monday lookup
tables are ported from QuantLib, under a permissive modified-BSD
license; see `THIRD-PARTY-NOTICES` in the repository.
