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

use crate::{Date, Fraction, Month};

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

// ---- Tests --------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
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
            for dc in [
                &Act360 as &dyn DayCount,
                &Act365Fixed,
                &Thirty360Bond,
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
            for dc in [
                &Act360 as &dyn DayCount,
                &Act365Fixed,
                &Thirty360Bond,
                &ActActISDA,
            ] {
                prop_assert_eq!(dc.day_count(a, b), -dc.day_count(b, a));
            }
        }
    }
}
