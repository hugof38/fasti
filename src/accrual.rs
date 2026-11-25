//! What a [`Schedule`](crate::Schedule) hands out: [`AccrualPeriod`]s,
//! [`PaymentDate`]s and the [`Coupon`]s pairing them, and the
//! [`AccrualDayCount`]s that measure a period.
//!
//! The types keep apart what one list of dates used to blur. An
//! accrual period can only come from a schedule, carries the reference
//! period and lattice ACT/ACT ICMA counts on, and is a different type
//! from the date its coupon is paid on.

use core::fmt;
use core::ops::Range;

use crate::{Date, DayCount, Fraction, Generation, TimeError};

/// One accrual period of a [`Schedule`](crate::Schedule): interest runs
/// from [`start`](Self::start) to [`end`](Self::end), half-open.
///
/// It also carries what schedule-defined day counts need: the
/// [`reference`](Self::reference) period a stub accrues against, and
/// the [`Generation`] it came from. Only a schedule constructs one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccrualPeriod {
    start: Date,
    end: Date,
    reference: Range<Date>,
    generation: Option<Generation>,
}

impl AccrualPeriod {
    pub(crate) const fn new(
        accrual: Range<Date>,
        reference: Range<Date>,
        generation: Option<Generation>,
    ) -> Self {
        Self {
            start: accrual.start,
            end: accrual.end,
            reference,
            generation,
        }
    }

    /// The first day of accrual.
    #[must_use]
    pub const fn start(&self) -> Date {
        self.start
    }

    /// The day accrual stops (exclusive): the coupon date, adjusted as
    /// the schedule's accrual convention says.
    #[must_use]
    pub const fn end(&self) -> Date {
        self.end
    }

    /// `start..end`.
    #[must_use]
    pub const fn range(&self) -> Range<Date> {
        self.start..self.end
    }

    /// The regular coupon period this one accrues against: itself unless
    /// it is a stub.
    #[must_use]
    pub const fn reference(&self) -> Range<Date> {
        self.reference.start..self.reference.end
    }

    /// The lattice the schedule was generated on, absent for a schedule
    /// built from a bare date list.
    #[must_use]
    pub const fn generation(&self) -> Option<Generation> {
        self.generation
    }

    /// `true` iff `date` falls in `start..end`.
    #[must_use]
    pub fn contains(&self, date: Date) -> bool {
        self.start <= date && date < self.end
    }

    /// This period cut short at `date`, for accrued interest: `None`
    /// unless `start <= date <= end`. On `start` the result has zero
    /// length, so a coupon date accrues nothing. Reference period and
    /// lattice are kept.
    ///
    /// ```
    /// use fasti::{Act365Fixed, AccrualDayCount, Date, Fraction, Month, Period, ScheduleBuilder, calendars};
    /// let s = ScheduleBuilder::new(
    ///     Date::from_ymd(2025, Month::Jan, 15)?,
    ///     Date::from_ymd(2026, Month::Jan, 15)?,
    ///     Period::Months(6),
    ///     calendars::WEEKENDS_ONLY,
    /// )
    /// .build()?;
    /// let settle = Date::from_ymd(2025, Month::Mar, 1)?;
    /// let to_date = s.accrual_period(settle).and_then(|p| p.until(settle)).unwrap();
    /// assert_eq!(Act365Fixed.accrual_fraction(&to_date)?, Fraction::new(45, 365)?);
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    #[must_use]
    pub fn until(&self, date: Date) -> Option<Self> {
        (self.start <= date && date <= self.end).then(|| Self {
            end: date,
            ..self.clone()
        })
    }
}

/// The date a coupon is paid: its accrual end rolled under the
/// schedule's payment convention, or a payment lag of business days
/// after it. A type of its own so it cannot stand in for an accrual
/// date; [`date`](Self::date) takes the [`Date`] out explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct PaymentDate(Date);

impl PaymentDate {
    pub(crate) const fn new(date: Date) -> Self {
        Self(date)
    }

    /// The calendar date of the payment.
    #[must_use]
    pub const fn date(self) -> Date {
        self.0
    }
}

impl fmt::Display for PaymentDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

/// One coupon of a [`Schedule`](crate::Schedule): what accrues, and
/// when it is paid.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Coupon {
    /// The period interest accrues over.
    pub accrual: AccrualPeriod,
    /// When the coupon is paid.
    pub payment: PaymentDate,
}

/// A convention measuring an [`AccrualPeriod`].
///
/// Every [`DayCount`] is one, measuring `start..end`. ACT/ACT ICMA
/// ([`ActActICMA`](crate::ActActICMA)) is *only* one: it also needs the
/// period's reference period and coupon frequency, which two bare dates
/// do not carry.
///
/// Fallible because ICMA needs a coupon frequency the period may not
/// name ([`TimeError::MissingFrequency`],
/// [`TimeError::NonCanonicalPeriod`]); a [`DayCount`] never fails here.
pub trait AccrualDayCount {
    /// The year fraction accrued over `period`.
    fn accrual_fraction(&self, period: &AccrualPeriod) -> Result<Fraction, TimeError>;
}

impl<T: DayCount + ?Sized> AccrualDayCount for T {
    fn accrual_fraction(&self, period: &AccrualPeriod) -> Result<Fraction, TimeError> {
        Ok(self.year_fraction(period.start, period.end))
    }
}
