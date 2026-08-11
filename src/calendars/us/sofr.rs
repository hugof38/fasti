//! SOFR fixing calendar — `QuantLib`'s `UnitedStates::SOFR`.

use crate::{
    Calendar, Date, EasterOffset, FixedDate, LastWeekday, Month, NthWeekday, OneOff, Ordinal, Rule,
    Weekday, Weekend, WeekendShift, Year, YearRange,
};

/// The SOFR fixing calendar — port of `QuantLib`'s
/// `UnitedStates::SOFR` market variant.
///
/// Same holiday set as [`GOVERNMENT_BOND`](super::GOVERNMENT_BOND)
/// *except* Good Friday is always observed (no post-1996 NFP exception —
/// SOFR has never fixed on Good Friday).
///
/// | Holiday | Rule |
/// |---|---|
/// | New Year's Day | Jan 1 + SunForward |
/// | Martin Luther King Jr. Day | 3rd Mon of January *[since 1983]* |
/// | Washington's Birthday (pre-1971) | Feb 22 + SatBack/SunForward *[through 1970]* |
/// | Washington's Birthday | 3rd Mon of February *[since 1971]* |
/// | Good Friday | Easter Sunday − 2 *(always, no NFP exception)* |
/// | Memorial Day (pre-1971) | May 30 + SatBack/SunForward *[through 1970]* |
/// | Memorial Day | Last Mon of May *[since 1971]* |
/// | Juneteenth | Jun 19 + SatBack/SunForward *[since 2022]* |
/// | Independence Day | Jul 4 + SatBack/SunForward |
/// | Labor Day | 1st Mon of September |
/// | Columbus Day | 2nd Mon of October *[since 1971]* |
/// | Veterans Day (pre-1971) | Nov 11 + SunForward *[through 1970]* |
/// | Veterans Day (Uniform Monday) | 4th Mon of October *[1971–1977]* |
/// | Veterans Day | Nov 11 + SunForward *[from 1978]* |
/// | Thanksgiving | 4th Thu of November |
/// | Christmas | Dec 25 + SatBack/SunForward |
/// | Special closings | Custom: 2018-12-5, 2012-10-30, 2004-06-11 |
pub const SOFR: Calendar<'static> = Calendar {
    name: "SOFR fixing calendar",
    weekend: Weekend::SAT_SUN,
    rules: &[
        // New Year's Day — SunForward.
        Rule::Fixed(FixedDate::new(Month::Jan, 1).shift(WeekendShift::SunForward)),
        // MLK — since 1983.
        Rule::NthWeekday(
            NthWeekday::new(Ordinal::Third, Weekday::Mon, Month::Jan)
                .from_year(Year::literal(1983)),
        ),
        // Washington's Birthday (pre-1971).
        Rule::Fixed(
            FixedDate::new(Month::Feb, 22)
                .shift(WeekendShift::SatBackSunForward)
                .years(YearRange::literal_through(1970)),
        ),
        // Washington's Birthday (1971+).
        Rule::NthWeekday(
            NthWeekday::new(Ordinal::Third, Weekday::Mon, Month::Feb)
                .from_year(Year::literal(1971)),
        ),
        // Good Friday — ALWAYS observed under SOFR.
        Rule::Easter(EasterOffset::good_friday()),
        // Memorial Day (pre-1971).
        Rule::Fixed(
            FixedDate::new(Month::May, 30)
                .shift(WeekendShift::SatBackSunForward)
                .years(YearRange::literal_through(1970)),
        ),
        // Memorial Day (1971+).
        Rule::LastWeekday(
            LastWeekday::new(Weekday::Mon, Month::May).from_year(Year::literal(1971)),
        ),
        // Juneteenth (2022+).
        Rule::Fixed(
            FixedDate::new(Month::Jun, 19)
                .shift(WeekendShift::SatBackSunForward)
                .from_year(Year::literal(2022)),
        ),
        // Independence Day.
        Rule::Fixed(FixedDate::new(Month::Jul, 4).shift(WeekendShift::SatBackSunForward)),
        // Labor Day.
        Rule::NthWeekday(NthWeekday::new(Ordinal::First, Weekday::Mon, Month::Sep)),
        // Columbus Day (1971+).
        Rule::NthWeekday(
            NthWeekday::new(Ordinal::Second, Weekday::Mon, Month::Oct)
                .from_year(Year::literal(1971)),
        ),
        // Veterans Day (pre-1971): SunForward.
        Rule::Fixed(
            FixedDate::new(Month::Nov, 11)
                .shift(WeekendShift::SunForward)
                .years(YearRange::literal_through(1970)),
        ),
        // Veterans Day (1971-1977): 4th Monday October.
        Rule::NthWeekday(
            NthWeekday::new(Ordinal::Fourth, Weekday::Mon, Month::Oct)
                .years(YearRange::literal_between(1971, 1977)),
        ),
        // Veterans Day (1978+): SunForward.
        Rule::Fixed(
            FixedDate::new(Month::Nov, 11)
                .shift(WeekendShift::SunForward)
                .from_year(Year::literal(1978)),
        ),
        // Thanksgiving.
        Rule::NthWeekday(NthWeekday::new(Ordinal::Fourth, Weekday::Thu, Month::Nov)),
        // Christmas.
        Rule::Fixed(FixedDate::new(Month::Dec, 25).shift(WeekendShift::SatBackSunForward)),
        // Special historic closings — same set as Government Bond.
        Rule::OneOff(OneOff::new(Date::literal(2004, Month::Jun, 11))), // Reagan funeral
        Rule::OneOff(OneOff::new(Date::literal(2012, Month::Oct, 30))), // Hurricane Sandy day 2
        Rule::OneOff(OneOff::new(Date::literal(2018, Month::Dec, 5))),  // Bush funeral
    ],
};

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn ymd(y: u16, m: Month, d: u8) -> Date {
        Date::from_ymd(y, m, d).unwrap()
    }

    /// 2024 SOFR holidays — 12 total, same as Government Bond.
    #[test]
    fn sofr_holidays_2024() {
        assert!(SOFR.is_holiday(ymd(2024, Month::Jan, 1)));
        assert!(SOFR.is_holiday(ymd(2024, Month::Jan, 15)));
        assert!(SOFR.is_holiday(ymd(2024, Month::Feb, 19)));
        assert!(SOFR.is_holiday(ymd(2024, Month::Mar, 29)));
        assert!(SOFR.is_holiday(ymd(2024, Month::May, 27)));
        assert!(SOFR.is_holiday(ymd(2024, Month::Jun, 19)));
        assert!(SOFR.is_holiday(ymd(2024, Month::Jul, 4)));
        assert!(SOFR.is_holiday(ymd(2024, Month::Sep, 2)));
        assert!(SOFR.is_holiday(ymd(2024, Month::Oct, 14)));
        assert!(SOFR.is_holiday(ymd(2024, Month::Nov, 11)));
        assert!(SOFR.is_holiday(ymd(2024, Month::Nov, 28)));
        assert!(SOFR.is_holiday(ymd(2024, Month::Dec, 25)));
    }

    /// SOFR observes Good Friday even in the NFP-colliding first week
    /// of April, unlike Government Bond.
    #[test]
    fn sofr_good_friday_always_observed() {
        // 2026 Good Friday = April 3 (day ≤ 7, post-1996). SOFR observes.
        assert!(SOFR.is_holiday(ymd(2026, Month::Apr, 3)));
        // 2021 Good Friday = April 2 (day ≤ 7). SOFR observes.
        assert!(SOFR.is_holiday(ymd(2021, Month::Apr, 2)));
        // 2015 Good Friday = April 3 (day ≤ 7). SOFR observes.
        assert!(SOFR.is_holiday(ymd(2015, Month::Apr, 3)));
    }

    /// SOFR shares Government Bond's special closings and shift behavior.
    #[test]
    fn sofr_matches_govbond_on_non_good_friday_behavior() {
        // Special closings.
        assert!(SOFR.is_holiday(ymd(2018, Month::Dec, 5)));
        assert!(SOFR.is_holiday(ymd(2012, Month::Oct, 30)));
        assert!(SOFR.is_holiday(ymd(2004, Month::Jun, 11)));
        // Veterans Day SunForward only.
        assert!(!SOFR.is_holiday(ymd(1995, Month::Nov, 10))); // Sat natural, no shift back
        assert!(SOFR.is_holiday(ymd(1990, Month::Nov, 12))); // Sun natural → Mon
        // New Year's SunForward only.
        assert!(!SOFR.is_holiday(ymd(2021, Month::Dec, 31))); // Sat Jan 1 2022 — no rollback
    }
}
