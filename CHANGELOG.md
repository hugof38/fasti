# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

Fixes from an independent usability review. Several are breaking; the
crate is pre-1.0 and they close silent-wrong-answer paths.

### Breaking

- **Accrual and payment dates are separate.** A `Schedule` holds its
  accrual dates and a payment date per period. `periods()` yields
  `AccrualPeriod`s (`start`, `end`, `range`, `reference`, `until`)
  instead of ranges; `coupons()` pairs each with a `PaymentDate`;
  `reference_periods()` is gone (each period has its `reference()`).
  Payments default to the accrual end rolled `Following`; set them
  with `ScheduleBuilder::with_payment_convention`,
  `with_payment_calendar` and `with_payment_lag`. A lag counts
  business days from the accrual end and ignores the convention, as
  QuantLib's `advance(end, lag, Days, convention)` does.
- **ACT/ACT ICMA measures periods.** `ActActICMA` is a unit struct
  implementing the new `AccrualDayCount` (also implemented for every
  `DayCount`), taking its frequency and stub grid from the period's
  schedule, so there is no frequency to mismatch. Like every
  convention it accrues on the schedule's accrual dates.
  `ActActICMA::new`, `bind`, `BoundActActICMA` and the
  `FrequencyMismatch` and `ScheduleTooShort` errors are removed;
  `year_fraction_with_reference` takes the frequency.
  `TryFrom<Vec<Date>>` schedules name no frequency (ICMA reports
  `MissingFrequency`); use `TryFrom<(Vec<Date>, Frequency)>`.
- `serde`: `Date` is `"YYYY-MM-DD"` and `Period` is `"6M"`. The 0.2
  forms (an integer serial, `{"Months": 6}`) are no longer read.
- `ActActICMA` no longer implements `DayCount`. Unbound, it returned one
  full coupon for any two dates, even a day apart. Measure a schedule's
  periods with `AccrualDayCount`, or use `year_fraction_with_reference`.
- `Calendar`'s fields are private. Build one with `const fn
  Calendar::new(name, weekend, rules)` and read it with `name()`,
  `weekend()`, `rules()`.
- `FixedDate::new` returns `Result` and refuses a day no year has in the
  month (Feb 30). `FixedDate::literal` is the `const` form.
- `ScheduleBuilder::new`'s defaults are now bond conventions:
  `Unadjusted` interior dates (was `ModifiedFollowing`) and end of month
  on (was off), alongside the unchanged `Unadjusted` termination and
  `Backward` generation. The old docs already called these "bond
  conventions"; ModifiedFollowing is what made a bond schedule silently
  wrong under ACT/ACT ICMA. Swap legs opt in with
  `with_convention(ModifiedFollowing)` and `with_end_of_month(false)`.
  The Python `Schedule` follows.
- `TimeError` is `#[non_exhaustive]`. `ActActICMA::bind` on a schedule
  with fewer than two dates now returns `ScheduleTooShort`.

### Changed

- `Date::to_ymd`, `Date::year` and the accessors built on them decompose
  a serial with the Neri–Schneider algorithm — branch-free, table-free
  integer arithmetic — instead of a year-table search. Results are
  identical (checked exhaustively against a day-by-day walk of the whole
  supported range); calendar evaluation is roughly 25–35% faster, e.g. a
  century of `is_business_day` on NYSE drops from ~4.9 ms to ~3.3 ms on
  the benchmark machine. No API change, no new dependency.

### Removed

- The `chrono` feature. chrono is unmaintained
  ([RUSTSEC-2026-0543](https://rustsec.org/advisories/RUSTSEC-2026-0543))
  with no safe upgrade. Convert from chrono through year, month and day
  (`Date::from_ymd`), or move to jiff, as the advisory recommends.

### Fixed

- Python: the `Calendar.advance` stub declared `end_of_month=True`;
  the default is `False`, and `Schedule`'s is `True`.
- `ScheduleBuilder::build` merges dates that business-day adjustment
  rolls onto one day, as QuantLib does, instead of failing with
  `ScheduleNotMonotonic`, so daily schedules build on real calendars.
- Deserializing untrusted input can no longer panic the library.
  `Date`, `Year`, `Weekend`, `YearRange`, `FixedDate`, `Fraction` and
  the dates inside a `Schedule` are validated on the way in; before,
  `{"date": 200000}` loaded and then panicked on first use.
- `CalendarBuilder::union` evaluates each joined calendar on its own, as
  QuantLib's `JointCalendar` does. Concatenating rule lists let one
  calendar's holiday push another's weekend substitute onto a day
  neither closes: UK joined with a Jan 2 holiday marked Jan 3, 2023 (and
  42 other dates) as a holiday. Each member also keeps its own weekend.
- The README Quickstart and `examples/treasury_schedule.rs` accrued a
  Treasury on ACT/ACT ISDA over adjusted dates. They now use ACT/ACT
  ICMA on the unadjusted grid with payment dates rolled separately, so
  every coupon on $1mm at 4.25% is exactly $21,250.00. The example also
  computes accrued interest for a T+1 settlement.
- `BoundActActICMA` no longer turns an arithmetic failure into a silent
  zero mid-sum; `try_year_fraction` reports it.
- `Date::literal` and `Year::literal` documented a compile error even
  at run time; the docs now say they panic outside a `const`.
- `Date`'s `Display` honours width and alignment, so dates line up in
  tables.

### Added

- `Schedule::accrual_period(date)`: the period running on a
  settlement date, half-open, so accrued interest on a coupon date is
  zero; cut it short with `AccrualPeriod::until(date)`.
- `Schedule::payment_dates`.
- Python: `AccrualPeriod` and `Coupon` classes; `Schedule.coupons()`,
  `accrual_period()`, `payment_dates()`; the
  `payment_convention`, `payment_calendar` and `payment_lag` keywords;
  `DayCount.accrual_fraction(period)`; `act_act_icma()` takes no
  frequency and `bind` is gone.
- A `jiff` feature: `TryFrom<jiff::civil::Date> for Date`,
  `From<Date> for jiff::civil::Date`, and `Weekday` conversions both
  ways. jiff has no month type; months cross as numbers
  (`Month::try_from_u8`, `Month::get`). Built with jiff's default
  features off, so it stays `no_std`.
- `Calendar::add_business_days(date, n)` for settlement lags. This is
  what QuantLib's `advance` does with a `Days` unit; fasti's `advance`
  with `Period::Days` counts calendar days, as its docs now say loudly.
- `Fraction::scale(amount, RoundingMode)` (amount × fraction, rounded
  once, via `i128`), `Fraction::round`, `From<i64>`, and `RoundingMode` (floor, ceiling, toward/away from
  zero, half away from zero, half even).
- `FromStr` for `Period` (`"6M"`), `Frequency`,
  `BusinessDayConvention` and `DateGenerationRule`, ignoring case and
  punctuation as the Python bindings do.
- `BoundActActICMA::try_year_fraction`, `ActActICMA::name`,
  `Calendar::is_joint`, `Weekend::bits` / `from_bits`,
  `FixedDate::literal`, `TryFrom<u16> for Year`, and the `TimeError`
  variants `ScheduleTooShort`, `InvalidPeriodString`, `UnknownName`,
  `InvalidWeekendMask`.
- Python: `Calendar.add_business_days` and
  `Calendar.previous_business_day`. Unbound `DayCount.act_act_icma`
  raises on `year_fraction` instead of returning a full coupon.

### Deprecated

- `Calendar::prev_business_day`, renamed `previous_business_day` to
  match `Schedule::previous_date`. (Python keeps both names.)

## [py-0.2.1] - 2026-08-25

`fasti-dates` only; the crate stays at 0.2.0. The two numbers are kept
equal where they can be, but a change to the bindings' own README is not
a reason to republish an identical crate, and PyPI will not let a
released description be edited — a new version is the only way to change
what the project page shows.

### Removed

- The README's "Vocabularies" section. The behaviour it described is
  unchanged: every argument taking a member still takes a string, and
  matching still ignores case and punctuation.

## [0.2.0] - 2026-08-24

Two artifacts at one version: the crate on crates.io and `fasti-dates` on
PyPI. They release together and carry the same number, so a reported
version identifies the same code whichever side of the boundary it was
seen from.

### Added

- Python bindings under `bindings/python`, published as `fasti-dates`
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
