//! Dates, calendars, business-day conventions, and day-count fractions.
//!
//! `fasti` is a standalone Rust library that provides the primitives
//! financial code needs to reason about time: calendar dates, holiday rules,
//! payment schedules, and day-count conventions. It is designed after
//! [QuantLib's `ql/time`](https://github.com/lballabio/QuantLib/tree/master/ql/time)
//! module and aims to cover the same capability surface over time.
//!
//! The crate has no runtime dependencies beyond [`thiserror`] and is
//! `#![no_std]`-compatible with `alloc` for the builder paths.
//!
//! # Design principles
//!
//! - **No float arithmetic.** Day-count fractions return integer rationals.
//! - **Const-first.** All primitive constructors and all built-in calendars
//!   are `const`; well-known calendars are `pub const` with zero runtime
//!   cost at the query site.
//! - **Integer serial dates.** [`Date`] is a thin newtype over [`u32`] days
//!   from 1901-01-01. The range is 1901-01-01 through 2199-12-31.
//! - **Rule-based calendars.** Holidays are expressed as composable rules,
//!   with a `fn(Date) -> bool` escape hatch for bespoke logic.
//!
//! # Roadmap
//!
//! Landed today: date primitives ([`Date`], [`Year`], [`Month`],
//! [`Weekday`], [`Ordinal`]), holiday rule primitives ([`FixedDate`],
//! [`NthWeekday`], [`LastWeekday`], [`OneOff`], [`EasterOffset`],
//! [`WeekendShift`]), Easter-Monday lookup tables ([`easter_monday`],
//! [`easter_sunday`]), year ranges ([`YearRange`]), calendars
//! ([`Calendar`], [`CalendarBuilder`]) with built-in `pub const`
//! values under [`calendars`], [`Period`] (a sum type over
//! `Days`/`Weeks`/`Months`/`Years`) and [`Frequency`] with
//! QuantLib-parity arithmetic, EoM-aware month/year arithmetic on
//! [`Date`] (via `Add<Period>` / `Sub<Period>` and the unit-specific
//! methods), [`BusinessDayConvention`] with [`Calendar::adjust`] and
//! [`Calendar::advance`], the [`Fraction`] integer-rational type that
//! day-count conventions return, the [`DayCount`] trait with
//! [`Act360`], [`Act365Fixed`], [`Thirty360Bond`], and [`ActActISDA`]
//! impls, [`Schedule`] / [`ScheduleBuilder`] with
//! [`DateGenerationRule::Forward`], [`DateGenerationRule::Backward`],
//! and [`DateGenerationRule::Zero`] generation rules, and the
//! [`TimeError`] type returned by fallible constructors.
//!
//! Planned:
//!
//! - Further built-in calendars (`TARGET`, `UK_BANK`)

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

mod business_day;
mod calendar;
pub mod calendars;
mod date;
mod daycount;
mod easter;
mod error;
mod fraction;
mod period;
mod rules;
mod schedule;
mod weekend;
mod year_range;

pub use business_day::BusinessDayConvention;
pub use calendar::{Calendar, CalendarBuilder};
pub use date::{Date, Month, Ordinal, Weekday, Year};
pub use daycount::{Act360, Act365Fixed, ActActISDA, DayCount, Thirty360Bond};
pub use easter::{EasterMethod, easter_monday, easter_sunday};
pub use error::TimeError;
pub use fraction::Fraction;
pub use period::{Frequency, Period};
pub use rules::{EasterOffset, FixedDate, LastWeekday, NthWeekday, OneOff, Rule, WeekendShift};
pub use schedule::{AccrualPeriod, DateGenerationRule, Schedule, ScheduleBuilder};
pub use weekend::Weekend;
pub use year_range::YearRange;
