//! [`DayCount`] — convention for measuring elapsed time between two
//! [`Date`]s as a [`Fraction`].
//!
//! Modeled on `QuantLib`'s
//! [`ql/time/daycounter.hpp`](https://github.com/lballabio/QuantLib/blob/master/ql/time/daycounter.hpp).
//! Concrete impls are zero-sized unit structs (`Act360`,
//! `Act365Fixed`, …) — this is the one place in the crate where
//! traits + generics carry their weight, driven by the goal of
//! covering `QuantLib`'s full day-count surface over time.
//!
//! # The trait contract
//!
//! Both `day_count` and `year_fraction` are signed by direction:
//! reversed inputs (`end < start`) produce negative results that
//! mirror the ordered ones. `year_fraction(d, d)` returns
//! [`Fraction::ZERO`].
//!
//! Implementations are pure functions of `(start, end)`. They hold
//! no state, so the trait is implemented on unit structs and trait
//! objects (`&dyn DayCount`) are cheap.
//!
//! # Additivity
//!
//! ACT-family conventions ([`Act360`], [`Act365Fixed`],
//! [`ActActISDA`]) are additive across splits:
//! `yf(a, b) + yf(b, c) == yf(a, c)` for any `a <= b <= c`. This is
//! tested as a proptest invariant on every ACT impl. 30/360 family
//! conventions are intentionally *not* additive — the day-count
//! adjustments mean the per-segment counts can diverge from the
//! whole-period count. That non-additivity is documented and tested
//! at the impl site.

use crate::{Date, Fraction, Frequency, Month, Period, Schedule, TimeError, Year};

/// A day-count convention.
///
/// ```
/// use fasti::{Act360, DayCount, Date, Month};
/// let dc = Act360;
/// let start = Date::from_ymd(2025, Month::Jan, 1)?;
/// let end = Date::from_ymd(2025, Month::Apr, 1)?;
/// // 90 days at 360-day basis = 90/360 = 1/4.
/// assert_eq!(dc.year_fraction(start, end).parts(), (1, 4));
/// # Ok::<(), fasti::TimeError>(())
/// ```
pub trait DayCount {
    /// A short human-readable name like `"Actual/360"`.
    fn name(&self) -> &'static str;

    /// Days between `start` and `end`, signed by direction.
    ///
    /// `day_count(d, d) == 0`. For ordered inputs (`start <= end`)
    /// the result is non-negative; for reversed inputs the result is
    /// the negation of the ordered count.
    fn day_count(&self, start: Date, end: Date) -> i64;

    /// The year fraction between `start` and `end` under this
    /// convention.
    ///
    /// Signed by direction: reversed inputs produce a negative
    /// fraction, mirroring [`day_count`](Self::day_count). Equal
    /// inputs return [`Fraction::ZERO`].
    ///
    /// Every implementation is a pure function of the two dates —
    /// conventions that additionally depend on schedule context
    /// (e.g. ACT/ACT ICMA) carry that context in the implementing
    /// value itself (see [`ActActICMA::bind`]), keeping this
    /// signature uniform across the whole crate.
    fn year_fraction(&self, start: Date, end: Date) -> Fraction;
}

// ---- Actual/360 --------------------------------------------------------

/// Actual/360 day-count convention.
///
/// Year fraction is the calendar day count divided by 360, regardless
/// of leap years. Standard for USD money-market and most floating-rate
/// structured-credit accruals.
///
/// ```
/// use fasti::{Act360, DayCount, Date, Month};
/// let dc = Act360;
/// let start = Date::from_ymd(2024, Month::Jan, 1)?;
/// let end = Date::from_ymd(2025, Month::Jan, 1)?;
/// // 2024 is a leap year — 366 days, year fraction 366/360 > 1.
/// assert_eq!(dc.year_fraction(start, end).parts(), (61, 60));
/// # Ok::<(), fasti::TimeError>(())
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Act360;

impl DayCount for Act360 {
    fn name(&self) -> &'static str {
        "Actual/360"
    }

    fn day_count(&self, start: Date, end: Date) -> i64 {
        i64::from(end.days_since(start))
    }

    fn year_fraction(&self, start: Date, end: Date) -> Fraction {
        // Denominator 360 is non-zero, so `new` cannot fail in
        // practice; `unwrap_or_default` returns `Fraction::ZERO`
        // on the unreachable `Err` arm and honours the no-panic
        // library contract.
        Fraction::new(self.day_count(start, end), 360).unwrap_or_default()
    }
}

// ---- Actual/365 (Fixed) ------------------------------------------------

/// Actual/365 (Fixed) day-count convention.
///
/// Year fraction is the calendar day count divided by 365, regardless
/// of leap years. Standard for GBP money market and many fixed-income
/// pricing libraries that want a constant-denominator ACT
/// convention.
///
/// ```
/// use fasti::{Act365Fixed, DayCount, Date, Month};
/// let dc = Act365Fixed;
/// let start = Date::from_ymd(2025, Month::Jan, 1)?;
/// let end = Date::from_ymd(2026, Month::Jan, 1)?;
/// // 2025 is non-leap — 365 days, year fraction 365/365 = 1.
/// assert_eq!(dc.year_fraction(start, end).parts(), (1, 1));
/// # Ok::<(), fasti::TimeError>(())
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Act365Fixed;

impl DayCount for Act365Fixed {
    fn name(&self) -> &'static str {
        "Actual/365 (Fixed)"
    }

    fn day_count(&self, start: Date, end: Date) -> i64 {
        i64::from(end.days_since(start))
    }

    fn year_fraction(&self, start: Date, end: Date) -> Fraction {
        // Denominator 365 is non-zero; see `Act360::year_fraction`
        // for the `unwrap_or_default` rationale.
        Fraction::new(self.day_count(start, end), 365).unwrap_or_default()
    }
}

// ---- 30/360 (Bond Basis) -----------------------------------------------

/// 30/360 Bond Basis day-count convention — the ISDA 2006
/// "30/360 or Bond Basis" definition.
///
/// Each calendar month is counted as 30 days and each year as 360
/// days, with two day adjustments before computing
/// `360·ΔY + 30·ΔM + ΔD`:
///
/// 1. `D1 := 30` if `D1 = 31`.
/// 2. `D2 := 30` if `D2 = 31` and `D1 = 30` (after the first
///    adjustment).
///
/// Used by many bond indentures and standard Eurobond swaps. The day
/// count is computed for `start <= end` and extended by negation
/// for reversed inputs.
///
/// # `QuantLib` parity
///
/// Bit-for-bit equivalent to `QuantLib`'s
/// [`Thirty360::ISMA_Impl`](https://github.com/lballabio/QuantLib/blob/master/ql/time/daycounters/thirty360.cpp),
/// reachable via the `Thirty360::BondBasis` enum value.
///
/// **Not** the same as `QuantLib`'s `Thirty360::USA` (NASD / SIA /
/// "U.S. corporate" convention), which adds a last-of-February
/// rule: when both `D1` and `D2` are the last calendar day of
/// February, both are treated as 30, and the `D2 = 31` adjustment
/// uses `D1 ≥ 30` against the *unadjusted* `D1`. Deal documents that
/// say "30/360 (Bond Basis)" want this type; documents that say
/// "NASD" or "SIA 30/360" want a separate impl that has not yet
/// been added.
///
/// # Non-additivity
///
/// 30/360 is *not* additive across splits in general:
/// `yf(a, b) + yf(b, c) ≠ yf(a, c)` for some triples. The
/// asymmetric day adjustments mean per-segment counts can diverge
/// from the whole-period count. The conventional approach in bond
/// modelling is to compute period-by-period and accept the small
/// per-period drift. The crate has a regression test
/// (`thirty_360_bond_is_not_additive_jan31_feb28_mar31`) demonstrating
/// the drift on the canonical example.
///
/// ```
/// use fasti::{DayCount, Date, Month, Thirty360Bond};
/// let dc = Thirty360Bond;
/// let start = Date::from_ymd(2025, Month::Jan, 1)?;
/// let end = Date::from_ymd(2025, Month::Jul, 1)?;
/// // 6 months at 30 days each = 180/360 = 1/2.
/// assert_eq!(dc.year_fraction(start, end).parts(), (1, 2));
/// # Ok::<(), fasti::TimeError>(())
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Thirty360Bond;

impl Thirty360Bond {
    /// 30/360 (Bond Basis) day count assuming `d1 <= d2`.
    ///
    /// Implementation of the ISDA 2006 "30/360 or Bond Basis"
    /// formula. Conversions widen `u16`/`u8` calendar fields to
    /// `i64` so the signed differences `Δyear`, `Δmonth`, `Δday`
    /// cannot overflow. Private helper called from
    /// [`day_count`](DayCount::day_count); the convention's
    /// asymmetric formula means callers must order the inputs
    /// before invoking this.
    fn ordered_count(d1: Date, d2: Date) -> i64 {
        let (y1, m1, dd1) = d1.to_ymd();
        let (y2, m2, dd2) = d2.to_ymd();
        // D1: 31 → 30.
        let dd1 = if dd1 == 31 { 30 } else { dd1 };
        // D2: 31 → 30 only when D1 (post-adjustment) is 30. This is
        // the ISDA condition "D1 ∈ {30, 31}" — equivalent because
        // D1 = 31 already became 30 on the previous line.
        let dd2 = if dd2 == 31 && dd1 == 30 { 30 } else { dd2 };
        let year_part = 360 * (i64::from(y2.get()) - i64::from(y1.get()));
        let month_part = 30 * (i64::from(m2.get()) - i64::from(m1.get()));
        let day_part = i64::from(dd2) - i64::from(dd1);
        year_part + month_part + day_part
    }
}

impl DayCount for Thirty360Bond {
    fn name(&self) -> &'static str {
        "30/360 (Bond Basis)"
    }

    fn day_count(&self, start: Date, end: Date) -> i64 {
        // The bond-basis formula is asymmetric — D2's adjustment
        // depends on D1. To keep the trait contract
        // `dc(a, b) == -dc(b, a)` total, compute the count for the
        // ordered pair and negate when the inputs are reversed.
        if start <= end {
            Self::ordered_count(start, end)
        } else {
            -Self::ordered_count(end, start)
        }
    }

    fn year_fraction(&self, start: Date, end: Date) -> Fraction {
        // Denominator 360 is non-zero; see `Act360::year_fraction`
        // for the `unwrap_or_default` rationale.
        Fraction::new(self.day_count(start, end), 360).unwrap_or_default()
    }
}

// ---- ACT/ACT (ISDA) ----------------------------------------------------

/// Actual/Actual (ISDA) day-count convention — the ISDA 2006
/// "Actual/Actual (ISDA)" definition (a.k.a. "Actual/Actual
/// (Historical)" in some literature).
///
/// Splits the period at calendar-year boundaries and weights leap
/// vs. non-leap days separately:
///
/// ```text
/// yf = days_in_leap_years / 366 + days_in_non_leap_years / 365
/// ```
///
/// Equivalently — and as `QuantLib`'s `ISDA_Impl` codes it:
///
/// ```text
/// yf = (y2 - y1 - 1) + first_partial / dib1 + last_partial / dib2
/// ```
///
/// where `dib1 = 366` if `y1` is leap else `365`, and `dib2`
/// similarly for `y2`. The two formulations are algebraically equal:
/// a full leap year contributes `366 / 366 = 1` and a full non-leap
/// year contributes `365 / 365 = 1`, so the middle full years add up
/// to `y2 - y1 - 1` regardless of how many of them are leap.
///
/// Standard for fixed-income accruals worldwide and the most common
/// "actual/actual" choice in ISDA documentation.
///
/// # `QuantLib` parity
///
/// Equivalent (modulo float vs. integer-rational representation) to
/// `QuantLib`'s
/// [`ActualActual::ISDA_Impl`](https://github.com/lballabio/QuantLib/blob/master/ql/time/daycounters/actualactual.cpp)
/// — same year-boundary split, same per-year denominators, same
/// signed-by-reversal contract.
///
/// # Additivity
///
/// ACT/ACT (ISDA) is additive across splits: `yf(a, b) + yf(b, c) =
/// yf(a, c)` for any `a ≤ b ≤ c`. The proptest exercises this.
///
/// ```
/// use fasti::{ActActISDA, DayCount, Date, Month};
/// let dc = ActActISDA;
/// // Period crossing a leap-year boundary: Nov 1 2003 → May 1 2004.
/// // 61 non-leap days in 2003 + 121 leap days in 2004
/// // = 61/365 + 121/366
/// // = (61·366 + 121·365) / (365·366)
/// // = (22326 + 44165) / 133590
/// // = 66491 / 133590  (already reduced — gcd = 1)
/// let start = Date::from_ymd(2003, Month::Nov, 1)?;
/// let end = Date::from_ymd(2004, Month::May, 1)?;
/// assert_eq!(dc.year_fraction(start, end).parts(), (66491, 133590));
/// # Ok::<(), fasti::TimeError>(())
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ActActISDA;

impl ActActISDA {
    /// ACT/ACT (ISDA) year fraction assuming `d1 <= d2`.
    fn ordered_year_fraction(d1: Date, d2: Date) -> Fraction {
        if d1 == d2 {
            return Fraction::ZERO;
        }
        let y1 = d1.year();
        let y2 = d2.year();
        if y1 == y2 {
            // Same year: ACT / year_length.
            let days = i64::from(d2.days_since(d1));
            let denom = u64::from(y1.length());
            return Fraction::new(days, denom).unwrap_or_default();
        }
        // Cross-year: yf = N + a/dib1 + b/dib2 where
        //   N    = y2 - y1 - 1     (full middle years)
        //   a    = days from d1 to Jan 1 of (y1+1)
        //   b    = days from Jan 1 of y2 to d2
        //   dib1 = 365 or 366 (length of y1)
        //   dib2 = 365 or 366 (length of y2)
        // Express as a single rational:
        //   yf = (N·dib1·dib2 + a·dib2 + b·dib1) / (dib1·dib2)
        let n = i64::from(y2.get()) - i64::from(y1.get()) - 1;
        let dib1 = i64::from(y1.length());
        let dib2 = i64::from(y2.length());
        // y1 < y2 ≤ Year::MAX ⇒ y1+1 ≤ y2 ≤ Year::MAX, both in range.
        let Ok(next_year_start) = Date::from_ymd(y1.get() + 1, Month::Jan, 1) else {
            return Fraction::ZERO;
        };
        let Ok(this_year_start) = Date::from_ymd(y2.get(), Month::Jan, 1) else {
            return Fraction::ZERO;
        };
        let a = i64::from(next_year_start.days_since(d1));
        let b = i64::from(d2.days_since(this_year_start));
        let num = n * dib1 * dib2 + a * dib2 + b * dib1;
        // dib1·dib2 ∈ {365², 365·366, 366²}, all positive and ≤ u64.
        #[allow(clippy::cast_sign_loss)]
        let denom = (dib1 * dib2) as u64;
        Fraction::new(num, denom).unwrap_or_default()
    }
}

impl DayCount for ActActISDA {
    fn name(&self) -> &'static str {
        "Actual/Actual (ISDA)"
    }

    fn day_count(&self, start: Date, end: Date) -> i64 {
        // QuantLib uses calendar days for ACT-family `dayCount`; the
        // convention's nuance lives in `year_fraction`.
        i64::from(end.days_since(start))
    }

    fn year_fraction(&self, start: Date, end: Date) -> Fraction {
        if start <= end {
            Self::ordered_year_fraction(start, end)
        } else {
            // Reverse direction: compute ordered, then negate.
            // `checked_neg` returns None only at i64::MIN, which the
            // ACT/ACT numerator (bounded by ~5·10⁷ across the
            // supported date range) never approaches.
            Self::ordered_year_fraction(end, start)
                .checked_neg()
                .unwrap_or_default()
        }
    }
}

// ---- ACT/ACT (ICMA) ----------------------------------------------------

/// Actual/Actual (ICMA) day-count convention — ICMA Rule 251, the
/// ISDA 2006 "Actual/Actual (ICMA)" definition (a.k.a. ISMA, or
/// "Actual/Actual (Bond)").
///
/// The standard convention for fixed-rate bond accruals (including US
/// Treasuries): a regular coupon period accrues exactly
/// `1 / frequency` of a year, and days within a period accrue
/// proportionally against that period's actual length:
///
/// ```text
/// yf = days(start, end) / (frequency × days(ref_start, ref_end))
/// ```
///
/// Unlike every other convention in this crate, ICMA is defined in
/// terms of the coupon **schedule**, not the accrual dates alone —
/// stub periods decompose into notional periods stepped from the
/// reference period. The usual way to supply that context is
/// [`bind`](Self::bind): binding to an unadjusted [`Schedule`] yields
/// a [`BoundActActICMA`] whose plain two-date
/// [`year_fraction`](DayCount::year_fraction) handles stubs
/// automatically, keeping full signature parity with the rest of the
/// [`DayCount`] impls. The inherent
/// [`year_fraction_with_reference`](Self::year_fraction_with_reference)
/// remains as a manual escape hatch. The *unbound* value's plain
/// [`year_fraction`](DayCount::year_fraction) treats its inputs as
/// one full regular coupon period and returns exactly
/// `1 / frequency` (matching `QuantLib`'s behavior when no reference
/// dates are supplied), which is only meaningful for regular periods.
///
/// # Construction
///
/// The coupon [`Frequency`] is part of the convention here, supplied
/// at construction. `QuantLib` instead infers a month count by
/// float-rounding the reference-period length
/// (`lround(12 × days / 365)`); taking the frequency explicitly
/// avoids both the float and the inference ambiguity, and lets the
/// notional-period grid step by the frequency's canonical
/// [`Period`] (so weekly frequencies work too, which the
/// months-only `QuantLib` walk does not support).
///
/// # `QuantLib` parity
///
/// Equivalent (modulo float vs. integer-rational representation) to
/// `QuantLib`'s reference-date `ActualActual::Old_ISMA_Impl`
/// algorithm in
/// [`actualactual.cpp`](https://github.com/lballabio/QuantLib/blob/master/ql/time/daycounters/actualactual.cpp),
/// with the same notional-period stepping by fresh multiples from the
/// anchor (avoiding the chained add-months clamp pathology), for
/// inputs where `QuantLib`'s month inference agrees with the supplied
/// frequency.
///
/// ```
/// use fasti::{ActActICMA, DayCount, Date, Frequency, Month};
/// let dc = ActActICMA::new(Frequency::Semiannual);
///
/// // A regular semiannual period accrues exactly half a year.
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

impl ActActICMA {
    /// Construct an ACT/ACT (ICMA) convention for the given coupon
    /// frequency.
    #[must_use]
    pub const fn new(frequency: Frequency) -> Self {
        Self { frequency }
    }

    /// The coupon frequency this convention was constructed with.
    #[must_use]
    pub const fn frequency(&self) -> Frequency {
        self.frequency
    }

    /// Bind this convention to a coupon [`Schedule`], producing a
    /// [`BoundActActICMA`] whose plain
    /// [`year_fraction`](DayCount::year_fraction) handles stub
    /// periods automatically — the uniform two-date [`DayCount`] API,
    /// with the schedule context carried in the value.
    ///
    /// Binding classifies the schedule's first and last periods as
    /// regular or stub by stepping the frequency's canonical period
    /// from the adjacent anchor (with the same end-of-month snapping
    /// as the schedule generator), precomputes the stubs' notional
    /// reference periods, and validates that their notional-period
    /// walks stay inside the supported date range — so the bound
    /// counter's `year_fraction` never fails afterwards. Interior
    /// periods are always treated as regular (fasti's
    /// forward/backward generation only ever produces irregular
    /// periods at the ends).
    ///
    /// Bind an **unadjusted** schedule: ICMA reference periods are
    /// defined on the natural coupon grid, and business-day-adjusted
    /// dates would misclassify regular periods as stubs. For a
    /// two-date schedule whose only period is a stub, the front-stub
    /// decomposition (grid anchored on the later date) is used,
    /// matching bond backward-generation convention.
    ///
    /// Returns [`TimeError::InvalidReferencePeriod`] for a schedule
    /// with fewer than two dates, and [`TimeError::DateOutOfRange`]
    /// if a stub's notional grid would escape the supported range.
    ///
    /// ```
    /// use fasti::{
    ///     ActActICMA, BusinessDayConvention, Date, DayCount, Frequency,
    ///     Month, Period, ScheduleBuilder, calendars,
    /// };
    /// // Semiannual bond with a long front stub:
    /// // Aug 15 2002 (issue) .. Jan 15 / Jul 15 coupons .. Jan 15 2004.
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
    ///
    /// // The stub decomposes against the notional grid: 153/368.
    /// let stub = dc.year_fraction(
    ///     Date::from_ymd(2002, Month::Aug, 15)?,
    ///     Date::from_ymd(2003, Month::Jan, 15)?,
    /// );
    /// assert_eq!(stub.parts(), (153, 368));
    ///
    /// // A regular period is exactly half a year.
    /// let regular = dc.year_fraction(
    ///     Date::from_ymd(2003, Month::Jan, 15)?,
    ///     Date::from_ymd(2003, Month::Jul, 15)?,
    /// );
    /// assert_eq!(regular.parts(), (1, 2));
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    pub fn bind(self, schedule: &Schedule) -> Result<BoundActActICMA<'_>, TimeError> {
        let dates = schedule.dates();
        let (&first, rest) = dates
            .split_first()
            .ok_or(TimeError::InvalidReferencePeriod)?;
        let &second = rest.first().ok_or(TimeError::InvalidReferencePeriod)?;
        // These unwraps-by-index are safe: len >= 2 established above.
        let last = dates[dates.len() - 1];
        let next_to_last = dates[dates.len() - 2];
        let period = Period::from(self.frequency);

        // Front period: regular iff stepping one period back from the
        // first coupon lands on the schedule start.
        let prev_notional = Self::notional_step(second, period, -1)?;
        let front_ref = if prev_notional == first {
            None
        } else {
            // Validate the backward walk the stub will need.
            let mut i: i32 = 1;
            while Self::notional_step(second, period, -i)? > first {
                i += 1;
            }
            Some((prev_notional, second))
        };

        // Back period (distinct from the front one only when the
        // schedule has at least three dates): regular iff stepping
        // one period forward from the last regular anchor lands on
        // the termination.
        let back_ref = if dates.len() >= 3 {
            let next_notional = Self::notional_step(next_to_last, period, 1)?;
            if next_notional == last {
                None
            } else {
                let mut i: i32 = 1;
                while Self::notional_step(next_to_last, period, i)? < last {
                    i += 1;
                }
                Some((next_to_last, next_notional))
            }
        } else {
            None
        };

        Ok(BoundActActICMA {
            inner: self,
            schedule,
            front_ref,
            back_ref,
        })
    }

    /// `anchor + i × period`, stepped as a fresh multiple from the
    /// anchor (never chained), with the same end-of-month snap rule
    /// as the `Schedule` generator: when the anchor is the last day
    /// of its month and the period is in months or years, the result
    /// snaps to the end of its own month.
    fn notional_step(anchor: Date, period: Period, i: i32) -> Result<Date, TimeError> {
        let scaled = period.checked_mul(i).ok_or(TimeError::DateOutOfRange)?;
        let stepped = (anchor + scaled)?;
        if anchor.is_end_of_month() && matches!(period, Period::Months(_) | Period::Years(_)) {
            Ok(stepped.end_of_month())
        } else {
            Ok(stepped)
        }
    }

    /// The year fraction between `start` and `end`, given the
    /// (unadjusted) regular coupon reference period
    /// `ref_start..ref_end` the accrual belongs to.
    ///
    /// This is the manual escape hatch for callers who track
    /// reference periods themselves; when the accruals come from a
    /// [`Schedule`], prefer [`bind`](Self::bind), which derives the
    /// reference periods once and exposes the uniform two-date
    /// [`DayCount`] API.
    ///
    /// Returns [`TimeError::InvalidReferencePeriod`] when
    /// `ref_start >= ref_end`, [`TimeError::DateOutOfRange`] when a
    /// stub's notional-period walk escapes the supported date range,
    /// and [`TimeError::FractionOverflow`] if the fraction arithmetic
    /// overflows.
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

    /// `days(lo, hi) / (freq × days(w_start, w_end))` for a chunk of
    /// accrual inside one notional period, as a reduced fraction.
    fn chunk_ratio(
        lo: Date,
        hi: Date,
        w_start: Date,
        w_end: Date,
        freq: u64,
    ) -> Result<Fraction, TimeError> {
        let window_days = u64::try_from(w_end.days_since(w_start))
            .map_err(|_| TimeError::InvalidReferencePeriod)?;
        let denom = freq
            .checked_mul(window_days)
            .ok_or(TimeError::FractionOverflow)?;
        Fraction::new(i64::from(hi.days_since(lo)), denom)
    }

    /// The ICMA year fraction for `d1 < d2` against the reference
    /// period `r1 < r2`.
    ///
    /// The accrual decomposes over a grid of notional periods
    /// anchored on the reference period: the reference period itself,
    /// plus periods stepped backward from `r1` and forward from `r2`
    /// by the frequency's canonical period. Each full notional period
    /// contributes exactly `1/freq`; partial chunks contribute
    /// proportionally against their own notional period's length.
    fn ordered_year_fraction(
        self,
        d1: Date,
        d2: Date,
        r1: Date,
        r2: Date,
    ) -> Result<Fraction, TimeError> {
        let freq = u64::from(self.frequency.per_year());
        let period = Period::from(self.frequency);
        let mut total = Fraction::ZERO;

        // Middle: the part of the accrual inside the reference period.
        let mid_lo = if d1 > r1 { d1 } else { r1 };
        let mid_hi = if d2 < r2 { d2 } else { r2 };
        if mid_lo < mid_hi {
            total = total
                .checked_add(Self::chunk_ratio(mid_lo, mid_hi, r1, r2, freq)?)
                .ok_or(TimeError::FractionOverflow)?;
        }

        // Backward: the part before `r1`, over notional periods
        // stepped back from `r1`.
        if d1 < r1 {
            let seg_hi = if d2 < r1 { d2 } else { r1 };
            let mut i: i32 = 1;
            loop {
                let w_start = Self::notional_step(r1, period, -i)?;
                let w_end = Self::notional_step(r1, period, -(i - 1))?;
                let lo = if d1 > w_start { d1 } else { w_start };
                let hi = if seg_hi < w_end { seg_hi } else { w_end };
                if lo < hi {
                    total = total
                        .checked_add(Self::chunk_ratio(lo, hi, w_start, w_end, freq)?)
                        .ok_or(TimeError::FractionOverflow)?;
                }
                if w_start <= d1 {
                    break;
                }
                i += 1;
            }
        }

        // Forward: the part after `r2`, over notional periods stepped
        // forward from `r2`.
        if d2 > r2 {
            let seg_lo = if d1 > r2 { d1 } else { r2 };
            let mut i: i32 = 0;
            loop {
                let w_start = Self::notional_step(r2, period, i)?;
                let w_end = Self::notional_step(r2, period, i + 1)?;
                let lo = if seg_lo > w_start { seg_lo } else { w_start };
                let hi = if d2 < w_end { d2 } else { w_end };
                if lo < hi {
                    total = total
                        .checked_add(Self::chunk_ratio(lo, hi, w_start, w_end, freq)?)
                        .ok_or(TimeError::FractionOverflow)?;
                }
                if w_end >= d2 {
                    break;
                }
                i += 1;
            }
        }

        Ok(total)
    }
}

impl DayCount for ActActICMA {
    fn name(&self) -> &'static str {
        "Actual/Actual (ICMA)"
    }

    fn day_count(&self, start: Date, end: Date) -> i64 {
        // Calendar days, like the rest of the ACT family; the
        // convention's nuance lives in the year fraction.
        i64::from(end.days_since(start))
    }

    /// Without schedule context, the accrual is treated as one full
    /// regular coupon period: exactly `1 / frequency`, signed by
    /// direction. For real schedules use [`ActActICMA::bind`], whose
    /// bound counter handles stubs through this same uniform
    /// signature; for manual reference periods use
    /// [`ActActICMA::year_fraction_with_reference`].
    fn year_fraction(&self, start: Date, end: Date) -> Fraction {
        if start == end {
            return Fraction::ZERO;
        }
        let freq = i64::from(self.frequency.per_year());
        let num = if start < end { 1 } else { -1 };
        // `freq >= 1` always, so the sign-loss cast and the
        // constructor are both infallible; see `Act360::year_fraction`
        // for the `unwrap_or_default` rationale.
        #[allow(clippy::cast_sign_loss)]
        Fraction::new(num, freq as u64).unwrap_or_default()
    }
}

/// [`ActActICMA`] bound to a coupon [`Schedule`] — produced by
/// [`ActActICMA::bind`].
///
/// Implements [`DayCount`] with the same two-date signature as every
/// other convention; the schedule context needed for stub handling
/// lives in the value, mirroring `QuantLib`'s schedule-carrying
/// `ActualActual(ISMA, schedule)`. Accruals decompose over the
/// schedule's periods: chunks in regular periods accrue against that
/// period, chunks in a stub period accrue against the stub's notional
/// reference grid (precomputed and range-validated at bind time).
///
/// Dates outside the schedule's span contribute nothing — the
/// schedule defines the instrument's life, and there is no accrual
/// before issue or after termination. This clamping is deliberate,
/// documented semantics, not a fallback.
#[derive(Debug, Clone, Copy)]
pub struct BoundActActICMA<'s> {
    inner: ActActICMA,
    schedule: &'s Schedule,
    /// Notional reference period for the first schedule period, when
    /// it is a stub (`None` = regular, self-referenced).
    front_ref: Option<(Date, Date)>,
    /// Likewise for the last schedule period.
    back_ref: Option<(Date, Date)>,
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

    /// The ICMA year fraction for `d1 < d2`, clamped to the
    /// schedule's span and summed over the overlapped periods.
    fn ordered_year_fraction(&self, d1: Date, d2: Date) -> Fraction {
        let dates = self.schedule.dates();
        let (Some(&first), Some(&last)) = (dates.first(), dates.last()) else {
            // Unreachable: bind requires at least two dates.
            return Fraction::ZERO;
        };
        let lo = if d1 > first { d1 } else { first };
        let hi = if d2 < last { d2 } else { last };
        if lo >= hi {
            return Fraction::ZERO;
        }
        let freq = u64::from(self.inner.frequency().per_year());
        let last_period = dates.len() - 2;
        let mut total = Fraction::ZERO;
        for (i, w) in dates.windows(2).enumerate() {
            let (s_start, s_end) = (w[0], w[1]);
            if s_end <= lo {
                continue;
            }
            if s_start >= hi {
                break;
            }
            let c_lo = if lo > s_start { lo } else { s_start };
            let c_hi = if hi < s_end { hi } else { s_end };
            if c_lo >= c_hi {
                continue;
            }
            let stub_ref = match i {
                0 => self.front_ref,
                _ if i == last_period => self.back_ref,
                _ => None,
            };
            // Both arms' error paths were validated at bind time
            // (notional walks in range, non-degenerate windows), so
            // the `unwrap_or_default` arms are unreachable and exist
            // only to honour the no-panic contract.
            let chunk = match stub_ref {
                Some((r1, r2)) => self
                    .inner
                    .ordered_year_fraction(c_lo, c_hi, r1, r2)
                    .unwrap_or_default(),
                None => {
                    ActActICMA::chunk_ratio(c_lo, c_hi, s_start, s_end, freq).unwrap_or_default()
                }
            };
            // Denominators are drawn from the small set of distinct
            // (frequency × period-length) values in one schedule, so
            // the running sum's reduced form stays far inside i64/u64.
            total = total.checked_add(chunk).unwrap_or_default();
        }
        total
    }
}

impl DayCount for BoundActActICMA<'_> {
    fn name(&self) -> &'static str {
        "Actual/Actual (ICMA)"
    }

    fn day_count(&self, start: Date, end: Date) -> i64 {
        // Calendar days, like the rest of the ACT family.
        i64::from(end.days_since(start))
    }

    fn year_fraction(&self, start: Date, end: Date) -> Fraction {
        if start == end {
            return Fraction::ZERO;
        }
        if start < end {
            self.ordered_year_fraction(start, end)
        } else {
            // The ordered numerators stay far below i64::MIN in
            // magnitude, so negation cannot fail; see
            // `ActActISDA::year_fraction` for the rationale.
            self.ordered_year_fraction(end, start)
                .checked_neg()
                .unwrap_or_default()
        }
    }
}

// ---- 30/360 variants ---------------------------------------------------

/// `true` iff `day` is the last day of February in `year`.
const fn is_last_of_february(day: u8, month: Month, year: Year) -> bool {
    matches!(month, Month::Feb) && day == Month::Feb.length(year)
}

/// 30/360 (US) day-count convention — SIA / "30/360 US" with the
/// last-day-of-February rule.
///
/// Like [`Thirty360Bond`] but with two additional adjustments applied
/// *before* the 31st-day rules: if the start date is the last day of
/// February its day becomes 30, and if additionally the end date is
/// also the last day of February, the end day becomes 30 too.
///
/// # `QuantLib` parity
///
/// Bit-for-bit equivalent to `QuantLib`'s `Thirty360::US_Impl`
/// (`Thirty360::USA`) in
/// [`thirty360.cpp`](https://github.com/lballabio/QuantLib/blob/master/ql/time/daycounters/thirty360.cpp).
/// (`QuantLib`'s separate `NASD` variant, which rolls a lone 31st
/// end-day into the next month, is not modeled.)
///
/// ```
/// use fasti::{DayCount, Date, Month, Thirty360US};
/// // Feb 28 2025 (last of February) -> Mar 31 2025:
/// // D1 = 30 (last-of-Feb), then D2 = 31 -> 30 since D1 >= 30.
/// let start = Date::from_ymd(2025, Month::Feb, 28)?;
/// let end = Date::from_ymd(2025, Month::Mar, 31)?;
/// assert_eq!(Thirty360US.day_count(start, end), 30);
/// # Ok::<(), fasti::TimeError>(())
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Thirty360US;

impl Thirty360US {
    /// 30/360 (US) day count assuming `d1 <= d2`.
    fn ordered_count(d1: Date, d2: Date) -> i64 {
        let (y1, m1, mut dd1) = d1.to_ymd();
        let (y2, m2, mut dd2) = d2.to_ymd();
        if is_last_of_february(dd1, m1, y1) {
            if is_last_of_february(dd2, m2, y2) {
                dd2 = 30;
            }
            dd1 = 30;
        }
        if dd2 == 31 && dd1 >= 30 {
            dd2 = 30;
        }
        if dd1 == 31 {
            dd1 = 30;
        }
        360 * (i64::from(y2.get()) - i64::from(y1.get()))
            + 30 * (i64::from(m2.get()) - i64::from(m1.get()))
            + (i64::from(dd2) - i64::from(dd1))
    }
}

impl DayCount for Thirty360US {
    fn name(&self) -> &'static str {
        "30/360 (US)"
    }

    fn day_count(&self, start: Date, end: Date) -> i64 {
        // Asymmetric formula; normalize via the ordered pair like
        // `Thirty360Bond` so `dc(a, b) == -dc(b, a)` holds.
        if start <= end {
            Self::ordered_count(start, end)
        } else {
            -Self::ordered_count(end, start)
        }
    }

    fn year_fraction(&self, start: Date, end: Date) -> Fraction {
        // Denominator 360 is non-zero; see `Act360::year_fraction`
        // for the `unwrap_or_default` rationale.
        Fraction::new(self.day_count(start, end), 360).unwrap_or_default()
    }
}

/// 30E/360 (Eurobond Basis) day-count convention — the ISDA 2006
/// "30E/360" definition, a.k.a. "30/360 European".
///
/// The simplest member of the family: both day-of-month values are
/// independently capped at 30 (`31 → 30`), with no February handling
/// and no coupling between the two adjustments.
///
/// # `QuantLib` parity
///
/// Bit-for-bit equivalent to `QuantLib`'s `Thirty360::EU_Impl`
/// (`Thirty360::European` / `Thirty360::EurobondBasis`).
///
/// ```
/// use fasti::{DayCount, Date, Month, Thirty360European};
/// // Jan 15 -> Mar 31: D2 = 31 -> 30 unconditionally (Bond Basis
/// // would keep 31 because D1 < 30).
/// let start = Date::from_ymd(2025, Month::Jan, 15)?;
/// let end = Date::from_ymd(2025, Month::Mar, 31)?;
/// assert_eq!(Thirty360European.day_count(start, end), 75);
/// # Ok::<(), fasti::TimeError>(())
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Thirty360European;

impl Thirty360European {
    /// 30E/360 day count assuming `d1 <= d2`.
    fn ordered_count(d1: Date, d2: Date) -> i64 {
        let (y1, m1, mut dd1) = d1.to_ymd();
        let (y2, m2, mut dd2) = d2.to_ymd();
        if dd1 == 31 {
            dd1 = 30;
        }
        if dd2 == 31 {
            dd2 = 30;
        }
        360 * (i64::from(y2.get()) - i64::from(y1.get()))
            + 30 * (i64::from(m2.get()) - i64::from(m1.get()))
            + (i64::from(dd2) - i64::from(dd1))
    }
}

impl DayCount for Thirty360European {
    fn name(&self) -> &'static str {
        "30E/360 (Eurobond Basis)"
    }

    fn day_count(&self, start: Date, end: Date) -> i64 {
        // The two adjustments are independent, so the formula is
        // naturally antisymmetric — ordering keeps the code uniform
        // with the other 30/360 variants at no behavioral cost.
        if start <= end {
            Self::ordered_count(start, end)
        } else {
            -Self::ordered_count(end, start)
        }
    }

    fn year_fraction(&self, start: Date, end: Date) -> Fraction {
        // Denominator 360 is non-zero; see `Act360::year_fraction`
        // for the `unwrap_or_default` rationale.
        Fraction::new(self.day_count(start, end), 360).unwrap_or_default()
    }
}

/// 30E/360 (ISDA) day-count convention — the ISDA 2006 "30E/360
/// (ISDA)" definition, a.k.a. "30/360 German".
///
/// Like [`Thirty360European`] but with February handling: a start
/// date on the last day of February counts as 30, and an end date on
/// the last day of February counts as 30 *unless* it is the
/// instrument's termination (maturity) date — which is why the
/// termination date is part of the convention's construction.
///
/// # `QuantLib` parity
///
/// Bit-for-bit equivalent to `QuantLib`'s `Thirty360::ISDA_Impl`
/// (`Thirty360::ISDA` / `Thirty360::German`), constructed with the
/// same termination date.
///
/// ```
/// use fasti::{DayCount, Date, Month, Thirty360ISDA};
/// let maturity = Date::from_ymd(2026, Month::Feb, 28)?;
/// let dc = Thirty360ISDA::new(maturity);
/// let start = Date::from_ymd(2025, Month::Aug, 31)?;
///
/// // Ending on last-of-February mid-schedule: D2 = 28 -> 30.
/// let feb28_2026_interim = Thirty360ISDA::new(Date::from_ymd(2030, Month::Jan, 1)?);
/// assert_eq!(feb28_2026_interim.day_count(start, maturity), 180);
///
/// // Same dates, but Feb 28 2026 IS the termination date: D2 stays 28.
/// assert_eq!(dc.day_count(start, maturity), 178);
/// # Ok::<(), fasti::TimeError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Thirty360ISDA {
    termination: Date,
}

impl Thirty360ISDA {
    /// Construct a 30E/360 (ISDA) convention for an instrument
    /// maturing on `termination`.
    #[must_use]
    pub const fn new(termination: Date) -> Self {
        Self { termination }
    }

    /// The termination (maturity) date this convention was
    /// constructed with.
    #[must_use]
    pub const fn termination(&self) -> Date {
        self.termination
    }

    /// 30E/360 (ISDA) day count assuming `d1 <= d2`.
    fn ordered_count(self, d1: Date, d2: Date) -> i64 {
        let (y1, m1, mut dd1) = d1.to_ymd();
        let (y2, m2, mut dd2) = d2.to_ymd();
        if dd1 == 31 {
            dd1 = 30;
        }
        if dd2 == 31 {
            dd2 = 30;
        }
        if is_last_of_february(dd1, m1, y1) {
            dd1 = 30;
        }
        if d2 != self.termination && is_last_of_february(dd2, m2, y2) {
            dd2 = 30;
        }
        360 * (i64::from(y2.get()) - i64::from(y1.get()))
            + 30 * (i64::from(m2.get()) - i64::from(m1.get()))
            + (i64::from(dd2) - i64::from(dd1))
    }
}

impl DayCount for Thirty360ISDA {
    fn name(&self) -> &'static str {
        "30E/360 (ISDA)"
    }

    fn day_count(&self, start: Date, end: Date) -> i64 {
        // The termination-date exception applies to the
        // chronologically later date; normalizing via the ordered
        // pair keeps `dc(a, b) == -dc(b, a)` total.
        if start <= end {
            self.ordered_count(start, end)
        } else {
            -self.ordered_count(end, start)
        }
    }

    fn year_fraction(&self, start: Date, end: Date) -> Fraction {
        // Denominator 360 is non-zero; see `Act360::year_fraction`
        // for the `unwrap_or_default` rationale.
        Fraction::new(self.day_count(start, end), 360).unwrap_or_default()
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
