# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-08-24

Two artifacts at one version: the crate on crates.io and `fasti-py` on
PyPI. They release together and carry the same number, so a reported
version identifies the same code whichever side of the boundary it was
seen from.

### Added

- Python bindings under `bindings/python`, published as `fasti-py`
  (import name `fasti`). They are their own cargo workspace with their
  own lockfile, and the root manifest excludes them from the packaged
  crate, so nothing here changes for a Rust dependent.

### Changed

- Calendar evaluation now expands rules instead of matching them:
  `Calendar::is_holiday` derives the query's year once and asks each
  rule which date it names that year, rather than probing every rule
  with three year/month/day decompositions per date. `Date::year` (and
  everything built on it) computes the year arithmetically from the
  400-year Gregorian cycle instead of binary-searching a table.
  Rule-bearing built-in calendars evaluate roughly 4–12× faster;
  behaviour over 1901..=2199 is unchanged for every built-in (verified
  exhaustively against the previous implementation). A `cargo bench`
  suite (divan, dev-dependency) covers every built-in.
- Substitute-day resolution is now jurisdiction-correct per
  `WeekendShift` variant instead of uniformly chained. `Forward`
  (UK/Commonwealth) still chains onto the next free weekday;
  `SunForward` and `SatBackSunForward` (Fed/SIFMA, US federal, NYSE
  Rule 51) now take a single fixed step, coinciding with — never
  leapfrogging — a holiday already on the target day. Built-in
  calendars are unaffected (their data never exercised the
  difference); hand-built calendars pairing a single-step Sunday
  holiday with a natural Monday holiday no longer observe an invented
  Tuesday. Conventions probing deeper than the two-day chain (Japan's
  Golden Week Wednesday substitute) are documented as `Rule::Custom`
  territory.

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
