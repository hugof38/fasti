//! [`DayCount`] — conventions measuring elapsed time between two
//! [`Date`]s as a [`Fraction`]. Modeled on `QuantLib`'s `ql/time`
//! day counters.
//!
//! All conventions are signed by direction (`yf(a, b) == -yf(b, a)`)
//! and return [`Fraction::ZERO`] for equal dates. ACT-family
//! conventions are additive across splits; the 30/360 family is
//! intentionally not — both facts are property-tested.

use core::ops::Range;

use crate::{
    AccrualDayCount, AccrualPeriod, Date, DateRange, Fraction, Frequency, Generation, Month,
    TimeError, Year,
};

/// A day-count convention.
///
/// ```
/// use fasti::{Act360, DayCount, Date, Month};
/// let start = Date::from_ymd(2025, Month::Jan, 1)?;
/// let end = Date::from_ymd(2025, Month::Apr, 1)?;
/// assert_eq!(Act360.year_fraction(start, end).parts(), (1, 4));
/// # Ok::<(), fasti::TimeError>(())
/// ```
pub trait DayCount {
    /// A short human-readable name like `"Actual/360"`.
    fn name(&self) -> &'static str;

    /// Days between `start` and `end`, signed by direction. Defaults
    /// to calendar days; the 30/360 family overrides.
    fn day_count(&self, start: Date, end: Date) -> i64 {
        (start..end).days()
    }

    /// The year fraction between `start` and `end`: signed by
    /// direction, [`Fraction::ZERO`] for equal dates. (ACT/ACT ICMA
    /// needs a schedule's period, so it is an [`AccrualDayCount`] only.)
    fn year_fraction(&self, start: Date, end: Date) -> Fraction;
}

// ---- Basis --------------------------------------------------------------

/// The denominator a day count divides by — a fixed 360/365 basis, or
/// a year length for the ACT/ACT family.
#[derive(Debug, Clone, Copy)]
struct Basis(u64);

impl Basis {
    const DAYS_360: Self = Self(360);
    const DAYS_365: Self = Self(365);

    const fn of(days: u64) -> Self {
        Self(days)
    }

    /// `days / self`, reduced. Every basis is non-zero, so the
    /// zero-denominator arm is unreachable and yields
    /// [`Fraction::ZERO`] to honour the no-panic contract.
    fn fraction(self, days: i64) -> Fraction {
        Fraction::new(days, self.0).unwrap_or_default()
    }
}

// ---- ACT family ---------------------------------------------------------

/// Actual/360: calendar days over 360. Standard for USD money-market
/// and most floating-rate structured-credit accruals.
///
/// ```
/// use fasti::{Act360, DayCount, Date, Month};
/// let start = Date::from_ymd(2024, Month::Jan, 1)?;
/// let end = Date::from_ymd(2025, Month::Jan, 1)?;
/// assert_eq!(Act360.year_fraction(start, end).parts(), (61, 60)); // 366/360
/// # Ok::<(), fasti::TimeError>(())
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Act360;

impl DayCount for Act360 {
    fn name(&self) -> &'static str {
        "Actual/360"
    }

    fn year_fraction(&self, start: Date, end: Date) -> Fraction {
        Basis::DAYS_360.fraction(self.day_count(start, end))
    }
}

/// Actual/365 (Fixed): calendar days over 365, regardless of leap
/// years. Standard for GBP money markets.
///
/// ```
/// use fasti::{Act365Fixed, DayCount, Date, Month};
/// let start = Date::from_ymd(2025, Month::Jan, 1)?;
/// let end = Date::from_ymd(2026, Month::Jan, 1)?;
/// assert_eq!(Act365Fixed.year_fraction(start, end).parts(), (1, 1));
/// # Ok::<(), fasti::TimeError>(())
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Act365Fixed;

impl DayCount for Act365Fixed {
    fn name(&self) -> &'static str {
        "Actual/365 (Fixed)"
    }

    fn year_fraction(&self, start: Date, end: Date) -> Fraction {
        Basis::DAYS_365.fraction(self.day_count(start, end))
    }
}

/// Actual/Actual (ISDA): splits at calendar-year boundaries, weighting
/// leap-year days by 1/366 and others by 1/365. Matches `QuantLib`'s
/// `ActualActual::ISDA_Impl`.
///
/// ```
/// use fasti::{ActActISDA, DayCount, Date, Month};
/// // 61 days in 2003 (/365) + 121 days in 2004 (/366):
/// let start = Date::from_ymd(2003, Month::Nov, 1)?;
/// let end = Date::from_ymd(2004, Month::May, 1)?;
/// assert_eq!(ActActISDA.year_fraction(start, end).parts(), (66491, 133590));
/// # Ok::<(), fasti::TimeError>(())
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ActActISDA;

impl ActActISDA {
    /// `(N·dib1·dib2 + a·dib2 + b·dib1) / (dib1·dib2)` for an ordered
    /// span: `N` full middle years, `a`/`b` partial days in the end
    /// years, `dib*` those years' lengths.
    fn ordered_year_fraction(span: &Range<Date>) -> Fraction {
        if span.start == span.end {
            return Fraction::ZERO;
        }
        let (y1, y2) = (span.start.year(), span.end.year());
        if y1 == y2 {
            return Basis::of(u64::from(y1.length())).fraction(span.days());
        }
        let n = i64::from(y2.get()) - i64::from(y1.get()) - 1;
        let dib1 = i64::from(y1.length());
        let dib2 = i64::from(y2.length());
        // y1 < y2 <= Year::MAX, so both constructions are in range.
        let (Ok(next_year_start), Ok(this_year_start)) = (
            Date::from_ymd(y1.get() + 1, Month::Jan, 1),
            Date::from_ymd(y2.get(), Month::Jan, 1),
        ) else {
            return Fraction::ZERO;
        };
        let a = (span.start..next_year_start).days();
        let b = (this_year_start..span.end).days();
        // dib1·dib2 <= 366², positive.
        #[allow(clippy::cast_sign_loss)]
        Basis::of((dib1 * dib2) as u64).fraction(n * dib1 * dib2 + a * dib2 + b * dib1)
    }
}

impl DayCount for ActActISDA {
    fn name(&self) -> &'static str {
        "Actual/Actual (ISDA)"
    }

    fn year_fraction(&self, start: Date, end: Date) -> Fraction {
        if start <= end {
            Self::ordered_year_fraction(&(start..end))
        } else {
            // Numerators stay far below |i64::MIN|; negation cannot fail.
            Self::ordered_year_fraction(&(end..start))
                .checked_neg()
                .unwrap_or_default()
        }
    }
}

// ---- ACT/ACT (ICMA) -----------------------------------------------------

/// Actual/Actual (ICMA), ICMA Rule 251 — the standard for fixed-rate
/// bond accruals: a regular coupon period accrues exactly
/// `1 / frequency`, days accrue against their period's actual length,
/// and stubs accrue against a notional coupon grid.
///
/// It measures [`AccrualPeriod`]s, not pairs of dates: it counts the
/// period's accrual dates against its reference period, and takes the
/// coupon frequency from the schedule that made the period, so there is
/// no second frequency to disagree with. It deliberately does not
/// implement [`DayCount`]. Like every other convention it accrues on the
/// dates the instrument accrues on: unadjusted for a bond (the
/// [`ScheduleBuilder`](crate::ScheduleBuilder) default), adjusted when
/// the terms say so. The algorithm matches `QuantLib`'s
/// `ActualActual::Old_ISMA_Impl`, with the frequency taken from the
/// schedule's tenor rather than float-rounded from a period length.
///
/// ```
/// use fasti::{ActActICMA, AccrualDayCount, Date, Fraction, Month, Period, ScheduleBuilder, calendars};
/// let schedule = ScheduleBuilder::new(
///     Date::from_ymd(2002, Month::Aug, 15)?,
///     Date::from_ymd(2004, Month::Jan, 15)?,
///     Period::Months(6),
///     calendars::NULL_CALENDAR,
/// )
/// .build()?;
/// // The front stub accrues against its notional grid.
/// let stub = schedule.periods().next().unwrap();
/// assert_eq!(ActActICMA.accrual_fraction(&stub)?, Fraction::new(153, 368)?);
/// # Ok::<(), fasti::TimeError>(())
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ActActICMA;

impl ActActICMA {
    /// `"Actual/Actual (ICMA)"`.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        "Actual/Actual (ICMA)"
    }

    /// The escape hatch for accruals with no schedule: the year
    /// fraction from `start` to `end` against the regular reference
    /// period `ref_start..ref_end` of a `frequency` coupon grid.
    ///
    /// Errors: [`TimeError::InvalidReferencePeriod`] when
    /// `ref_start >= ref_end`; [`TimeError::DateOutOfRange`] when a
    /// notional walk escapes the supported range;
    /// [`TimeError::FractionOverflow`] on overflow.
    ///
    /// ```
    /// use fasti::{ActActICMA, Date, Fraction, Frequency, Month};
    /// let start = Date::from_ymd(2003, Month::Nov, 1)?;
    /// let end = Date::from_ymd(2004, Month::May, 1)?;
    /// let yf = ActActICMA.year_fraction_with_reference(start, end, start, end, Frequency::Semiannual)?;
    /// assert_eq!(yf, Fraction::new(1, 2)?);
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    pub fn year_fraction_with_reference(
        self,
        start: Date,
        end: Date,
        ref_start: Date,
        ref_end: Date,
        frequency: Frequency,
    ) -> Result<Fraction, TimeError> {
        if ref_start >= ref_end {
            return Err(TimeError::InvalidReferencePeriod);
        }
        let (lattice, reference) = (Generation::of(frequency), ref_start..ref_end);
        if start <= end {
            accrue(start..end, &reference, lattice)
        } else {
            accrue(end..start, &reference, lattice)?
                .checked_neg()
                .ok_or(TimeError::FractionOverflow)
        }
    }
}

impl AccrualDayCount for ActActICMA {
    /// Accrues the period against its reference period. Errors with
    /// [`TimeError::MissingFrequency`] for a period of a schedule built
    /// from a bare date list, and [`TimeError::NonCanonicalPeriod`] for a
    /// tenor that is not a whole number of coupons a year.
    fn accrual_fraction(&self, period: &AccrualPeriod) -> Result<Fraction, TimeError> {
        let lattice = period.generation().ok_or(TimeError::MissingFrequency)?;
        accrue(period.range(), &period.reference(), lattice)
    }
}

/// Accrue `span` over the grid anchored on `reference`: descend to the
/// lowest overlapping window, then add each window's share of a coupon
/// until the span is covered. An empty span accrues zero.
fn accrue(
    span: Range<Date>,
    reference: &Range<Date>,
    lattice: Generation,
) -> Result<Fraction, TimeError> {
    let frequency = Frequency::try_from(lattice.tenor)?;
    if span.is_empty() {
        return Ok(Fraction::ZERO);
    }
    let mut i = 0;
    while lattice.window(reference, i)?.start > span.start {
        i -= 1;
    }
    let mut total = Fraction::ZERO;
    loop {
        let window = lattice.window(reference, i)?;
        if let Some(chunk) = span.intersect(&window) {
            let window_days =
                u64::try_from(window.days()).map_err(|_| TimeError::InvalidReferencePeriod)?;
            let denominator = u64::from(frequency.per_year())
                .checked_mul(window_days)
                .ok_or(TimeError::FractionOverflow)?;
            total = total
                .checked_add(Fraction::new(chunk.days(), denominator)?)
                .ok_or(TimeError::FractionOverflow)?;
        }
        if window.end >= span.end {
            return Ok(total);
        }
        i += 1;
    }
}

// ---- 30/360 family ------------------------------------------------------

/// A date decomposed into the fields 30/360 conventions adjust,
/// keeping the original [`Date`] for identity comparisons.
#[derive(Debug, Clone, Copy)]
struct Thirty360Date {
    date: Date,
    year: Year,
    month: Month,
    day: u8,
}

impl Thirty360Date {
    fn of(date: Date) -> Self {
        let (year, month, day) = date.to_ymd();
        Self {
            date,
            year,
            month,
            day,
        }
    }

    /// Apply a variant's `rule` to the ordered pair, negating for
    /// reversed inputs so `dc(a, b) == -dc(b, a)` holds for the
    /// family's asymmetric formulas.
    fn signed(start: Date, end: Date, rule: impl FnOnce(Self, Self) -> i64) -> i64 {
        if start <= end {
            rule(Self::of(start), Self::of(end))
        } else {
            -rule(Self::of(end), Self::of(start))
        }
    }

    fn is_last_of_february(self) -> bool {
        matches!(self.month, Month::Feb) && self.day == Month::Feb.length(self.year)
    }

    /// The 31st counted as the 30th.
    fn capped(self) -> Self {
        self.with_day(if self.day == 31 { 30 } else { self.day })
    }

    fn with_day(self, day: u8) -> Self {
        Self { day, ..self }
    }

    /// The family's shared formula, `360·Δy + 30·Δm + Δd`, over the
    /// variant's already-adjusted day-of-month pair.
    fn days_to(self, end: Self) -> i64 {
        360 * (i64::from(end.year.get()) - i64::from(self.year.get()))
            + 30 * (i64::from(end.month.get()) - i64::from(self.month.get()))
            + (i64::from(end.day) - i64::from(self.day))
    }
}

/// 30/360 Bond Basis (ISDA 2006). Matches `QuantLib`'s
/// `Thirty360::BondBasis`. Not additive across splits.
///
/// ```
/// use fasti::{DayCount, Date, Month, Thirty360Bond};
/// let start = Date::from_ymd(2025, Month::Jan, 1)?;
/// let end = Date::from_ymd(2025, Month::Jul, 1)?;
/// assert_eq!(Thirty360Bond.year_fraction(start, end).parts(), (1, 2));
/// # Ok::<(), fasti::TimeError>(())
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Thirty360Bond;

impl DayCount for Thirty360Bond {
    fn name(&self) -> &'static str {
        "30/360 (Bond Basis)"
    }

    fn day_count(&self, start: Date, end: Date) -> i64 {
        Thirty360Date::signed(start, end, |start, end| {
            let start = start.capped();
            let end = if end.day == 31 && start.day == 30 {
                end.with_day(30)
            } else {
                end
            };
            start.days_to(end)
        })
    }

    fn year_fraction(&self, start: Date, end: Date) -> Fraction {
        Basis::DAYS_360.fraction(self.day_count(start, end))
    }
}

/// 30/360 (US) — SIA convention: Bond Basis plus the
/// last-day-of-February rule, applied first. Matches `QuantLib`'s
/// `Thirty360::USA`.
///
/// ```
/// use fasti::{DayCount, Date, Month, Thirty360US};
/// // Last-of-Feb start: D1 -> 30, then D2 = 31 -> 30.
/// let start = Date::from_ymd(2025, Month::Feb, 28)?;
/// let end = Date::from_ymd(2025, Month::Mar, 31)?;
/// assert_eq!(Thirty360US.day_count(start, end), 30);
/// # Ok::<(), fasti::TimeError>(())
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Thirty360US;

impl DayCount for Thirty360US {
    fn name(&self) -> &'static str {
        "30/360 (US)"
    }

    fn day_count(&self, start: Date, end: Date) -> i64 {
        Thirty360Date::signed(start, end, |start, end| {
            let (start, end) = if start.is_last_of_february() {
                let end = if end.is_last_of_february() {
                    end.with_day(30)
                } else {
                    end
                };
                (start.with_day(30), end)
            } else {
                (start, end)
            };
            let end = if end.day == 31 && start.day >= 30 {
                end.with_day(30)
            } else {
                end
            };
            start.capped().days_to(end)
        })
    }

    fn year_fraction(&self, start: Date, end: Date) -> Fraction {
        Basis::DAYS_360.fraction(self.day_count(start, end))
    }
}

/// 30E/360 (Eurobond Basis): both day-of-month values independently
/// capped at 30. Matches `QuantLib`'s `Thirty360::EurobondBasis`.
///
/// ```
/// use fasti::{DayCount, Date, Month, Thirty360European};
/// let start = Date::from_ymd(2025, Month::Jan, 15)?;
/// let end = Date::from_ymd(2025, Month::Mar, 31)?;
/// assert_eq!(Thirty360European.day_count(start, end), 75);
/// # Ok::<(), fasti::TimeError>(())
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Thirty360European;

impl DayCount for Thirty360European {
    fn name(&self) -> &'static str {
        "30E/360 (Eurobond Basis)"
    }

    fn day_count(&self, start: Date, end: Date) -> i64 {
        Thirty360Date::signed(start, end, |start, end| {
            start.capped().days_to(end.capped())
        })
    }

    fn year_fraction(&self, start: Date, end: Date) -> Fraction {
        Basis::DAYS_360.fraction(self.day_count(start, end))
    }
}

/// 30E/360 (ISDA), a.k.a. 30/360 German: [`Thirty360European`] plus
/// February handling — a last-of-February day counts as 30, except an
/// end date equal to the instrument's termination date (hence the
/// constructor argument). Matches `QuantLib`'s `Thirty360::ISDA`.
///
/// ```
/// use fasti::{DayCount, Date, Month, Thirty360ISDA};
/// let start = Date::from_ymd(2025, Month::Aug, 31)?;
/// let feb_end = Date::from_ymd(2026, Month::Feb, 28)?;
/// // Mid-schedule Feb end counts as 30; at maturity it stays 28.
/// assert_eq!(Thirty360ISDA::new(Date::MAX).day_count(start, feb_end), 180);
/// assert_eq!(Thirty360ISDA::new(feb_end).day_count(start, feb_end), 178);
/// # Ok::<(), fasti::TimeError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Thirty360ISDA {
    termination: Date,
}

impl Thirty360ISDA {
    /// Construct for an instrument maturing on `termination`.
    #[must_use]
    pub const fn new(termination: Date) -> Self {
        Self { termination }
    }

    /// The termination (maturity) date.
    #[must_use]
    pub const fn termination(&self) -> Date {
        self.termination
    }
}

impl DayCount for Thirty360ISDA {
    fn name(&self) -> &'static str {
        "30E/360 (ISDA)"
    }

    fn day_count(&self, start: Date, end: Date) -> i64 {
        Thirty360Date::signed(start, end, |start, end| {
            let start = if start.is_last_of_february() {
                start.with_day(30)
            } else {
                start.capped()
            };
            let end = if end.date != self.termination && end.is_last_of_february() {
                end.with_day(30)
            } else {
                end.capped()
            };
            start.days_to(end)
        })
    }

    fn year_fraction(&self, start: Date, end: Date) -> Fraction {
        Basis::DAYS_360.fraction(self.day_count(start, end))
    }
}

// ---- Tests --------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    extern crate alloc;

    use super::*;
    use crate::{Month, Period, Schedule};
    use proptest::prelude::*;

    fn ymd(y: u16, m: Month, d: u8) -> Date {
        Date::from_ymd(y, m, d).unwrap()
    }

    // ---- Names ---------------------------------------------------------

    #[test]
    fn names_are_canonical() {
        assert_eq!(Act360.name(), "Actual/360");
        assert_eq!(Act365Fixed.name(), "Actual/365 (Fixed)");
    }

    // ---- day_count -----------------------------------------------------

    #[test]
    fn day_count_zero_for_same_date() {
        let d = ymd(2025, Month::Jul, 4);
        assert_eq!(Act360.day_count(d, d), 0);
        assert_eq!(Act365Fixed.day_count(d, d), 0);
    }

    #[test]
    fn day_count_signs_by_direction() {
        let a = ymd(2025, Month::Jan, 1);
        let b = ymd(2025, Month::Jan, 31);
        assert_eq!(Act360.day_count(a, b), 30);
        assert_eq!(Act360.day_count(b, a), -30);
    }

    // ---- Act360 examples ----------------------------------------------

    #[test]
    fn act360_zero_period_is_zero_fraction() {
        let d = ymd(2025, Month::Jul, 4);
        assert!(Act360.year_fraction(d, d).is_zero());
    }

    #[test]
    fn act360_30_day_period() {
        // Jan 1 to Jan 31 = 30 days = 30/360 = 1/12.
        let start = ymd(2025, Month::Jan, 1);
        let end = ymd(2025, Month::Jan, 31);
        assert_eq!(Act360.year_fraction(start, end).parts(), (1, 12));
    }

    #[test]
    fn act360_full_non_leap_year() {
        // 2025 has 365 days; ACT/360 fraction is 365/360 = 73/72.
        let start = ymd(2025, Month::Jan, 1);
        let end = ymd(2026, Month::Jan, 1);
        assert_eq!(Act360.year_fraction(start, end).parts(), (73, 72));
    }

    #[test]
    fn act360_full_leap_year() {
        // 2024 has 366 days; ACT/360 fraction is 366/360 = 61/60.
        let start = ymd(2024, Month::Jan, 1);
        let end = ymd(2025, Month::Jan, 1);
        assert_eq!(Act360.year_fraction(start, end).parts(), (61, 60));
    }

    // ---- Act365Fixed examples -----------------------------------------

    #[test]
    fn act365f_zero_period_is_zero_fraction() {
        let d = ymd(2025, Month::Jul, 4);
        assert!(Act365Fixed.year_fraction(d, d).is_zero());
    }

    #[test]
    fn act365f_full_non_leap_year_is_unity() {
        // 2025 has 365 days; ACT/365F fraction is 365/365 = 1/1.
        let start = ymd(2025, Month::Jan, 1);
        let end = ymd(2026, Month::Jan, 1);
        assert_eq!(Act365Fixed.year_fraction(start, end).parts(), (1, 1));
    }

    #[test]
    fn act365f_full_leap_year_exceeds_unity() {
        // 2024 has 366 days; ACT/365F fraction is 366/365 (not reducible).
        let start = ymd(2024, Month::Jan, 1);
        let end = ymd(2025, Month::Jan, 1);
        assert_eq!(Act365Fixed.year_fraction(start, end).parts(), (366, 365));
    }

    #[test]
    fn act365f_quarter_period() {
        // Jan 1 to Apr 1 in non-leap 2025: 31 + 28 + 31 = 90 days; 90/365 = 18/73.
        let start = ymd(2025, Month::Jan, 1);
        let end = ymd(2025, Month::Apr, 1);
        assert_eq!(Act365Fixed.year_fraction(start, end).parts(), (18, 73));
    }

    // ---- Thirty360Bond examples ---------------------------------------

    #[test]
    fn thirty_360_bond_name() {
        assert_eq!(Thirty360Bond.name(), "30/360 (Bond Basis)");
    }

    #[test]
    fn thirty_360_bond_zero_period() {
        let d = ymd(2025, Month::Jul, 4);
        assert_eq!(Thirty360Bond.day_count(d, d), 0);
        assert!(Thirty360Bond.year_fraction(d, d).is_zero());
    }

    #[test]
    fn thirty_360_bond_six_months_is_half_year() {
        // Aug 28 2003 → Feb 28 2004: 6 months. count = 360 + 30·(-6) = 180.
        let start = ymd(2003, Month::Aug, 28);
        let end = ymd(2004, Month::Feb, 28);
        assert_eq!(Thirty360Bond.day_count(start, end), 180);
        assert_eq!(Thirty360Bond.year_fraction(start, end).parts(), (1, 2));
    }

    #[test]
    fn thirty_360_bond_d1_31_adjusts_to_30() {
        // Jan 31 → Feb 28: D1=31→30. D2=28 (no adjust).
        // count = 0 + 30 + (28-30) = 28.
        let start = ymd(2025, Month::Jan, 31);
        let end = ymd(2025, Month::Feb, 28);
        assert_eq!(Thirty360Bond.day_count(start, end), 28);
    }

    #[test]
    fn thirty_360_bond_d2_31_does_not_adjust_when_d1_lt_30() {
        // Feb 28 → Mar 31: D1=28 (no adjust). D2=31, but D1≠30 so D2 stays 31.
        // count = 0 + 30 + (31-28) = 33.
        let start = ymd(2025, Month::Feb, 28);
        let end = ymd(2025, Month::Mar, 31);
        assert_eq!(Thirty360Bond.day_count(start, end), 33);
    }

    #[test]
    fn thirty_360_bond_both_31_adjust() {
        // Jan 31 → Mar 31: D1=31→30. D2=31, D1=30 so D2→30.
        // count = 0 + 60 + 0 = 60.
        let start = ymd(2025, Month::Jan, 31);
        let end = ymd(2025, Month::Mar, 31);
        assert_eq!(Thirty360Bond.day_count(start, end), 60);
        assert_eq!(Thirty360Bond.year_fraction(start, end).parts(), (1, 6));
    }

    #[test]
    fn thirty_360_bond_year_crossing_with_d2_31() {
        // Aug 28 2023 → Aug 31 2024: D1=28, D2=31 (no adjust since D1≠30).
        // count = 360 + 0 + (31-28) = 363.
        let start = ymd(2023, Month::Aug, 28);
        let end = ymd(2024, Month::Aug, 31);
        assert_eq!(Thirty360Bond.day_count(start, end), 363);
    }

    /// Canonical non-additivity case: stepping Jan 31 → Feb 28 → Mar 31
    /// gives 28 + 33 = 61, but Jan 31 → Mar 31 directly is 60. The
    /// difference is the day adjustments interacting with the split
    /// point's day-of-month.
    #[test]
    fn thirty_360_bond_is_not_additive_jan31_feb28_mar31() {
        let a = ymd(2025, Month::Jan, 31);
        let b = ymd(2025, Month::Feb, 28);
        let c = ymd(2025, Month::Mar, 31);
        let split = Thirty360Bond.day_count(a, b) + Thirty360Bond.day_count(b, c);
        let direct = Thirty360Bond.day_count(a, c);
        assert_eq!(split, 61);
        assert_eq!(direct, 60);
        assert_ne!(split, direct);
    }

    // ---- Thirty360US / European / ISDA examples -----------------------

    #[test]
    fn thirty_360_variant_names() {
        assert_eq!(Thirty360US.name(), "30/360 (US)");
        assert_eq!(Thirty360European.name(), "30E/360 (Eurobond Basis)");
        let isda = Thirty360ISDA::new(ymd(2030, Month::Jan, 1));
        assert_eq!(isda.name(), "30E/360 (ISDA)");
    }

    /// The canonical case distinguishing the three 31st-day rules:
    /// Jan 15 → Mar 31.
    #[test]
    fn thirty_360_variants_disagree_on_lone_31_end() {
        let start = ymd(2025, Month::Jan, 15);
        let end = ymd(2025, Month::Mar, 31);
        // Bond Basis and US keep D2 = 31 because D1 < 30.
        assert_eq!(Thirty360Bond.day_count(start, end), 76);
        assert_eq!(Thirty360US.day_count(start, end), 76);
        // European caps D2 unconditionally.
        assert_eq!(Thirty360European.day_count(start, end), 75);
        assert_eq!(
            Thirty360ISDA::new(ymd(2030, Month::Jan, 1)).day_count(start, end),
            75,
        );
    }

    /// The last-of-February rule distinguishing US from Bond Basis.
    #[test]
    fn thirty_360_us_february_rule() {
        // Feb 28 2025 (last of Feb, non-leap) → Mar 31 2025.
        // US: D1 = 30 (Feb rule), then D2 = 31 → 30. Count = 30.
        // Bond: D1 = 28 stays, D2 = 31 stays (D1 ≠ 30). Count = 33.
        let start = ymd(2025, Month::Feb, 28);
        let end = ymd(2025, Month::Mar, 31);
        assert_eq!(Thirty360US.day_count(start, end), 30);
        assert_eq!(Thirty360Bond.day_count(start, end), 33);

        // Both dates last-of-Feb: Feb 29 2024 → Feb 28 2025.
        // US: both → 30, count = 360. Bond: 360 + (28 - 29) = 359.
        let leap_feb = ymd(2024, Month::Feb, 29);
        let next_feb = ymd(2025, Month::Feb, 28);
        assert_eq!(Thirty360US.day_count(leap_feb, next_feb), 360);
        assert_eq!(Thirty360Bond.day_count(leap_feb, next_feb), 359);

        // Feb 28 in a leap year is NOT the last of February — no rule.
        let feb28_leap = ymd(2024, Month::Feb, 28);
        assert_eq!(
            Thirty360US.day_count(feb28_leap, ymd(2024, Month::Mar, 31)),
            33,
        );
    }

    #[test]
    fn thirty_360_isda_termination_exception() {
        // Aug 31 2025 → Feb 28 2026 (last of Feb).
        // Mid-schedule: D1 = 31 → 30, D2 = 28 → 30 (Feb rule): 180.
        let start = ymd(2025, Month::Aug, 31);
        let feb_end = ymd(2026, Month::Feb, 28);
        let interim = Thirty360ISDA::new(ymd(2030, Month::Jan, 1));
        assert_eq!(interim.day_count(start, feb_end), 180);
        // Same dates but Feb 28 2026 IS the termination: D2 stays 28,
        // count = 360·1 + 30·(2−8) + (28−30) = 178.
        let at_maturity = Thirty360ISDA::new(feb_end);
        assert_eq!(at_maturity.day_count(start, feb_end), 178);
        // Start-side Feb rule has no termination exception.
        assert_eq!(
            at_maturity.day_count(ymd(2025, Month::Feb, 28), ymd(2025, Month::Aug, 15)),
            165, // D1 = 28 → 30: 30·6 + (15 − 30)
        );
    }

    // ---- ActActICMA -----------------------------------------------------

    fn icma(s: Date, e: Date, rs: Date, re: Date, f: Frequency) -> Result<Fraction, TimeError> {
        ActActICMA.year_fraction_with_reference(s, e, rs, re, f)
    }

    fn frac(n: i64, d: u64) -> Fraction {
        Fraction::new(n, d).unwrap()
    }

    /// EMU paper / `QuantLib` anchors through the reference-period form:
    /// a regular Treasury period, short and long front stubs, a short
    /// back stub, and a forward notional walk past the reference.
    #[test]
    fn act_act_icma_with_reference_matches_published_examples() {
        use Frequency::{Annual, Semiannual};
        let cases = [
            (
                (2003, Month::Nov, 1),
                (2004, Month::May, 1),
                (2003, Month::Nov, 1),
                (2004, Month::May, 1),
                Semiannual,
                (1, 2),
            ),
            (
                (1999, Month::Feb, 1),
                (1999, Month::Jul, 1),
                (1998, Month::Jul, 1),
                (1999, Month::Jul, 1),
                Annual,
                (30, 73),
            ),
            (
                (2002, Month::Aug, 15),
                (2003, Month::Jul, 15),
                (2003, Month::Jan, 15),
                (2003, Month::Jul, 15),
                Semiannual,
                (337, 368),
            ),
            (
                (2000, Month::Jan, 30),
                (2000, Month::Jun, 30),
                (2000, Month::Jan, 30),
                (2000, Month::Jul, 30),
                Semiannual,
                (38, 91),
            ),
            (
                (2003, Month::Jan, 15),
                (2004, Month::Jul, 30),
                (2003, Month::Jan, 15),
                (2003, Month::Jul, 15),
                Semiannual,
                (567, 368),
            ),
        ];
        for (s, e, rs, re, f, (n, d)) in cases {
            let d8 = |(y, m, d)| ymd(y, m, d);
            assert_eq!(
                icma(d8(s), d8(e), d8(rs), d8(re), f).unwrap(),
                frac(n, d),
                "{s:?}"
            );
        }
        let (d, later) = (ymd(2025, Month::Jan, 15), ymd(2025, Month::Jul, 15));
        assert_eq!(
            icma(d, later, d, d, Semiannual),
            Err(TimeError::InvalidReferencePeriod)
        );
        assert_eq!(icma(later, d, d, later, Semiannual).unwrap(), frac(-1, 2));
    }

    fn semiannual(effective: Date, termination: Date) -> crate::ScheduleBuilder<'static> {
        crate::ScheduleBuilder::new(
            effective,
            termination,
            Period::Months(6),
            crate::calendars::NULL_CALENDAR,
        )
    }

    #[test]
    fn act_act_icma_accrues_schedule_periods_and_their_stubs() {
        // Long-ish front stub (EMU example), backward from maturity.
        let s = semiannual(ymd(2002, Month::Aug, 15), ymd(2004, Month::Jan, 15))
            .build()
            .unwrap();
        let fractions: alloc::vec::Vec<_> = s
            .periods()
            .map(|p| ActActICMA.accrual_fraction(&p).unwrap())
            .collect();
        assert_eq!(fractions, [frac(153, 368), frac(1, 2), frac(1, 2)]);
        // Part of a regular period is proportional.
        let partial = s
            .periods()
            .nth(1)
            .unwrap()
            .until(ymd(2003, Month::Apr, 15))
            .unwrap();
        assert_eq!(
            ActActICMA.accrual_fraction(&partial).unwrap(),
            frac(45, 181)
        );

        // Short back stub, forward: 167 days of a 182-day notional period.
        let s = semiannual(ymd(2003, Month::Jan, 15), ymd(2004, Month::Jun, 30))
            .forwards()
            .build()
            .unwrap();
        let stub = s.periods().last().unwrap();
        assert_eq!(ActActICMA.accrual_fraction(&stub).unwrap(), frac(167, 364));

        // A 12-month front stub reaches past its reference period into
        // two notional ones: 1/2 + 1/2 + 5/368.
        let s = semiannual(ymd(2002, Month::Jan, 10), ymd(2004, Month::Jan, 15))
            .with_first_date(ymd(2003, Month::Jan, 15))
            .build()
            .unwrap();
        let stub = s.periods().next().unwrap();
        assert_eq!(
            stub.reference(),
            ymd(2002, Month::Jul, 15)..ymd(2003, Month::Jan, 15)
        );
        assert_eq!(ActActICMA.accrual_fraction(&stub).unwrap(), frac(373, 368));
    }

    /// The review's bond. With the default bond conventions it accrues
    /// unadjusted and ICMA gives 5/368 at 2026-03-20 and 17/46 for the
    /// front stub. Accrued on adjusted dates (as some terms say), ICMA
    /// counts those dates, as every convention and `QuantLib` do:
    /// 2/183 and 137/370.
    #[test]
    fn act_act_icma_accrues_on_the_schedules_own_dates() {
        let build = |convention| {
            crate::ScheduleBuilder::new(
                ymd(2024, Month::May, 2),
                ymd(2029, Month::Sep, 15),
                Period::Months(6),
                crate::calendars::us::SETTLEMENT,
            )
            .with_convention(convention)
            .build()
            .unwrap()
        };
        let settle = ymd(2026, Month::Mar, 20);
        let check = |s: &Schedule, accrued: Fraction, stub: Fraction| {
            let to_date = s
                .accrual_period(settle)
                .and_then(|p| p.until(settle))
                .unwrap();
            assert_eq!(ActActICMA.accrual_fraction(&to_date).unwrap(), accrued);
            let front = s.periods().next().unwrap();
            assert_eq!(ActActICMA.accrual_fraction(&front).unwrap(), stub);
            for p in s.periods().skip(1).take(s.periods().len() - 2) {
                assert_eq!(ActActICMA.accrual_fraction(&p).unwrap(), frac(1, 2));
            }
        };
        check(
            &build(crate::BusinessDayConvention::Unadjusted),
            frac(5, 368),
            frac(17, 46),
        );
        check(
            &build(crate::BusinessDayConvention::ModifiedFollowing),
            frac(2, 183),
            frac(137, 370),
        );
    }

    #[test]
    fn act_act_icma_needs_a_coupon_frequency() {
        let dates = alloc::vec![ymd(2025, Month::Jan, 15), ymd(2025, Month::Jul, 15)];
        let bare = Schedule::try_from(dates.clone()).unwrap();
        let period = bare.periods().next().unwrap();
        assert_eq!(
            ActActICMA.accrual_fraction(&period),
            Err(TimeError::MissingFrequency)
        );
        let framed = Schedule::try_from((dates, Frequency::Semiannual)).unwrap();
        let period = framed.periods().next().unwrap();
        assert_eq!(ActActICMA.accrual_fraction(&period), Ok(frac(1, 2)));
        assert_eq!(ActActICMA.name(), "Actual/Actual (ICMA)");
    }

    /// The accrual math treats a `Range<Date>` as the half-open
    /// interval its syntax implies.
    #[test]
    fn date_ranges_are_half_open() {
        let range = ymd(2025, Month::Jan, 1)..ymd(2025, Month::Apr, 1);
        assert_eq!(range.days(), 90);
        assert!(range.contains(&ymd(2025, Month::Jan, 1))); // start included
        assert!(!range.contains(&ymd(2025, Month::Apr, 1))); // end excluded
        // Overlapping ranges intersect to their shared days.
        assert_eq!(
            range.intersect(&(ymd(2025, Month::Mar, 1)..ymd(2025, Month::Jun, 1))),
            Some(ymd(2025, Month::Mar, 1)..ymd(2025, Month::Apr, 1)),
        );
        // Disjoint ranges, and ranges that merely touch at a boundary,
        // share no days.
        assert_eq!(
            range.intersect(&(ymd(2025, Month::Jun, 1)..ymd(2025, Month::Jul, 1))),
            None,
        );
        assert_eq!(
            range.intersect(&(ymd(2025, Month::Apr, 1)..ymd(2025, Month::May, 1))),
            None,
        );
        // Reversed inputs give a negative day count.
        assert_eq!(
            (ymd(2025, Month::Apr, 1)..ymd(2025, Month::Jan, 1)).days(),
            -90
        );
    }

    // ---- ActActISDA examples ------------------------------------------

    #[test]
    fn act_act_isda_name() {
        assert_eq!(ActActISDA.name(), "Actual/Actual (ISDA)");
    }

    #[test]
    fn act_act_isda_zero_period() {
        let d = ymd(2025, Month::Jul, 4);
        assert!(ActActISDA.year_fraction(d, d).is_zero());
    }

    #[test]
    fn act_act_isda_same_non_leap_year() {
        // Jan 1 to Jul 1 in non-leap 2025: 181 days / 365.
        let start = ymd(2025, Month::Jan, 1);
        let end = ymd(2025, Month::Jul, 1);
        assert_eq!(ActActISDA.year_fraction(start, end).parts(), (181, 365));
    }

    #[test]
    fn act_act_isda_same_leap_year() {
        // Jan 1 to Jul 1 in leap 2024: 182 days / 366 = 91/183.
        let start = ymd(2024, Month::Jan, 1);
        let end = ymd(2024, Month::Jul, 1);
        assert_eq!(ActActISDA.year_fraction(start, end).parts(), (91, 183));
    }

    #[test]
    fn act_act_isda_full_non_leap_year_is_one() {
        let start = ymd(2025, Month::Jan, 1);
        let end = ymd(2026, Month::Jan, 1);
        assert_eq!(ActActISDA.year_fraction(start, end).parts(), (1, 1));
    }

    #[test]
    fn act_act_isda_full_leap_year_is_one() {
        let start = ymd(2024, Month::Jan, 1);
        let end = ymd(2025, Month::Jan, 1);
        // 366 days / 366 = 1.
        assert_eq!(ActActISDA.year_fraction(start, end).parts(), (1, 1));
    }

    /// The canonical ISDA 2006 paper example: a coupon period
    /// straddling a leap-year boundary should split into the two
    /// per-year buckets and combine via cross-multiplication.
    #[test]
    fn act_act_isda_isda_paper_nov_2003_to_may_2004() {
        // Nov 1 2003 (non-leap) → May 1 2004 (leap).
        // 61 days in 2003 / 365 + 121 days in 2004 / 366
        // = (61·366 + 121·365) / (365·366)
        // = (22326 + 44165) / 133590
        // = 66491 / 133590 (already reduced — gcd = 1).
        let start = ymd(2003, Month::Nov, 1);
        let end = ymd(2004, Month::May, 1);
        assert_eq!(
            ActActISDA.year_fraction(start, end).parts(),
            (66491, 133_590),
        );
    }

    #[test]
    fn act_act_isda_multi_year_with_full_middle_years() {
        // 2000-03-01 → 2005-03-01: 5 years; 2000 and 2004 are leap.
        // QL formula:
        //   N = y2 - y1 - 1 = 4
        //   a = days from 2000-03-01 to 2001-01-01 = 306
        //   b = days from 2005-01-01 to 2005-03-01 = 59
        //   dib1 = 366 (2000), dib2 = 365 (2005)
        //   num = 4·366·365 + 306·365 + 59·366
        //       = 534360 + 111690 + 21594 = 667644
        //   denom = 365·366 = 133590
        //   gcd(667644, 133590) = 6, so reduced = 111274/22265.
        let start = ymd(2000, Month::Mar, 1);
        let end = ymd(2005, Month::Mar, 1);
        assert_eq!(
            ActActISDA.year_fraction(start, end).parts(),
            (111_274, 22_265),
        );
    }

    #[test]
    fn act_act_isda_reversed_is_negation() {
        let a = ymd(2003, Month::Nov, 1);
        let b = ymd(2004, Month::May, 1);
        let forward = ActActISDA.year_fraction(a, b);
        let reverse = ActActISDA.year_fraction(b, a);
        assert_eq!(reverse, forward.checked_neg().unwrap());
    }

    #[test]
    fn act_act_isda_additive_across_known_split() {
        // Nov 1 2003 → Jul 15 2004 split at Jan 1 2004 must equal
        // the direct fraction.
        let a = ymd(2003, Month::Nov, 1);
        let b = ymd(2004, Month::Jan, 1);
        let c = ymd(2004, Month::Jul, 15);
        let split = ActActISDA
            .year_fraction(a, b)
            .checked_add(ActActISDA.year_fraction(b, c))
            .unwrap();
        let direct = ActActISDA.year_fraction(a, c);
        assert_eq!(split, direct);
    }

    #[test]
    fn thirty_360_bond_reversed_is_negation() {
        // Asymmetry in the formula is normalized by the trait
        // contract: dc(a, b) == -dc(b, a).
        let a = ymd(2025, Month::Jan, 31);
        let b = ymd(2025, Month::Feb, 15);
        let forward = Thirty360Bond.day_count(a, b);
        let reverse = Thirty360Bond.day_count(b, a);
        assert_eq!(forward, -reverse);
    }

    // ---- Negative direction examples ----------------------------------

    #[test]
    fn act360_reversed_period_is_negated_fraction() {
        // 30 days forward = 1/12; 30 days backward = -1/12.
        let a = ymd(2025, Month::Jan, 1);
        let b = ymd(2025, Month::Jan, 31);
        assert_eq!(Act360.year_fraction(a, b).parts(), (1, 12));
        assert_eq!(Act360.year_fraction(b, a).parts(), (-1, 12));
    }

    #[test]
    fn act365f_reversed_period_is_negated_fraction() {
        // 90 days forward = 18/73; 90 days backward = -18/73.
        let a = ymd(2025, Month::Jan, 1);
        let b = ymd(2025, Month::Apr, 1);
        assert_eq!(Act365Fixed.year_fraction(a, b).parts(), (18, 73));
        assert_eq!(Act365Fixed.year_fraction(b, a).parts(), (-18, 73));
    }

    #[test]
    fn forward_plus_reverse_cancels() {
        // yf(a, b) + yf(b, a) == 0 — useful sanity check on the sign
        // mirror.
        let a = ymd(2025, Month::Jan, 1);
        let b = ymd(2025, Month::Aug, 14);
        let sum = Act360
            .year_fraction(a, b)
            .checked_add(Act360.year_fraction(b, a))
            .unwrap();
        assert_eq!(sum, Fraction::ZERO);
    }

    // ---- ACT-family additivity (example) ------------------------------

    #[test]
    fn act360_additive_across_a_known_split() {
        let a = ymd(2025, Month::Jan, 1);
        let b = ymd(2025, Month::Apr, 1);
        let c = ymd(2025, Month::Oct, 1);
        let lhs = Act360
            .year_fraction(a, b)
            .checked_add(Act360.year_fraction(b, c))
            .unwrap();
        let rhs = Act360.year_fraction(a, c);
        assert_eq!(lhs, rhs);
    }

    #[test]
    fn act365f_additive_across_year_boundary() {
        // Span includes the leap-day region; ACT/365F is unaware of it.
        let a = ymd(2023, Month::Nov, 1);
        let b = ymd(2024, Month::Mar, 1);
        let c = ymd(2024, Month::Aug, 1);
        let lhs = Act365Fixed
            .year_fraction(a, b)
            .checked_add(Act365Fixed.year_fraction(b, c))
            .unwrap();
        let rhs = Act365Fixed.year_fraction(a, c);
        assert_eq!(lhs, rhs);
    }

    // ---- Property tests -----------------------------------------------

    fn three_ordered_dates() -> impl Strategy<Value = (Date, Date, Date)> {
        // Sample three serials in a comfortable interior range, then sort.
        (
            10u32..(Date::MAX.serial() - 10),
            10u32..(Date::MAX.serial() - 10),
            10u32..(Date::MAX.serial() - 10),
        )
            .prop_map(|(x, y, z)| {
                let mut s = [x, y, z];
                s.sort_unstable();
                (
                    Date::from_serial(s[0]).unwrap(),
                    Date::from_serial(s[1]).unwrap(),
                    Date::from_serial(s[2]).unwrap(),
                )
            })
    }

    proptest! {
        /// `yf(d, d) == 0` for every ACT convention.
        #[test]
        fn act_yf_zero_period(serial in 0u32..=Date::MAX.serial()) {
            let d = Date::from_serial(serial).unwrap();
            prop_assert!(Act360.year_fraction(d, d).is_zero());
            prop_assert!(Act365Fixed.year_fraction(d, d).is_zero());
            prop_assert!(ActActISDA.year_fraction(d, d).is_zero());
        }

        /// ACT/360 additivity: `yf(a, b) + yf(b, c) == yf(a, c)` for
        /// `a <= b <= c`.
        #[test]
        fn act360_additive((a, b, c) in three_ordered_dates()) {
            let lhs = Act360
                .year_fraction(a, b)
                .checked_add(Act360.year_fraction(b, c))
                .expect("ACT/360 numerators stay well within u64");
            let rhs = Act360.year_fraction(a, c);
            prop_assert_eq!(lhs, rhs);
        }

        /// ACT/365F additivity: same property.
        #[test]
        fn act365f_additive((a, b, c) in three_ordered_dates()) {
            let lhs = Act365Fixed
                .year_fraction(a, b)
                .checked_add(Act365Fixed.year_fraction(b, c))
                .expect("ACT/365F numerators stay well within u64");
            let rhs = Act365Fixed.year_fraction(a, c);
            prop_assert_eq!(lhs, rhs);
        }

        /// ACT/ACT ISDA additivity: same property. The leap/non-leap
        /// split allocates additively at any internal date because
        /// each calendar day belongs to exactly one of the two
        /// buckets regardless of where the split falls.
        #[test]
        fn act_act_isda_additive((a, b, c) in three_ordered_dates()) {
            let lhs = ActActISDA
                .year_fraction(a, b)
                .checked_add(ActActISDA.year_fraction(b, c))
                .expect("ACT/ACT ISDA numerators stay within i64");
            let rhs = ActActISDA.year_fraction(a, c);
            prop_assert_eq!(lhs, rhs);
        }

        /// `day_count` matches the underlying `days_since` regardless
        /// of order.
        #[test]
        fn day_count_matches_days_since(
            x in 0u32..=Date::MAX.serial(),
            y in 0u32..=Date::MAX.serial(),
        ) {
            let a = Date::from_serial(x).unwrap();
            let b = Date::from_serial(y).unwrap();
            prop_assert_eq!(Act360.day_count(a, b), i64::from(b.days_since(a)));
            prop_assert_eq!(Act365Fixed.day_count(a, b), i64::from(b.days_since(a)));
        }

        /// For ordered inputs the year fraction has the expected
        /// fixed denominator.
        #[test]
        fn act360_denominator_is_360_after_no_reduction(
            x in 0u32..(Date::MAX.serial()),
            offset in 1u32..=1_000,
        ) {
            let a = Date::from_serial(x).unwrap();
            let b_serial = x.saturating_add(offset).min(Date::MAX.serial());
            let b = Date::from_serial(b_serial).unwrap();
            let yf = Act360.year_fraction(a, b);
            // Reconstruct the un-reduced rational; `days_since` is
            // non-negative because b_serial >= x by construction.
            let days = i64::from(b.days_since(a));
            let raw = Fraction::new(days, 360).unwrap();
            prop_assert_eq!(yf, raw);
        }

        /// Reversing the inputs negates the year fraction:
        /// `yf(a, b) + yf(b, a) == 0`. Holds for every DayCount
        /// convention by trait contract — even non-additive ones
        /// like 30/360, where the trait normalizes asymmetric
        /// formulas via explicit negation.
        #[test]
        fn yf_reverses_to_negation(
            x in 0u32..=Date::MAX.serial(),
            y in 0u32..=Date::MAX.serial(),
        ) {
            let a = Date::from_serial(x).unwrap();
            let b = Date::from_serial(y).unwrap();
            let isda_30e = Thirty360ISDA::new(Date::MAX);
            for dc in [
                &Act360 as &dyn DayCount,
                &Act365Fixed,
                &Thirty360Bond,
                &Thirty360US,
                &Thirty360European,
                &isda_30e,
                &ActActISDA,
            ] {
                let sum = dc.year_fraction(a, b)
                    .checked_add(dc.year_fraction(b, a))
                    .expect("denominators are constants");
                prop_assert_eq!(sum, Fraction::ZERO);
            }
        }

        /// `day_count(a, b) == -day_count(b, a)` for every
        /// convention, including Thirty360Bond whose formula is
        /// asymmetric under reversal (the trait normalizes it).
        #[test]
        fn day_count_is_signed_by_direction(
            x in 0u32..=Date::MAX.serial(),
            y in 0u32..=Date::MAX.serial(),
        ) {
            let a = Date::from_serial(x).unwrap();
            let b = Date::from_serial(y).unwrap();
            let isda_30e = Thirty360ISDA::new(Date::MAX);
            for dc in [
                &Act360 as &dyn DayCount,
                &Act365Fixed,
                &Thirty360Bond,
                &Thirty360US,
                &Thirty360European,
                &isda_30e,
                &ActActISDA,
            ] {
                prop_assert_eq!(dc.day_count(a, b), -dc.day_count(b, a));
            }
        }

        /// A full reference period under ICMA accrues exactly
        /// `1 / frequency`, whatever its actual calendar length.
        #[test]
        fn act_act_icma_reference_period_is_one_over_frequency(
            serial in 400u32..(Date::MAX.serial() - 400),
            len in 1u32..=370,
        ) {
            let r1 = Date::from_serial(serial).unwrap();
            let r2 = Date::from_serial(serial + len).unwrap();
            for freq in [
                Frequency::Annual,
                Frequency::Semiannual,
                Frequency::Quarterly,
                Frequency::Monthly,
                Frequency::Weekly,
            ] {
                let yf = ActActICMA.year_fraction_with_reference(r1, r2, r1, r2, freq).unwrap();
                prop_assert_eq!(yf.parts(), (1, u64::from(freq.per_year())));
            }
        }

        /// ICMA is additive across any split inside one reference
        /// period — the chunks share a denominator.
        #[test]
        fn act_act_icma_additive_within_reference(
            serial in 400u32..(Date::MAX.serial() - 400),
            o1 in 0u32..=300,
            o2 in 0u32..=300,
            o3 in 0u32..=300,
        ) {
            let mut offsets = [o1, o2, o3];
            offsets.sort_unstable();
            let r1 = Date::from_serial(serial).unwrap();
            let r2 = Date::from_serial(serial + 301).unwrap();
            let a = Date::from_serial(serial + offsets[0]).unwrap();
            let b = Date::from_serial(serial + offsets[1]).unwrap();
            let c = Date::from_serial(serial + offsets[2]).unwrap();
            let split = icma(a, b, r1, r2, Frequency::Semiannual).unwrap()
                .checked_add(icma(b, c, r1, r2, Frequency::Semiannual).unwrap())
                .expect("shared denominator");
            let direct = icma(a, c, r1, r2, Frequency::Semiannual).unwrap();
            prop_assert_eq!(split, direct);
        }
    }
}
