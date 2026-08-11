//! Holiday rule primitives and the [`Rule`] enum that dispatches across
//! them.
//!
//! Each concrete rule struct answers `is_holiday(date) -> bool` for a
//! specific holiday pattern:
//!
//! - [`FixedDate`] — fixed calendar day, optionally weekend-shifted
//! - [`NthWeekday`] — Nth occurrence of a weekday in a month
//! - [`LastWeekday`] — last occurrence of a weekday in a month
//! - [`EasterOffset`] — a fixed offset from Easter Monday
//! - [`OneOff`] — a single specific date
//!
//! The [`Rule`] enum wraps all five built-in variants plus a
//! [`Rule::Custom`] escape hatch carrying a plain `fn(Date) -> bool`
//! pointer. Calendars hold `&[Rule]` slices and delegate
//! `is_holiday` to the enum.

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

/// A holiday rule. Calendars are sequences of [`Rule`]s; a date is a
/// holiday iff at least one rule marks it as such.
///
/// The [`Rule::Custom`] variant carries a plain function pointer — not a
/// trait object — which keeps the whole enum `const`-constructible and
/// lets built-in calendars live in `pub const` values. The tradeoff is
/// that `Custom` rules cannot carry per-instance state; if you need
/// state, add a concrete variant to this enum or encode the state in
/// the function's body.
///
/// `Rule` does not implement `Serialize` / `Deserialize` (`fn` pointers
/// are not serializable) nor `PartialEq` / `Eq` (function-pointer
/// equality is not reliable across codegen units). Scenarios that need
/// to round-trip rules through YAML/JSON should do so at the
/// concrete-struct level (`FixedDate`, `NthWeekday`, …) and reconstruct
/// `Rule` at load time.
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
    /// A user-supplied predicate. Cannot carry state — encode it in the
    /// function's body if needed.
    Custom(fn(Date) -> bool),
}

impl Rule {
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

    // The whole point of fn-pointer Custom + const constructors is
    // that a calendar's rule slice lives in a `const`. These items
    // exist at module scope (not inside the test fn) because
    // clippy::items_after_statements disallows the latter.
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
