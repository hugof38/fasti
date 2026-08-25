# fasti

[![CI](https://github.com/hugof38/fasti/actions/workflows/ci.yml/badge.svg)](https://github.com/hugof38/fasti/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/fasti.svg)](https://crates.io/crates/fasti)
[![docs.rs](https://img.shields.io/docsrs/fasti)](https://docs.rs/fasti)
[![license](https://img.shields.io/badge/license-Apache--2.0%20OR%20MIT-blue.svg)](#license)

Dates, calendars, business-day conventions, and day-count fractions for
financial code — a native Rust library, `no_std` and free of
floating-point arithmetic, designed after [QuantLib]'s `ql/time`.

Named for the *fasti*, the ancient Roman calendar that marked the *dies
fasti* — the days on which business could lawfully be conducted.

[QuantLib]: https://github.com/lballabio/QuantLib

## Why

Financial code needs to answer questions like *"when is the next coupon
date?"*, *"how many days of interest accrued?"*, and *"is the market open
on Friday?"* — precisely, deterministically, and without floating-point
drift. General-purpose date libraries answer none of these; QuantLib
answers all of them but brings C++ and `double`s. `fasti` covers the
same capability surface with:

- **No float arithmetic — anywhere.** Day-count fractions are integer
  rationals (`Fraction`, an `i64/u64` reduced fraction). Scaling money
  by an accrual fraction stays exact.
- **`no_std` + `alloc`.** No I/O, no clock, no timezone database, no
  runtime dependencies beyond `thiserror`. The library has no concept
  of "now" — you tell it the dates. CI compiles it for a bare-metal
  target (`thumbv7em-none-eabihf`), so the claim is checked, not
  asserted.
- **Const-first.** Every primitive constructor is `const fn`, and every
  built-in calendar is a `pub const` — zero allocation, zero setup.
- **No panics in library code.** Fallible operations return
  `Result<_, TimeError>`. `unwrap`/`expect`/`panic` are clippy-walled.
- **Property-tested invariants.** Conservation laws (ACT-family
  additivity, adjust idempotence, schedule monotonicity) are proptest
  suites, not comments. A separate integration test compiles against
  the crate from outside, so the public API is exercised the way a
  dependent sees it.

The constraints do not cost speed. Walking `is_business_day` across a
century of dates (36 525 days) takes ~1.7 ms for `us::SETTLEMENT` and
~4.2 ms for NYSE, the heaviest built-in — roughly 48 ns and 116 ns per
day, evaluated from const rules with no allocation and no runtime
setup. Reproduce with `cargo bench --bench calendar`; those are
medians from one 2.1 GHz Xeon core, so read them as an order of
magnitude, not a promise.

## Comparison

**Dates and times** — [`chrono`], [`jiff`], [`time`]. Use them. fasti
is not competing: it has no clock, no time zones, and no resolution
finer than a day. None of the three knows what a business day is.

**Business days** — [`bdays`], [`workdays`]. These answer *is this a
holiday* and *add five business days*, and stop there: no day-count
fractions, no business-day conventions past the basics, no schedule
generation.

**Quant libraries** — [`RustQuant_time`] covers much of the same
ground, with wider calendar coverage, as one crate of a broader
quantitative-finance framework. fasti is deliberately narrower: one
job, exact rationals instead of `f64`, and no `std`.

[`chrono`]: https://crates.io/crates/chrono
[`jiff`]: https://crates.io/crates/jiff
[`time`]: https://crates.io/crates/time
[`bdays`]: https://crates.io/crates/bdays
[`workdays`]: https://crates.io/crates/workdays
[`RustQuant_time`]: https://crates.io/crates/RustQuant_time

## What's in the box

| Area | Types |
|---|---|
| Date primitives | `Date` (serial, 1901-01-01..=2199-12-31), `Year`, `Month`, `Weekday`, `Ordinal` |
| Durations | `Period` (days/weeks/months/years), `Frequency` |
| Holiday rules | `Rule`: fixed-date (with weekend-shift policies), nth/last weekday, Easter offsets (Western & Orthodox), one-offs, custom `fn(Date) -> bool` |
| Calendars | `Calendar` / `CalendarBuilder`; built-ins: TARGET, UK Settlement, US Settlement, NYSE, Federal Reserve, Government Bond, SOFR, NERC, France Settlement & Exchange, plus `WEEKENDS_ONLY` / `NULL_CALENDAR` baselines. Business-day and holiday enumeration over a date range, month edges, and joint calendars via `CalendarBuilder::union` |
| Business days | `BusinessDayConvention` (Following, ModifiedFollowing, Preceding, ModifiedPreceding, Unadjusted), `adjust`, `advance` |
| Day counts | `DayCount`: ACT/360, ACT/365F, 30/360 (Bond Basis, US, 30E/360, 30E/360 ISDA), ACT/ACT (ISDA and schedule-aware ICMA) — all returning `Fraction` |
| Schedules | `Schedule` / `ScheduleBuilder`: forward/backward/zero generation, stubs, end-of-month preservation |

Four jurisdictions ship today. If yours is not among them you are not
blocked: the built-ins hold no privileged position — `uk::SETTLEMENT`
is a `pub const Calendar` assembled from the same public `Rule` values
you have, and about sixty of them describe England and Wales, Jubilees
and all. Calendars get added as people need them; the issue template
asks for a published source, because that is what lets a calendar be
reviewed rather than trusted. The list stays short deliberately — a
holiday table is a standing obligation, and state funerals and
coronations arrive without notice.

Built-ins apply their modern holiday regime across the whole supported
range, following QuantLib: UK dates before 1978 are the present-day
rule projected backwards, not what was observed at the time.

## Python

The bindings live in [`bindings/python`](./bindings/python) and ship as
`fasti-dates` on PyPI; the import name is `fasti`, matching the crate.
(The `fasti` name on PyPI is taken by an unrelated project.)

```console
$ pip install fasti-dates
```

```python
>>> import datetime
>>> from fasti.calendars import us
>>> us.SETTLEMENT.adjust(datetime.date(2024, 7, 4), "following")
datetime.date(2024, 7, 5)

```

Dates cross as `datetime.date` and year fractions come back as
`fractions.Fraction`, so the float-free guarantee survives the boundary.

## Install

```toml
[dependencies]
fasti = "0.2"

# Optional features (both off by default):
#   serde  — Serialize/Deserialize on the data types
#   chrono — From/TryFrom conversions with chrono::NaiveDate,
#            chrono::Weekday, and chrono::Month
fasti = { version = "0.2", features = ["serde", "chrono"] }
```

The minimum supported Rust version is **1.90** (edition 2024). CI
tests against it on every change, using a committed lockfile pinned to
a dependency resolution known to build there. Raising the MSRV is a
minor-version bump, never a patch.

Coming from `chrono`? Enable the `chrono` feature and convert at the
boundary — `let d: fasti::Date = naive_date.try_into()?;` (fallible
only because fasti's supported range is 1901..=2199) and
`chrono::NaiveDate::from(d)` on the way out. fasti never uses chrono
internally.

## Quickstart

```rust
use fasti::{
    ActActISDA, BusinessDayConvention, Date, DayCount, Month, Period,
    ScheduleBuilder, calendars::us,
};

fn main() -> Result<(), fasti::TimeError> {
    // Is July 4 a US market holiday?
    let d = Date::from_ymd(2026, Month::Jul, 3)?; // Sat Jul 4 observed Friday
    assert!(us::SETTLEMENT.is_holiday(d));

    // Build a 5-year semiannual coupon schedule (backward generation,
    // ModifiedFollowing interior dates, unadjusted maturity).
    let schedule = ScheduleBuilder::new(
        Date::from_ymd(2025, Month::Jan, 15)?,
        Date::from_ymd(2030, Month::Jan, 15)?,
        Period::Months(6),
        us::SETTLEMENT,
    )
    .backwards()
    .with_convention(BusinessDayConvention::ModifiedFollowing)
    .with_termination_convention(BusinessDayConvention::Unadjusted)
    .build()?;

    // Accrue each period under ACT/ACT (ISDA) — exact integer rationals.
    for period in schedule.periods() {
        let (num, den) = ActActISDA
            .year_fraction(period.start, period.end)
            .parts();
        println!("{} -> {}: {num}/{den}", period.start, period.end);
    }
    Ok(())
}
```

Output — note that the January 2028 coupon lands on the 18th (the 15th
is a Saturday, and the Monday behind it is Martin Luther King Jr. Day),
and that the periods spanning a leap boundary are the ones a `double`
would have to round:

```text
2025-01-15 -> 2025-07-15: 181/365
2025-07-15 -> 2026-01-15: 184/365
2026-01-15 -> 2026-07-15: 181/365
2026-07-15 -> 2027-01-15: 184/365
2027-01-15 -> 2027-07-15: 181/365
2027-07-15 -> 2028-01-18: 13685/26718
2028-01-18 -> 2028-07-17: 181/366
2028-07-17 -> 2029-01-16: 2227/4453
2029-01-16 -> 2029-07-16: 181/365
2029-07-16 -> 2030-01-15: 183/365
```

Run the fuller example:

```bash
cargo run --example treasury_schedule
```

## Design notes

See [`ARCHITECTURE.md`](./ARCHITECTURE.md) for the design constraints:
serial dates, the rule-based calendar model, why `Fraction` instead of
floats, and the supported 1901..=2199 date range.

QuantLib's `ql/time` is the design reference; ported holiday tables and
lookup data are attributed in the module docs and in
[`THIRD-PARTY-NOTICES`](./THIRD-PARTY-NOTICES). The Easter tables are
additionally cross-validated in tests against independent computus
implementations, so they are reproducible from first principles.

## Status

Pre-1.0. The public surface is small and deliberate but may still move.
Planned next: CDS/IMM schedule generation rules and the long-tail day
counts (Business/252, ACT/365 Canadian, NASD 30/360).

## Contributing

Issues and pull requests are welcome. [`CONTRIBUTING.md`](./CONTRIBUTING.md)
covers the development workflow and the ground rules;
[`ARCHITECTURE.md`](./ARCHITECTURE.md) covers the constraints behind
them. Participation is under the
[Code of Conduct](./CODE_OF_CONDUCT.md).

Calendar and day-count data bugs are the most useful thing you can
report. The issue template for them asks for the published source the
fix will be checked against, because that is what makes such a bug
fixable in one pass. For anything that looks like a vulnerability, read
[`SECURITY.md`](./SECURITY.md) first — it also explains why wrong
holiday data, deliberately planted or not, is out of scope there: an
inaccurate holiday table is a correctness bug, so it belongs in a
public issue where others can check it against the source, not in a
private advisory.

## License

Dual-licensed under either of

- [Apache License, Version 2.0](./LICENSE-APACHE)
- [MIT license](./LICENSE-MIT)

at your option.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the
Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.

### Third-party material

The Easter-Monday lookup tables in `src/easter.rs` are ported from
QuantLib, which is distributed under a permissive modified-BSD
(3-clause) license. That license permits the redistribution above; its
full text and copyright notice are reproduced in
[`THIRD-PARTY-NOTICES`](./THIRD-PARTY-NOTICES), which ships inside the
published crate. If your license tooling flags fasti for third-party
content, this is what it has found — there is no copyleft anywhere in
the dependency graph, and `cargo deny check` enforces that on every
commit.
