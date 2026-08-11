//! [`Span`] — a half-open date interval.

use core::ops::{Bound, Range, RangeBounds};

use crate::Date;

/// A half-open date interval `[start, end)` — an accrual window, a
/// coupon period, or a notional period on a reference grid.
///
/// Built from range syntax, whose half-open meaning is exactly this
/// type's, and implements [`RangeBounds`] so it composes with
/// range-taking code:
///
/// ```
/// use fasti::{Date, Month, Span};
/// use core::ops::RangeBounds;
///
/// let span = Span::from(Date::from_ymd(2025, Month::Jan, 1)?..Date::from_ymd(2025, Month::Apr, 1)?);
/// assert_eq!(span.days(), 90);
/// assert!(span.contains(&Date::from_ymd(2025, Month::Jan, 1)?));
/// assert!(!span.contains(&Date::from_ymd(2025, Month::Apr, 1)?));
/// # Ok::<(), fasti::TimeError>(())
/// ```
///
/// It is not [`Range<Date>`] itself because `Range` is an iterator and
/// therefore not [`Copy`], while spans are passed by value throughout;
/// the `Copy` range types of RFC 3550 are still unstable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Span {
    /// First date of the interval, included.
    pub start: Date,
    /// Last date of the interval, excluded.
    pub end: Date,
}

impl Span {
    /// Elapsed days, signed by direction.
    #[must_use]
    pub fn days(self) -> i64 {
        i64::from(self.end.days_since(self.start))
    }

    /// The overlap with `other`, if the two share any days. Spans that
    /// merely touch at a boundary share none.
    #[must_use]
    pub fn intersect(self, other: Self) -> Option<Self> {
        let both = Self::from(self.start.max(other.start)..self.end.min(other.end));
        (both.start < both.end).then_some(both)
    }
}

impl From<Range<Date>> for Span {
    fn from(range: Range<Date>) -> Self {
        Self {
            start: range.start,
            end: range.end,
        }
    }
}

impl From<Span> for Range<Date> {
    fn from(span: Span) -> Self {
        span.start..span.end
    }
}

impl RangeBounds<Date> for Span {
    fn start_bound(&self) -> Bound<&Date> {
        Bound::Included(&self.start)
    }

    fn end_bound(&self) -> Bound<&Date> {
        Bound::Excluded(&self.end)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::Month;

    fn ymd(y: u16, m: Month, d: u8) -> Date {
        Date::from_ymd(y, m, d).unwrap()
    }

    #[test]
    fn behaves_as_a_half_open_range() {
        let span = Span::from(ymd(2025, Month::Jan, 1)..ymd(2025, Month::Apr, 1));
        assert!(span.contains(&ymd(2025, Month::Jan, 1))); // start included
        assert!(span.contains(&ymd(2025, Month::Mar, 31)));
        assert!(!span.contains(&ymd(2025, Month::Apr, 1))); // end excluded
        assert_eq!(span.days(), 90);
        assert_eq!(
            Range::from(span),
            ymd(2025, Month::Jan, 1)..ymd(2025, Month::Apr, 1)
        );
    }

    #[test]
    fn intersect_shares_days_only() {
        let span = Span::from(ymd(2025, Month::Jan, 1)..ymd(2025, Month::Apr, 1));
        assert_eq!(
            span.intersect(Span::from(
                ymd(2025, Month::Mar, 1)..ymd(2025, Month::Jun, 1)
            )),
            Some(Span::from(
                ymd(2025, Month::Mar, 1)..ymd(2025, Month::Apr, 1)
            )),
        );
        // Disjoint.
        assert_eq!(
            span.intersect(Span::from(
                ymd(2025, Month::Jun, 1)..ymd(2025, Month::Jul, 1)
            )),
            None,
        );
        // Touching at a boundary shares no days.
        assert_eq!(
            span.intersect(Span::from(
                ymd(2025, Month::Apr, 1)..ymd(2025, Month::May, 1)
            )),
            None,
        );
    }
}
