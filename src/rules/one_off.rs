//! [`OneOff`]: a holiday observed on a single specific date.
//!
//! Useful for bespoke custom calendars, market-closure days, and
//! special-case one-time observances that do not recur.

use crate::Date;

/// A holiday observed on exactly one date.
///
/// ```
/// use fasti::{Date, Month, OneOff};
/// let rule = OneOff::new(Date::from_ymd(2026, Month::Aug, 15)?);
/// assert!(rule.is_holiday(Date::from_ymd(2026, Month::Aug, 15)?));
/// assert!(!rule.is_holiday(Date::from_ymd(2026, Month::Aug, 16)?));
/// # Ok::<(), fasti::TimeError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OneOff {
    date: Date,
}

impl OneOff {
    /// Construct a one-off rule for the given date.
    #[must_use]
    pub const fn new(date: Date) -> Self {
        Self { date }
    }

    /// The observed date.
    #[must_use]
    pub const fn date(&self) -> Date {
        self.date
    }

    /// `true` iff `date` equals the rule's observed date.
    #[must_use]
    pub const fn is_holiday(&self, date: Date) -> bool {
        self.date.serial() == date.serial()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::Month;

    #[test]
    fn matches_exact_date_only() {
        let target = Date::from_ymd(2026, Month::Aug, 15).unwrap();
        let rule = OneOff::new(target);
        assert!(rule.is_holiday(target));
        assert!(!rule.is_holiday(Date::from_ymd(2026, Month::Aug, 14).unwrap()));
        assert!(!rule.is_holiday(Date::from_ymd(2026, Month::Aug, 16).unwrap()));
    }
}
