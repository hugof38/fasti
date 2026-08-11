//! [`EasterOffset`]: holidays a fixed number of days from Easter Sunday —
//! Good Friday (−2), Easter Monday (+1), Ascension (+39), Whit Monday (+50).
//! Offsets anchor on Easter *Sunday*; `QuantLib` indexes on Easter Monday
//! and the constructors here translate.

use crate::{Date, EasterMethod, Month, Year, YearRange, easter_monday};

/// A holiday rule fired `days` away from Easter Sunday under a given
/// [`EasterMethod`]. Constructors default to Western (Gregorian) Easter;
/// use [`new_orthodox`](Self::new_orthodox) for Julian.
///
/// ```
/// use fasti::{Date, EasterOffset, Month};
///
/// let good_friday = EasterOffset::good_friday();
/// // 2024 Easter Sunday = March 31, so Good Friday = March 29.
/// assert!(good_friday.is_holiday(Date::from_ymd(2024, Month::Mar, 29)?));
/// # Ok::<(), fasti::TimeError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EasterOffset {
    /// Offset in days from Easter Sunday (Good Friday = −2, Easter
    /// Monday = +1, Ascension = +39, Whit Monday = +50).
    days: i16,
    method: EasterMethod,
    years: YearRange,
}

impl EasterOffset {
    /// Construct a Western (Gregorian) Easter-offset rule; `days` is
    /// measured from Easter Sunday.
    ///
    /// ```
    /// use fasti::EasterOffset;
    /// // Ascension Thursday, 39 days after Easter Sunday.
    /// let ascension = EasterOffset::new(39);
    /// ```
    #[must_use]
    pub const fn new(days: i16) -> Self {
        Self {
            days,
            method: EasterMethod::Western,
            years: YearRange::ALWAYS,
        }
    }

    /// Construct an Orthodox (Julian) Easter-offset rule; `days` is
    /// measured from Orthodox Easter Sunday (Gregorian-expressed).
    ///
    /// ```
    /// use fasti::{Date, EasterOffset, Month};
    /// // Orthodox Easter Monday, 1 day after Orthodox Easter Sunday.
    /// let rule = EasterOffset::new_orthodox(1);
    /// // Orthodox Easter 2024 was May 5 (Gregorian); Monday May 6.
    /// assert!(rule.is_holiday(Date::from_ymd(2024, Month::May, 6)?));
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    #[must_use]
    pub const fn new_orthodox(days: i16) -> Self {
        Self {
            days,
            method: EasterMethod::Orthodox,
            years: YearRange::ALWAYS,
        }
    }

    /// Good Friday — Easter Sunday − 2.
    #[must_use]
    pub const fn good_friday() -> Self {
        Self::new(-2)
    }

    /// Easter Sunday.
    #[must_use]
    pub const fn easter_sunday() -> Self {
        Self::new(0)
    }

    /// Easter Monday — Easter Sunday + 1.
    #[must_use]
    pub const fn easter_monday() -> Self {
        Self::new(1)
    }

    /// Ascension Thursday — Easter Sunday + 39.
    #[must_use]
    pub const fn ascension() -> Self {
        Self::new(39)
    }

    /// Whit Monday / Lundi de Pentecôte — Easter Sunday + 50.
    #[must_use]
    pub const fn whit_monday() -> Self {
        Self::new(50)
    }

    /// Corpus Christi — Easter Sunday + 60.
    #[must_use]
    pub const fn corpus_christi() -> Self {
        Self::new(60)
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

    /// The offset in days from Easter Sunday.
    #[must_use]
    pub const fn days(&self) -> i16 {
        self.days
    }

    /// The computus variant (Western or Orthodox).
    #[must_use]
    pub const fn method(&self) -> EasterMethod {
        self.method
    }

    /// The years over which this rule is active.
    #[must_use]
    pub const fn year_range(&self) -> YearRange {
        self.years
    }

    /// `true` iff `date` is observed as this Easter-relative holiday.
    /// Only `date.year()` is checked; offsets extreme enough to cross a
    /// year boundary need [`Rule::Custom`](super::Rule::Custom).
    #[must_use]
    pub fn is_holiday(&self, date: Date) -> bool {
        let year = date.year();
        if !self.years.contains(year) {
            return false;
        }
        // Lookup returns Easter Monday; offsets are Sunday-relative.
        let em_doy = easter_monday(year, self.method);
        let Ok(jan1) = Date::from_ymd(year.get(), Month::Jan, 1) else {
            return false;
        };
        // em_doy − 2 = zero-based offset of Easter Sunday from Jan 1.
        let Ok(easter_sun) = jan1.add_days(i32::from(em_doy) - 2) else {
            return false;
        };
        let Ok(observed) = easter_sun.add_days(i32::from(self.days)) else {
            return false;
        };
        observed == date
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
    fn good_friday_anchors() {
        let rule = EasterOffset::good_friday();
        // 2024 Western Easter Sunday = March 31 → Good Friday = March 29.
        assert!(rule.is_holiday(ymd(2024, Month::Mar, 29)));
        assert!(!rule.is_holiday(ymd(2024, Month::Mar, 28)));
        // 2025 Western Easter Sunday = April 20 → Good Friday = April 18.
        assert!(rule.is_holiday(ymd(2025, Month::Apr, 18)));
        // 2026 Western Easter Sunday = April 5 → Good Friday = April 3.
        assert!(rule.is_holiday(ymd(2026, Month::Apr, 3)));
    }

    #[test]
    fn easter_monday_anchors() {
        let rule = EasterOffset::easter_monday();
        // 2024: Easter Monday = April 1.
        assert!(rule.is_holiday(ymd(2024, Month::Apr, 1)));
        // 2026: Easter Monday = April 6.
        assert!(rule.is_holiday(ymd(2026, Month::Apr, 6)));
    }

    #[test]
    fn ascension_whit_corpus_christi() {
        // 2024: Ascension May 9, Whit Monday May 20, Corpus Christi May 30.
        assert!(EasterOffset::ascension().is_holiday(ymd(2024, Month::May, 9)));
        assert!(EasterOffset::whit_monday().is_holiday(ymd(2024, Month::May, 20)));
        assert!(EasterOffset::corpus_christi().is_holiday(ymd(2024, Month::May, 30)));
    }

    #[test]
    fn orthodox_method() {
        // 2024 Orthodox Easter Sunday = May 5 (Gregorian), Monday = May 6.
        let ortho_sunday = EasterOffset::new_orthodox(0);
        let ortho_monday = EasterOffset::new_orthodox(1);
        assert!(ortho_sunday.is_holiday(ymd(2024, Month::May, 5)));
        assert!(ortho_monday.is_holiday(ymd(2024, Month::May, 6)));
        // Western Easter Monday 2024 was April 1 — not an Orthodox match.
        assert!(!ortho_monday.is_holiday(ymd(2024, Month::Apr, 1)));
    }

    #[test]
    fn year_range_filter() {
        let rule = EasterOffset::good_friday().from_year(Year::new(2025).unwrap());
        // 2024 Good Friday — excluded.
        assert!(!rule.is_holiday(ymd(2024, Month::Mar, 29)));
        // 2025 Good Friday — included.
        assert!(rule.is_holiday(ymd(2025, Month::Apr, 18)));
    }

    #[test]
    fn rejects_unrelated_dates() {
        let rule = EasterOffset::easter_monday();
        // January dates are never Easter.
        assert!(!rule.is_holiday(ymd(2024, Month::Jan, 1)));
        // December dates are never Easter.
        assert!(!rule.is_holiday(ymd(2024, Month::Dec, 25)));
    }
}
