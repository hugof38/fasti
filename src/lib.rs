//! Dates, calendars, business-day conventions, and day-count fractions.
//!
//! `fasti` is a standalone Rust time library for financial code, designed
//! after [QuantLib's `ql/time`](https://github.com/lballabio/QuantLib/tree/master/ql/time).
//! Runtime deps: [`thiserror`] only; `#![no_std]`-compatible with `alloc`.
//! Opt-in features: `serde` (validated formats: dates as `"YYYY-MM-DD"`,
//! periods as `"6M"`) and `jiff` (conversions).
//!
//! # Where to start
//!
//! | You want to… | Start with |
//! |---|---|
//! | know whether a market is open | a built-in [`calendars`] constant, [`Calendar::is_business_day`] |
//! | roll a date onto a business day | [`Calendar::adjust`] with a [`BusinessDayConvention`] |
//! | add a settlement lag (T+2) | [`Calendar::add_business_days`] |
//! | step by a tenor (3M, 1Y), then roll | [`Calendar::advance`] |
//! | describe a market fasti does not ship | [`Calendar::new`] (as a `const`) or [`CalendarBuilder`], from [`Rule`]s |
//! | trade across two markets | [`CalendarBuilder::union`] |
//! | lay out coupon dates | [`ScheduleBuilder`] → [`Schedule`] |
//! | count accrual | a [`DayCount`]; for ACT/ACT ICMA, [`ActActICMA::bind`] to an unadjusted schedule |
//! | turn an accrual into money | [`Fraction::scale`] with a [`RoundingMode`] |
//!
//! Errors are one enum, [`TimeError`]: fallible operations return it
//! rather than panicking.
//!
//! # Design principles
//!
//! - **No float arithmetic.** Day-count fractions return integer rationals.
//! - **Const-first.** Primitive constructors and built-in calendars are `const`.
//! - **Integer serial dates.** [`Date`] is a [`u32`] newtype; range 1901-01-01..=2199-12-31.
//! - **Rule-based calendars.** Composable holiday rules with a `fn(Date) -> bool` escape hatch.
//!
//! Not yet covered: CDS/IMM schedule rules and the long-tail day counts.

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
#[cfg(feature = "jiff")]
mod jiff_interop;
mod names;
mod period;
mod rules;
mod schedule;
#[cfg(feature = "serde")]
mod serde_impls;
mod weekend;
mod year_range;

pub use business_day::BusinessDayConvention;
pub use calendar::{Calendar, CalendarBuilder};
pub use date::{Date, DateRange, Month, Ordinal, Weekday, Year};
pub use daycount::{
    Act360, Act365Fixed, ActActICMA, ActActISDA, BoundActActICMA, DayCount, Thirty360Bond,
    Thirty360European, Thirty360ISDA, Thirty360US,
};
pub use easter::{EasterMethod, easter_monday, easter_sunday};
pub use error::TimeError;
pub use fraction::{Fraction, RoundingMode};
pub use period::{Frequency, Period};
pub use rules::{EasterOffset, FixedDate, LastWeekday, NthWeekday, OneOff, Rule, WeekendShift};
pub use schedule::{DateGenerationRule, Generation, Schedule, ScheduleBuilder};
pub use weekend::Weekend;
pub use year_range::YearRange;
