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

use crate::Date;

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
    /// An [`EasterOffset`] rule — a fixed offset from Easter Sunday.
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
