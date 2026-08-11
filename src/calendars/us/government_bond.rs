//! US government bond market calendar — `QuantLib`'s
//! `UnitedStates::GovernmentBond`.

use crate::{
    Calendar, Date, EasterMethod, EasterOffset, FixedDate, LastWeekday, Month, NthWeekday, OneOff,
    Ordinal, Rule, Weekday, Weekend, WeekendShift, Year, YearRange, easter_monday,
};

/// The US government bond market calendar — port of `QuantLib`'s
/// `UnitedStates::GovernmentBond` market variant.
///
/// Same holiday set as [`SETTLEMENT`](super::SETTLEMENT) but New Year's
/// and Veterans Day shift Sunday-forward only, Good Friday is observed
/// (with a post-1996 NFP exception), and there are 3 historic closings.
///
/// | Holiday | Rule |
/// |---|---|
/// | New Year's Day | Jan 1 + SunForward |
/// | Martin Luther King Jr. Day | 3rd Mon of January *[since 1983]* |
/// | Washington's Birthday (pre-1971) | Feb 22 + SatBack/SunForward *[through 1970]* |
/// | Washington's Birthday | 3rd Mon of February *[since 1971]* |
/// | Good Friday (pre-1996) | Easter Sunday − 2 *[through 1995]* |
/// | Good Friday (post-1996, non-NFP) | Custom: skipped if Apr 1–7 |
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
pub const GOVERNMENT_BOND: Calendar<'static> = Calendar {
    name: "US government bond market",
    weekend: Weekend::SAT_SUN,
    rules: &[
        // New Year's Day — SunForward only.
        Rule::Fixed(FixedDate::new(Month::Jan, 1).shift(WeekendShift::SunForward)),
        // MLK — 3rd Monday of January, since 1983.
        Rule::NthWeekday(
            NthWeekday::new(Ordinal::Third, Weekday::Mon, Month::Jan)
                .from_year(Year::literal(1983)),
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
        // Good Friday — pre-1996: always observed.
        Rule::Easter(EasterOffset::good_friday().years(YearRange::literal_through(1995))),
        // Good Friday — 1996+: observed except when NFP-colliding.
        Rule::Custom(good_friday_post_1996),
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
        // Juneteenth (2022+): Jun 19 + full shift.
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
        // Veterans Day (pre-1971): Nov 11 + SunForward.
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
        // Veterans Day (1978+): Nov 11 + SunForward.
        Rule::Fixed(
            FixedDate::new(Month::Nov, 11)
                .shift(WeekendShift::SunForward)
                .from_year(Year::literal(1978)),
        ),
        // Thanksgiving.
        Rule::NthWeekday(NthWeekday::new(Ordinal::Fourth, Weekday::Thu, Month::Nov)),
        // Christmas.
        Rule::Fixed(FixedDate::new(Month::Dec, 25).shift(WeekendShift::SatBackSunForward)),
        // Special historic closings.
        Rule::OneOff(OneOff::new(Date::literal(2004, Month::Jun, 11))), // Reagan funeral
        Rule::OneOff(OneOff::new(Date::literal(2012, Month::Oct, 30))), // Hurricane Sandy day 2
        Rule::OneOff(OneOff::new(Date::literal(2018, Month::Dec, 5))),  // Bush funeral
    ],
};

/// Good Friday from 1996 onward, except when day-of-month ≤ 7 — the
/// first-Friday NFP release makes it an early close, not a holiday
/// (per `QuantLib`'s note citing sifma.org).
fn good_friday_post_1996(d: Date) -> bool {
    let y = d.year().get();
    if y < 1996 || d.day() <= 7 {
        return false;
    }
    // Good Friday's day-of-year = Easter Monday − 3.
    let em_doy = easter_monday(d.year(), EasterMethod::Western);
    em_doy >= 3 && d.day_of_year() + 3 == em_doy
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn ymd(y: u16, m: Month, d: u8) -> Date {
        Date::from_ymd(y, m, d).unwrap()
    }

    /// 2024 Government Bond holidays — Settlement's 11 plus Good Friday.
    #[test]
    fn govbond_holidays_2024() {
        assert!(GOVERNMENT_BOND.is_holiday(ymd(2024, Month::Jan, 1)));
        assert!(GOVERNMENT_BOND.is_holiday(ymd(2024, Month::Jan, 15)));
        assert!(GOVERNMENT_BOND.is_holiday(ymd(2024, Month::Feb, 19)));
        assert!(GOVERNMENT_BOND.is_holiday(ymd(2024, Month::Mar, 29))); // Good Friday
        assert!(GOVERNMENT_BOND.is_holiday(ymd(2024, Month::May, 27)));
        assert!(GOVERNMENT_BOND.is_holiday(ymd(2024, Month::Jun, 19)));
        assert!(GOVERNMENT_BOND.is_holiday(ymd(2024, Month::Jul, 4)));
        assert!(GOVERNMENT_BOND.is_holiday(ymd(2024, Month::Sep, 2)));
        assert!(GOVERNMENT_BOND.is_holiday(ymd(2024, Month::Oct, 14)));
        assert!(GOVERNMENT_BOND.is_holiday(ymd(2024, Month::Nov, 11)));
        assert!(GOVERNMENT_BOND.is_holiday(ymd(2024, Month::Nov, 28)));
        assert!(GOVERNMENT_BOND.is_holiday(ymd(2024, Month::Dec, 25)));
    }

    /// Good Friday NFP exception: since 1996, skipped when day ≤ 7.
    #[test]
    fn good_friday_nfp_exception_since_1996() {
        // 1995 Apr 14 Good Friday (day 14 > 7; also pre-1996) — observed.
        assert!(GOVERNMENT_BOND.is_holiday(ymd(1995, Month::Apr, 14)));
        // 2024 Mar 29 Good Friday (day 29 > 7) — observed.
        assert!(GOVERNMENT_BOND.is_holiday(ymd(2024, Month::Mar, 29)));
        // 2026 Good Friday = April 3 (day ≤ 7, post-1996) — NOT observed.
        assert!(!GOVERNMENT_BOND.is_holiday(ymd(2026, Month::Apr, 3)));
        // 1988 Apr 1 Good Friday — pre-1996, exception does not apply.
        assert!(GOVERNMENT_BOND.is_holiday(ymd(1988, Month::Apr, 1)));
    }

    /// Veterans Day is `SunForward` only under govbond.
    #[test]
    fn govbond_veterans_day_no_saturday_back() {
        // 1994 Nov 11 was Friday — observed natural.
        assert!(GOVERNMENT_BOND.is_holiday(ymd(1994, Month::Nov, 11)));
        // 1995 Nov 11 (Sat): Settlement observes Fri Nov 10; govbond does not.
        assert!(!GOVERNMENT_BOND.is_holiday(ymd(1995, Month::Nov, 10)));
        // 1990 Nov 11 was Sunday — observed Monday Nov 12.
        assert!(GOVERNMENT_BOND.is_holiday(ymd(1990, Month::Nov, 12)));
    }

    /// New Year's Day is `SunForward` only — no Dec 31 rollback.
    #[test]
    fn govbond_new_year_sun_forward_only() {
        // Jan 1 2022 (Sat): Settlement observes Fri Dec 31 2021; govbond does not.
        assert!(!GOVERNMENT_BOND.is_holiday(ymd(2021, Month::Dec, 31)));
        // Jan 1 2023 was Sunday — observed Mon Jan 2.
        assert!(GOVERNMENT_BOND.is_holiday(ymd(2023, Month::Jan, 2)));
    }

    /// Special closings: the three ported historic dates.
    #[test]
    fn govbond_special_closings() {
        assert!(GOVERNMENT_BOND.is_holiday(ymd(2018, Month::Dec, 5))); // Bush funeral
        assert!(GOVERNMENT_BOND.is_holiday(ymd(2012, Month::Oct, 30))); // Sandy day 2
        assert!(!GOVERNMENT_BOND.is_holiday(ymd(2012, Month::Oct, 29))); // Sandy day 1 — NOT govbond
        assert!(GOVERNMENT_BOND.is_holiday(ymd(2004, Month::Jun, 11))); // Reagan funeral
    }
}
