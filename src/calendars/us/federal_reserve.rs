//! US Federal Reserve Bankwire System calendar — `QuantLib`'s
//! `UnitedStates::FederalReserve`.

use crate::{
    Calendar, FixedDate, LastWeekday, Month, NthWeekday, Ordinal, Rule, Weekday, Weekend,
    WeekendShift, Year, YearRange,
};

/// The Federal Reserve Bankwire System calendar — a port of
/// `QuantLib`'s `UnitedStates::FederalReserve` market variant.
///
/// Same holiday set as [`SETTLEMENT`](super::SETTLEMENT), but modern
/// weekend shifts are Sunday-forward only; the legacy pre-1971 variants
/// keep the full SatBack/SunForward shift, matching `QuantLib`.
///
/// | Holiday | Rule |
/// |---|---|
/// | New Year's Day | Jan 1 + SunForward |
/// | Martin Luther King Jr. Day | 3rd Mon of January *[since 1983]* |
/// | Washington's Birthday (pre-1971) | Feb 22 + SatBack/SunForward *[through 1970]* |
/// | Washington's Birthday | 3rd Mon of February *[since 1971]* |
/// | Memorial Day (pre-1971) | May 30 + SatBack/SunForward *[through 1970]* |
/// | Memorial Day | Last Mon of May *[since 1971]* |
/// | Juneteenth | Jun 19 + SunForward *[since 2022]* |
/// | Independence Day | Jul 4 + SunForward |
/// | Labor Day | 1st Mon of September |
/// | Columbus Day | 2nd Mon of October *[since 1971]* |
/// | Veterans Day | Nov 11 + SunForward *[through 1970]* |
/// | Veterans Day (Uniform Monday) | 4th Mon of October *[1971–1977]* |
/// | Veterans Day | Nov 11 + SunForward *[from 1978]* |
/// | Thanksgiving | 4th Thu of November |
/// | Christmas | Dec 25 + SunForward |
pub const FEDERAL_RESERVE: Calendar<'static> = Calendar {
    name: "Federal Reserve Bankwire System",
    weekend: Weekend::SAT_SUN,
    rules: &[
        // New Year's Day — SunForward.
        Rule::Fixed(FixedDate::new(Month::Jan, 1).shift(WeekendShift::SunForward)),
        // MLK — 3rd Monday of January, since 1983.
        Rule::NthWeekday(
            NthWeekday::new(Ordinal::Third, Weekday::Mon, Month::Jan)
                .from_year(Year::literal(1983)),
        ),
        // Washington's Birthday (pre-1971): Feb 22 with full weekend shift.
        Rule::Fixed(
            FixedDate::new(Month::Feb, 22)
                .shift(WeekendShift::SatBackSunForward)
                .years(YearRange::literal_through(1970)),
        ),
        // Washington's Birthday (1971+): 3rd Monday of February.
        Rule::NthWeekday(
            NthWeekday::new(Ordinal::Third, Weekday::Mon, Month::Feb)
                .from_year(Year::literal(1971)),
        ),
        // Memorial Day (pre-1971): May 30 with full weekend shift.
        Rule::Fixed(
            FixedDate::new(Month::May, 30)
                .shift(WeekendShift::SatBackSunForward)
                .years(YearRange::literal_through(1970)),
        ),
        // Memorial Day (1971+): Last Monday of May.
        Rule::LastWeekday(
            LastWeekday::new(Weekday::Mon, Month::May).from_year(Year::literal(1971)),
        ),
        // Juneteenth (2022+): Jun 19 SunForward.
        Rule::Fixed(
            FixedDate::new(Month::Jun, 19)
                .shift(WeekendShift::SunForward)
                .from_year(Year::literal(2022)),
        ),
        // Independence Day — SunForward.
        Rule::Fixed(FixedDate::new(Month::Jul, 4).shift(WeekendShift::SunForward)),
        // Labor Day — 1st Monday of September.
        Rule::NthWeekday(NthWeekday::new(Ordinal::First, Weekday::Mon, Month::Sep)),
        // Columbus Day (1971+): 2nd Monday of October.
        Rule::NthWeekday(
            NthWeekday::new(Ordinal::Second, Weekday::Mon, Month::Oct)
                .from_year(Year::literal(1971)),
        ),
        // Veterans Day (pre-1971): Nov 11 SunForward.
        Rule::Fixed(
            FixedDate::new(Month::Nov, 11)
                .shift(WeekendShift::SunForward)
                .years(YearRange::literal_through(1970)),
        ),
        // Veterans Day (1971-1977): 4th Monday of October.
        Rule::NthWeekday(
            NthWeekday::new(Ordinal::Fourth, Weekday::Mon, Month::Oct)
                .years(YearRange::literal_between(1971, 1977)),
        ),
        // Veterans Day (1978+): Nov 11 SunForward.
        Rule::Fixed(
            FixedDate::new(Month::Nov, 11)
                .shift(WeekendShift::SunForward)
                .from_year(Year::literal(1978)),
        ),
        // Thanksgiving — 4th Thursday of November.
        Rule::NthWeekday(NthWeekday::new(Ordinal::Fourth, Weekday::Thu, Month::Nov)),
        // Christmas — Dec 25 SunForward.
        Rule::Fixed(FixedDate::new(Month::Dec, 25).shift(WeekendShift::SunForward)),
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

    /// All 11 Federal Reserve holidays for 2024 (same set as Settlement).
    #[test]
    fn federal_reserve_holidays_2024() {
        assert!(FEDERAL_RESERVE.is_holiday(ymd(2024, Month::Jan, 1)));
        assert!(FEDERAL_RESERVE.is_holiday(ymd(2024, Month::Jan, 15)));
        assert!(FEDERAL_RESERVE.is_holiday(ymd(2024, Month::Feb, 19)));
        assert!(FEDERAL_RESERVE.is_holiday(ymd(2024, Month::May, 27)));
        assert!(FEDERAL_RESERVE.is_holiday(ymd(2024, Month::Jun, 19)));
        assert!(FEDERAL_RESERVE.is_holiday(ymd(2024, Month::Jul, 4)));
        assert!(FEDERAL_RESERVE.is_holiday(ymd(2024, Month::Sep, 2)));
        assert!(FEDERAL_RESERVE.is_holiday(ymd(2024, Month::Oct, 14)));
        assert!(FEDERAL_RESERVE.is_holiday(ymd(2024, Month::Nov, 11)));
        assert!(FEDERAL_RESERVE.is_holiday(ymd(2024, Month::Nov, 28)));
        assert!(FEDERAL_RESERVE.is_holiday(ymd(2024, Month::Dec, 25)));
    }

    /// Key diff vs Settlement: no Friday observance for Saturday holidays.
    #[test]
    fn federal_reserve_no_saturday_back_shift() {
        // Jul 4 2026 (Sat): Settlement observes Fri Jul 3; Fed does not.
        assert!(!FEDERAL_RESERVE.is_holiday(ymd(2026, Month::Jul, 3)));
        // Jan 1 2022 (Sat): Settlement observes Fri Dec 31 2021; Fed does not.
        assert!(!FEDERAL_RESERVE.is_holiday(ymd(2021, Month::Dec, 31)));
    }

    /// Sunday→Monday shifts do apply under Fed Reserve.
    #[test]
    fn federal_reserve_sunday_forward() {
        // Christmas 2022 (Sun) → observed Mon Dec 26.
        assert!(FEDERAL_RESERVE.is_holiday(ymd(2022, Month::Dec, 26)));
        // Jul 4 2021 (Sun) → observed Mon Jul 5.
        assert!(FEDERAL_RESERVE.is_holiday(ymd(2021, Month::Jul, 5)));
    }

    /// Pre-1971 Washington's Birthday keeps the full SatBack/SunForward shift.
    #[test]
    fn federal_reserve_legacy_washington_uses_sat_back() {
        assert!(FEDERAL_RESERVE.is_holiday(ymd(1970, Month::Feb, 23))); // observed
        // The natural Sunday stays a holiday; the Monday is its substitute.
        assert!(FEDERAL_RESERVE.is_holiday(ymd(1970, Month::Feb, 22)));
        // 1969 Feb 22 (Sat) → observed Friday Feb 21 under the legacy rule.
        assert!(FEDERAL_RESERVE.is_holiday(ymd(1969, Month::Feb, 21)));
    }

    /// Veterans Day 1971-1977 Uniform Monday variant applies identically.
    #[test]
    fn federal_reserve_veterans_day_uniform_monday() {
        assert!(FEDERAL_RESERVE.is_holiday(ymd(1971, Month::Oct, 25)));
        assert!(FEDERAL_RESERVE.is_holiday(ymd(1977, Month::Oct, 24)));
        assert!(!FEDERAL_RESERVE.is_holiday(ymd(1978, Month::Oct, 23)));
    }
}
