//! TARGET — the euro-area settlement calendar.

use crate::{Calendar, Date, EasterOffset, FixedDate, Month, OneOff, Rule, Weekend, Year};

/// TARGET / T2, the Eurosystem's real-time gross settlement calendar —
/// a port of `QuantLib`'s
/// [`TARGET`](https://github.com/lballabio/QuantLib/blob/master/ql/time/calendars/target.cpp).
/// The euro money-market default.
///
/// | Holiday | Rule |
/// |---|---|
/// | New Year's Day | Jan 1 |
/// | Good Friday | Easter Sunday − 2 *[since 2000]* |
/// | Easter Monday | Easter Sunday + 1 *[since 2000]* |
/// | Labour Day | May 1 *[since 2000]* |
/// | Christmas | Dec 25 |
/// | Day of Goodwill | Dec 26 *[since 2000]* |
/// | Millennium / changeover closings | Dec 31 of 1998, 1999 and 2001 |
///
/// No weekend shift: a holiday falling on a weekend is simply lost.
///
/// ```
/// use fasti::{Date, Month, calendars};
/// // 2024: Good Friday and Easter Monday both closed.
/// assert!(calendars::TARGET.is_holiday(Date::from_ymd(2024, Month::Mar, 29)?));
/// assert!(calendars::TARGET.is_holiday(Date::from_ymd(2024, Month::Apr, 1)?));
/// // US Independence Day is an ordinary business day.
/// assert!(calendars::TARGET.is_business_day(Date::from_ymd(2024, Month::Jul, 4)?));
/// # Ok::<(), fasti::TimeError>(())
/// ```
pub const TARGET: Calendar<'static> = Calendar {
    name: "TARGET",
    weekend: Weekend::SAT_SUN,
    rules: &[
        // New Year's Day.
        Rule::Fixed(FixedDate::new(Month::Jan, 1)),
        // Good Friday.
        Rule::Easter(EasterOffset::good_friday().from_year(Year::literal(2000))),
        // Easter Monday.
        Rule::Easter(EasterOffset::easter_monday().from_year(Year::literal(2000))),
        // Labour Day.
        Rule::Fixed(FixedDate::new(Month::May, 1).from_year(Year::literal(2000))),
        // Christmas.
        Rule::Fixed(FixedDate::new(Month::Dec, 25)),
        // Day of Goodwill.
        Rule::Fixed(FixedDate::new(Month::Dec, 26).from_year(Year::literal(2000))),
        // Euro changeover and millennium closings — three years only.
        Rule::OneOff(OneOff::new(Date::literal(1998, Month::Dec, 31))),
        Rule::OneOff(OneOff::new(Date::literal(1999, Month::Dec, 31))),
        Rule::OneOff(OneOff::new(Date::literal(2001, Month::Dec, 31))),
    ],
};

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn ymd(y: u16, m: Month, d: u8) -> Date {
        Date::from_ymd(y, m, d).unwrap()
    }

    /// Every TARGET closing in a year, from the published Eurosystem
    /// calendars. Weekend closings are excluded.
    fn holidays(year: u16) -> alloc::vec::Vec<Date> {
        let jan1 = ymd(year, Month::Jan, 1);
        let range = jan1..ymd(year, Month::Dec, 31).add_days(1).unwrap();
        TARGET
            .holidays(range)
            .filter(|d| !TARGET.is_weekend(*d))
            .collect()
    }

    #[test]
    fn published_closings_2024() {
        assert_eq!(
            holidays(2024),
            [
                ymd(2024, Month::Jan, 1),  // New Year's Day (Mon)
                ymd(2024, Month::Mar, 29), // Good Friday
                ymd(2024, Month::Apr, 1),  // Easter Monday
                ymd(2024, Month::May, 1),  // Labour Day (Wed)
                ymd(2024, Month::Dec, 25), // Christmas (Wed)
                ymd(2024, Month::Dec, 26), // Day of Goodwill (Thu)
            ],
        );
    }

    #[test]
    fn published_closings_2026() {
        assert_eq!(
            holidays(2026),
            [
                ymd(2026, Month::Jan, 1), // Thursday
                ymd(2026, Month::Apr, 3), // Good Friday
                ymd(2026, Month::Apr, 6), // Easter Monday
                ymd(2026, Month::May, 1), // Friday
                ymd(2026, Month::Dec, 25), // Friday
                                          // Dec 26 2026 is a Saturday — lost, no shift.
            ],
        );
    }

    #[test]
    fn easter_labour_and_goodwill_start_in_2000() {
        // 1999: Good Friday Apr 2, Easter Monday Apr 5, May 1 (Sat),
        // Dec 26 (Sun) — none observed before 2000.
        for d in [
            ymd(1999, Month::Apr, 2),
            ymd(1999, Month::Apr, 5),
            ymd(1999, Month::May, 1),
            ymd(1999, Month::Dec, 26),
        ] {
            assert!(!TARGET.is_holiday(d), "{d} should not be a TARGET holiday");
        }
        // New Year's Day and Christmas apply in every year.
        assert!(TARGET.is_holiday(ymd(1999, Month::Jan, 1)));
        assert!(TARGET.is_holiday(ymd(1999, Month::Dec, 25)));
    }

    #[test]
    fn year_end_closings_are_three_years_only() {
        assert!(TARGET.is_holiday(ymd(1998, Month::Dec, 31)));
        assert!(TARGET.is_holiday(ymd(1999, Month::Dec, 31)));
        assert!(TARGET.is_holiday(ymd(2001, Month::Dec, 31)));
        // Not 2000, and not since.
        assert!(!TARGET.is_holiday(ymd(2000, Month::Dec, 31)));
        assert!(!TARGET.is_holiday(ymd(2024, Month::Dec, 31)));
    }

    #[test]
    fn weekend_holidays_are_lost_not_shifted() {
        // Jan 1 2022 was a Saturday; no substitute Monday.
        assert!(!TARGET.is_holiday(ymd(2022, Month::Jan, 3)));
        assert!(TARGET.is_business_day(ymd(2022, Month::Jan, 3)));
    }
}
