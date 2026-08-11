# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - Unreleased

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
- Calendars: `Calendar` / `CalendarBuilder`; built-ins for US
  Settlement, NYSE, Federal Reserve, Government Bond, SOFR, NERC,
  France Settlement and Exchange, plus the `WEEKENDS_ONLY` and
  `NULL_CALENDAR` baselines.
- Business-day conventions (Following, ModifiedFollowing, Preceding,
  ModifiedPreceding, Unadjusted) with `Calendar::adjust` and
  `Calendar::advance`.
- Day counts returning integer-rational `Fraction`s: ACT/360,
  ACT/365F, 30/360 Bond Basis, ACT/ACT ISDA.
- `Schedule` / `ScheduleBuilder`: forward/backward/zero generation,
  stub anchors, per-termination convention, end-of-month preservation.
- Optional `serde` feature (off by default); `no_std` + `alloc`
  throughout.
