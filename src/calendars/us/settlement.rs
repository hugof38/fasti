//! US generic settlement calendar — `QuantLib`'s `UnitedStates::Settlement`.

use crate::{
    Calendar, FixedDate, LastWeekday, Month, NthWeekday, Ordinal, Rule, Weekday, Weekend,
    WeekendShift, Year, YearRange,
};

/// The US generic settlement calendar — a port of `QuantLib`'s
/// `UnitedStates::Settlement` market variant.
///
/// | Holiday | Rule |
/// |---|---|
/// | New Year's Day | Jan 1 + SatBack/SunForward |
/// | Martin Luther King Jr. Day | 3rd Mon of January *[since 1983]* |
/// | Washington's Birthday / Presidents' Day | 3rd Mon of February *[since 1971]* |
/// | Washington's Birthday (pre-1971) | Feb 22 + SatBack/SunForward *[through 1970]* |
/// | Memorial Day | Last Mon of May *[since 1971]* |
/// | Memorial Day (pre-1971) | May 30 + SatBack/SunForward *[through 1970]* |
/// | Juneteenth | Jun 19 + SatBack/SunForward *[since 2022]* |
/// | Independence Day | Jul 4 + SatBack/SunForward |
/// | Labor Day | 1st Mon of September |
/// | Columbus Day | 2nd Mon of October *[since 1971]* |
/// | Veterans Day | Nov 11 + SatBack/SunForward *[through 1970]* |
/// | Veterans Day (Uniform Monday Act) | 4th Mon of October *[1971–1977]* |
/// | Veterans Day | Nov 11 + SatBack/SunForward *[from 1978]* |
/// | Thanksgiving | 4th Thu of November |
/// | Christmas | Dec 25 + SatBack/SunForward |
///
/// Notes on `QuantLib` compatibility:
///
/// - MLK effective 1983 and Juneteenth effective 2022 per `QuantLib` (not the federal 1986/2021).
/// - Veterans Day 1971–1977 follows the Uniform Monday Holiday Act (4th Monday of October).
pub const SETTLEMENT: Calendar<'static> = Calendar {
    name: "US settlement",
    weekend: Weekend::SAT_SUN,
    rules: &[
        // New Year's Day.
        Rule::Fixed(FixedDate::new(Month::Jan, 1).shift(WeekendShift::SatBackSunForward)),
        // Martin Luther King Jr. Day — 3rd Monday of January, since 1983.
        Rule::NthWeekday(
            NthWeekday::new(Ordinal::Third, Weekday::Mon, Month::Jan)
                .from_year(Year::literal(1983)),
        ),
        // Washington's Birthday (pre-1971): Feb 22 with weekend shift.
        Rule::Fixed(
            FixedDate::new(Month::Feb, 22)
                .shift(WeekendShift::SatBackSunForward)
                .years(YearRange::literal_through(1970)),
        ),
        // Washington's Birthday / Presidents' Day (1971+): 3rd Monday of February.
        Rule::NthWeekday(
            NthWeekday::new(Ordinal::Third, Weekday::Mon, Month::Feb)
                .from_year(Year::literal(1971)),
        ),
        // Memorial Day (pre-1971): May 30 with weekend shift.
        Rule::Fixed(
            FixedDate::new(Month::May, 30)
                .shift(WeekendShift::SatBackSunForward)
                .years(YearRange::literal_through(1970)),
        ),
        // Memorial Day (1971+): Last Monday of May.
        Rule::LastWeekday(
            LastWeekday::new(Weekday::Mon, Month::May).from_year(Year::literal(1971)),
        ),
        // Juneteenth (2022+): Jun 19 with weekend shift.
        Rule::Fixed(
            FixedDate::new(Month::Jun, 19)
                .shift(WeekendShift::SatBackSunForward)
                .from_year(Year::literal(2022)),
        ),
        // Independence Day.
        Rule::Fixed(FixedDate::new(Month::Jul, 4).shift(WeekendShift::SatBackSunForward)),
        // Labor Day — 1st Monday of September.
        Rule::NthWeekday(NthWeekday::new(Ordinal::First, Weekday::Mon, Month::Sep)),
        // Columbus Day (1971+): 2nd Monday of October.
        Rule::NthWeekday(
            NthWeekday::new(Ordinal::Second, Weekday::Mon, Month::Oct)
                .from_year(Year::literal(1971)),
        ),
        // Veterans Day (pre-1971): Nov 11 with weekend shift.
        Rule::Fixed(
            FixedDate::new(Month::Nov, 11)
                .shift(WeekendShift::SatBackSunForward)
                .years(YearRange::literal_through(1970)),
        ),
        // Veterans Day (1971–1977): 4th Monday of October (Uniform Monday Act).
        Rule::NthWeekday(
            NthWeekday::new(Ordinal::Fourth, Weekday::Mon, Month::Oct)
                .years(YearRange::literal_between(1971, 1977)),
        ),
        // Veterans Day (1978+): Nov 11 with weekend shift.
        Rule::Fixed(
            FixedDate::new(Month::Nov, 11)
                .shift(WeekendShift::SatBackSunForward)
                .from_year(Year::literal(1978)),
        ),
        // Thanksgiving — 4th Thursday of November.
        Rule::NthWeekday(NthWeekday::new(Ordinal::Fourth, Weekday::Thu, Month::Nov)),
        // Christmas.
        Rule::Fixed(FixedDate::new(Month::Dec, 25).shift(WeekendShift::SatBackSunForward)),
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

    /// 2024 US settlement holidays, verified against OPM's list.
    #[test]
    fn settlement_holidays_2024() {
        assert!(SETTLEMENT.is_holiday(ymd(2024, Month::Jan, 1))); // New Year's — Mon
        assert!(SETTLEMENT.is_holiday(ymd(2024, Month::Jan, 15))); // MLK — Mon
        assert!(SETTLEMENT.is_holiday(ymd(2024, Month::Feb, 19))); // Presidents' — Mon
        assert!(SETTLEMENT.is_holiday(ymd(2024, Month::May, 27))); // Memorial — Mon
        assert!(SETTLEMENT.is_holiday(ymd(2024, Month::Jun, 19))); // Juneteenth — Wed
        assert!(SETTLEMENT.is_holiday(ymd(2024, Month::Jul, 4))); // Independence — Thu
        assert!(SETTLEMENT.is_holiday(ymd(2024, Month::Sep, 2))); // Labor — Mon
        assert!(SETTLEMENT.is_holiday(ymd(2024, Month::Oct, 14))); // Columbus — Mon
        assert!(SETTLEMENT.is_holiday(ymd(2024, Month::Nov, 11))); // Veterans — Mon
        assert!(SETTLEMENT.is_holiday(ymd(2024, Month::Nov, 28))); // Thanksgiving — Thu
        assert!(SETTLEMENT.is_holiday(ymd(2024, Month::Dec, 25))); // Christmas — Wed
    }

    /// 2024 has no weekend-shifted holidays: count = 11.
    #[test]
    fn settlement_2024_count_is_eleven() {
        let mut count = 0;
        let mut d = ymd(2024, Month::Jan, 1);
        let last = ymd(2024, Month::Dec, 31);
        while d.serial() <= last.serial() {
            if SETTLEMENT.is_holiday(d) && !SETTLEMENT.is_weekend(d) {
                count += 1;
            }
            d = d.add_days(1).unwrap();
        }
        assert_eq!(count, 11);
    }

    /// MLK effective from 1983 (`QuantLib` choice, not federal 1986).
    #[test]
    fn mlk_effective_from_1983() {
        // 1982 third Monday of January is Jan 18 — NOT a holiday.
        assert!(!SETTLEMENT.is_holiday(ymd(1982, Month::Jan, 18)));
        // 1983 third Monday of January is Jan 17 — first MLK Day.
        assert!(SETTLEMENT.is_holiday(ymd(1983, Month::Jan, 17)));
        // 1985 (pre-federal-observance but MLK per `QuantLib`): Jan 21.
        assert!(SETTLEMENT.is_holiday(ymd(1985, Month::Jan, 21)));
    }

    /// Juneteenth effective from 2022 (`QuantLib` choice, not federal 2021).
    #[test]
    fn juneteenth_effective_from_2022() {
        assert!(!SETTLEMENT.is_holiday(ymd(2020, Month::Jun, 19)));
        assert!(!SETTLEMENT.is_holiday(ymd(2021, Month::Jun, 18)));
        assert!(!SETTLEMENT.is_holiday(ymd(2021, Month::Jun, 19))); // Saturday
        // 2022-06-19 was Sunday — observed Monday June 20.
        assert!(SETTLEMENT.is_holiday(ymd(2022, Month::Jun, 20)));
        // 2023-06-19 was Monday — observed natural.
        assert!(SETTLEMENT.is_holiday(ymd(2023, Month::Jun, 19)));
    }

    /// Washington's Birthday pre-1971: Feb 22 with weekend shift.
    #[test]
    fn washington_pre_1971() {
        // 1970: Feb 22 was a Sunday — observed Monday Feb 23.
        assert!(SETTLEMENT.is_holiday(ymd(1970, Month::Feb, 23)));
        assert!(SETTLEMENT.is_holiday(ymd(1970, Month::Feb, 22))); // natural date too
        // 1969: Feb 22 was a Saturday — observed Friday Feb 21.
        assert!(SETTLEMENT.is_holiday(ymd(1969, Month::Feb, 21)));
        // 1971: moves to 3rd Monday (= Feb 15).
        assert!(SETTLEMENT.is_holiday(ymd(1971, Month::Feb, 15)));
        // 1971 Feb 22 (a Monday, but not the 3rd) — no longer a holiday.
        assert!(!SETTLEMENT.is_holiday(ymd(1971, Month::Feb, 22)));
    }

    /// Memorial Day pre-1971: May 30 with weekend shift.
    #[test]
    fn memorial_day_pre_1971() {
        // 1970: May 30 was a Saturday — observed Friday May 29.
        assert!(SETTLEMENT.is_holiday(ymd(1970, Month::May, 29)));
        // 1968: May 30 was a Thursday — observed natural.
        assert!(SETTLEMENT.is_holiday(ymd(1968, Month::May, 30)));
        // 1971: moves to last Monday (= May 31).
        assert!(SETTLEMENT.is_holiday(ymd(1971, Month::May, 31)));
    }

    /// Veterans Day 1971–1977: 4th Monday of October; Nov 11 from 1978.
    #[test]
    fn veterans_day_uniform_monday_act() {
        // 1971: 4th Monday of October is Oct 25.
        assert!(SETTLEMENT.is_holiday(ymd(1971, Month::Oct, 25)));
        assert!(!SETTLEMENT.is_holiday(ymd(1971, Month::Nov, 11)));
        // 1977: 4th Monday of October is Oct 24 — last Uniform year.
        assert!(SETTLEMENT.is_holiday(ymd(1977, Month::Oct, 24)));
        assert!(!SETTLEMENT.is_holiday(ymd(1977, Month::Nov, 11)));
        // 1978: reverted — Nov 11 (Saturday → Friday Nov 10).
        assert!(SETTLEMENT.is_holiday(ymd(1978, Month::Nov, 10)));
        assert!(!SETTLEMENT.is_holiday(ymd(1978, Month::Oct, 23)));
        // Pre-1971: Nov 11 always (1970 Nov 11 was a Wednesday).
        assert!(SETTLEMENT.is_holiday(ymd(1970, Month::Nov, 11)));
    }

    /// Columbus Day effective from 1971 per `QuantLib`.
    #[test]
    fn columbus_effective_from_1971() {
        assert!(!SETTLEMENT.is_holiday(ymd(1970, Month::Oct, 12))); // 2nd Monday 1970
        assert!(SETTLEMENT.is_holiday(ymd(1971, Month::Oct, 11))); // 2nd Monday 1971
    }

    /// Weekend-shift regressions — common observed-date cases.
    #[test]
    fn weekend_shifts() {
        // Jul 4 2026 (Sat) observed Friday July 3.
        assert!(SETTLEMENT.is_holiday(ymd(2026, Month::Jul, 3)));
        assert!(SETTLEMENT.is_holiday(ymd(2026, Month::Jul, 4))); // natural date too
        // Christmas 2022 (Sun) observed Monday Dec 26.
        assert!(SETTLEMENT.is_holiday(ymd(2022, Month::Dec, 26)));
        assert!(SETTLEMENT.is_holiday(ymd(2022, Month::Dec, 25))); // natural date too
        // Jan 1 2022 (Sat) observed Friday Dec 31 2021 — cross-year.
        assert!(SETTLEMENT.is_holiday(ymd(2021, Month::Dec, 31)));
    }

    #[test]
    fn business_day_navigation_around_independence_day() {
        let wed = ymd(2024, Month::Jul, 3);
        assert_eq!(
            SETTLEMENT.next_business_day(wed),
            Some(ymd(2024, Month::Jul, 5)),
        );
        let fri = ymd(2024, Month::Jul, 5);
        assert_eq!(
            SETTLEMENT.prev_business_day(fri),
            Some(ymd(2024, Month::Jul, 3)),
        );
    }
}
