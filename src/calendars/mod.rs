//! Built-in calendars, grouped by country / market. Every calendar is a
//! `pub const Calendar<'static>`, passed by value (`Copy`).
//!
//! ```
//! use fasti::{Date, Month, calendars::us};
//! assert!(us::SETTLEMENT.is_holiday(Date::from_ymd(2024, Month::Jul, 4)?));
//! # Ok::<(), fasti::TimeError>(())
//! ```
//!
//! Market-neutral baselines: [`WEEKENDS_ONLY`] and [`NULL_CALENDAR`].
//! [`TARGET`] is currency-wide rather than national, so it sits here too.

use crate::{Calendar, Weekend};

pub mod france;
mod target;
pub mod uk;
pub mod us;

pub use target::TARGET;

/// Saturday/Sunday weekend, no holidays — the default when no market
/// holiday set applies.
///
/// ```
/// use fasti::{Date, Month, calendars};
/// let sat = Date::from_ymd(2024, Month::Jul, 6)?;
/// assert!(!calendars::WEEKENDS_ONLY.is_business_day(sat));
/// assert!(calendars::WEEKENDS_ONLY.is_business_day(sat.add_days(2)?));
/// # Ok::<(), fasti::TimeError>(())
/// ```
pub const WEEKENDS_ONLY: Calendar<'static> = Calendar {
    name: "Weekends only",
    weekend: Weekend::SAT_SUN,
    rules: &[],
};

/// No weekend, no holidays — every day is a business day, as in
/// `QuantLib`'s `NullCalendar`. Adjustment is always the identity.
///
/// ```
/// use fasti::{Date, Month, calendars};
/// let sun = Date::from_ymd(2024, Month::Jul, 7)?;
/// assert!(calendars::NULL_CALENDAR.is_business_day(sun));
/// # Ok::<(), fasti::TimeError>(())
/// ```
pub const NULL_CALENDAR: Calendar<'static> = Calendar {
    name: "Null calendar",
    weekend: Weekend::NONE,
    rules: &[],
};

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::{BusinessDayConvention, Date, Month};

    #[test]
    fn weekends_only_skips_weekends_and_nothing_else() {
        // A famous holiday on a weekday is still a business day.
        let jul4 = Date::from_ymd(2024, Month::Jul, 4).unwrap(); // Thursday
        assert!(WEEKENDS_ONLY.is_business_day(jul4));
        // Saturday and Sunday are not.
        let sat = Date::from_ymd(2024, Month::Jul, 6).unwrap();
        assert!(!WEEKENDS_ONLY.is_business_day(sat));
        assert!(!WEEKENDS_ONLY.is_business_day(sat.add_days(1).unwrap()));
        assert!(!WEEKENDS_ONLY.is_holiday(sat)); // weekend ≠ holiday
    }

    #[test]
    fn null_calendar_treats_every_day_as_business() {
        // Scan a full leap year: every day is a business day.
        let mut d = Date::from_ymd(2024, Month::Jan, 1).unwrap();
        let last = Date::from_ymd(2024, Month::Dec, 31).unwrap();
        while d <= last {
            assert!(NULL_CALENDAR.is_business_day(d));
            assert_eq!(
                NULL_CALENDAR
                    .adjust(d, BusinessDayConvention::ModifiedFollowing)
                    .unwrap(),
                d,
            );
            d = d.add_days(1).unwrap();
        }
    }
}
