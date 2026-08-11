//! [`NthWeekday`]: holidays that fall on the Nth occurrence of a specific
//! weekday in a given month — "third Monday of January" (MLK Day),
//! "fourth Thursday of November" (Thanksgiving), etc.

use crate::{Date, Month, Ordinal, Weekday, Year, YearRange};

/// A holiday rule that fires on the Nth occurrence of a weekday in a
/// month (e.g. "Third Monday of January").
///
/// ```
/// use fasti::{Date, Month, NthWeekday, Ordinal, Weekday};
///
/// // MLK Day: third Monday of January.
/// let mlk = NthWeekday::new(Ordinal::Third, Weekday::Mon, Month::Jan);
///
/// // 2026-01-19 is the third Monday of January 2026.
/// assert!(mlk.is_holiday(Date::from_ymd(2026, Month::Jan, 19)?));
///
/// // Fourth Monday is not MLK Day.
/// assert!(!mlk.is_holiday(Date::from_ymd(2026, Month::Jan, 26)?));
/// # Ok::<(), fasti::TimeError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NthWeekday {
    n: Ordinal,
    weekday: Weekday,
    month: Month,
    years: YearRange,
}

impl NthWeekday {
    /// Construct an Nth-weekday-of-month rule, covering all supported
    /// years. Refine with [`from_year`](Self::from_year) or
    /// [`years`](Self::years).
    #[must_use]
    pub const fn new(n: Ordinal, weekday: Weekday, month: Month) -> Self {
        Self {
            n,
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

    /// The ordinal position within the month.
    #[must_use]
    pub const fn n(&self) -> Ordinal {
        self.n
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

    /// `true` iff `date` is the Nth occurrence of this weekday in this
    /// month of its year.
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
        // Day-of-month `d` → occurrence `(d - 1) / 7 + 1`:
        //   1..=7  → 1st, 8..=14 → 2nd, 15..=21 → 3rd, ...
        let occurrence = (date.day() - 1) / 7 + 1;
        occurrence == self.n.get()
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
    fn third_monday_of_january_is_mlk() {
        let mlk = NthWeekday::new(Ordinal::Third, Weekday::Mon, Month::Jan);
        // Third Mondays of January across several years (independently
        // verified).
        assert!(mlk.is_holiday(ymd(2024, Month::Jan, 15)));
        assert!(mlk.is_holiday(ymd(2025, Month::Jan, 20)));
        assert!(mlk.is_holiday(ymd(2026, Month::Jan, 19)));
    }

    #[test]
    fn fourth_thursday_of_november_is_thanksgiving() {
        let tg = NthWeekday::new(Ordinal::Fourth, Weekday::Thu, Month::Nov);
        assert!(tg.is_holiday(ymd(2024, Month::Nov, 28)));
        assert!(tg.is_holiday(ymd(2025, Month::Nov, 27)));
        assert!(tg.is_holiday(ymd(2026, Month::Nov, 26)));
    }

    #[test]
    fn rejects_other_weekdays_and_months() {
        let mlk = NthWeekday::new(Ordinal::Third, Weekday::Mon, Month::Jan);
        // Third Tuesday of January is not MLK.
        assert!(!mlk.is_holiday(ymd(2026, Month::Jan, 20)));
        // Third Monday of February is not MLK (though it is Presidents').
        assert!(!mlk.is_holiday(ymd(2026, Month::Feb, 16)));
    }

    #[test]
    fn fifth_occurrence_may_not_exist() {
        let rule = NthWeekday::new(Ordinal::Fifth, Weekday::Fri, Month::Jan);
        // January 2026 has a fifth Friday (30th).
        assert!(rule.is_holiday(ymd(2026, Month::Jan, 30)));
        // January 2027 — fifth Friday is 29th.
        assert!(rule.is_holiday(ymd(2027, Month::Jan, 29)));
        // The fourth Friday of January 2026 is the 23rd; not a match.
        assert!(!rule.is_holiday(ymd(2026, Month::Jan, 23)));
    }

    #[test]
    fn year_range_filter() {
        // MLK Day federal since 1986.
        let rule = NthWeekday::new(Ordinal::Third, Weekday::Mon, Month::Jan)
            .from_year(Year::new(1986).unwrap());
        // 1985 third Monday of January is January 21 — NOT a holiday
        // under this rule (pre-federal).
        assert!(!rule.is_holiday(ymd(1985, Month::Jan, 21)));
        assert!(rule.is_holiday(ymd(1986, Month::Jan, 20)));
    }
}
