//! [`BusinessDayConvention`] — how to roll a date that is not a business
//! day onto one that is.
//!
//! Matches `QuantLib`'s semantics.

use core::fmt;

/// How to roll a non-business date onto a business date.
///
/// Apply via [`Calendar::adjust`](crate::Calendar::adjust); `adjust`
/// only fails when the search would leave the supported date range.
///
/// ```
/// use fasti::{BusinessDayConvention, Date, Month, calendars};
///
/// // Saturday rolls forward to Monday under Following.
/// let sat = Date::from_ymd(2024, Month::Jul, 6)?;
/// let adjusted = calendars::WEEKENDS_ONLY.adjust(sat, BusinessDayConvention::Following)?;
/// assert_eq!(adjusted, Date::from_ymd(2024, Month::Jul, 8)?);
/// # Ok::<(), fasti::TimeError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BusinessDayConvention {
    /// Return the input date unchanged, even if it is not a business day.
    Unadjusted,
    /// First business day on or after the input.
    Following,
    /// Like [`Following`](Self::Following), but if rolling forward
    /// would cross a calendar-month boundary, fall back to the first
    /// business day on or before the input.
    ModifiedFollowing,
    /// First business day on or before the input.
    Preceding,
    /// Like [`Preceding`](Self::Preceding), but if rolling backward
    /// would cross a calendar-month boundary, fall back to the first
    /// business day on or after the input.
    ModifiedPreceding,
}

impl fmt::Display for BusinessDayConvention {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Unadjusted => "Unadjusted",
            Self::Following => "Following",
            Self::ModifiedFollowing => "ModifiedFollowing",
            Self::Preceding => "Preceding",
            Self::ModifiedPreceding => "ModifiedPreceding",
        };
        f.write_str(name)
    }
}
