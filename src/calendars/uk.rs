//! UK market calendars — a port of `QuantLib`'s
//! [`UnitedKingdom`](https://github.com/lballabio/QuantLib/blob/master/ql/time/calendars/unitedkingdom.cpp).
//!
//! Only the settlement (bank holiday) variant is ported; `QuantLib`'s
//! `Exchange` and `Metals` variants add nothing to it in current releases.

use crate::{
    Calendar, Date, EasterOffset, FixedDate, LastWeekday, Month, NthWeekday, OneOff, Ordinal, Rule,
    Weekday, Weekend, WeekendShift, Year, YearRange,
};

/// England & Wales bank holidays — `QuantLib`'s
/// `UnitedKingdom::Settlement`.
///
/// | Holiday | Rule |
/// |---|---|
/// | New Year's Day | Jan 1 + [`NextWeekday`](crate::WeekendShift::Forward) |
/// | Good Friday | Easter Sunday − 2 |
/// | Easter Monday | Easter Sunday + 1 |
/// | Early May Bank Holiday | 1st Mon of May *[except 1995 and 2020]* |
/// | V-E Day | May 8 *[1995 and 2020 only]* |
/// | Spring Bank Holiday | Last Mon of May *[except 2002, 2012 and 2022]* |
/// | Golden Jubilee | Jun 3–4 *[2002 only]* |
/// | Diamond Jubilee | Jun 4–5 *[2012 only]* |
/// | Platinum Jubilee | Jun 2–3 *[2022 only]* |
/// | Summer Bank Holiday | Last Mon of August |
/// | Christmas | Dec 25 + [`NextWeekday`](crate::WeekendShift::Forward) |
/// | Boxing Day | Dec 26 + [`NextWeekday`](crate::WeekendShift::Forward) |
/// | Queen Elizabeth II's funeral | Sep 19 *[2022 only]* |
/// | King Charles III's coronation | May 8 *[2023 only]* |
/// | Millennium Day | Dec 31 *[1999 only]* |
///
/// Like `QuantLib`, this applies the modern (post-1978) bank holiday
/// regime to every supported year, so dates before then are the
/// present-day rule projected backwards rather than what was actually
/// observed.
///
/// ```
/// use fasti::{Date, Month, calendars};
/// // Jan 1 2022 fell on a Saturday — observed Monday Jan 3.
/// assert!(calendars::uk::SETTLEMENT.is_holiday(Date::from_ymd(2022, Month::Jan, 3)?));
/// // Christmas 2022 fell on a Sunday — Boxing Day keeps the Monday and
/// // Christmas moves to the Tuesday.
/// assert!(calendars::uk::SETTLEMENT.is_holiday(Date::from_ymd(2022, Month::Dec, 26)?));
/// assert!(calendars::uk::SETTLEMENT.is_holiday(Date::from_ymd(2022, Month::Dec, 27)?));
/// # Ok::<(), fasti::TimeError>(())
/// ```
pub const SETTLEMENT: Calendar<'static> = Calendar {
    name: "UK settlement",
    weekend: Weekend::SAT_SUN,
    rules: &[
        // New Year's Day, with a substitute Monday.
        Rule::Fixed(FixedDate::new(Month::Jan, 1).shift(WeekendShift::Forward)),
        // Good Friday and Easter Monday.
        Rule::Easter(EasterOffset::good_friday()),
        Rule::Easter(EasterOffset::easter_monday()),
        // Early May Bank Holiday — 1st Monday of May, displaced by V-E
        // Day commemorations in 1995 and 2020.
        Rule::NthWeekday(
            NthWeekday::new(Ordinal::First, Weekday::Mon, Month::May)
                .years(YearRange::literal_through(1994)),
        ),
        Rule::NthWeekday(
            NthWeekday::new(Ordinal::First, Weekday::Mon, Month::May)
                .years(YearRange::literal_between(1996, 2019)),
        ),
        Rule::NthWeekday(
            NthWeekday::new(Ordinal::First, Weekday::Mon, Month::May)
                .from_year(Year::literal(2021)),
        ),
        Rule::OneOff(OneOff::new(Date::literal(1995, Month::May, 8))),
        Rule::OneOff(OneOff::new(Date::literal(2020, Month::May, 8))),
        // Spring Bank Holiday — last Monday of May, displaced into June
        // by the Golden, Diamond and Platinum Jubilees.
        Rule::LastWeekday(
            LastWeekday::new(Weekday::Mon, Month::May).years(YearRange::literal_through(2001)),
        ),
        Rule::LastWeekday(
            LastWeekday::new(Weekday::Mon, Month::May)
                .years(YearRange::literal_between(2003, 2011)),
        ),
        Rule::LastWeekday(
            LastWeekday::new(Weekday::Mon, Month::May)
                .years(YearRange::literal_between(2013, 2021)),
        ),
        Rule::LastWeekday(
            LastWeekday::new(Weekday::Mon, Month::May).from_year(Year::literal(2023)),
        ),
        // Golden Jubilee, 2002.
        Rule::OneOff(OneOff::new(Date::literal(2002, Month::Jun, 3))),
        Rule::OneOff(OneOff::new(Date::literal(2002, Month::Jun, 4))),
        // Diamond Jubilee, 2012.
        Rule::OneOff(OneOff::new(Date::literal(2012, Month::Jun, 4))),
        Rule::OneOff(OneOff::new(Date::literal(2012, Month::Jun, 5))),
        // Platinum Jubilee, 2022.
        Rule::OneOff(OneOff::new(Date::literal(2022, Month::Jun, 2))),
        Rule::OneOff(OneOff::new(Date::literal(2022, Month::Jun, 3))),
        // Summer Bank Holiday.
        Rule::LastWeekday(LastWeekday::new(Weekday::Mon, Month::Aug)),
        // Both take the next free weekday, so a weekend Christmas
        // pushes Boxing Day's substitute on by one.
        Rule::Fixed(FixedDate::new(Month::Dec, 25).shift(WeekendShift::Forward)),
        Rule::Fixed(FixedDate::new(Month::Dec, 26).shift(WeekendShift::Forward)),
        // Queen Elizabeth II's state funeral.
        Rule::OneOff(OneOff::new(Date::literal(2022, Month::Sep, 19))),
        // King Charles III's coronation.
        Rule::OneOff(OneOff::new(Date::literal(2023, Month::May, 8))),
        // Millennium Day.
        Rule::OneOff(OneOff::new(Date::literal(1999, Month::Dec, 31))),
    ],
};

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::DateRange;
    use alloc::vec::Vec;

    fn ymd(y: u16, m: Month, d: u8) -> Date {
        Date::from_ymd(y, m, d).unwrap()
    }

    fn year(year: u16) -> core::ops::Range<Date> {
        ymd(year, Month::Jan, 1)..ymd(year, Month::Dec, 31).add_days(1).unwrap()
    }

    /// The days actually taken off in `year`. `holidays` also reports a
    /// holiday's natural date when that falls on a weekend, which is
    /// never a day off, so those are filtered out.
    fn observed(y: u16) -> Vec<Date> {
        SETTLEMENT
            .holidays(year(y))
            .filter(|d| !SETTLEMENT.is_weekend(*d))
            .collect()
    }

    /// Each year's list is the one published by the UK government for
    /// England and Wales. Every entry falls on a weekday, so nothing is
    /// hidden behind the weekend filter.
    fn assert_year(year: u16, expected: &[(Month, u8)]) {
        let expected: Vec<Date> = expected.iter().map(|(m, d)| ymd(year, *m, *d)).collect();
        assert_eq!(observed(year), expected, "{year}");
    }

    #[test]
    fn published_bank_holidays_2024() {
        assert_year(
            2024,
            &[
                (Month::Jan, 1),  // New Year's Day (Mon)
                (Month::Mar, 29), // Good Friday
                (Month::Apr, 1),  // Easter Monday
                (Month::May, 6),  // Early May
                (Month::May, 27), // Spring
                (Month::Aug, 26), // Summer
                (Month::Dec, 25),
                (Month::Dec, 26),
            ],
        );
    }

    #[test]
    fn published_bank_holidays_2023_include_the_coronation() {
        assert_year(
            2023,
            &[
                (Month::Jan, 2),  // Jan 1 was a Sunday
                (Month::Apr, 7),  // Good Friday
                (Month::Apr, 10), // Easter Monday
                (Month::May, 1),  // Early May
                (Month::May, 8),  // Coronation
                (Month::May, 29), // Spring
                (Month::Aug, 28), // Summer
                (Month::Dec, 25),
                (Month::Dec, 26),
            ],
        );
    }

    #[test]
    fn published_bank_holidays_2022_include_jubilee_and_funeral() {
        assert_year(
            2022,
            &[
                (Month::Jan, 3),  // Jan 1 was a Saturday
                (Month::Apr, 15), // Good Friday
                (Month::Apr, 18), // Easter Monday
                (Month::May, 2),  // Early May
                (Month::Jun, 2),  // Spring, moved for the Platinum Jubilee
                (Month::Jun, 3),  // Platinum Jubilee
                (Month::Aug, 29), // Summer
                (Month::Sep, 19), // Queen Elizabeth II's funeral
                (Month::Dec, 26), // Boxing Day keeps the Monday
                (Month::Dec, 27), // Christmas (Sun) moves to the Tuesday
            ],
        );
        // The regular Spring Bank Holiday is suppressed that year.
        assert!(!SETTLEMENT.is_holiday(ymd(2022, Month::May, 30)));
    }

    #[test]
    fn published_bank_holidays_2021_split_christmas_across_monday_and_tuesday() {
        assert_year(
            2021,
            &[
                (Month::Jan, 1),
                (Month::Apr, 2),
                (Month::Apr, 5),
                (Month::May, 3),
                (Month::May, 31),
                (Month::Aug, 30),
                (Month::Dec, 27), // Christmas (Sat) → Monday
                (Month::Dec, 28), // Boxing Day (Sun) → Tuesday
            ],
        );
    }

    #[test]
    fn published_bank_holidays_2020_move_early_may_to_ve_day() {
        assert_year(
            2020,
            &[
                (Month::Jan, 1),
                (Month::Apr, 10),
                (Month::Apr, 13),
                (Month::May, 8),  // V-E Day 75th, not the first Monday
                (Month::May, 25), // Spring
                (Month::Aug, 31), // Summer
                (Month::Dec, 25),
                (Month::Dec, 28), // Boxing Day (Sat) → Monday
            ],
        );
        assert!(!SETTLEMENT.is_holiday(ymd(2020, Month::May, 4)));
    }

    #[test]
    fn published_bank_holidays_2012_include_the_diamond_jubilee() {
        assert_year(
            2012,
            &[
                (Month::Jan, 2), // Jan 1 was a Sunday
                (Month::Apr, 6),
                (Month::Apr, 9),
                (Month::May, 7),
                (Month::Jun, 4), // Spring, moved
                (Month::Jun, 5), // Diamond Jubilee
                (Month::Aug, 27),
                (Month::Dec, 25),
                (Month::Dec, 26),
            ],
        );
        assert!(!SETTLEMENT.is_holiday(ymd(2012, Month::May, 28)));
    }

    #[test]
    fn published_bank_holidays_2002_include_the_golden_jubilee() {
        assert_year(
            2002,
            &[
                (Month::Jan, 1),
                (Month::Mar, 29),
                (Month::Apr, 1),
                (Month::May, 6),
                (Month::Jun, 3), // Golden Jubilee
                (Month::Jun, 4), // Spring, moved
                (Month::Aug, 26),
                (Month::Dec, 25),
                (Month::Dec, 26),
            ],
        );
        assert!(!SETTLEMENT.is_holiday(ymd(2002, Month::May, 27)));
    }

    #[test]
    fn published_bank_holidays_1999_include_millennium_day() {
        assert_year(
            1999,
            &[
                (Month::Jan, 1),
                (Month::Apr, 2),
                (Month::Apr, 5),
                (Month::May, 3),
                (Month::May, 31),
                (Month::Aug, 30),
                (Month::Dec, 27), // Christmas (Sat) → Monday
                (Month::Dec, 28), // Boxing Day (Sun) → Tuesday
                (Month::Dec, 31), // Millennium Day
            ],
        );
    }

    #[test]
    fn published_bank_holidays_1995_move_early_may_to_ve_day() {
        assert_year(
            1995,
            &[
                (Month::Jan, 2), // Jan 1 was a Sunday
                (Month::Apr, 14),
                (Month::Apr, 17),
                (Month::May, 8),  // V-E Day 50th
                (Month::May, 29), // Spring
                (Month::Aug, 28), // Summer
                (Month::Dec, 25),
                (Month::Dec, 26),
            ],
        );
        assert!(!SETTLEMENT.is_holiday(ymd(1995, Month::May, 1)));
    }

    #[test]
    fn no_substitute_is_ever_lost_to_a_collision() {
        // Every holiday yields exactly one day off: weekday holidays
        // keep their own date, weekend ones get a distinct substitute.
        // A collision would silently merge two into one.
        for y in 1996..=2060 {
            let natural: Vec<Date> = year(y)
                .dates()
                .filter(|d| SETTLEMENT.rules.iter().any(|r| r.is_holiday(*d)))
                .collect();
            assert_eq!(observed(y).len(), natural.len(), "{y}: {natural:?}");
        }
    }

    #[test]
    fn eight_or_more_bank_holidays_every_modern_year() {
        // England & Wales has had eight standard bank holidays since
        // 1978; extra one-offs only ever add to that.
        for year in 1996..=2030 {
            assert!(observed(year).len() >= 8, "{year}: {:?}", observed(year));
        }
    }
}
