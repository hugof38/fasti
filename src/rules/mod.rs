//! Holiday rule primitives and the [`Rule`] enum that dispatches across
//! them: [`FixedDate`], [`NthWeekday`], [`LastWeekday`], [`EasterOffset`],
//! [`OneOff`], plus [`Rule::Custom`] for arbitrary predicates.

mod easter_offset;
mod fixed;
mod last_weekday;
mod nth_weekday;
mod one_off;

pub use easter_offset::EasterOffset;
pub use fixed::{FixedDate, WeekendShift};
pub use last_weekday::LastWeekday;
pub use nth_weekday::NthWeekday;
pub use one_off::OneOff;

use crate::{Date, Year};

/// A holiday rule matching a holiday's natural date.
/// [`Rule::Custom`] holds a plain `fn` pointer to stay `const`-constructible,
/// which is why `Rule` implements neither serde traits nor `PartialEq`.
#[derive(Debug, Clone, Copy)]
pub enum Rule {
    /// A [`FixedDate`] rule — specific month/day, optional shift.
    Fixed(FixedDate),
    /// An [`NthWeekday`] rule — Nth weekday in a month.
    NthWeekday(NthWeekday),
    /// A [`LastWeekday`] rule — last weekday in a month.
    LastWeekday(LastWeekday),
    /// An [`EasterOffset`] rule — a fixed offset from Easter Monday.
    Easter(EasterOffset),
    /// A [`OneOff`] rule — a single specific date.
    OneOff(OneOff),
    /// A user-supplied predicate; cannot carry per-instance state.
    Custom(fn(Date) -> bool),
}

/// What a [`Rule`] names in one particular year — the answer to
/// "which date, if any, is yours in this year?".
///
/// Every rule but [`Rule::Custom`] is year-parameterised: it names at
/// most one date per year and can compute it outright.
///
/// ```
/// use fasti::{Date, FixedDate, Month, Rule, Occurrence, Year};
///
/// let christmas = Rule::Fixed(FixedDate::new(Month::Dec, 25));
/// assert_eq!(
///     christmas.natural_date(Year::new(2026)?),
///     Occurrence::On(Date::from_ymd(2026, Month::Dec, 25)?),
/// );
///
/// // A predicate can only be called, never asked.
/// let custom = Rule::Custom(|d| d.day() == 13);
/// assert_eq!(custom.natural_date(Year::new(2026)?), Occurrence::Opaque);
/// # Ok::<(), fasti::TimeError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Occurrence {
    /// The rule names no date in this year: it is outside its
    /// [`YearRange`](crate::YearRange), or the year has no such day —
    /// February 29 outside a leap year, a fifth Monday the month has no
    /// room for, an Easter offset landing in the next year.
    None,
    /// The rule names exactly this date, before any
    /// [`WeekendShift`] the calendar may apply.
    On(Date),
    /// A [`Rule::Custom`] predicate. It cannot say what it names, only
    /// answer for a date it is handed, so it has to be probed per day.
    Opaque,
}

impl Rule {
    /// The date this rule names in `year`, or why it names none.
    ///
    /// The dual of [`is_holiday`](Self::is_holiday): for every date `d`,
    /// a non-[`Custom`](Self::Custom) rule satisfies `r.is_holiday(d) ==
    /// (r.natural_date(d.year()) == Occurrence::On(d))`. It names the
    /// *natural* date only; substitute days are the calendar's to
    /// resolve, as
    /// [`Calendar::is_holiday`](crate::Calendar::is_holiday) documents.
    ///
    /// ```
    /// use fasti::{Date, Month, NthWeekday, Ordinal, Rule, Occurrence, Weekday, Year};
    ///
    /// // Thanksgiving 2026: the fourth Thursday of November.
    /// let rule = Rule::NthWeekday(NthWeekday::new(Ordinal::Fourth, Weekday::Thu, Month::Nov));
    /// assert_eq!(
    ///     rule.natural_date(Year::new(2026)?),
    ///     Occurrence::On(Date::from_ymd(2026, Month::Nov, 26)?),
    /// );
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    #[must_use]
    pub const fn natural_date(&self, year: Year) -> Occurrence {
        let resolved = match self {
            Self::Fixed(r) => r.natural_date(year),
            Self::NthWeekday(r) => r.natural_date(year),
            Self::LastWeekday(r) => r.natural_date(year),
            Self::Easter(r) => r.natural_date(year),
            Self::OneOff(r) => r.natural_date(year),
            // A fn pointer cannot be interrogated, only called.
            Self::Custom(_) => return Occurrence::Opaque,
        };
        match resolved {
            Some(date) => Occurrence::On(date),
            None => Occurrence::None,
        }
    }

    /// The rule's weekend-shift direction. Only [`FixedDate`] carries
    /// one; nth/last-weekday rules never land on a weekend, and Easter
    /// offsets and one-offs name an exact observed date already.
    pub(crate) fn weekend_shift(&self) -> WeekendShift {
        match self {
            Self::Fixed(r) => r.weekend_shift(),
            _ => WeekendShift::None,
        }
    }

    /// `true` iff any underlying rule marks `date` as a holiday.
    #[must_use]
    pub fn is_holiday(&self, date: Date) -> bool {
        match self {
            Self::Fixed(r) => r.is_holiday(date),
            Self::NthWeekday(r) => r.is_holiday(date),
            Self::LastWeekday(r) => r.is_holiday(date),
            Self::Easter(r) => r.is_holiday(date),
            Self::OneOff(r) => r.is_holiday(date),
            Self::Custom(f) => f(date),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    extern crate alloc;

    use super::*;
    use crate::{Month, Ordinal, Weekday, Year};

    fn ymd(y: u16, m: Month, d: u8) -> Date {
        Date::from_ymd(y, m, d).unwrap()
    }

    #[test]
    fn rule_dispatches_to_each_variant() {
        let rules = [
            Rule::Fixed(FixedDate::new(Month::Jul, 4)),
            Rule::NthWeekday(NthWeekday::new(Ordinal::Third, Weekday::Mon, Month::Jan)),
            Rule::LastWeekday(LastWeekday::new(Weekday::Mon, Month::May)),
            Rule::Easter(EasterOffset::easter_monday()),
            Rule::OneOff(OneOff::new(ymd(2026, Month::Aug, 15))),
        ];
        // One probe per rule.
        assert!(rules[0].is_holiday(ymd(2024, Month::Jul, 4)));
        assert!(rules[1].is_holiday(ymd(2024, Month::Jan, 15)));
        assert!(rules[2].is_holiday(ymd(2024, Month::May, 27)));
        assert!(rules[3].is_holiday(ymd(2024, Month::Apr, 1)));
        assert!(rules[4].is_holiday(ymd(2026, Month::Aug, 15)));
    }

    #[test]
    fn custom_rule_invokes_fn_pointer() {
        fn every_friday_13th(d: Date) -> bool {
            d.day() == 13 && matches!(d.weekday(), Weekday::Fri)
        }
        let rule = Rule::Custom(every_friday_13th);
        // 2026-02-13 is a Friday.
        assert!(rule.is_holiday(ymd(2026, Month::Feb, 13)));
        assert!(!rule.is_holiday(ymd(2026, Month::Feb, 14)));
    }

    /// Every rule of every built-in calendar.
    fn built_in_rules() -> alloc::vec::Vec<Rule> {
        use crate::calendars;
        [
            calendars::TARGET,
            calendars::france::SETTLEMENT,
            calendars::france::EXCHANGE,
            calendars::uk::SETTLEMENT,
            calendars::us::SETTLEMENT,
            calendars::us::FEDERAL_RESERVE,
            calendars::us::GOVERNMENT_BOND,
            calendars::us::SOFR,
            calendars::us::NERC,
            calendars::us::NYSE,
        ]
        .iter()
        .flat_map(|cal| cal.rules.iter().copied())
        .collect()
    }

    /// The date a rule names in a year is *in* that year. No calendar
    /// query observes a rule that breaks this, but a caller resolving a
    /// year at a time would collect a holiday that is not one.
    #[test]
    fn natural_date_names_a_date_inside_the_year() {
        for (i, rule) in built_in_rules().iter().enumerate() {
            for year in Year::MIN.get()..=Year::MAX.get() {
                let Ok(year) = Year::new(year) else { continue };
                if let Occurrence::On(named) = rule.natural_date(year) {
                    assert_eq!(named.year(), year, "rule {i} in {year}");
                }
            }
        }
    }

    /// An Easter offset far enough out lands in the next year, which
    /// `is_holiday` never matches, so `natural_date` must not name it
    /// either. Easter Sunday falls between March 22 and April 25, so 300
    /// days on is always the year after.
    #[test]
    fn an_easter_offset_that_leaves_its_year_names_nothing() {
        let rule = Rule::Easter(EasterOffset::new(300));
        for year in Year::MIN.get()..=Year::MAX.get() {
            let Ok(year) = Year::new(year) else { continue };
            assert_eq!(rule.natural_date(year), Occurrence::None, "{year}");
        }
    }

    /// An opaque rule stays opaque in every year: `Rule::Custom` is a
    /// predicate, and a predicate cannot be asked what it names.
    #[test]
    fn a_custom_rule_is_opaque_in_every_year() {
        let rule = Rule::Custom(|d| d.day() == 13);
        for year in Year::MIN.get()..=Year::MAX.get() {
            let Ok(year) = Year::new(year) else { continue };
            assert!(
                matches!(rule.natural_date(year), Occurrence::Opaque),
                "{year}"
            );
        }
    }

    // Const-constructibility check; module scope for clippy::items_after_statements.
    const JULY_FOURTH: Rule = Rule::Fixed(FixedDate::new(Month::Jul, 4));
    const MLK: Rule = Rule::NthWeekday(NthWeekday::new(Ordinal::Third, Weekday::Mon, Month::Jan));
    const GOOD_FRIDAY: Rule = Rule::Easter(EasterOffset::good_friday());
    const RULES: &[Rule] = &[JULY_FOURTH, MLK, GOOD_FRIDAY];
    const ORTHO: Rule = Rule::Easter(EasterOffset::new_orthodox(1));

    #[test]
    fn const_context_construction() {
        assert_eq!(RULES.len(), 3);
        assert!(matches!(ORTHO, Rule::Easter(_)));
        // Touch the Year re-export so unused-import pruning doesn't trip.
        let _ = Year::MAX;
    }
}
