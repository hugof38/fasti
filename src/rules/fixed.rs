//! [`FixedDate`]: a holiday that falls on a specific calendar day each
//! year (e.g. July 4), optionally rolled to a nearby business day when
//! the natural date lands on a weekend.

use crate::{Date, Month, Weekday, Year, YearRange};

/// Policy for observing a fixed-date holiday when its natural date falls
/// on a Saturday or Sunday.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum WeekendShift {
    /// Do not shift; a weekend holiday is simply lost.
    None,
    /// US federal convention: Saturday → Friday, Sunday → Monday.
    SatBackSunForward,
    /// UK convention: Sunday → Monday; Saturday unchanged.
    SunForward,
}

/// A fixed-date holiday rule.
///
/// ```
/// use fasti::{Date, FixedDate, Month, WeekendShift};
///
/// // US Independence Day: July 4 with federal weekend shift.
/// let rule = FixedDate::new(Month::Jul, 4).shift(WeekendShift::SatBackSunForward);
///
/// // 2024: July 4 is a Thursday — observed on the natural date.
/// assert!(rule.is_holiday(Date::from_ymd(2024, Month::Jul, 4)?));
///
/// // 2026: July 4 is a Saturday — observed back on Friday July 3.
/// assert!(rule.is_holiday(Date::from_ymd(2026, Month::Jul, 3)?));
/// assert!(!rule.is_holiday(Date::from_ymd(2026, Month::Jul, 4)?));
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

    /// `true` iff `date` is observed as this holiday. Shifts crossing a
    /// year boundary (e.g. Jan 1 → Dec 31) are handled.
    #[must_use]
    pub fn is_holiday(&self, date: Date) -> bool {
        // A shift can cross a year boundary; check a ±1-year window.
        let mid = date.year().get();
        let candidates = [
            mid.saturating_sub(1),
            mid,
            mid.saturating_add(1).min(Year::MAX.get()),
        ];
        for cand_year in candidates {
            let Ok(y) = Year::new(cand_year) else {
                continue;
            };
            if !self.years.contains(y) {
                continue;
            }
            let Ok(natural) = Date::from_ymd(y.get(), self.month, self.day) else {
                continue;
            };
            let Some(observed) = self.apply_shift(natural) else {
                continue;
            };
            if observed == date {
                return true;
            }
        }
        false
    }

    /// Observed date under the shift policy, or [`None`] if it would
    /// leave the supported date range.
    fn apply_shift(self, natural: Date) -> Option<Date> {
        match self.shift {
            WeekendShift::None => Some(natural),
            WeekendShift::SatBackSunForward => match natural.weekday() {
                Weekday::Sat => natural.add_days(-1).ok(),
                Weekday::Sun => natural.add_days(1).ok(),
                _ => Some(natural),
            },
            WeekendShift::SunForward => match natural.weekday() {
                Weekday::Sun => natural.add_days(1).ok(),
                _ => Some(natural),
            },
        }
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
    fn natural_no_shift() {
        let rule = FixedDate::new(Month::Jul, 4);
        assert!(rule.is_holiday(ymd(2024, Month::Jul, 4)));
        // 2026-07-04 is a Saturday — still the holiday with no shift.
        assert!(rule.is_holiday(ymd(2026, Month::Jul, 4)));
        assert!(!rule.is_holiday(ymd(2026, Month::Jul, 3)));
    }

    #[test]
    fn sat_back_sun_forward() {
        let rule = FixedDate::new(Month::Jul, 4).shift(WeekendShift::SatBackSunForward);
        // 2024-07-04 = Thursday → natural.
        assert!(rule.is_holiday(ymd(2024, Month::Jul, 4)));
        // 2026-07-04 = Saturday → observed Friday 2026-07-03.
        assert!(rule.is_holiday(ymd(2026, Month::Jul, 3)));
        assert!(!rule.is_holiday(ymd(2026, Month::Jul, 4)));
        // 2021-07-04 = Sunday → observed Monday 2021-07-05.
        assert!(rule.is_holiday(ymd(2021, Month::Jul, 5)));
        assert!(!rule.is_holiday(ymd(2021, Month::Jul, 4)));
    }

    #[test]
    fn sun_forward_only() {
        // Boxing Day, UK style: Sat stays, Sun shifts to Mon.
        let rule = FixedDate::new(Month::Dec, 26).shift(WeekendShift::SunForward);
        // 2021-12-26 = Sun → observed Mon 2021-12-27.
        assert!(rule.is_holiday(ymd(2021, Month::Dec, 27)));
        assert!(!rule.is_holiday(ymd(2021, Month::Dec, 26)));
        // 2020-12-26 = Sat → unchanged under SunForward.
        assert!(rule.is_holiday(ymd(2020, Month::Dec, 26)));
    }

    #[test]
    fn cross_year_shift() {
        // Jan 1 2022 = Saturday → observed Fri Dec 31 2021 under SatBack.
        let rule = FixedDate::new(Month::Jan, 1).shift(WeekendShift::SatBackSunForward);
        assert!(rule.is_holiday(ymd(2021, Month::Dec, 31)));
        assert!(!rule.is_holiday(ymd(2022, Month::Jan, 1)));
    }

    #[test]
    fn year_range_filter() {
        // Juneteenth from 2021 onwards.
        let rule = FixedDate::new(Month::Jun, 19)
            .shift(WeekendShift::SatBackSunForward)
            .from_year(Year::new(2021).unwrap());
        assert!(!rule.is_holiday(ymd(2020, Month::Jun, 19)));
        assert!(rule.is_holiday(ymd(2021, Month::Jun, 18))); // 2021-06-19 was Saturday
        assert!(rule.is_holiday(ymd(2024, Month::Jun, 19))); // Wednesday
    }

    #[test]
    fn leap_day_rule_falls_off_in_non_leap_years() {
        // Fictitious "leap day holiday" — Feb 29.
        let rule = FixedDate::new(Month::Feb, 29);
        assert!(rule.is_holiday(ymd(2024, Month::Feb, 29)));
        assert!(!rule.is_holiday(ymd(2025, Month::Mar, 1))); // no shift
    }
}
