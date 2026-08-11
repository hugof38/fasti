//! Conversions to and from [`chrono`] types, behind the `chrono`
//! feature flag.
//!
//! fasti never uses `chrono` internally — the serial [`Date`]
//! representation stays — but codebases already holding
//! [`chrono::NaiveDate`]s should not have to decompose them by hand
//! at every call site. With this feature enabled, the boundary is one
//! conversion:
//!
//! ```
//! use fasti::{Date, calendars::us};
//!
//! let naive = chrono::NaiveDate::from_ymd_opt(2024, 7, 4).unwrap();
//! let date: Date = naive.try_into()?;
//! assert!(us::SETTLEMENT.is_holiday(date));
//!
//! // And back — infallible, since every fasti Date is a valid
//! // chrono date.
//! let round_trip: chrono::NaiveDate = date.into();
//! assert_eq!(round_trip, naive);
//! # Ok::<(), fasti::TimeError>(())
//! ```
//!
//! The forward direction is [`TryFrom`] because `chrono`'s year range
//! is far wider than fasti's supported 1901..=2199; out-of-range
//! dates return [`TimeError::YearOutOfRange`]. [`Weekday`] and
//! [`Month`] convert infallibly in both directions.

use crate::{Date, Month, TimeError, Weekday};
use chrono::Datelike;

impl TryFrom<chrono::NaiveDate> for Date {
    type Error = TimeError;

    /// Convert a [`chrono::NaiveDate`], refusing dates outside the
    /// supported 1901-01-01..=2199-12-31 range with
    /// [`TimeError::YearOutOfRange`].
    fn try_from(naive: chrono::NaiveDate) -> Result<Self, Self::Error> {
        let year = u16::try_from(naive.year()).map_err(|_| TimeError::YearOutOfRange)?;
        // chrono months are 1..=12 and days 1..=31 by construction, so
        // both narrowings are exact and `from_ymd` can only fail on
        // the year range (checked again inside).
        #[allow(clippy::cast_possible_truncation)]
        let month = Month::try_from_u8(naive.month() as u8)?;
        #[allow(clippy::cast_possible_truncation)]
        let day = naive.day() as u8;
        Self::from_ymd(year, month, day)
    }
}

impl From<Date> for chrono::NaiveDate {
    /// Convert to a [`chrono::NaiveDate`]. Infallible — every date in
    /// fasti's supported range is representable in `chrono`.
    fn from(date: Date) -> Self {
        let (y, m, d) = date.to_ymd();
        // `from_ymd_opt` is `Some` for every valid Gregorian date in
        // chrono's range, which contains all of 1901..=2199; the
        // `unwrap_or_default` arm is unreachable and exists only to
        // honour the crate's no-panic contract.
        Self::from_ymd_opt(i32::from(y.get()), u32::from(m.get()), u32::from(d)).unwrap_or_default()
    }
}

impl From<chrono::Weekday> for Weekday {
    /// Both types use ISO numbering (Monday = 1 .. Sunday = 7), so the
    /// mapping is direct.
    fn from(w: chrono::Weekday) -> Self {
        match w {
            chrono::Weekday::Mon => Self::Mon,
            chrono::Weekday::Tue => Self::Tue,
            chrono::Weekday::Wed => Self::Wed,
            chrono::Weekday::Thu => Self::Thu,
            chrono::Weekday::Fri => Self::Fri,
            chrono::Weekday::Sat => Self::Sat,
            chrono::Weekday::Sun => Self::Sun,
        }
    }
}

impl From<Weekday> for chrono::Weekday {
    fn from(w: Weekday) -> Self {
        match w {
            Weekday::Mon => Self::Mon,
            Weekday::Tue => Self::Tue,
            Weekday::Wed => Self::Wed,
            Weekday::Thu => Self::Thu,
            Weekday::Fri => Self::Fri,
            Weekday::Sat => Self::Sat,
            Weekday::Sun => Self::Sun,
        }
    }
}

impl From<chrono::Month> for Month {
    fn from(m: chrono::Month) -> Self {
        match m {
            chrono::Month::January => Self::Jan,
            chrono::Month::February => Self::Feb,
            chrono::Month::March => Self::Mar,
            chrono::Month::April => Self::Apr,
            chrono::Month::May => Self::May,
            chrono::Month::June => Self::Jun,
            chrono::Month::July => Self::Jul,
            chrono::Month::August => Self::Aug,
            chrono::Month::September => Self::Sep,
            chrono::Month::October => Self::Oct,
            chrono::Month::November => Self::Nov,
            chrono::Month::December => Self::Dec,
        }
    }
}

impl From<Month> for chrono::Month {
    fn from(m: Month) -> Self {
        match m {
            Month::Jan => Self::January,
            Month::Feb => Self::February,
            Month::Mar => Self::March,
            Month::Apr => Self::April,
            Month::May => Self::May,
            Month::Jun => Self::June,
            Month::Jul => Self::July,
            Month::Aug => Self::August,
            Month::Sep => Self::September,
            Month::Oct => Self::October,
            Month::Nov => Self::November,
            Month::Dec => Self::December,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn naive_date_converts_at_anchors() {
        for (y, m, d) in [(1901, 1, 1), (2024, 2, 29), (2199, 12, 31)] {
            let naive = chrono::NaiveDate::from_ymd_opt(y, m, d).unwrap();
            let date = Date::try_from(naive).unwrap();
            assert_eq!(i32::from(date.year().get()), y);
            assert_eq!(u32::from(date.month().get()), m);
            assert_eq!(u32::from(date.day()), d);
        }
    }

    #[test]
    fn out_of_range_naive_dates_are_refused() {
        for (y, m, d) in [(1900, 12, 31), (2200, 1, 1), (-1, 6, 15), (30000, 1, 1)] {
            let naive = chrono::NaiveDate::from_ymd_opt(y, m, d).unwrap();
            assert_eq!(
                Date::try_from(naive),
                Err(TimeError::YearOutOfRange),
                "{naive}"
            );
        }
    }

    #[test]
    fn weekday_and_month_map_both_ways() {
        for n in 1u8..=7 {
            let ours = Weekday::try_from_u8(n).unwrap();
            let theirs = chrono::Weekday::from(ours);
            assert_eq!(theirs.number_from_monday(), u32::from(n));
            assert_eq!(Weekday::from(theirs), ours);
        }
        for n in 1u8..=12 {
            let ours = Month::try_from_u8(n).unwrap();
            let theirs = chrono::Month::from(ours);
            assert_eq!(theirs.number_from_month(), u32::from(n));
            assert_eq!(Month::from(theirs), ours);
        }
    }

    proptest! {
        /// Every fasti date round-trips through chrono losslessly, and
        /// the two libraries agree on the weekday along the way.
        #[test]
        fn date_round_trips_through_chrono(serial in 0u32..=Date::MAX.serial()) {
            let date = Date::from_serial(serial).unwrap();
            let naive = chrono::NaiveDate::from(date);
            prop_assert_eq!(Date::try_from(naive).unwrap(), date);
            prop_assert_eq!(Weekday::from(naive.weekday()), date.weekday());
        }
    }
}
