//! [`OneOff`]: a holiday observed on a single specific date.

use crate::{Date, Year};

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

    /// The rule's date if it falls in `year`, else [`None`] — the dual of
    /// [`is_holiday`](Self::is_holiday).
    pub(crate) const fn natural_date(self, year: Year) -> Option<Date> {
        if self.date.year().get() == year.get() {
            Some(self.date)
        } else {
            None
        }
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
