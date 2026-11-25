# Architecture

The constraints below are deliberate. If a change fights one of them,
the constraint wins unless the design discussion says otherwise.

## Independence

- **No external date libraries in the core.** No `chrono`, no `time`,
  no `jiff` as required dependencies or internal representations. The
  custom serial `Date(u32)` representation is the point: single-word
  comparisons, trivial arithmetic, const evaluation. The opt-in
  `jiff` feature adds boundary conversions (`From`/`TryFrom`) and
  nothing else — enabling it never changes fasti's own arithmetic.
- **Runtime deps: `thiserror` only.** Adding any other runtime
  dependency is a design-review item, not a convenience choice.
- **`serde` is always behind a feature flag.** Never required.
- **`#![no_std]` + `alloc`.** No `std::` imports. The library has no
  clock, no notion of "now", no timezone database, and no I/O — callers
  supply every date. Tests use `alloc` explicitly via
  `extern crate alloc;` inside the test module.

## Const-first idioms

- Every primitive constructor (`Date::from_ymd`, `Year::new`,
  `Month::try_from_u8`, …) is `const fn`.
- Every built-in calendar is a `pub const Calendar<'static>`, not a
  factory function. Zero allocation, zero vtable lookups at the call
  site.
- Every rule constructor (`FixedDate::new`, `NthWeekday::new`, …) is a
  `const fn` builder.
- When adding a new type, ask: *can this be `const`?* If yes, make it
  so. If no, document why in the item's doc comment.

## Rule-based calendar model

- Holidays are expressed via the `Rule` enum. To add a new kind of
  holiday logic, add a variant — do not introduce a new trait.
- The escape hatch is `Rule::Custom(fn(Date) -> bool)`. There is
  deliberately **no** `HolidayRule` trait: the fn-pointer variant keeps
  the whole enum `const`-constructible so built-in calendars can live
  in `pub const` values.
- `Weekend` is a bitmask over weekdays and lives outside the rule list.
- `Calendar<'a>` is a borrowed, `Copy` view with private fields,
  built by `const fn Calendar::new`. The owned form is
  `CalendarBuilder`, which exposes `view() -> Calendar<'_>`.
- A joint calendar (`CalendarBuilder::union`) keeps each member's rules
  and weekend apart and asks each member separately, as `QuantLib`'s
  `JointCalendar` does. Concatenating rule lists is wrong: one member's
  holiday would push another's chaining substitute onto a day neither
  closes.

## Day-count conventions

- **Trait-based.** `DayCount` is a trait over two dates; concrete
  impls are mostly zero-sized structs (`Act360`, `Thirty360Bond`, …),
  `Thirty360ISDA` carrying its termination date. `AccrualDayCount` is
  the trait over an `AccrualPeriod`, with a blanket impl for every
  `DayCount`.
- **Schedule-defined conventions measure periods, not dates.** ACT/ACT
  ICMA implements only `AccrualDayCount`: an `AccrualPeriod` carries its
  reference period and the `Generation` it came from, so ICMA reads its
  coupon frequency and any stub's notional grid from there, and there is
  no frequency to pass that could disagree with the schedule. Like every
  convention it accrues on the schedule's accrual dates: unadjusted for
  a bond (the builder's default), adjusted when the terms say so, as
  `QuantLib` and rateslib do. `year_fraction_with_reference` stays as the
  explicit escape hatch. Where QuantLib infers the frequency by
  float-rounding a period length, fasti takes it from the tenor.
- **Each concept is owned once.** A `Schedule` keeps its accrual dates
  and one payment date per period (the payment convention, calendar and
  lag applied to each accrual end). Payment dates are stored because a
  schedule owns no calendar to recompute them; unadjusted dates are not,
  because nothing that measures accrual should read them. The schedule
  computes the stub reference boundaries and retains the `Generation`
  it was built from. Only the ends can deviate from the regular grid,
  so it stores those two boundaries rather than a reference date per
  period. Extending a reference period into notional windows is
  accrual math, so it lives with ICMA.
- **`year_fraction` returns a `Fraction`** — an `i64 / u64` integer
  rational, signed by direction. Never `f64`, never a decimal type.
  Reversed inputs (`end < start`) produce a negative fraction that
  mirrors the ordered one; `yf(a, b) + yf(b, a) == 0` is a tested
  invariant.
- Callers scale an amount with `Fraction::scale(amount, RoundingMode)`:
  the product `amount * num` is formed in `i128` and divided once, so
  rounding happens exactly once and only where the caller names the
  mode. Fold the rate into the fraction first (`rate.checked_mul(yf)`).
  Intermediates inside `Fraction::checked_add`, `Fraction::checked_mul`,
  and `cmp_cross` likewise widen to `i128`.
- The type is named `Fraction` (not `YearFraction`) because the same
  algebra serves day-count fractions and the rates multiplied into
  them. It is not a general scalar: repeated products such as
  compounding `(1 + r·τ)` exhaust `i64/u64` within a few steps, and
  pricing is out of scope.

## Supported date range

- **1901-01-01 through 2199-12-31 inclusive.** Every fallible
  constructor refuses out-of-range inputs with `TimeError`.
- The Easter-Monday lookup tables and the serial-date arithmetic are
  both sized for this range. Widening it is a spec change, not a local
  edit.

## Porting from QuantLib

- QuantLib is the design reference for calendars, day counts, and
  schedule semantics. It is distributed under a permissive
  modified-BSD license, compatible with this crate's
  `Apache-2.0 OR MIT`; its notice is reproduced in
  `THIRD-PARTY-NOTICES`.
- **Cross-check ported tables independently.** For Easter, the test
  suite implements the Anonymous/Meeus Gregorian computus and the
  Meeus Julian computus and asserts every table entry matches. The
  test runs once; if it passes, table and algorithm validate each
  other — and the tables are reproducible without the upstream source.
- Attribute QuantLib in the module doc comment of any file that ports
  from it, and document deliberate deviations (e.g. France's
  Ascension/Whit Monday are proper Easter offsets here, where QuantLib
  encodes fixed dates — a known upstream bug).

## Error handling

- `thiserror`, one crate-wide `TimeError` enum.
- No `unwrap()` / `expect()` / `panic!()` / `todo!()` in library code;
  clippy warns on all of them and CI runs clippy with `-D warnings`.
  Tests may use them, gated with
  `#[allow(clippy::unwrap_used, clippy::expect_used)]` on the test
  module.
- `Type::literal(...)` constructors are the `const`-context escape
  hatch: they validate at const-eval time, so a bad literal is a
  compile error rather than a runtime panic.

## Integer casts

`clippy::pedantic` flags many `as` casts as possibly
truncating/wrapping/sign-losing. In this crate those casts are common
because the math is inherently in `i32`/`u32`/`u8` and bounded by local
invariants clippy cannot see. Annotate the invariant at each cast site
(or at module level with a doc block listing the invariants, as
`src/date.rs` does) — do not sprinkle bare `#[allow]`s.

## Property-test invariants (the ones that matter here)

- Every rule type: the set of dates it marks as holidays is a subset
  of its `YearRange`.
- Calendar: `is_business_day(d) == !is_weekend(d) && !is_holiday(d)`.
- `adjust` is idempotent for every convention, and
  `ModifiedFollowing` / `ModifiedPreceding` never cross a month
  boundary.
- `Schedule` dates are strictly monotonically increasing, as are its
  parallel reference dates; the two lists differ only at stub ends.
- `DayCount::year_fraction(d, d)` is zero; reversal negates.
- ACT-family day counts are additive across splits —
  `yf(a, b) + yf(b, c) == yf(a, c)`. 30/360 is intentionally NOT
  additive; the non-additivity is documented and regression-tested.
