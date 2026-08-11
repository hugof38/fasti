//! Error type returned by fallible `fasti` constructors.
//!
//! The crate is deliberately strict at API boundaries: years outside
//! 1901–2199, months outside 1–12, and day-of-month values that do not
//! exist in the given month/year are all refused at construction rather
//! than silently coerced. Every such refusal surfaces as a [`TimeError`].

/// Errors produced by `fasti` constructors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TimeError {
    /// A [`Year`](crate::Year) was constructed outside the supported range
    /// 1901–2199.
    #[error("year out of range: supported range is 1901..=2199")]
    YearOutOfRange,
    /// A [`Month`](crate::Month) was constructed from a [`u8`] outside
    /// 1..=12.
    #[error("month out of range: must be 1..=12")]
    MonthOutOfRange,
    /// A day-of-month was zero or exceeded the month's length for the
    /// given year.
    #[error("day out of range for the given month and year")]
    DayOutOfRange,
    /// An [`Ordinal`](crate::Ordinal) was constructed from a value outside
    /// 1..=5.
    #[error("ordinal out of range: must be 1..=5")]
    OrdinalOutOfRange,
    /// A [`Weekday`](crate::Weekday) was constructed from a [`u8`] outside
    /// 1..=7 (ISO 8601).
    #[error("weekday out of range: must be 1..=7 (ISO: Mon=1..Sun=7)")]
    WeekdayOutOfRange,
    /// A date arithmetic operation produced a result outside the supported
    /// serial range.
    #[error("date arithmetic result out of range")]
    DateOutOfRange,
    /// A [`YearRange`](crate::YearRange) was constructed with an upper
    /// bound below its lower bound.
    #[error("year range upper bound is below lower bound")]
    InvalidYearRange,
    /// A [`Period`](crate::Period) did not correspond to a canonical
    /// [`Frequency`](crate::Frequency) — e.g. every 5 months, every
    /// 3 weeks. The period is fine on its own; only the conversion
    /// fails.
    #[error("period does not correspond to a canonical frequency")]
    NonCanonicalPeriod,
}
