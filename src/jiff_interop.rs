//! Conversions to and from [`jiff`] types, behind the `jiff` feature
//! flag.
//!
//! ```
//! use fasti::{Date, calendars::us};
//!
//! let civil = jiff::civil::date(2024, 7, 4);
//! let date: Date = civil.try_into()?;
//! assert!(us::SETTLEMENT.is_holiday(date));
//!
//! // And back — infallible, since every fasti Date is a valid jiff
//! // date.
//! let round_trip: jiff::civil::Date = date.into();
//! assert_eq!(round_trip, civil);
//! # Ok::<(), fasti::TimeError>(())
//! ```
//!
//! [`Date`] from jiff is [`TryFrom`] (jiff's year range is wider);
//! [`Weekday`] converts infallibly in both directions. jiff has no month
//! type, so a month crosses the boundary as its number:
//! [`Month::try_from_u8`] one way, [`Month::get`] the other.
//!
//! fasti never uses jiff internally: enabling the feature adds these
//! conversions and nothing else.

use crate::{Date, Month, TimeError, Weekday};

impl TryFrom<jiff::civil::Date> for Date {
    type Error = TimeError;

    /// Convert a [`jiff::civil::Date`], refusing dates outside the
    /// supported 1901-01-01..=2199-12-31 range with
    /// [`TimeError::YearOutOfRange`].
    fn try_from(civil: jiff::civil::Date) -> Result<Self, Self::Error> {
        let year = u16::try_from(civil.year()).map_err(|_| TimeError::YearOutOfRange)?;
        // jiff months are 1..=12 and days 1..=31, so both conversions succeed.
        let month = u8::try_from(civil.month()).map_err(|_| TimeError::MonthOutOfRange)?;
        let day = u8::try_from(civil.day()).map_err(|_| TimeError::DayOutOfRange)?;
        Self::from_ymd(year, Month::try_from_u8(month)?, day)
    }
}

impl From<Date> for jiff::civil::Date {
    /// Convert to a [`jiff::civil::Date`]. Infallible — every date in
    /// fasti's supported range is representable in jiff.
    fn from(date: Date) -> Self {
        let (y, m, d) = date.to_ymd();
        // Years are at most 2199, months 12 and days 31, so every
        // narrowing is exact.
        #[allow(clippy::cast_possible_wrap)]
        let (year, month, day) = (y.get() as i16, m.get() as i8, d as i8);
        // The `unwrap_or_default` arm is unreachable (jiff covers all of
        // 1901..=2199); it exists only to honour the no-panic contract.
        Self::new(year, month, day).unwrap_or_default()
    }
}

impl From<jiff::civil::Weekday> for Weekday {
    /// Both types use ISO numbering (Monday = 1 .. Sunday = 7), so the
    /// mapping is direct.
    fn from(w: jiff::civil::Weekday) -> Self {
        match w {
            jiff::civil::Weekday::Monday => Self::Mon,
            jiff::civil::Weekday::Tuesday => Self::Tue,
            jiff::civil::Weekday::Wednesday => Self::Wed,
            jiff::civil::Weekday::Thursday => Self::Thu,
            jiff::civil::Weekday::Friday => Self::Fri,
            jiff::civil::Weekday::Saturday => Self::Sat,
            jiff::civil::Weekday::Sunday => Self::Sun,
        }
    }
}

impl From<Weekday> for jiff::civil::Weekday {
    fn from(w: Weekday) -> Self {
        match w {
            Weekday::Mon => Self::Monday,
            Weekday::Tue => Self::Tuesday,
            Weekday::Wed => Self::Wednesday,
            Weekday::Thu => Self::Thursday,
            Weekday::Fri => Self::Friday,
            Weekday::Sat => Self::Saturday,
            Weekday::Sun => Self::Sunday,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn civil_date_converts_at_anchors() {
        for (y, m, d) in [(1901, 1, 1), (2024, 2, 29), (2199, 12, 31)] {
            let civil = jiff::civil::date(y, m, d);
            let date = Date::try_from(civil).unwrap();
            assert_eq!(i32::from(date.year().get()), i32::from(y));
            assert_eq!(i32::from(date.month().get()), i32::from(m));
            assert_eq!(i32::from(date.day()), i32::from(d));
        }
    }

    #[test]
    fn out_of_range_civil_dates_are_refused() {
        for (y, m, d) in [(1900, 12, 31), (2200, 1, 1), (-1, 6, 15), (9999, 1, 1)] {
            let civil = jiff::civil::date(y, m, d);
            assert_eq!(
                Date::try_from(civil),
                Err(TimeError::YearOutOfRange),
                "{civil}"
            );
        }
    }

    #[test]
    fn weekday_maps_both_ways() {
        for n in 1u8..=7 {
            let ours = Weekday::try_from_u8(n).unwrap();
            let theirs = jiff::civil::Weekday::from(ours);
            assert_eq!(theirs.to_monday_one_offset(), i8::try_from(n).unwrap());
            assert_eq!(Weekday::from(theirs), ours);
        }
    }

    proptest! {
        /// Every fasti date round-trips through jiff losslessly, and
        /// the two libraries agree on the weekday along the way.
        #[test]
        fn date_round_trips_through_jiff(serial in 0u32..=Date::MAX.serial()) {
            let date = Date::from_serial(serial).unwrap();
            let civil = jiff::civil::Date::from(date);
            prop_assert_eq!(Date::try_from(civil).unwrap(), date);
            prop_assert_eq!(Weekday::from(civil.weekday()), date.weekday());
        }
    }
}
