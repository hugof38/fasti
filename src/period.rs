//! [`Period`] and [`Frequency`] — the building blocks of scheduled
//! date arithmetic.
//!
//! Matches `QuantLib`'s period/frequency semantics; the unit is the enum
//! variant and lengths are signed [`i32`], so negative periods are first-class.

use core::{
    fmt,
    ops::{Mul, Neg},
};

use crate::TimeError;

// ---- Frequency ----------------------------------------------------------

/// How often cashflows recur per calendar year. Matches `QuantLib`'s
/// canonical frequencies; [`Frequency::per_year`] is always positive.
///
/// ```
/// use fasti::{Frequency, Period};
/// // 3 months == Quarterly.
/// assert_eq!(Frequency::try_from(Period::Months(3))?, Frequency::Quarterly);
/// // Total in this direction — every variant has a canonical period.
/// assert_eq!(Period::from(Frequency::Quarterly), Period::Months(3));
/// # Ok::<(), fasti::TimeError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u16)]
pub enum Frequency {
    /// Once a year.
    Annual = 1,
    /// Twice a year.
    Semiannual = 2,
    /// Every fourth month — 3 times a year.
    EveryFourthMonth = 3,
    /// Every third month — 4 times a year.
    Quarterly = 4,
    /// Every second month — 6 times a year.
    Bimonthly = 6,
    /// Once a month.
    Monthly = 12,
    /// Every fourth week — 13 times a year.
    EveryFourthWeek = 13,
    /// Every second week — 26 times a year.
    Biweekly = 26,
    /// Once a week.
    Weekly = 52,
    /// Once a day.
    Daily = 365,
}

impl Frequency {
    /// The number of recurrences per year. Always positive.
    ///
    /// ```
    /// use fasti::Frequency;
    /// assert_eq!(Frequency::Quarterly.per_year(), 4);
    /// assert_eq!(Frequency::Daily.per_year(), 365);
    /// ```
    #[must_use]
    pub const fn per_year(self) -> u16 {
        self as u16
    }
}

impl fmt::Display for Frequency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Annual => "Annual",
            Self::Semiannual => "Semiannual",
            Self::EveryFourthMonth => "EveryFourthMonth",
            Self::Quarterly => "Quarterly",
            Self::Bimonthly => "Bimonthly",
            Self::Monthly => "Monthly",
            Self::EveryFourthWeek => "EveryFourthWeek",
            Self::Biweekly => "Biweekly",
            Self::Weekly => "Weekly",
            Self::Daily => "Daily",
        };
        f.write_str(name)
    }
}

// ---- Period -------------------------------------------------------------

/// A signed duration tagged by its calendar unit.
///
/// Each variant carries an [`i32`] length; the unit is the variant.
///
/// ```
/// use fasti::Period;
/// let p = Period::Months(3);
/// match p {
///     Period::Days(n) => unreachable!("not days, n={n}"),
///     Period::Months(n) => assert_eq!(n, 3),
///     _ => unreachable!(),
/// }
/// // `12M` normalizes to `1Y`.
/// assert_eq!(Period::Months(12).normalized(), Period::Years(1));
/// // Scalar multiplication scales the length.
/// assert_eq!(Period::Months(3) * 4, Period::Months(12));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Period {
    /// Calendar days.
    Days(i32),
    /// Weeks — 7 calendar days, no calendar dependency.
    Weeks(i32),
    /// Calendar months — variable length (28..=31 days).
    Months(i32),
    /// Calendar years — variable length (365 or 366 days).
    Years(i32),
}

impl Period {
    /// The zero period — `0 Days`.
    pub const ZERO: Self = Self::Days(0);

    /// The signed length component.
    ///
    /// ```
    /// use fasti::Period;
    /// assert_eq!(Period::Months(3).length(), 3);
    /// assert_eq!(Period::Days(-7).length(), -7);
    /// ```
    #[must_use]
    pub const fn length(self) -> i32 {
        match self {
            Self::Days(n) | Self::Weeks(n) | Self::Months(n) | Self::Years(n) => n,
        }
    }

    /// `true` iff the period has zero length, regardless of unit.
    ///
    /// ```
    /// use fasti::Period;
    /// assert!(Period::ZERO.is_zero());
    /// assert!(Period::Months(0).is_zero());
    /// assert!(!Period::Days(1).is_zero());
    /// ```
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.length() == 0
    }

    /// Replace the length while preserving the unit.
    const fn with_length(self, length: i32) -> Self {
        match self {
            Self::Days(_) => Self::Days(length),
            Self::Weeks(_) => Self::Weeks(length),
            Self::Months(_) => Self::Months(length),
            Self::Years(_) => Self::Years(length),
        }
    }

    /// Canonicalize the period — `12 Months` → `1 Year`, `7 Days` → `1 Week`;
    /// non-multiples are unchanged and zero normalizes to `0 Days`.
    ///
    /// ```
    /// use fasti::Period;
    /// assert_eq!(Period::Months(24).normalized(), Period::Years(2));
    /// assert_eq!(Period::Days(14).normalized(), Period::Weeks(2));
    /// // Non-multiples stay put.
    /// assert_eq!(Period::Months(5).normalized(), Period::Months(5));
    /// // Zero normalizes to 0 Days regardless of input unit.
    /// assert_eq!(Period::Years(0).normalized(), Period::Days(0));
    /// ```
    #[must_use]
    pub const fn normalized(self) -> Self {
        match self {
            Self::Days(0) | Self::Weeks(0) | Self::Months(0) | Self::Years(0) => Self::ZERO,
            Self::Months(n) if n % 12 == 0 => Self::Years(n / 12),
            Self::Days(n) if n % 7 == 0 => Self::Weeks(n / 7),
            other => other,
        }
    }

    /// Negate the length, returning [`None`] on overflow
    /// (`i32::MIN` has no positive counterpart).
    ///
    /// ```
    /// use fasti::Period;
    /// assert_eq!(Period::Months(3).checked_neg(), Some(Period::Months(-3)));
    /// assert_eq!(Period::Days(i32::MIN).checked_neg(), None);
    /// ```
    #[must_use]
    pub const fn checked_neg(self) -> Option<Self> {
        match self.length().checked_neg() {
            Some(n) => Some(self.with_length(n)),
            None => None,
        }
    }

    /// Scale the length by `n`, returning [`None`] on overflow.
    ///
    /// ```
    /// use fasti::Period;
    /// assert_eq!(Period::Months(3).checked_mul(4), Some(Period::Months(12)));
    /// assert_eq!(Period::Days(i32::MAX).checked_mul(2), None);
    /// ```
    #[must_use]
    pub const fn checked_mul(self, n: i32) -> Option<Self> {
        match self.length().checked_mul(n) {
            Some(length) => Some(self.with_length(length)),
            None => None,
        }
    }
}

// ---- Period ↔ Frequency -------------------------------------------------

impl From<Frequency> for Period {
    /// Map a [`Frequency`] to its canonical [`Period`]. `Annual` maps to
    /// `12 Months`; call [`Period::normalized`] to get `1 Year`.
    fn from(frequency: Frequency) -> Self {
        match frequency {
            Frequency::Annual => Self::Months(12),
            Frequency::Semiannual => Self::Months(6),
            Frequency::EveryFourthMonth => Self::Months(4),
            Frequency::Quarterly => Self::Months(3),
            Frequency::Bimonthly => Self::Months(2),
            Frequency::Monthly => Self::Months(1),
            Frequency::EveryFourthWeek => Self::Weeks(4),
            Frequency::Biweekly => Self::Weeks(2),
            Frequency::Weekly => Self::Weeks(1),
            Frequency::Daily => Self::Days(1),
        }
    }
}

impl TryFrom<Period> for Frequency {
    type Error = TimeError;

    /// Map a [`Period`] to the canonical [`Frequency`], normalizing first.
    /// Non-canonical, zero, and negative periods return
    /// [`TimeError::NonCanonicalPeriod`].
    fn try_from(period: Period) -> Result<Self, Self::Error> {
        match period.normalized() {
            Period::Years(1) => Ok(Self::Annual),
            Period::Months(1) => Ok(Self::Monthly),
            Period::Months(2) => Ok(Self::Bimonthly),
            Period::Months(3) => Ok(Self::Quarterly),
            Period::Months(4) => Ok(Self::EveryFourthMonth),
            Period::Months(6) => Ok(Self::Semiannual),
            Period::Weeks(1) => Ok(Self::Weekly),
            Period::Weeks(2) => Ok(Self::Biweekly),
            Period::Weeks(4) => Ok(Self::EveryFourthWeek),
            Period::Days(1) => Ok(Self::Daily),
            _ => Err(TimeError::NonCanonicalPeriod),
        }
    }
}

// ---- Period scalar ops --------------------------------------------------

impl Neg for Period {
    type Output = Self;

    /// Negate the length. **Wraps** on `i32::MIN`; use
    /// [`Period::checked_neg`] to detect overflow.
    fn neg(self) -> Self::Output {
        self.with_length(self.length().wrapping_neg())
    }
}

impl Mul<i32> for Period {
    type Output = Self;

    /// Scale the length by `n`, preserving the unit. **Wraps** on overflow;
    /// use [`Period::checked_mul`] to detect it.
    fn mul(self, n: i32) -> Self::Output {
        self.with_length(self.length().wrapping_mul(n))
    }
}

impl Mul<Period> for i32 {
    type Output = Period;

    /// Scalar multiplication with the scalar on the left.
    fn mul(self, period: Period) -> Self::Output {
        period * self
    }
}

// ---- Period display -----------------------------------------------------

impl fmt::Display for Period {
    /// Format as `QuantLib` does: `3M`, `1Y`, `2W`, `14D`.
    /// Zero-length periods format as `0D`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (n, suffix) = match self {
            Self::Days(n) => (n, 'D'),
            Self::Weeks(n) => (n, 'W'),
            Self::Months(n) => (n, 'M'),
            Self::Years(n) => (n, 'Y'),
        };
        write!(f, "{n}{suffix}")
    }
}

// ---- Tests --------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    extern crate alloc;

    use super::*;
    use proptest::prelude::*;

    // ---- Frequency -----------------------------------------------------

    #[test]
    fn frequency_per_year_matches_quantlib_values() {
        assert_eq!(Frequency::Annual.per_year(), 1);
        assert_eq!(Frequency::Semiannual.per_year(), 2);
        assert_eq!(Frequency::EveryFourthMonth.per_year(), 3);
        assert_eq!(Frequency::Quarterly.per_year(), 4);
        assert_eq!(Frequency::Bimonthly.per_year(), 6);
        assert_eq!(Frequency::Monthly.per_year(), 12);
        assert_eq!(Frequency::EveryFourthWeek.per_year(), 13);
        assert_eq!(Frequency::Biweekly.per_year(), 26);
        assert_eq!(Frequency::Weekly.per_year(), 52);
        assert_eq!(Frequency::Daily.per_year(), 365);
    }

    // ---- Period --------------------------------------------------------

    #[test]
    fn constructors_are_const() {
        const ONE_YEAR: Period = Period::Years(1);
        const THREE_MONTHS: Period = Period::Months(3);
        const ZERO: Period = Period::ZERO;
        assert_eq!(ONE_YEAR.length(), 1);
        assert!(matches!(THREE_MONTHS, Period::Months(3)));
        assert_eq!(ZERO, Period::Days(0));
    }

    #[test]
    fn length_extracts_the_signed_count() {
        assert_eq!(Period::Days(0).length(), 0);
        assert_eq!(Period::Months(3).length(), 3);
        assert_eq!(Period::Years(-1).length(), -1);
        assert_eq!(Period::Weeks(i32::MAX).length(), i32::MAX);
    }

    #[test]
    fn negation_flips_length() {
        assert_eq!(-Period::Months(3), Period::Months(-3));
        assert_eq!(-Period::Years(-1), Period::Years(1));
    }

    #[test]
    fn neg_wraps_at_i32_min_documented_behavior() {
        // `i32::MIN` has no positive counterpart: the operator wraps, the checked variant returns None.
        let edge = Period::Days(i32::MIN);
        assert_eq!((-edge).length(), i32::MIN);
        assert_eq!(edge.checked_neg(), None);
        // One above MIN negates cleanly under both APIs.
        let safe = Period::Days(i32::MIN + 1);
        assert_eq!(safe.checked_neg(), Some(Period::Days(i32::MAX)));
        assert_eq!(-safe, Period::Days(i32::MAX));
    }

    #[test]
    fn checked_mul_detects_overflow() {
        assert_eq!(Period::Months(3).checked_mul(4), Some(Period::Months(12)));
        assert_eq!(Period::Days(i32::MAX).checked_mul(2), None);
        assert_eq!(Period::Days(i32::MIN).checked_mul(-1), None);
        // Zero scaling is well-defined and lossless.
        assert_eq!(
            Period::Weeks(i32::MAX).checked_mul(0),
            Some(Period::Weeks(0)),
        );
    }

    #[test]
    fn scalar_multiplication_scales_length() {
        assert_eq!(Period::Months(3) * 4, Period::Months(12));
        assert_eq!(4 * Period::Months(3), Period::Months(12));
        // `erasing_op` allow: `* 0` is the semantics under test.
        #[allow(clippy::erasing_op)]
        let zeroed = Period::Days(7) * 0;
        assert_eq!(zeroed, Period::Days(0));
        assert_eq!(Period::Weeks(2) * -1, Period::Weeks(-2));
    }

    #[test]
    fn normalize_canonicalizes_months_and_days() {
        assert_eq!(Period::Months(12).normalized(), Period::Years(1));
        assert_eq!(Period::Months(24).normalized(), Period::Years(2));
        assert_eq!(Period::Days(7).normalized(), Period::Weeks(1));
        assert_eq!(Period::Days(14).normalized(), Period::Weeks(2));
    }

    #[test]
    fn normalize_zero_collapses_to_days() {
        assert_eq!(Period::Years(0).normalized(), Period::Days(0));
        assert_eq!(Period::Months(0).normalized(), Period::Days(0));
        assert_eq!(Period::Weeks(0).normalized(), Period::Days(0));
        assert_eq!(Period::Days(0).normalized(), Period::Days(0));
    }

    #[test]
    fn normalize_leaves_non_multiples_alone() {
        assert_eq!(Period::Months(5).normalized(), Period::Months(5));
        assert_eq!(Period::Months(13).normalized(), Period::Months(13));
        assert_eq!(Period::Days(10).normalized(), Period::Days(10));
    }

    #[test]
    fn period_display_matches_quantlib_shorthand() {
        assert_eq!(alloc::format!("{}", Period::Days(14)), "14D");
        assert_eq!(alloc::format!("{}", Period::Weeks(2)), "2W");
        assert_eq!(alloc::format!("{}", Period::Months(3)), "3M");
        assert_eq!(alloc::format!("{}", Period::Years(5)), "5Y");
        assert_eq!(alloc::format!("{}", Period::Months(-1)), "-1M");
    }

    // ---- Period ↔ Frequency --------------------------------------------

    #[test]
    fn period_to_frequency_canonical_values() {
        assert_eq!(
            Frequency::try_from(Period::Years(1)).unwrap(),
            Frequency::Annual,
        );
        assert_eq!(
            Frequency::try_from(Period::Months(12)).unwrap(),
            Frequency::Annual,
        );
        assert_eq!(
            Frequency::try_from(Period::Months(6)).unwrap(),
            Frequency::Semiannual,
        );
        assert_eq!(
            Frequency::try_from(Period::Months(4)).unwrap(),
            Frequency::EveryFourthMonth,
        );
        assert_eq!(
            Frequency::try_from(Period::Months(3)).unwrap(),
            Frequency::Quarterly,
        );
        assert_eq!(
            Frequency::try_from(Period::Months(2)).unwrap(),
            Frequency::Bimonthly,
        );
        assert_eq!(
            Frequency::try_from(Period::Months(1)).unwrap(),
            Frequency::Monthly,
        );
        assert_eq!(
            Frequency::try_from(Period::Weeks(4)).unwrap(),
            Frequency::EveryFourthWeek,
        );
        assert_eq!(
            Frequency::try_from(Period::Weeks(2)).unwrap(),
            Frequency::Biweekly,
        );
        assert_eq!(
            Frequency::try_from(Period::Weeks(1)).unwrap(),
            Frequency::Weekly,
        );
        assert_eq!(
            Frequency::try_from(Period::Days(1)).unwrap(),
            Frequency::Daily,
        );
    }

    #[test]
    fn period_to_frequency_non_canonical_errors() {
        for non_canonical in [
            // Non-canonical positive periods.
            Period::Months(5),
            Period::Days(3),
            Period::Weeks(3),
            Period::Years(2),
            // Zero — no recurrence rate.
            Period::Days(0),
            Period::Months(0),
            // Negative — recurrence rates are positive by definition.
            Period::Months(-3),
            Period::Weeks(-1),
            Period::Years(-1),
        ] {
            assert_eq!(
                Frequency::try_from(non_canonical),
                Err(TimeError::NonCanonicalPeriod),
                "expected error for {non_canonical:?}",
            );
        }
    }

    #[test]
    fn frequency_to_period_is_total_and_round_trips() {
        for f in [
            Frequency::Annual,
            Frequency::Semiannual,
            Frequency::EveryFourthMonth,
            Frequency::Quarterly,
            Frequency::Bimonthly,
            Frequency::Monthly,
            Frequency::EveryFourthWeek,
            Frequency::Biweekly,
            Frequency::Weekly,
            Frequency::Daily,
        ] {
            // From is total — no Result, no expect.
            let p = Period::from(f);
            assert_eq!(
                Frequency::try_from(p).unwrap(),
                f,
                "round-trip failed for {f:?} via {p:?}",
            );
        }
    }

    // ---- property tests ------------------------------------------------

    /// Strategy: any `Period` in the given length range, uniform across units.
    fn any_period(length_range: core::ops::RangeInclusive<i32>) -> impl Strategy<Value = Period> {
        (length_range, 0u8..=3).prop_map(|(length, kind)| match kind {
            0 => Period::Days(length),
            1 => Period::Weeks(length),
            2 => Period::Months(length),
            _ => Period::Years(length),
        })
    }

    proptest! {
        #[test]
        fn normalize_is_idempotent(p in any_period(-1000..=1000)) {
            let once = p.normalized();
            let twice = once.normalized();
            prop_assert_eq!(once, twice);
        }

        #[test]
        fn scalar_mul_commutes(length in -100i32..=100, n in -10i32..=10) {
            let p = Period::Months(length);
            prop_assert_eq!(p * n, n * p);
        }

        #[test]
        fn double_negate_is_identity(length in -1000i32..=1000) {
            let p = Period::Weeks(length);
            prop_assert_eq!(-(-p), p);
        }

        /// `length` and `with_length` round-trip.
        #[test]
        fn with_length_round_trips(p in any_period(i32::MIN..=i32::MAX)) {
            prop_assert_eq!(p.with_length(p.length()), p);
        }
    }
}
