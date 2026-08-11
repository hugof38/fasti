//! France generic settlement calendar — `QuantLib`'s `France::Settlement`
//! with Ascension and Whit Monday corrected to proper Easter offsets.

use crate::{Calendar, EasterOffset, FixedDate, Month, Rule, Weekend, WeekendShift};

/// The France generic settlement calendar (civil holidays observed by
/// French banks and non-exchange settlement systems).
///
/// | Holiday | Rule |
/// |---|---|
/// | Jour de l'An / New Year's Day | Jan 1 |
/// | Lundi de Pâques / Easter Monday | Easter Monday |
/// | Fête du Travail / Labour Day | May 1 |
/// | Victoire 1945 / V-E Day | May 8 |
/// | Jeudi de l'Ascension / Ascension | Easter Monday + 38 |
/// | Lundi de Pentecôte / Whit Monday | Easter Monday + 49 |
/// | Fête nationale / Bastille Day | Jul 14 |
/// | Assomption / Assumption | Aug 15 |
/// | Toussaint / All Saints' | Nov 1 |
/// | Armistice 1918 / Armistice Day | Nov 11 |
/// | Noël / Christmas | Dec 25 |
///
/// No weekend-shift convention — holidays falling on a Saturday or
/// Sunday are observed only on that date and produce no compensatory
/// weekday observance.
pub const SETTLEMENT: Calendar<'static> = Calendar {
    name: "France settlement",
    weekend: Weekend::SAT_SUN,
    rules: &[
        // Jour de l'An.
        Rule::Fixed(FixedDate::new(Month::Jan, 1).shift(WeekendShift::None)),
        // Lundi de Pâques.
        Rule::Easter(EasterOffset::easter_monday()),
        // Fête du Travail.
        Rule::Fixed(FixedDate::new(Month::May, 1).shift(WeekendShift::None)),
        // Victoire 1945.
        Rule::Fixed(FixedDate::new(Month::May, 8).shift(WeekendShift::None)),
        // Jeudi de l'Ascension.
        Rule::Easter(EasterOffset::ascension()),
        // Lundi de Pentecôte.
        Rule::Easter(EasterOffset::whit_monday()),
        // Fête nationale.
        Rule::Fixed(FixedDate::new(Month::Jul, 14).shift(WeekendShift::None)),
        // Assomption.
        Rule::Fixed(FixedDate::new(Month::Aug, 15).shift(WeekendShift::None)),
        // Toussaint.
        Rule::Fixed(FixedDate::new(Month::Nov, 1).shift(WeekendShift::None)),
        // Armistice 1918.
        Rule::Fixed(FixedDate::new(Month::Nov, 11).shift(WeekendShift::None)),
        // Noël.
        Rule::Fixed(FixedDate::new(Month::Dec, 25).shift(WeekendShift::None)),
    ],
};

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::Date;

    fn ymd(y: u16, m: Month, d: u8) -> Date {
        Date::from_ymd(y, m, d).unwrap()
    }

    /// All 11 French Settlement holidays for 2024. Easter Sunday 2024
    /// = March 31, so Easter Monday = April 1, Ascension = May 9,
    /// Whit Monday = May 20.
    #[test]
    fn france_settlement_holidays_2024() {
        assert!(SETTLEMENT.is_holiday(ymd(2024, Month::Jan, 1))); // New Year's
        assert!(SETTLEMENT.is_holiday(ymd(2024, Month::Apr, 1))); // Easter Monday
        assert!(SETTLEMENT.is_holiday(ymd(2024, Month::May, 1))); // Labour
        assert!(SETTLEMENT.is_holiday(ymd(2024, Month::May, 8))); // V-E Day
        assert!(SETTLEMENT.is_holiday(ymd(2024, Month::May, 9))); // Ascension
        assert!(SETTLEMENT.is_holiday(ymd(2024, Month::May, 20))); // Whit Monday
        assert!(SETTLEMENT.is_holiday(ymd(2024, Month::Jul, 14))); // Bastille
        assert!(SETTLEMENT.is_holiday(ymd(2024, Month::Aug, 15))); // Assumption
        assert!(SETTLEMENT.is_holiday(ymd(2024, Month::Nov, 1))); // All Saints'
        assert!(SETTLEMENT.is_holiday(ymd(2024, Month::Nov, 11))); // Armistice
        assert!(SETTLEMENT.is_holiday(ymd(2024, Month::Dec, 25))); // Christmas
    }

    /// Easter-anchored holidays shift with Easter across years.
    #[test]
    fn france_easter_anchors_shift_by_year() {
        // 2025 Easter Sunday = April 20 → Easter Monday = April 21.
        assert!(SETTLEMENT.is_holiday(ymd(2025, Month::Apr, 21)));
        // 2025 Ascension = May 29.
        assert!(SETTLEMENT.is_holiday(ymd(2025, Month::May, 29)));
        // 2025 Whit Monday = June 9.
        assert!(SETTLEMENT.is_holiday(ymd(2025, Month::Jun, 9)));
    }

    /// Civil holidays falling on weekends are simply lost — no
    /// weekday observance.
    #[test]
    fn france_no_weekend_shift() {
        // Jul 14 2024 was a Sunday. No Monday observance.
        assert!(!SETTLEMENT.is_holiday(ymd(2024, Month::Jul, 15)));
        // But it is still a holiday on the natural date (which is
        // also a weekend, so not a business day either way).
        assert!(SETTLEMENT.is_holiday(ymd(2024, Month::Jul, 14)));
        assert!(!SETTLEMENT.is_business_day(ymd(2024, Month::Jul, 14)));
        // May 1 2022 was a Sunday. No Monday observance.
        assert!(!SETTLEMENT.is_holiday(ymd(2022, Month::May, 2)));
    }

    /// Settlement does NOT observe Good Friday (that's Exchange only).
    #[test]
    fn france_settlement_no_good_friday() {
        // 2024 Good Friday = March 29.
        assert!(!SETTLEMENT.is_holiday(ymd(2024, Month::Mar, 29)));
    }
}
