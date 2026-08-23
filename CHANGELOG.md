# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `Rule::natural_date(year) -> RuleDate`: the date a rule names in a
  given year, or `RuleDate::Opaque` for the `Rule::Custom` predicate
  that cannot say. The dual of `Rule::is_holiday`, and what per-year
  resolution is built on — callers precomputing their own holiday sets
  need nothing else.
- `cargo bench --bench calendar`: per-day and per-call costs for every
  built-in calendar. No benchmarking dependency; the harness is a
  `harness = false` binary, and an unoptimised build does a smoke run
  so `cargo test --all-targets` keeps it compiling.

### Changed

- Calendar rule evaluation is an order of magnitude faster, per call
  and per day walked, and `Calendar::business_days` / `holidays` carry
  a per-year memo in their own iterator state on top of that. Holiday
  answers are unchanged, byte for byte, for every date in 1901..=2199
  — `tests/equivalence.rs` checks each built-in and nine synthetic
  calendars against the original algorithm.
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
