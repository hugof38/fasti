//! [`DayCount`] — conventions measuring elapsed time between two
//! [`Date`]s as a [`Fraction`]. Modeled on `QuantLib`'s `ql/time`
//! day counters.
//!
//! All conventions are signed by direction (`yf(a, b) == -yf(b, a)`)
//! and return [`Fraction::ZERO`] for equal dates. ACT-family
//! conventions are additive across splits; the 30/360 family is
//! intentionally not — both facts are property-tested.

use crate::{Date, Fraction, Frequency, Month, Period, Schedule, TimeError, Year};

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
    /// to calendar days; 30/360-style conventions override.
    fn day_count(&self, start: Date, end: Date) -> i64 {
        i64::from(end.days_since(start))
    }

    /// The year fraction between `start` and `end`: signed by
    /// direction, [`Fraction::ZERO`] for equal dates.
    ///
    /// Conventions needing schedule context (ACT/ACT ICMA) carry it
    /// in the implementing value ([`ActActICMA::bind`]), keeping this
    /// signature uniform.
    fn year_fraction(&self, start: Date, end: Date) -> Fraction;
}

// ---- shared helpers -----------------------------------------------------

/// `count / den` reduced. Infallible for the constant non-zero
/// denominators used below; the `ZERO` fallback honours the no-panic
/// contract on the unreachable arm.
fn fraction_of(count: i64, den: u64) -> Fraction {
    Fraction::new(count, den).unwrap_or_default()
}

/// Evaluate `count` on the ordered pair, negating for reversed
/// inputs — makes an asymmetric formula satisfy `f(a, b) == -f(b, a)`.
fn signed_by_order(start: Date, end: Date, count: impl FnOnce(Date, Date) -> i64) -> i64 {
    if start <= end {
        count(start, end)
    } else {
        -count(end, start)
    }
}

/// The intersection of two half-open date intervals, if non-empty.
fn overlap(a: (Date, Date), b: (Date, Date)) -> Option<(Date, Date)> {
    let lo = a.0.max(b.0);
    let hi = a.1.min(b.1);
    (lo < hi).then_some((lo, hi))
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
        fraction_of(self.day_count(start, end), 360)
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
        fraction_of(self.day_count(start, end), 365)
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
    /// `(N·dib1·dib2 + a·dib2 + b·dib1) / (dib1·dib2)` for `d1 <= d2`:
    /// `N` full middle years, `a`/`b` partial days in the end years,
    /// `dib*` those years' lengths.
    fn ordered_year_fraction(d1: Date, d2: Date) -> Fraction {
        if d1 == d2 {
            return Fraction::ZERO;
        }
        let (y1, y2) = (d1.year(), d2.year());
        if y1 == y2 {
            return fraction_of(i64::from(d2.days_since(d1)), u64::from(y1.length()));
        }
        let n = i64::from(y2.get()) - i64::from(y1.get()) - 1;
        let dib1 = i64::from(y1.length());
        let dib2 = i64::from(y2.length());
        // y1 < y2 <= Year::MAX, so both constructions are in range.
        let Ok(next_year_start) = Date::from_ymd(y1.get() + 1, Month::Jan, 1) else {
            return Fraction::ZERO;
        };
        let Ok(this_year_start) = Date::from_ymd(y2.get(), Month::Jan, 1) else {
            return Fraction::ZERO;
        };
        let a = i64::from(next_year_start.days_since(d1));
        let b = i64::from(d2.days_since(this_year_start));
        let num = n * dib1 * dib2 + a * dib2 + b * dib1;
        // dib1·dib2 <= 366², positive.
        #[allow(clippy::cast_sign_loss)]
        fraction_of(num, (dib1 * dib2) as u64)
    }
}

impl DayCount for ActActISDA {
    fn name(&self) -> &'static str {
        "Actual/Actual (ISDA)"
    }

    fn year_fraction(&self, start: Date, end: Date) -> Fraction {
        if start <= end {
            Self::ordered_year_fraction(start, end)
        } else {
            // Numerators stay far below |i64::MIN|; negation cannot fail.
            Self::ordered_year_fraction(end, start)
                .checked_neg()
                .unwrap_or_default()
        }
    }
}

// ---- ACT/ACT (ICMA) -----------------------------------------------------

/// Actual/Actual (ICMA), ICMA Rule 251 — the standard for fixed-rate
/// bond accruals: a regular coupon period accrues exactly
/// `1 / frequency`, days accrue proportionally against their period's
/// actual length, and stubs decompose over a notional-period grid.
///
/// ICMA needs schedule context: [`bind`](Self::bind) a [`Schedule`]
/// (whose parallel reference dates carry the regular grid) to get a
/// [`BoundActActICMA`] answering plain two-date
/// [`year_fraction`](DayCount::year_fraction) calls;
/// [`year_fraction_with_reference`](Self::year_fraction_with_reference)
/// is the manual escape hatch. Unbound, `year_fraction` returns
/// `1 / frequency`, matching `QuantLib` without reference dates.
///
/// The coupon [`Frequency`] is explicit (where `QuantLib` float-rounds
/// a month count from the reference-period length); the algorithm
/// matches `QuantLib`'s `ActualActual::Old_ISMA_Impl`.
///
/// ```
/// use fasti::{ActActICMA, DayCount, Date, Frequency, Month};
/// let dc = ActActICMA::new(Frequency::Semiannual);
/// let start = Date::from_ymd(2003, Month::Nov, 1)?;
/// let end = Date::from_ymd(2004, Month::May, 1)?;
/// assert_eq!(
///     dc.year_fraction_with_reference(start, end, start, end)?.parts(),
///     (1, 2),
/// );
/// # Ok::<(), fasti::TimeError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ActActICMA {
    frequency: Frequency,
}

/// `anchor + i·period` as a fresh multiple (never chained), with the
/// schedule generator's end-of-month snap.
fn grid_step(anchor: Date, period: Period, i: i32) -> Result<Date, TimeError> {
    let stepped = (anchor + period.checked_mul(i).ok_or(TimeError::DateOutOfRange)?)?;
    Ok(
        if anchor.is_end_of_month() && matches!(period, Period::Months(_) | Period::Years(_)) {
            stepped.end_of_month()
        } else {
            stepped
        },
    )
}

/// The reference period extended into one bi-directional grid:
/// window 0 is the reference period itself; negative windows step
/// back from its start, positive windows forward from its end.
#[derive(Debug, Clone, Copy)]
struct ReferenceGrid {
    reference: (Date, Date),
    period: Period,
}

impl ReferenceGrid {
    fn window(self, i: i32) -> Result<(Date, Date), TimeError> {
        let (anchor, k) = match i {
            0 => return Ok(self.reference),
            i if i < 0 => (self.reference.0, i),
            i => (self.reference.1, i - 1),
        };
        Ok((
            grid_step(anchor, self.period, k)?,
            grid_step(anchor, self.period, k + 1)?,
        ))
    }
}

impl ActActICMA {
    /// Construct for the given coupon frequency.
    #[must_use]
    pub const fn new(frequency: Frequency) -> Self {
        Self { frequency }
    }

    /// The coupon frequency this convention was constructed with.
    #[must_use]
    pub const fn frequency(&self) -> Frequency {
        self.frequency
    }

    /// Bind to a coupon [`Schedule`], yielding a [`BoundActActICMA`]
    /// that reads the schedule's parallel reference dates — stub
    /// periods accrue against their notional grid automatically.
    ///
    /// Bind an **unadjusted** schedule (ICMA's grid is the natural
    /// coupon grid). Errors with
    /// [`TimeError::InvalidReferencePeriod`] if the schedule has
    /// fewer than two dates.
    ///
    /// ```
    /// use fasti::{
    ///     ActActICMA, BusinessDayConvention, Date, DayCount, Frequency,
    ///     Month, Period, ScheduleBuilder, calendars,
    /// };
    /// let schedule = ScheduleBuilder::new(
    ///     Date::from_ymd(2002, Month::Aug, 15)?,
    ///     Date::from_ymd(2004, Month::Jan, 15)?,
    ///     Period::Months(6),
    ///     calendars::NULL_CALENDAR,
    /// )
    /// .backwards()
    /// .with_convention(BusinessDayConvention::Unadjusted)
    /// .build()?;
    ///
    /// let dc = ActActICMA::new(Frequency::Semiannual).bind(&schedule)?;
    /// // The long front stub decomposes against the notional grid:
    /// let stub = dc.year_fraction(
    ///     Date::from_ymd(2002, Month::Aug, 15)?,
    ///     Date::from_ymd(2003, Month::Jan, 15)?,
    /// );
    /// assert_eq!(stub.parts(), (153, 368));
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    pub fn bind(self, schedule: &Schedule) -> Result<BoundActActICMA<'_>, TimeError> {
        if schedule.len() < 2 {
            return Err(TimeError::InvalidReferencePeriod);
        }
        Ok(BoundActActICMA {
            inner: self,
            schedule,
        })
    }

    /// The year fraction given the (unadjusted) regular reference
    /// period `ref_start..ref_end` the accrual belongs to. Prefer
    /// [`bind`](Self::bind) when the accruals come from a
    /// [`Schedule`].
    ///
    /// Errors: [`TimeError::InvalidReferencePeriod`] when
    /// `ref_start >= ref_end`; [`TimeError::DateOutOfRange`] when a
    /// notional walk escapes the supported range;
    /// [`TimeError::FractionOverflow`] on overflow.
    pub fn year_fraction_with_reference(
        &self,
        start: Date,
        end: Date,
        ref_start: Date,
        ref_end: Date,
    ) -> Result<Fraction, TimeError> {
        if ref_start >= ref_end {
            return Err(TimeError::InvalidReferencePeriod);
        }
        if start == end {
            return Ok(Fraction::ZERO);
        }
        if start < end {
            self.ordered_year_fraction(start, end, ref_start, ref_end)
        } else {
            self.ordered_year_fraction(end, start, ref_start, ref_end)?
                .checked_neg()
                .ok_or(TimeError::FractionOverflow)
        }
    }

    /// `days(lo, hi) / (freq × days(w))` for an accrual chunk inside
    /// one notional window.
    fn chunk_ratio(lo: Date, hi: Date, w: (Date, Date), freq: u64) -> Result<Fraction, TimeError> {
        let window_days =
            u64::try_from(w.1.days_since(w.0)).map_err(|_| TimeError::InvalidReferencePeriod)?;
        let denom = freq
            .checked_mul(window_days)
            .ok_or(TimeError::FractionOverflow)?;
        Fraction::new(i64::from(hi.days_since(lo)), denom)
    }

    /// The ICMA year fraction for `d1 < d2` against `r1 < r2`: one
    /// ascending walk over the extended reference grid, accruing each
    /// window's overlap with the accrual.
    fn ordered_year_fraction(
        self,
        d1: Date,
        d2: Date,
        r1: Date,
        r2: Date,
    ) -> Result<Fraction, TimeError> {
        let freq = u64::from(self.frequency.per_year());
        let grid = ReferenceGrid {
            reference: (r1, r2),
            period: Period::from(self.frequency),
        };
        // Descend to the lowest window overlapping the accrual, then
        // accrue upward until the accrual is covered.
        let mut i = 0;
        while grid.window(i)?.0 > d1 {
            i -= 1;
        }
        let mut total = Fraction::ZERO;
        loop {
            let w = grid.window(i)?;
            if let Some((lo, hi)) = overlap((d1, d2), w) {
                total = total
                    .checked_add(Self::chunk_ratio(lo, hi, w, freq)?)
                    .ok_or(TimeError::FractionOverflow)?;
            }
            if w.1 >= d2 {
                break;
            }
            i += 1;
        }
        Ok(total)
    }
}

impl DayCount for ActActICMA {
    fn name(&self) -> &'static str {
        "Actual/Actual (ICMA)"
    }

    /// Unbound: the accrual counts as one full regular coupon period,
    /// `±1 / frequency`. Use [`ActActICMA::bind`] for real schedules.
    fn year_fraction(&self, start: Date, end: Date) -> Fraction {
        if start == end {
            return Fraction::ZERO;
        }
        let num = if start < end { 1 } else { -1 };
        fraction_of(num, u64::from(self.frequency.per_year()))
    }
}

/// [`ActActICMA`] bound to a coupon [`Schedule`] by
/// [`ActActICMA::bind`]: the uniform two-date [`DayCount`] API with
/// stub handling driven by the schedule's parallel reference dates,
/// mirroring `QuantLib`'s schedule-carrying ISMA day counter.
///
/// Dates outside the schedule's span accrue nothing — the schedule
/// defines the instrument's life. Deliberate semantics, not a
/// fallback.
#[derive(Debug, Clone, Copy)]
pub struct BoundActActICMA<'s> {
    inner: ActActICMA,
    schedule: &'s Schedule,
}

impl BoundActActICMA<'_> {
    /// The coupon frequency of the underlying convention.
    #[must_use]
    pub const fn frequency(&self) -> Frequency {
        self.inner.frequency()
    }

    /// The schedule this counter is bound to.
    #[must_use]
    pub const fn schedule(&self) -> &Schedule {
        self.schedule
    }

    /// Sum of per-period chunks for `d1 < d2`, each accrued against
    /// its own reference period, clamped to the schedule's span.
    fn ordered_year_fraction(&self, d1: Date, d2: Date) -> Fraction {
        let (dates, refs) = (self.schedule.dates(), self.schedule.reference_dates());
        let (Some(&first), Some(&last)) = (dates.first(), dates.last()) else {
            // Unreachable: bind requires at least two dates.
            return Fraction::ZERO;
        };
        let Some(span) = overlap((d1, d2), (first, last)) else {
            return Fraction::ZERO;
        };
        let mut total = Fraction::ZERO;
        for (period, reference) in dates.windows(2).zip(refs.windows(2)) {
            if period[0] >= span.1 {
                break;
            }
            let Some((lo, hi)) = overlap(span, (period[0], period[1])) else {
                continue;
            };
            // A regular period is its own reference, so the walk is a
            // single window; a stub extends it. Errors need a window
            // outside the supported range, where ZERO is the
            // documented fallback, and the denominators of one
            // schedule keep the running sum in range.
            let chunk = self
                .inner
                .ordered_year_fraction(lo, hi, reference[0], reference[1])
                .unwrap_or_default();
            total = total.checked_add(chunk).unwrap_or_default();
        }
        total
    }
}

impl DayCount for BoundActActICMA<'_> {
    fn name(&self) -> &'static str {
        "Actual/Actual (ICMA)"
    }

    fn year_fraction(&self, start: Date, end: Date) -> Fraction {
        if start == end {
            return Fraction::ZERO;
        }
        if start < end {
            self.ordered_year_fraction(start, end)
        } else {
            self.ordered_year_fraction(end, start)
                .checked_neg()
                .unwrap_or_default()
        }
    }
}

// ---- 30/360 family ------------------------------------------------------

/// One date split into the fields the 30/360 adjustments read.
#[derive(Debug, Clone, Copy)]
struct Split {
    year: Year,
    month: Month,
    day: u8,
}

impl Split {
    fn of(d: Date) -> Self {
        let (year, month, day) = d.to_ymd();
        Self { year, month, day }
    }

    fn is_last_of_february(self) -> bool {
        matches!(self.month, Month::Feb) && self.day == Month::Feb.length(self.year)
    }
}

/// The shared 30/360 formula, `360·Δy + 30·Δm + Δd`, over the
/// variant's adjusted day-of-month pair.
fn thirty360(s: Split, e: Split, dd1: u8, dd2: u8) -> i64 {
    360 * (i64::from(e.year.get()) - i64::from(s.year.get()))
        + 30 * (i64::from(e.month.get()) - i64::from(s.month.get()))
        + (i64::from(dd2) - i64::from(dd1))
}

/// Cap a day-of-month at 30.
const fn cap30(day: u8) -> u8 {
    if day == 31 { 30 } else { day }
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
        signed_by_order(start, end, |a, b| {
            let (s, e) = (Split::of(a), Split::of(b));
            let dd1 = cap30(s.day);
            let dd2 = if e.day == 31 && dd1 == 30 { 30 } else { e.day };
            thirty360(s, e, dd1, dd2)
        })
    }

    fn year_fraction(&self, start: Date, end: Date) -> Fraction {
        fraction_of(self.day_count(start, end), 360)
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
        signed_by_order(start, end, |a, b| {
            let (s, e) = (Split::of(a), Split::of(b));
            let (mut dd1, mut dd2) = (s.day, e.day);
            if s.is_last_of_february() {
                if e.is_last_of_february() {
                    dd2 = 30;
                }
                dd1 = 30;
            }
            if dd2 == 31 && dd1 >= 30 {
                dd2 = 30;
            }
            dd1 = cap30(dd1);
            thirty360(s, e, dd1, dd2)
        })
    }

    fn year_fraction(&self, start: Date, end: Date) -> Fraction {
        fraction_of(self.day_count(start, end), 360)
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
        signed_by_order(start, end, |a, b| {
            let (s, e) = (Split::of(a), Split::of(b));
            thirty360(s, e, cap30(s.day), cap30(e.day))
        })
    }

    fn year_fraction(&self, start: Date, end: Date) -> Fraction {
        fraction_of(self.day_count(start, end), 360)
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
        signed_by_order(start, end, |a, b| {
            let (s, e) = (Split::of(a), Split::of(b));
            let mut dd1 = cap30(s.day);
            let mut dd2 = cap30(e.day);
            if s.is_last_of_february() {
                dd1 = 30;
            }
            if b != self.termination && e.is_last_of_february() {
                dd2 = 30;
            }
            thirty360(s, e, dd1, dd2)
        })
    }

    fn year_fraction(&self, start: Date, end: Date) -> Fraction {
        fraction_of(self.day_count(start, end), 360)
    }
}

// ---- Tests --------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    extern crate alloc;

    use super::*;
    use crate::Month;
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

    // ---- ActActICMA examples ------------------------------------------

    #[test]
    fn act_act_icma_name_and_frequency() {
        let dc = ActActICMA::new(Frequency::Semiannual);
        assert_eq!(dc.name(), "Actual/Actual (ICMA)");
        assert_eq!(dc.frequency(), Frequency::Semiannual);
    }

    /// Without reference dates a period counts as one full coupon:
    /// exactly 1/frequency, signed by direction.
    #[test]
    fn act_act_icma_without_reference_is_one_over_frequency() {
        let dc = ActActICMA::new(Frequency::Quarterly);
        let a = ymd(2025, Month::Jan, 15);
        let b = ymd(2025, Month::Apr, 15);
        assert_eq!(dc.year_fraction(a, b).parts(), (1, 4));
        assert_eq!(dc.year_fraction(b, a).parts(), (-1, 4));
        assert!(dc.year_fraction(a, a).is_zero());
        // Length doesn't matter without schedule context — QL parity.
        assert_eq!(
            dc.year_fraction(a, ymd(2025, Month::Jan, 16)).parts(),
            (1, 4),
        );
    }

    /// ISDA "EMU and Market Conventions" / `QuantLib` test-suite anchor:
    /// a regular semiannual US Treasury period accrues exactly 1/2.
    #[test]
    fn act_act_icma_regular_period() {
        let dc = ActActICMA::new(Frequency::Semiannual);
        let start = ymd(2003, Month::Nov, 1);
        let end = ymd(2004, Month::May, 1);
        assert_eq!(
            dc.year_fraction_with_reference(start, end, start, end)
                .unwrap()
                .parts(),
            (1, 2),
        );
    }

    /// EMU paper: short first calculation period. Accrual
    /// 1999-02-01 → 1999-07-01 against annual reference
    /// 1998-07-01 → 1999-07-01: 150 / (1 × 365) = 30/73.
    #[test]
    fn act_act_icma_short_front_stub() {
        let dc = ActActICMA::new(Frequency::Annual);
        let yf = dc
            .year_fraction_with_reference(
                ymd(1999, Month::Feb, 1),
                ymd(1999, Month::Jul, 1),
                ymd(1998, Month::Jul, 1),
                ymd(1999, Month::Jul, 1),
            )
            .unwrap();
        assert_eq!(yf.parts(), (30, 73)); // 150/365 reduced
    }

    /// EMU paper / `QuantLib` test-suite: long first calculation
    /// period. Accrual 2002-08-15 → 2003-07-15, reference period
    /// 2003-01-15 → 2003-07-15, semiannual. Decomposes as the full
    /// reference period (181/362 = 1/2) plus the chunk
    /// 2002-08-15 → 2003-01-15 against the notional period
    /// 2002-07-15 → 2003-01-15 (153 / (2 × 184)):
    /// 1/2 + 153/368 = 337/368.
    #[test]
    fn act_act_icma_long_front_stub() {
        let dc = ActActICMA::new(Frequency::Semiannual);
        let yf = dc
            .year_fraction_with_reference(
                ymd(2002, Month::Aug, 15),
                ymd(2003, Month::Jul, 15),
                ymd(2003, Month::Jan, 15),
                ymd(2003, Month::Jul, 15),
            )
            .unwrap();
        assert_eq!(yf.parts(), (337, 368));
    }

    /// EMU paper: short final calculation period. The regular period
    /// 1999-07-30 → 2000-01-30 is exactly 1/2; the short final
    /// accrual 2000-01-30 → 2000-06-30 against its reference period
    /// 2000-01-30 → 2000-07-30 is 152 / (2 × 182) = 38/91.
    #[test]
    fn act_act_icma_short_back_stub() {
        let dc = ActActICMA::new(Frequency::Semiannual);
        let regular = dc
            .year_fraction_with_reference(
                ymd(1999, Month::Jul, 30),
                ymd(2000, Month::Jan, 30),
                ymd(1999, Month::Jul, 30),
                ymd(2000, Month::Jan, 30),
            )
            .unwrap();
        assert_eq!(regular.parts(), (1, 2));
        let stub = dc
            .year_fraction_with_reference(
                ymd(2000, Month::Jan, 30),
                ymd(2000, Month::Jun, 30),
                ymd(2000, Month::Jan, 30),
                ymd(2000, Month::Jul, 30),
            )
            .unwrap();
        assert_eq!(stub.parts(), (38, 91)); // 152/364 reduced
    }

    /// Accrual extending several periods past the reference period
    /// exercises the forward notional walk: mid 1/2, two full
    /// notional periods (1/2 each), and a 15-day partial against a
    /// 184-day window → 3/2 + 15/368 = 567/368.
    #[test]
    fn act_act_icma_forward_notional_walk() {
        let dc = ActActICMA::new(Frequency::Semiannual);
        let yf = dc
            .year_fraction_with_reference(
                ymd(2003, Month::Jan, 15),
                ymd(2004, Month::Jul, 30),
                ymd(2003, Month::Jan, 15),
                ymd(2003, Month::Jul, 15),
            )
            .unwrap();
        assert_eq!(yf.parts(), (567, 368));
    }

    #[test]
    fn act_act_icma_rejects_degenerate_reference_period() {
        let dc = ActActICMA::new(Frequency::Semiannual);
        let d = ymd(2025, Month::Jan, 15);
        let later = ymd(2025, Month::Jul, 15);
        assert_eq!(
            dc.year_fraction_with_reference(d, later, d, d),
            Err(TimeError::InvalidReferencePeriod),
        );
        assert_eq!(
            dc.year_fraction_with_reference(d, later, later, d),
            Err(TimeError::InvalidReferencePeriod),
        );
    }

    // ---- BoundActActICMA (schedule-bound) -----------------------------

    /// Build an unadjusted schedule for the bound-counter tests.
    fn unadjusted_schedule(
        effective: Date,
        termination: Date,
        rule: crate::DateGenerationRule,
    ) -> Schedule {
        crate::ScheduleBuilder::new(
            effective,
            termination,
            Period::Months(6),
            crate::calendars::NULL_CALENDAR,
        )
        .with_rule(rule)
        .with_convention(crate::BusinessDayConvention::Unadjusted)
        .with_termination_convention(crate::BusinessDayConvention::Unadjusted)
        .build()
        .unwrap()
    }

    /// Long-front-stub schedule (the EMU example, via bind): the stub
    /// decomposes automatically through the plain two-date API.
    #[test]
    fn bound_icma_front_stub_schedule() {
        let schedule = unadjusted_schedule(
            ymd(2002, Month::Aug, 15),
            ymd(2004, Month::Jan, 15),
            crate::DateGenerationRule::Backward,
        );
        assert_eq!(
            schedule.dates(),
            &[
                ymd(2002, Month::Aug, 15),
                ymd(2003, Month::Jan, 15),
                ymd(2003, Month::Jul, 15),
                ymd(2004, Month::Jan, 15),
            ],
        );
        let dc = ActActICMA::new(Frequency::Semiannual)
            .bind(&schedule)
            .unwrap();
        assert_eq!(dc.name(), "Actual/Actual (ICMA)");
        // Stub period: 153 days against the notional 184-day period.
        let stub = dc.year_fraction(ymd(2002, Month::Aug, 15), ymd(2003, Month::Jan, 15));
        assert_eq!(stub.parts(), (153, 368));
        // Regular periods: exactly 1/2 each.
        let regular = dc.year_fraction(ymd(2003, Month::Jan, 15), ymd(2003, Month::Jul, 15));
        assert_eq!(regular.parts(), (1, 2));
        // Whole instrument life: stub + two regular periods.
        let whole = dc.year_fraction(ymd(2002, Month::Aug, 15), ymd(2004, Month::Jan, 15));
        assert_eq!(whole.parts(), (521, 368)); // 153/368 + 1
        // Mid-period accrual inside a regular period is proportional.
        let partial = dc.year_fraction(ymd(2003, Month::Jan, 15), ymd(2003, Month::Apr, 15));
        assert_eq!(partial.parts(), (45, 181)); // 90/(2×181)
    }

    /// Short-back-stub schedule via forward generation.
    #[test]
    fn bound_icma_back_stub_schedule() {
        let schedule = unadjusted_schedule(
            ymd(2003, Month::Jan, 15),
            ymd(2004, Month::Jun, 30),
            crate::DateGenerationRule::Forward,
        );
        assert_eq!(
            schedule.dates(),
            &[
                ymd(2003, Month::Jan, 15),
                ymd(2003, Month::Jul, 15),
                ymd(2004, Month::Jan, 15),
                ymd(2004, Month::Jun, 30),
            ],
        );
        let dc = ActActICMA::new(Frequency::Semiannual)
            .bind(&schedule)
            .unwrap();
        // The back stub accrues against its notional period
        // Jan 15 2004 → Jul 15 2004 (182 days): 167/(2×182).
        let stub = dc.year_fraction(ymd(2004, Month::Jan, 15), ymd(2004, Month::Jun, 30));
        assert_eq!(stub.parts(), (167, 364));
        // Regular front periods stay exact halves.
        let regular = dc.year_fraction(ymd(2003, Month::Jan, 15), ymd(2003, Month::Jul, 15));
        assert_eq!(regular.parts(), (1, 2));
    }

    /// Dates outside the schedule's span accrue nothing — deliberate
    /// clamping semantics.
    #[test]
    fn bound_icma_clamps_outside_schedule_span() {
        let schedule = unadjusted_schedule(
            ymd(2003, Month::Jan, 15),
            ymd(2004, Month::Jan, 15),
            crate::DateGenerationRule::Backward,
        );
        let dc = ActActICMA::new(Frequency::Semiannual)
            .bind(&schedule)
            .unwrap();
        // Entirely before / after the schedule: zero.
        assert!(
            dc.year_fraction(ymd(2002, Month::Jan, 1), ymd(2003, Month::Jan, 14))
                .is_zero()
        );
        assert!(
            dc.year_fraction(ymd(2004, Month::Feb, 1), ymd(2005, Month::Jan, 1))
                .is_zero()
        );
        // Straddling the start: only the in-schedule part counts.
        assert_eq!(
            dc.year_fraction(ymd(2002, Month::Nov, 1), ymd(2003, Month::Jul, 15))
                .parts(),
            (1, 2),
        );
    }

    #[test]
    fn bound_icma_reversal_negates() {
        let schedule = unadjusted_schedule(
            ymd(2002, Month::Aug, 15),
            ymd(2004, Month::Jan, 15),
            crate::DateGenerationRule::Backward,
        );
        let dc = ActActICMA::new(Frequency::Semiannual)
            .bind(&schedule)
            .unwrap();
        let a = ymd(2002, Month::Sep, 1);
        let b = ymd(2003, Month::Oct, 1);
        let sum = dc
            .year_fraction(a, b)
            .checked_add(dc.year_fraction(b, a))
            .unwrap();
        assert_eq!(sum, Fraction::ZERO);
        assert!(dc.year_fraction(a, a).is_zero());
    }

    /// Bound accruals are additive across any split of the schedule
    /// span, including splits at and across stub boundaries.
    #[test]
    fn bound_icma_additive_across_periods() {
        let schedule = unadjusted_schedule(
            ymd(2002, Month::Aug, 15),
            ymd(2004, Month::Jan, 15),
            crate::DateGenerationRule::Backward,
        );
        let dc = ActActICMA::new(Frequency::Semiannual)
            .bind(&schedule)
            .unwrap();
        let a = ymd(2002, Month::Sep, 1);
        let b = ymd(2003, Month::Mar, 1);
        let c = ymd(2003, Month::Dec, 1);
        let split = dc
            .year_fraction(a, b)
            .checked_add(dc.year_fraction(b, c))
            .unwrap();
        assert_eq!(split, dc.year_fraction(a, c));
    }

    #[test]
    fn bound_icma_rejects_too_short_schedules() {
        let single = Schedule::try_from(alloc::vec![ymd(2003, Month::Jan, 15)]).unwrap();
        assert!(matches!(
            ActActICMA::new(Frequency::Semiannual).bind(&single),
            Err(TimeError::InvalidReferencePeriod),
        ));
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
            let icma = ActActICMA::new(Frequency::Semiannual);
            let isda_30e = Thirty360ISDA::new(Date::MAX);
            for dc in [
                &Act360 as &dyn DayCount,
                &Act365Fixed,
                &Thirty360Bond,
                &Thirty360US,
                &Thirty360European,
                &isda_30e,
                &ActActISDA,
                &icma,
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
            let icma = ActActICMA::new(Frequency::Quarterly);
            let isda_30e = Thirty360ISDA::new(Date::MAX);
            for dc in [
                &Act360 as &dyn DayCount,
                &Act365Fixed,
                &Thirty360Bond,
                &Thirty360US,
                &Thirty360European,
                &isda_30e,
                &ActActISDA,
                &icma,
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
                let dc = ActActICMA::new(freq);
                let yf = dc.year_fraction_with_reference(r1, r2, r1, r2).unwrap();
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
            let dc = ActActICMA::new(Frequency::Semiannual);
            let split = dc
                .year_fraction_with_reference(a, b, r1, r2).unwrap()
                .checked_add(dc.year_fraction_with_reference(b, c, r1, r2).unwrap())
                .expect("shared denominator");
            let direct = dc.year_fraction_with_reference(a, c, r1, r2).unwrap();
            prop_assert_eq!(split, direct);
        }
    }
}
