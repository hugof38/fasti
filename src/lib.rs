//! Dates, calendars, business-day conventions, and day-count fractions.
//!
//! `fasti` is a standalone Rust time library for financial code, designed
//! after [QuantLib's `ql/time`](https://github.com/lballabio/QuantLib/tree/master/ql/time).
//! Runtime deps: [`thiserror`] only; `#![no_std]`-compatible with `alloc`.
//! Opt-in features: `serde` (derives) and `chrono` (conversions).
//!
//! # Design principles
//!
//! - **No float arithmetic.** Day-count fractions return integer rationals.
//! - **Const-first.** Primitive constructors and built-in calendars are `const`.
//! - **Integer serial dates.** [`Date`] is a [`u32`] newtype; range 1901-01-01..=2199-12-31.
//! - **Rule-based calendars.** Composable holiday rules with a `fn(Date) -> bool` escape hatch.
//!
//! # Roadmap
//!
//! Landed today: date primitives ([`Date`], [`Year`], [`Month`],
//! [`Weekday`], [`Ordinal`]), holiday rules ([`Rule`] and friends),
//! Easter tables ([`easter_monday`], [`easter_sunday`]), [`YearRange`],
//! [`Calendar`] / [`CalendarBuilder`] with built-ins under [`calendars`],
//! [`Period`] / [`Frequency`] arithmetic, [`BusinessDayConvention`] with
//! [`Calendar::adjust`] and [`Calendar::advance`], the [`Fraction`] type,
//! the [`DayCount`] trait and its conventions, [`Schedule`] /
//! [`ScheduleBuilder`], and the [`TimeError`] type.
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
#[cfg(feature = "chrono")]
mod chrono_interop;
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
pub use daycount::{
    Act360, Act365Fixed, ActActICMA, ActActISDA, BoundActActICMA, DayCount, Thirty360Bond,
    Thirty360European, Thirty360ISDA, Thirty360US,
};
pub use easter::{EasterMethod, easter_monday, easter_sunday};
pub use error::TimeError;
pub use fraction::Fraction;
pub use period::{Frequency, Period};
pub use rules::{EasterOffset, FixedDate, LastWeekday, NthWeekday, OneOff, Rule, WeekendShift};
pub use schedule::{AccrualPeriod, DateGenerationRule, Schedule, ScheduleBuilder};
pub use weekend::Weekend;
pub use year_range::YearRange;
