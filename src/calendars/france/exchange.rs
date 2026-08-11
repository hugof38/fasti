//! France exchange (Euronext Paris / Paris Bourse) calendar — port of
//! `QuantLib`'s `France::Exchange`.

use crate::{Calendar, EasterOffset, FixedDate, Month, Rule, Weekend, WeekendShift};

/// The France exchange calendar (Euronext Paris).
///
/// A reduced set versus Settlement — the stock exchange does not
/// observe civil-only French holidays (Bastille Day, V-E Day, All
/// Saints', Armistice, Assumption) but does observe Good Friday and
/// the December exchange closures (Christmas Eve, Dec 26, New
/// Year's Eve).
///
/// | Holiday | Rule |
/// |---|---|
/// | Jour de l'An | Jan 1 |
/// | Vendredi saint / Good Friday | Easter Monday − 3 |
/// | Lundi de Pâques / Easter Monday | Easter Monday |
/// | Fête du Travail | May 1 |
/// | Veille de Noël / Christmas Eve | Dec 24 |
/// | Noël / Christmas | Dec 25 |
/// | Lendemain de Noël / Second day of Christmas | Dec 26 |
/// | Saint-Sylvestre / New Year's Eve | Dec 31 |
///
/// Note: Dec 26 is not a French civil holiday outside Alsace-Moselle,
/// but Euronext closes its markets pan-European on that date — this
/// entry is an exchange convention, not a civil observance. Some
/// sources (including `QuantLib`'s port) label it "Boxing Day"; the
/// native French label is `Lendemain de Noël`.
pub const EXCHANGE: Calendar<'static> = Calendar {
    name: "France exchange",
    weekend: Weekend::SAT_SUN,
    rules: &[
        Rule::Fixed(FixedDate::new(Month::Jan, 1).shift(WeekendShift::None)),
        Rule::Easter(EasterOffset::good_friday()),
        Rule::Easter(EasterOffset::easter_monday()),
        Rule::Fixed(FixedDate::new(Month::May, 1).shift(WeekendShift::None)),
        Rule::Fixed(FixedDate::new(Month::Dec, 24).shift(WeekendShift::None)),
        Rule::Fixed(FixedDate::new(Month::Dec, 25).shift(WeekendShift::None)),
        Rule::Fixed(FixedDate::new(Month::Dec, 26).shift(WeekendShift::None)),
        Rule::Fixed(FixedDate::new(Month::Dec, 31).shift(WeekendShift::None)),
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

    /// 2024 Paris exchange holidays.
    #[test]
    fn france_exchange_holidays_2024() {
        assert!(EXCHANGE.is_holiday(ymd(2024, Month::Jan, 1)));
        assert!(EXCHANGE.is_holiday(ymd(2024, Month::Mar, 29))); // Good Friday
        assert!(EXCHANGE.is_holiday(ymd(2024, Month::Apr, 1))); // Easter Monday
        assert!(EXCHANGE.is_holiday(ymd(2024, Month::May, 1)));
        assert!(EXCHANGE.is_holiday(ymd(2024, Month::Dec, 24)));
        assert!(EXCHANGE.is_holiday(ymd(2024, Month::Dec, 25)));
        assert!(EXCHANGE.is_holiday(ymd(2024, Month::Dec, 26)));
        assert!(EXCHANGE.is_holiday(ymd(2024, Month::Dec, 31)));
    }

    /// Exchange excludes civil holidays that Settlement observes.
    #[test]
    fn france_exchange_excludes_civil_holidays() {
        assert!(!EXCHANGE.is_holiday(ymd(2024, Month::May, 8))); // V-E Day
        assert!(!EXCHANGE.is_holiday(ymd(2024, Month::May, 9))); // Ascension
        assert!(!EXCHANGE.is_holiday(ymd(2024, Month::May, 20))); // Whit Monday
        assert!(!EXCHANGE.is_holiday(ymd(2024, Month::Jul, 14))); // Bastille
        assert!(!EXCHANGE.is_holiday(ymd(2024, Month::Aug, 15))); // Assumption
        assert!(!EXCHANGE.is_holiday(ymd(2024, Month::Nov, 1))); // All Saints'
        assert!(!EXCHANGE.is_holiday(ymd(2024, Month::Nov, 11))); // Armistice
    }

    /// Good Friday shifts with Easter.
    #[test]
    fn france_exchange_good_friday_shifts() {
        // 2025 Good Friday = April 18.
        assert!(EXCHANGE.is_holiday(ymd(2025, Month::Apr, 18)));
        // 2026 Good Friday = April 3.
        assert!(EXCHANGE.is_holiday(ymd(2026, Month::Apr, 3)));
    }
}
