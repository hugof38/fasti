//! Built-in calendars, grouped by country / market.
//!
//! Every calendar is a `pub const Calendar<'static>` — zero allocation,
//! zero vtable, construction-free at the call site. Callers pass by
//! value (`Calendar` is `Copy`).
//!
//! ```
//! use fasti::{Date, Month, calendars::us};
//! assert!(us::SETTLEMENT.is_holiday(Date::from_ymd(2024, Month::Jul, 4)?));
//! # Ok::<(), fasti::TimeError>(())
//! ```

pub mod france;
pub mod us;
