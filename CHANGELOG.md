# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Python bindings (`bindings/python`), published to PyPI as `fasti-py` and
  imported as `fasti`: a PyO3 extension exposing calendars, business-day conventions, day
  counts, schedules and holiday rules to Python. Dates cross the
  boundary as `datetime.date` and nothing else — a string or a
  `datetime.datetime` is refused with the conversion to write — and
  year fractions as `fractions.Fraction`, so the crate's float-free
  arithmetic is preserved. Ships type stubs, `abi3` wheels for CPython 3.10+, and its
  own release workflow on `py-v*` tags. Every type is picklable — a
  value rebuilds by replaying the constructor calls that made it, so
  calendars and schedules can cross a process boundary intact — and the
  extension is free-thread-safe, with wheels for free-threaded CPython
  3.14 alongside the `abi3` ones. Values compare and hash structurally —
  calendars and rules by how they were built, so an unpickled calendar
  equals its original and either can be a dict key — and the methods
  that extend a calendar take a single date or rule as readily as a
  list.

### Fixed

- `Rule::Easter`'s doc comment named Easter Monday as the anchor its
  offsets are measured from; `EasterOffset` documents and implements
  Easter Sunday.

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
