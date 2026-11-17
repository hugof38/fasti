//! Error type returned by fallible `fasti` constructors.
//!
//! Invalid inputs are refused at construction rather than silently
//! coerced; every refusal surfaces as a [`TimeError`].

/// Errors produced by `fasti` constructors.
///
/// Marked `#[non_exhaustive]`: new variants may arrive in minor
/// releases, so a `match` on it needs a wildcard arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
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
    /// A string passed to [`Date`](crate::Date)'s [`FromStr`](core::str::FromStr)
    /// impl was not in `YYYY-MM-DD` form.
    #[error("date string is not in YYYY-MM-DD form")]
    InvalidDateString,
    /// A [`YearRange`](crate::YearRange) was constructed with an upper
    /// bound below its lower bound.
    #[error("year range upper bound is below lower bound")]
    InvalidYearRange,
    /// A [`Period`](crate::Period) did not correspond to a canonical
    /// [`Frequency`](crate::Frequency) — e.g. every 5 months.
    #[error("period does not correspond to a canonical frequency")]
    NonCanonicalPeriod,
    /// A [`Fraction`](crate::Fraction) was constructed with a
    /// zero denominator.
    #[error("fraction denominator must be non-zero")]
    ZeroDenominator,
    /// An [`ActActICMA`](crate::ActActICMA) reference period did not have
    /// its start strictly before its end.
    #[error("reference period start must be strictly before its end")]
    InvalidReferencePeriod,
    /// [`Fraction`](crate::Fraction) arithmetic overflowed the
    /// `i64`/`u64` representation (or `i64` when scaling an amount).
    #[error("fraction arithmetic overflowed")]
    FractionOverflow,
    /// A day count was bound to a [`Schedule`](crate::Schedule) whose
    /// tenor disagrees with the convention's coupon frequency.
    #[error("day-count frequency does not match the schedule's tenor")]
    FrequencyMismatch,
    /// A [`Schedule`](crate::Schedule) builder was given an
    /// effective date at or after the termination date.
    #[error("schedule effective date must be strictly before termination")]
    EffectiveAfterTermination,
    /// A [`Schedule`](crate::Schedule) builder was given a zero
    /// tenor on a non-`Zero` generation rule.
    #[error("schedule tenor must be non-zero for Forward/Backward rules")]
    ZeroTenor,
    /// A [`Schedule`](crate::Schedule) builder stub date did not fall
    /// strictly between effective and termination, or the stub dates
    /// were out of order.
    #[error("schedule stub date is out of (effective, termination) range")]
    StubDateOutOfRange,
    /// A [`Schedule`](crate::Schedule) build produced dates that are not
    /// strictly monotonically increasing after business-day adjustment.
    #[error("schedule dates are not strictly monotonic after adjustment")]
    ScheduleNotMonotonic,
    /// A day count was bound to a [`Schedule`](crate::Schedule) with
    /// fewer than two dates, so it has no period to accrue over.
    #[error("schedule has fewer than two dates")]
    ScheduleTooShort,
    /// A [`Period`](crate::Period) string was not a signed integer
    /// followed by one of `D`, `W`, `M`, `Y` (for example `6M`).
    #[error("period string is not in `<n><D|W|M|Y>` form, e.g. `6M`")]
    InvalidPeriodString,
    /// A name passed to a `FromStr` implementation — a
    /// [`Frequency`](crate::Frequency) or a
    /// [`BusinessDayConvention`](crate::BusinessDayConvention) — matched
    /// no member.
    #[error("name does not match any known member")]
    UnknownName,
    /// A [`Weekend`](crate::Weekend) bit mask set bits beyond the seven
    /// weekdays.
    #[error("weekend mask must only use the low seven bits")]
    InvalidWeekendMask,
}
