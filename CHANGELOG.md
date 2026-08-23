# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `HolidayCache`: a caller-owned memo that resolves a `Calendar`'s
  rules once per year instead of once per day, answering `is_holiday`,
  `is_business_day`, `next`/`prev_business_day`, `adjust` and
  `advance` identically to the calendar it wraps. `Calendar` stays a
  borrowed `Copy` view and holds no cache; this is where one lives.
- `Rule::natural_date(year) -> RuleDate`: the date a rule names in a
  given year, or `RuleDate::Opaque` for the `Rule::Custom` predicate
  that cannot say. The dual of `Rule::is_holiday`, and what the
  per-year resolution is built on.
- `cargo bench --bench calendar`: per-day and per-call costs for every
  built-in calendar. No benchmarking dependency; the harness is a
  `harness = false` binary, and an unoptimised build does a smoke run
  so `cargo test --all-targets` keeps it compiling.

### Changed

- Calendar rule evaluation is 6-12x faster per day walked and per call
  (US settlement: 923 -> 76 ns per day, 1146 -> 91 ns per
  `is_business_day`, 1755 -> 148 ns per `adjust`); the iterators from
  `Calendar::business_days` and `Calendar::holidays` carry a per-year
  memo and are 20-45x faster (US settlement: 923 -> 20 ns per day).
  Holiday answers are unchanged, byte for byte, for every date in
  1901..=2199 — `tests/equivalence.rs` checks each built-in and twelve
  synthetic calendars against the original algorithm.
- `Date::year` derives the year arithmetically instead of binary
  searching the cumulative-days table, which speeds up every
  `to_ymd`-based accessor with it.

## [0.1.0] - 2026-08-21

Initial release.

### Added

- Date primitives: `Date` (serial representation over
  1901-01-01..=2199-12-31), `Year`, `Month`, `Weekday`, `Ordinal` —
  all `const`-constructible, with EoM-aware month/year arithmetic and
  strict ISO-8601 (`YYYY-MM-DD`) parsing via `FromStr`.
- `Period` / `Frequency` with QuantLib-parity normalization and
  checked scalar arithmetic.
- Holiday rules: `FixedDate` (with weekend-shift policies),
  `NthWeekday`, `LastWeekday`, `EasterOffset` (Western and Orthodox),
  `OneOff`, and the `Rule::Custom` fn-pointer escape hatch.
- Easter-Monday lookup tables for 1901..=2199, validated in tests
  against independent Gregorian and Julian computus implementations.
- Calendars: `Calendar` / `CalendarBuilder`; built-ins for TARGET, UK
  Settlement, US Settlement, NYSE, Federal Reserve, Government Bond,
  SOFR, NERC, France Settlement and Exchange, plus the
  `WEEKENDS_ONLY` and `NULL_CALENDAR` baselines. Business-day and
  holiday enumeration over a date range, month edges, and joint
  calendars via `CalendarBuilder::union`.
- Business-day conventions (Following, ModifiedFollowing, Preceding,
  ModifiedPreceding, Unadjusted) with `Calendar::adjust` and
  `Calendar::advance`.
- Day counts returning integer-rational `Fraction`s: ACT/360,
  ACT/365F, 30/360 Bond Basis, 30/360 US, 30E/360 (Eurobond Basis),
  30E/360 ISDA, ACT/ACT ISDA, and ACT/ACT ICMA; ICMA binds to a
  coupon `Schedule` (`ActActICMA::bind`) so stub handling flows
  through the same two-date `year_fraction` API as every other
  convention.
- `Schedule` / `ScheduleBuilder`: forward/backward/zero generation,
  stub anchors, per-termination convention, end-of-month preservation,
  `reference_periods` giving the regular coupon grid each period
  accrues against, plus the `Generation` parameters the schedule was
  built from.
- Optional `serde` feature (off by default); `no_std` + `alloc`
  throughout.
- Optional `chrono` feature (off by default): `From`/`TryFrom`
  conversions between `Date`/`Weekday`/`Month` and
  `chrono::NaiveDate`/`chrono::Weekday`/`chrono::Month`.
