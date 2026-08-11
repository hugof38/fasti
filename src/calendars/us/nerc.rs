//! US North American Energy Reliability Council off-peak calendar —
//! `QuantLib`'s `UnitedStates::NERC`.

use crate::{
    Calendar, FixedDate, LastWeekday, Month, NthWeekday, Ordinal, Rule, Weekday, Weekend,
    WeekendShift, Year, YearRange,
};

/// The NERC off-peak calendar — a minimal 6-holiday set (plus the
/// pre-1971 Memorial Day variant) used for North American electricity
/// off-peak-hours calculations. Port of `QuantLib`'s
/// `UnitedStates::NERC`.
///
/// All weekend shifts are Sunday-forward only — Saturday observances
/// are not moved back to Friday.
///
/// | Holiday | Rule |
/// |---|---|
/// | New Year's Day | Jan 1 + SunForward |
/// | Memorial Day (pre-1971) | May 30 + SatBack/SunForward *[through 1970]* |
/// | Memorial Day | Last Mon of May *[since 1971]* |
/// | Independence Day | Jul 4 + SunForward |
/// | Labor Day | 1st Mon of September |
/// | Thanksgiving | 4th Thu of November |
/// | Christmas | Dec 25 + SunForward |
pub const NERC: Calendar<'static> = Calendar {
    name: "North American Energy Reliability Council",
    weekend: Weekend::SAT_SUN,
    rules: &[
        // New Year's Day — Jan 1, Sunday→Monday only.
        Rule::Fixed(FixedDate::new(Month::Jan, 1).shift(WeekendShift::SunForward)),
        // Memorial Day (pre-1971): May 30 with full weekend shift
        // (QuantLib's `isMemorialDay` keeps the SatBack/SunForward form
        // for the pre-1971 branch).
        Rule::Fixed(
            FixedDate::new(Month::May, 30)
                .shift(WeekendShift::SatBackSunForward)
                .years(YearRange::literal_through(1970)),
        ),
        // Memorial Day (1971+): Last Monday of May.
        Rule::LastWeekday(
            LastWeekday::new(Weekday::Mon, Month::May).from_year(Year::literal(1971)),
        ),
        // Independence Day — Jul 4, Sunday→Monday only.
        Rule::Fixed(FixedDate::new(Month::Jul, 4).shift(WeekendShift::SunForward)),
        // Labor Day — 1st Monday of September.
        Rule::NthWeekday(NthWeekday::new(Ordinal::First, Weekday::Mon, Month::Sep)),
        // Thanksgiving — 4th Thursday of November.
        Rule::NthWeekday(NthWeekday::new(Ordinal::Fourth, Weekday::Thu, Month::Nov)),
        // Christmas — Dec 25, Sunday→Monday only.
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

    /// The six 2024 NERC holidays.
    #[test]
    fn nerc_holidays_2024() {
        assert!(NERC.is_holiday(ymd(2024, Month::Jan, 1))); // New Year's
        assert!(NERC.is_holiday(ymd(2024, Month::May, 27))); // Memorial Day
        assert!(NERC.is_holiday(ymd(2024, Month::Jul, 4))); // Independence Day
        assert!(NERC.is_holiday(ymd(2024, Month::Sep, 2))); // Labor Day
        assert!(NERC.is_holiday(ymd(2024, Month::Nov, 28))); // Thanksgiving
        assert!(NERC.is_holiday(ymd(2024, Month::Dec, 25))); // Christmas
    }

    /// NERC observes no MLK, no Washington's, no Juneteenth, no
    /// Columbus, no Veterans Day.
    #[test]
    fn nerc_excludes_non_peak_holidays() {
        assert!(!NERC.is_holiday(ymd(2024, Month::Jan, 15))); // MLK
        assert!(!NERC.is_holiday(ymd(2024, Month::Feb, 19))); // Washington's
        assert!(!NERC.is_holiday(ymd(2024, Month::Jun, 19))); // Juneteenth
        assert!(!NERC.is_holiday(ymd(2024, Month::Oct, 14))); // Columbus
        assert!(!NERC.is_holiday(ymd(2024, Month::Nov, 11))); // Veterans
    }

    /// Sunday→Monday shift but Saturday natural dates are NOT observed
    /// the prior Friday under NERC.
    #[test]
    fn nerc_sunday_forward_only() {
        // Christmas 2022 (Sun) → observed Mon Dec 26.
        assert!(NERC.is_holiday(ymd(2022, Month::Dec, 26)));
        assert!(!NERC.is_holiday(ymd(2022, Month::Dec, 25)));
        // Jul 4 2026 (Sat) → NO Friday observance under NERC.
        assert!(!NERC.is_holiday(ymd(2026, Month::Jul, 3)));
        // Jul 4 2026 itself is Saturday — also a weekend, not a
        // business day, but the rule doesn't mark it as a holiday per
        // se. Regardless, it's not a business day.
        assert!(!NERC.is_business_day(ymd(2026, Month::Jul, 4)));
    }
}
