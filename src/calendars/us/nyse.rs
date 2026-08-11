//! New York Stock Exchange calendar — `QuantLib`'s
//! `UnitedStates::NYSE`.

use crate::{
    Calendar, Date, EasterOffset, FixedDate, LastWeekday, Month, NthWeekday, OneOff, Ordinal, Rule,
    Weekday, Weekend, WeekendShift, Year, YearRange,
};

/// The New York Stock Exchange calendar — a port of `QuantLib`'s
/// `UnitedStates::NYSE` market variant. No Columbus/Veterans Day; adds
/// Good Friday, pre-1981 election days, and ~20 historic closings.
///
/// | Holiday | Rule |
/// |---|---|
/// | New Year's Day | Jan 1 + SunForward |
/// | Martin Luther King Jr. Day | 3rd Mon of January *[since 1998]* |
/// | Washington's Birthday (pre-1971) | Feb 22 + SatBack/SunForward *[through 1970]* |
/// | Washington's Birthday | 3rd Mon of February *[since 1971]* |
/// | Good Friday | Easter Sunday − 2 |
/// | Memorial Day (pre-1971) | May 30 + SatBack/SunForward *[through 1970]* |
/// | Memorial Day | Last Mon of May *[since 1971]* |
/// | Juneteenth | Jun 19 + SatBack/SunForward *[since 2022]* |
/// | Independence Day | Jul 4 + SatBack/SunForward |
/// | Labor Day | 1st Mon of September |
/// | Thanksgiving | 4th Thu of November |
/// | Christmas | Dec 25 + SatBack/SunForward |
/// | Presidential election days | Custom: every year ≤ 1968 + 1972/1976/1980 |
/// | Historic single-day closings | 20× `Rule::OneOff` |
/// | Hurricane Sandy (Oct 29–30 2012) | 2× `Rule::Fixed` with year filter |
/// | September 11 (Sep 11–14 2001) | 4× `Rule::Fixed` with year filter |
/// | 1968 Paperwork Crisis Wednesdays | Custom weekly pattern |
pub const NYSE: Calendar<'static> = Calendar {
    name: "New York stock exchange",
    weekend: Weekend::SAT_SUN,
    rules: &[
        // New Year's Day — SunForward only.
        Rule::Fixed(FixedDate::new(Month::Jan, 1).shift(WeekendShift::SunForward)),
        // MLK — 3rd Monday of January, since 1998 (NYSE-specific).
        Rule::NthWeekday(
            NthWeekday::new(Ordinal::Third, Weekday::Mon, Month::Jan)
                .from_year(Year::literal(1998)),
        ),
        // Washington's Birthday (pre-1971): Feb 22 + full shift.
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
        // Good Friday.
        Rule::Easter(EasterOffset::good_friday()),
        // Memorial Day (pre-1971): May 30 + full shift.
        Rule::Fixed(
            FixedDate::new(Month::May, 30)
                .shift(WeekendShift::SatBackSunForward)
                .years(YearRange::literal_through(1970)),
        ),
        // Memorial Day (1971+): last Monday of May.
        Rule::LastWeekday(
            LastWeekday::new(Weekday::Mon, Month::May).from_year(Year::literal(1971)),
        ),
        // Juneteenth (since 2022).
        Rule::Fixed(
            FixedDate::new(Month::Jun, 19)
                .shift(WeekendShift::SatBackSunForward)
                .from_year(Year::literal(2022)),
        ),
        // Independence Day.
        Rule::Fixed(FixedDate::new(Month::Jul, 4).shift(WeekendShift::SatBackSunForward)),
        // Labor Day.
        Rule::NthWeekday(NthWeekday::new(Ordinal::First, Weekday::Mon, Month::Sep)),
        // Thanksgiving.
        Rule::NthWeekday(NthWeekday::new(Ordinal::Fourth, Weekday::Thu, Month::Nov)),
        // Christmas.
        Rule::Fixed(FixedDate::new(Month::Dec, 25).shift(WeekendShift::SatBackSunForward)),
        // Presidential election days.
        Rule::Custom(election_day),
        // Hurricane Sandy — Oct 29 and Oct 30, 2012.
        Rule::Fixed(
            // Hurricane Sandy day 1 (2012-10-29).
            FixedDate::new(Month::Oct, 29)
                .shift(WeekendShift::None)
                .years(YearRange::literal_between(2012, 2012)),
        ),
        Rule::Fixed(
            // Hurricane Sandy day 2 (2012-10-30).
            FixedDate::new(Month::Oct, 30)
                .shift(WeekendShift::None)
                .years(YearRange::literal_between(2012, 2012)),
        ),
        // September 11 2001 attacks — markets closed Sep 11 through Sep 14.
        Rule::Fixed(
            // September 11 day 1 (2001-09-11) — attacks.
            FixedDate::new(Month::Sep, 11)
                .shift(WeekendShift::None)
                .years(YearRange::literal_between(2001, 2001)),
        ),
        Rule::Fixed(
            // September 11 day 2 (2001-09-12).
            FixedDate::new(Month::Sep, 12)
                .shift(WeekendShift::None)
                .years(YearRange::literal_between(2001, 2001)),
        ),
        Rule::Fixed(
            // September 11 day 3 (2001-09-13).
            FixedDate::new(Month::Sep, 13)
                .shift(WeekendShift::None)
                .years(YearRange::literal_between(2001, 2001)),
        ),
        Rule::Fixed(
            // September 11 day 4 (2001-09-14) — markets reopened Monday 17th.
            FixedDate::new(Month::Sep, 14)
                .shift(WeekendShift::None)
                .years(YearRange::literal_between(2001, 2001)),
        ),
        // Single-day historic closings, chronological.
        Rule::OneOff(OneOff::new(Date::literal(1954, Month::Dec, 24))), // Christmas Eve
        Rule::OneOff(OneOff::new(Date::literal(1956, Month::Dec, 24))), // Christmas Eve
        Rule::OneOff(OneOff::new(Date::literal(1958, Month::Dec, 26))), // Day after Christmas
        Rule::OneOff(OneOff::new(Date::literal(1961, Month::May, 29))), // Day before Decoration Day
        Rule::OneOff(OneOff::new(Date::literal(1963, Month::Nov, 25))), // Kennedy funeral
        Rule::OneOff(OneOff::new(Date::literal(1965, Month::Dec, 24))), // Christmas Eve
        Rule::OneOff(OneOff::new(Date::literal(1968, Month::Apr, 9))),  // MLK assassination
        Rule::OneOff(OneOff::new(Date::literal(1968, Month::Jul, 5))),  // Day after Independence
        Rule::OneOff(OneOff::new(Date::literal(1969, Month::Feb, 10))), // Heavy snow
        Rule::OneOff(OneOff::new(Date::literal(1969, Month::Mar, 31))), // Eisenhower funeral
        Rule::OneOff(OneOff::new(Date::literal(1969, Month::Jul, 21))), // Lunar-exploration day
        Rule::OneOff(OneOff::new(Date::literal(1972, Month::Dec, 28))), // Truman funeral
        Rule::OneOff(OneOff::new(Date::literal(1973, Month::Jan, 25))), // LBJ funeral
        Rule::OneOff(OneOff::new(Date::literal(1977, Month::Jul, 14))), // 1977 Blackout
        Rule::OneOff(OneOff::new(Date::literal(1985, Month::Sep, 27))), // Hurricane Gloria
        Rule::OneOff(OneOff::new(Date::literal(1994, Month::Apr, 27))), // Nixon funeral
        Rule::OneOff(OneOff::new(Date::literal(2004, Month::Jun, 11))), // Reagan funeral
        Rule::OneOff(OneOff::new(Date::literal(2007, Month::Jan, 2))),  // Ford funeral
        Rule::OneOff(OneOff::new(Date::literal(2018, Month::Dec, 5))),  // Bush funeral
        Rule::OneOff(OneOff::new(Date::literal(2025, Month::Jan, 9))),  // Carter funeral
        // 1968 Paperwork Crisis: every Wednesday Jun 12 – Dec 31.
        Rule::Custom(paperwork_crisis_1968),
    ],
};

/// NYSE election-day closures: every year through 1968, then 1972/1976/1980.
/// Matches `QuantLib`'s simplified "first Tuesday of November, day ≤ 7" rule,
/// intentionally keeping its Nov-1-Tuesday edge-case behavior.
fn election_day(d: Date) -> bool {
    if !matches!(d.month(), Month::Nov) {
        return false;
    }
    if d.day() > 7 {
        return false;
    }
    if !matches!(d.weekday(), Weekday::Tue) {
        return false;
    }
    let y = d.year().get();
    y <= 1968 || (y <= 1980 && y.is_multiple_of(4))
}

/// 1968 "Paperwork Crisis": every Wednesday from Jun 12 through Dec 31 1968.
/// Keeps `QuantLib`'s `dd >= 163` bound; the Wednesday filter makes the
/// slightly permissive bound harmless.
fn paperwork_crisis_1968(d: Date) -> bool {
    d.year().get() == 1968 && d.day_of_year() >= 163 && matches!(d.weekday(), Weekday::Wed)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn ymd(y: u16, m: Month, d: u8) -> Date {
        Date::from_ymd(y, m, d).unwrap()
    }

    /// 2024 NYSE holidays — 10 in total, all on weekdays.
    #[test]
    fn nyse_holidays_2024() {
        assert!(NYSE.is_holiday(ymd(2024, Month::Jan, 1)));
        assert!(NYSE.is_holiday(ymd(2024, Month::Jan, 15)));
        assert!(NYSE.is_holiday(ymd(2024, Month::Feb, 19)));
        assert!(NYSE.is_holiday(ymd(2024, Month::Mar, 29))); // Good Friday
        assert!(NYSE.is_holiday(ymd(2024, Month::May, 27)));
        assert!(NYSE.is_holiday(ymd(2024, Month::Jun, 19)));
        assert!(NYSE.is_holiday(ymd(2024, Month::Jul, 4)));
        assert!(NYSE.is_holiday(ymd(2024, Month::Sep, 2)));
        assert!(NYSE.is_holiday(ymd(2024, Month::Nov, 28)));
        assert!(NYSE.is_holiday(ymd(2024, Month::Dec, 25)));
    }

    /// Columbus Day and Veterans Day are NOT NYSE holidays.
    #[test]
    fn nyse_excludes_columbus_and_veterans() {
        assert!(!NYSE.is_holiday(ymd(2024, Month::Oct, 14))); // Columbus
        assert!(!NYSE.is_holiday(ymd(2024, Month::Nov, 11))); // Veterans
    }

    /// MLK Day effective for NYSE only from 1998.
    #[test]
    fn nyse_mlk_effective_from_1998() {
        // 1997 Jan 20: federal MLK but not yet NYSE.
        assert!(!NYSE.is_holiday(ymd(1997, Month::Jan, 20)));
        // 1998 Jan 19: first NYSE MLK Day.
        assert!(NYSE.is_holiday(ymd(1998, Month::Jan, 19)));
    }

    /// Good Friday is an NYSE closure (Settlement does not observe it).
    #[test]
    fn nyse_observes_good_friday() {
        assert!(NYSE.is_holiday(ymd(2024, Month::Mar, 29)));
        assert!(NYSE.is_holiday(ymd(2025, Month::Apr, 18)));
        assert!(NYSE.is_holiday(ymd(2026, Month::Apr, 3)));
    }

    /// New Year's Day: Sunday→Monday only, no Saturday roll back.
    #[test]
    fn nyse_new_year_sun_forward_only() {
        // 2012-01-01 was a Sunday → observed Mon Jan 2.
        assert!(NYSE.is_holiday(ymd(2012, Month::Jan, 2)));
        // 2022-01-01 Sat: NYSE does not observe Fri Dec 31 (Settlement does).
        assert!(!NYSE.is_holiday(ymd(2021, Month::Dec, 31)));
    }

    /// Election days: every year ≤ 1968, then only 1972/1976/1980.
    #[test]
    fn nyse_election_days() {
        // 1968 Nov 5 (Tuesday) — observed.
        assert!(NYSE.is_holiday(ymd(1968, Month::Nov, 5)));
        // 1972 Nov 7 (Tuesday, presidential year) — observed.
        assert!(NYSE.is_holiday(ymd(1972, Month::Nov, 7)));
        // 1976 Nov 2 (Tuesday, presidential year) — observed.
        assert!(NYSE.is_holiday(ymd(1976, Month::Nov, 2)));
        // 1980 Nov 4 (Tuesday, presidential year) — observed.
        assert!(NYSE.is_holiday(ymd(1980, Month::Nov, 4)));
        // 1984 Nov 6 (presidential year, but post-1980) — NOT observed.
        assert!(!NYSE.is_holiday(ymd(1984, Month::Nov, 6)));
        // 1970 Nov 3 (non-presidential, post-1968) — NOT observed.
        assert!(!NYSE.is_holiday(ymd(1970, Month::Nov, 3)));
    }

    /// Special closings: verify sample dates against the ported list.
    #[test]
    fn nyse_special_closings_sample() {
        assert!(NYSE.is_holiday(ymd(2025, Month::Jan, 9))); // Carter funeral
        assert!(NYSE.is_holiday(ymd(2018, Month::Dec, 5))); // Bush funeral
        assert!(NYSE.is_holiday(ymd(2012, Month::Oct, 29))); // Sandy day 1
        assert!(NYSE.is_holiday(ymd(2012, Month::Oct, 30))); // Sandy day 2
        assert!(!NYSE.is_holiday(ymd(2012, Month::Oct, 31))); // Sandy day 3 — reopened
        assert!(NYSE.is_holiday(ymd(2001, Month::Sep, 11)));
        assert!(NYSE.is_holiday(ymd(2001, Month::Sep, 14)));
        assert!(!NYSE.is_holiday(ymd(2001, Month::Sep, 17))); // Reopened Mon
        assert!(NYSE.is_holiday(ymd(1977, Month::Jul, 14))); // Blackout
        assert!(NYSE.is_holiday(ymd(1963, Month::Nov, 25))); // Kennedy
    }

    /// Paperwork Crisis 1968: every Wednesday from Jun 12 to Dec 31.
    #[test]
    fn nyse_paperwork_crisis_1968() {
        // Jun 12 1968 was a Wednesday — observed.
        assert!(NYSE.is_holiday(ymd(1968, Month::Jun, 12)));
        // Jun 11 was a Tuesday — NOT observed.
        assert!(!NYSE.is_holiday(ymd(1968, Month::Jun, 11)));
        // Jun 19 was a Wednesday — observed.
        assert!(NYSE.is_holiday(ymd(1968, Month::Jun, 19)));
        // Nov 27 1968 was a Wednesday — observed.
        assert!(NYSE.is_holiday(ymd(1968, Month::Nov, 27)));
        // 1969: no more Paperwork closures.
        assert!(!NYSE.is_holiday(ymd(1969, Month::Jun, 11))); // Wed
        assert!(!NYSE.is_holiday(ymd(1969, Month::Jun, 18))); // Wed
    }

    /// Christmas Eve closures 1954, 1956, 1965 but not other years.
    #[test]
    fn nyse_christmas_eve_closures() {
        assert!(NYSE.is_holiday(ymd(1954, Month::Dec, 24)));
        assert!(NYSE.is_holiday(ymd(1956, Month::Dec, 24)));
        assert!(NYSE.is_holiday(ymd(1965, Month::Dec, 24)));
        // Not a Christmas Eve closure year.
        assert!(!NYSE.is_holiday(ymd(1955, Month::Dec, 24)));
        assert!(!NYSE.is_holiday(ymd(2024, Month::Dec, 24)));
    }
}
