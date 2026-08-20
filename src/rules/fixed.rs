//! [`FixedDate`]: a holiday that falls on a specific calendar day each
//! year (e.g. July 4), optionally rolled to a nearby business day when
//! the natural date lands on a weekend.

use crate::{Date, Month, Year, YearRange};

/// Which way a fixed-date holiday moves when its natural date falls on
/// a Saturday or Sunday.
///
/// There is one rule for every variant: the holiday is observed on the
/// **first free weekday in that direction** — free meaning no other
/// holiday and no other holiday's substitute is already there. The
/// variants differ only in which weekend day moves, and which way.
/// A weekend is two days, and two rules naming one day are still one
/// holiday, so at most two substitutes ever queue.
///
/// Only [`Calendar`](crate::Calendar) can apply that rule, since only
/// it can see what the other rules have taken; see
/// [`Calendar::is_holiday`](crate::Calendar::is_holiday).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum WeekendShift {
    /// Neither day moves; a weekend holiday is simply lost. France and
    /// TARGET.
    #[default]
    None,
    /// Both days move forwards — the UK and Commonwealth substitute day.
    Forward,
    /// Sunday moves forwards, Saturday does not — the Fed and SIFMA
    /// convention.
    SunForward,
    /// Saturday moves backwards, Sunday forwards — the US federal
    /// convention.
    SatBackSunForward,
}

/// A fixed-date holiday rule.
///
/// A rule matches its holiday's *natural* date only. The substitute
/// day, if the shift grants one, is resolved by the calendar — see
/// [`Calendar::is_holiday`](crate::Calendar::is_holiday).
///
/// ```
/// use fasti::{Date, FixedDate, Month, WeekendShift};
///
/// // US Independence Day: July 4 with the federal weekend shift.
/// let rule = FixedDate::new(Month::Jul, 4).shift(WeekendShift::SatBackSunForward);
/// assert!(rule.is_holiday(Date::from_ymd(2024, Month::Jul, 4)?));
///
/// // 2026: July 4 is a Saturday. The rule still matches the natural
/// // date; the observed Friday comes from the calendar.
/// assert!(rule.is_holiday(Date::from_ymd(2026, Month::Jul, 4)?));
/// assert!(!rule.is_holiday(Date::from_ymd(2026, Month::Jul, 3)?));
/// # Ok::<(), fasti::TimeError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FixedDate {
    month: Month,
    day: u8,
    shift: WeekendShift,
    years: YearRange,
}

impl FixedDate {
    /// Construct a fixed-date rule with no weekend shift, covering all
    /// supported years; refine with [`shift`](Self::shift) etc.
    #[must_use]
    pub const fn new(month: Month, day: u8) -> Self {
        Self {
            month,
            day,
            shift: WeekendShift::None,
            years: YearRange::ALWAYS,
        }
    }

    /// Set the weekend-shift policy.
    #[must_use]
    pub const fn shift(mut self, shift: WeekendShift) -> Self {
        self.shift = shift;
        self
    }

    /// Restrict to years `year..`.
    #[must_use]
    pub const fn from_year(mut self, year: Year) -> Self {
        self.years = YearRange::from_year(year);
        self
    }

    /// Restrict to an explicit year range.
    #[must_use]
    pub const fn years(mut self, range: YearRange) -> Self {
        self.years = range;
        self
    }

    /// The natural month.
    #[must_use]
    pub const fn month(&self) -> Month {
        self.month
    }

    /// The natural day-of-month.
    #[must_use]
    pub const fn day(&self) -> u8 {
        self.day
    }

    /// The weekend-shift policy.
    #[must_use]
    pub const fn weekend_shift(&self) -> WeekendShift {
        self.shift
    }

    /// The years over which this rule is active.
    #[must_use]
    pub const fn year_range(&self) -> YearRange {
        self.years
    }

    /// `true` iff `date` is this holiday's natural date.
    #[must_use]
    pub fn is_holiday(&self, date: Date) -> bool {
        self.years.contains(date.year()) && date.month() == self.month && date.day() == self.day
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn ymd(y: u16, m: Month, d: u8) -> Date {
        Date::from_ymd(y, m, d).unwrap()
    }

    #[test]
    fn matches_the_natural_date_whatever_the_shift() {
        // The shift is a hint for the calendar; it never moves what the
        // rule itself matches.
        for shift in [
            WeekendShift::None,
            WeekendShift::SatBackSunForward,
            WeekendShift::SunForward,
            WeekendShift::Forward,
        ] {
            let rule = FixedDate::new(Month::Jul, 4).shift(shift);
            assert!(rule.is_holiday(ymd(2024, Month::Jul, 4)), "{shift:?}");
            // 2026-07-04 is a Saturday.
            assert!(rule.is_holiday(ymd(2026, Month::Jul, 4)), "{shift:?}");
            assert!(!rule.is_holiday(ymd(2026, Month::Jul, 3)), "{shift:?}");
            assert_eq!(rule.weekend_shift(), shift);
        }
    }

    #[test]
    fn does_not_match_another_month_or_day() {
        let rule = FixedDate::new(Month::Jul, 4);
        assert!(!rule.is_holiday(ymd(2024, Month::Aug, 4)));
        assert!(!rule.is_holiday(ymd(2024, Month::Jul, 5)));
    }

    #[test]
    fn leap_day_rule_matches_only_in_leap_years() {
        let rule = FixedDate::new(Month::Feb, 29);
        assert!(rule.is_holiday(ymd(2024, Month::Feb, 29)));
        assert!(!rule.is_holiday(ymd(2025, Month::Feb, 28)));
    }

    #[test]
    fn year_range_filter() {
        // Juneteenth from 2021 onwards.
        let rule = FixedDate::new(Month::Jun, 19).from_year(Year::new(2021).unwrap());
        assert!(!rule.is_holiday(ymd(2020, Month::Jun, 19)));
        assert!(rule.is_holiday(ymd(2021, Month::Jun, 19)));
        assert!(rule.is_holiday(ymd(2030, Month::Jun, 19)));
        assert_eq!(
            rule.year_range(),
            YearRange::from_year(Year::new(2021).unwrap())
        );
    }

    #[test]
    fn bounded_year_range() {
        let rule = FixedDate::new(Month::Feb, 22).years(YearRange::literal_through(1970));
        assert!(rule.is_holiday(ymd(1970, Month::Feb, 22)));
        assert!(!rule.is_holiday(ymd(1971, Month::Feb, 22)));
    }

    #[test]
    fn accessors_round_trip() {
        let rule = FixedDate::new(Month::Dec, 25);
        assert_eq!(rule.month(), Month::Dec);
        assert_eq!(rule.day(), 25);
        assert_eq!(rule.weekend_shift(), WeekendShift::None);
    }
}
