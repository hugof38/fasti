//! [`LastWeekday`]: holidays that fall on the last occurrence of a
//! specific weekday in a given month — "last Monday of May" (US Memorial
//! Day), "last Saturday of April" (some regional observances).

use crate::{Date, Month, Weekday, Year, YearRange};

/// A holiday rule that fires on the last occurrence of a weekday in a
/// month.
///
/// ```
/// use fasti::{Date, LastWeekday, Month, Weekday};
///
/// // US Memorial Day: last Monday of May.
/// let memorial = LastWeekday::new(Weekday::Mon, Month::May);
/// assert!(memorial.is_holiday(Date::from_ymd(2026, Month::May, 25)?));
/// assert!(!memorial.is_holiday(Date::from_ymd(2026, Month::May, 18)?));
/// # Ok::<(), fasti::TimeError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LastWeekday {
    weekday: Weekday,
    month: Month,
    years: YearRange,
}

impl LastWeekday {
    /// Construct a last-weekday-of-month rule covering all supported years.
    #[must_use]
    pub const fn new(weekday: Weekday, month: Month) -> Self {
        Self {
            weekday,
            month,
            years: YearRange::ALWAYS,
        }
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

    /// The target weekday.
    #[must_use]
    pub const fn weekday(&self) -> Weekday {
        self.weekday
    }

    /// The target month.
    #[must_use]
    pub const fn month(&self) -> Month {
        self.month
    }

    /// The years over which this rule is active.
    #[must_use]
    pub const fn year_range(&self) -> YearRange {
        self.years
    }

    /// `true` iff `date` is the last occurrence of this weekday in its
    /// month (and the month / year match the rule).
    #[must_use]
    pub fn is_holiday(&self, date: Date) -> bool {
        if !self.years.contains(date.year()) {
            return false;
        }
        if date.month() as u8 != self.month as u8 {
            return false;
        }
        if date.weekday() as u8 != self.weekday as u8 {
            return false;
        }
        // Last occurrence iff stepping forward 7 days leaves the month.
        match date.add_days(7) {
            Ok(next) => next.month() as u8 != self.month as u8,
            Err(_) => true,
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
    fn last_monday_of_may_is_memorial_day() {
        let rule = LastWeekday::new(Weekday::Mon, Month::May);
        // Independently verified last-Mondays.
        assert!(rule.is_holiday(ymd(2024, Month::May, 27)));
        assert!(rule.is_holiday(ymd(2025, Month::May, 26)));
        assert!(rule.is_holiday(ymd(2026, Month::May, 25)));
    }

    #[test]
    fn earlier_mondays_are_not_last() {
        let rule = LastWeekday::new(Weekday::Mon, Month::May);
        // 2026 Mondays in May: 4, 11, 18, 25.
        assert!(!rule.is_holiday(ymd(2026, Month::May, 4)));
        assert!(!rule.is_holiday(ymd(2026, Month::May, 11)));
        assert!(!rule.is_holiday(ymd(2026, Month::May, 18)));
        assert!(rule.is_holiday(ymd(2026, Month::May, 25)));
    }

    #[test]
    fn rejects_other_weekdays_and_months() {
        let rule = LastWeekday::new(Weekday::Mon, Month::May);
        // Last Tuesday of May isn't a match.
        assert!(!rule.is_holiday(ymd(2026, Month::May, 26)));
        // Last Monday of June isn't a match.
        assert!(!rule.is_holiday(ymd(2026, Month::Jun, 29)));
    }

    #[test]
    fn last_day_of_month_edge_cases() {
        // Last Friday of February 2024 (leap year): Feb 23.
        let rule = LastWeekday::new(Weekday::Fri, Month::Feb);
        assert!(rule.is_holiday(ymd(2024, Month::Feb, 23)));
        assert!(!rule.is_holiday(ymd(2024, Month::Feb, 16)));
    }
}
