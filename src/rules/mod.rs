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

impl Rule {
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

    /// The natural date this rule names in `year`, if any. Every
    /// describable rule names at most one date per year, and for a date
    /// `d` in `year`, `natural_date_in(year) == Some(d)` iff
    /// [`is_holiday(d)`](Self::is_holiday) — the property the calendar
    /// evaluation rests on, pinned by `expansion_agrees_with_probing`.
    ///
    /// [`Rule::Custom`] is a predicate, not a description: it answers
    /// [`None`] here and callers must probe it per date instead.
    pub(crate) const fn natural_date_in(&self, year: Year) -> Option<Date> {
        match self {
            Self::Fixed(r) => r.natural_date_in(year),
            Self::NthWeekday(r) => r.natural_date_in(year),
            Self::LastWeekday(r) => r.natural_date_in(year),
            Self::Easter(r) => r.natural_date_in(year),
            Self::OneOff(r) => r.natural_date_in(year),
            Self::Custom(_) => None,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
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

    /// Rules covering every describable variant and the edge shapes
    /// that make expansion interesting: leap-day fixed dates, fifth
    /// ordinals that may not exist, offsets that leave their year, and
    /// bounded year ranges.
    fn expansion_cases() -> [Rule; 12] {
        use crate::{FixedDate, LastWeekday, NthWeekday, OneOff, WeekendShift, YearRange};
        [
            Rule::Fixed(FixedDate::new(Month::Jul, 4).shift(WeekendShift::SatBackSunForward)),
            Rule::Fixed(FixedDate::new(Month::Feb, 29)),
            Rule::Fixed(
                FixedDate::new(Month::Jan, 1).years(YearRange::literal_between(1971, 1977)),
            ),
            Rule::Fixed(FixedDate::new(Month::Dec, 31).shift(WeekendShift::Forward)),
            Rule::NthWeekday(NthWeekday::new(Ordinal::Third, Weekday::Mon, Month::Jan)),
            Rule::NthWeekday(NthWeekday::new(Ordinal::Fifth, Weekday::Sun, Month::Feb)),
            Rule::LastWeekday(LastWeekday::new(Weekday::Mon, Month::May)),
            Rule::LastWeekday(
                LastWeekday::new(Weekday::Wed, Month::Dec).years(YearRange::literal_through(1950)),
            ),
            Rule::Easter(EasterOffset::good_friday()),
            Rule::Easter(EasterOffset::new_orthodox(1)),
            Rule::Easter(EasterOffset::new(280)), // can leave its year
            Rule::OneOff(OneOff::new(Date::literal(2026, Month::Aug, 15))),
        ]
    }

    proptest! {
        /// Asking "is this date yours?" and "what date do you name in
        /// this year?" are the same question: for every describable
        /// rule and every date, probing agrees with expansion.
        #[test]
        fn expansion_agrees_with_probing(serial in 0u32..=Date::MAX.serial()) {
            let d = Date::from_serial(serial).unwrap();
            for rule in expansion_cases() {
                prop_assert_eq!(
                    rule.is_holiday(d),
                    rule.natural_date_in(d.year()) == Some(d),
                    "{:?} at {}", rule, d,
                );
            }
        }
    }

    /// Both directions, exhaustively: for every case rule and every
    /// year, the expanded date is exactly the set of days the rule
    /// probes as holidays — no invented dates, none missed.
    #[test]
    fn expansion_matches_probing_over_every_year() {
        use alloc::vec::Vec;
        extern crate alloc;
        for rule in expansion_cases() {
            for y in Year::MIN.get()..=Year::MAX.get() {
                let year = Year::new(y).unwrap();
                let jan1 = Date::from_ymd(y, Month::Jan, 1).unwrap();
                let probed: Vec<Date> = (0..u32::from(year.length()))
                    .map(|offset| Date::from_serial(jan1.serial() + offset).unwrap())
                    .filter(|d| rule.is_holiday(*d))
                    .collect();
                let expanded: Vec<Date> = rule.natural_date_in(year).into_iter().collect();
                assert_eq!(expanded, probed, "{rule:?} in {y}");
            }
        }
    }

    use proptest::prelude::*;

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
