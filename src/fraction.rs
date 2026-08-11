//! [`Fraction`] — an integer rational `num / den`.
//!
//! The crate-wide convention is that rationals are integer fractions —
//! never `f64`, never `rust_decimal`. The primary client is the
//! [`DayCount`](crate::DayCount) trait, which returns a `Fraction` from
//! `year_fraction(start, end)`; downstream crates also use `Fraction`
//! as the scalar that scales monetary amounts (basis-point rates,
//! coverage thresholds, etc.). The type is `Fraction` rather than
//! `YearFraction` because the math is the same regardless of whether
//! the numerator and denominator carry "year" units or not.
//!
//! # Operations
//!
//! - [`checked_add`](Fraction::checked_add) for additivity across
//!   adjacent intervals (the day-count splits proptest).
//! - [`checked_mul`](Fraction::checked_mul) for composing rates with
//!   year fractions.
//! - [`cmp_cross`](Fraction::cmp_cross) and [`Ord`] for ordering and
//!   equality across alternative representations.
//! - [`parts`](Fraction::parts) for callers that want to scale an
//!   [`Amount`] with checked integer ops.
//!
//! # Storage and equality
//!
//! Fractions are stored in reduced form — every constructor divides
//! through by `gcd(|num|, den)` — so two `Fraction`s compare
//! equal via the derived `PartialEq` iff they represent the same
//! rational. `1/2` and `2/4` are the same value because they reduce
//! to the same `(num, den)` pair on construction.
//!
//! # Sign and width
//!
//! The numerator is `i64` and the denominator is `u64`. The
//! denominator is always positive; sign lives on the numerator.
//! This mirrors [`DayCount::day_count`](crate::DayCount::day_count),
//! which is signed: a reversed period produces a negative day count
//! and therefore a negative year fraction.
//!
//! `cmp_cross` and `checked_add` widen to `i128` for intermediates,
//! which always fits the product of an `i64` and a `u64` — so
//! cross-multiplication is total and the sum's intermediates never
//! spuriously overflow on inputs whose reduced result still fits.
//!
//! # Why not `num-rational`?
//!
//! `num-rational` would be a perfectly fine fit, but pulling it in
//! breaks the "runtime deps: `thiserror` only" constraint of this
//! crate. Our needs (addition, multiplication, an ordering
//! comparison) fit in ~120 lines of `i128`-widened arithmetic. If a
//! future use case outgrows this — extended-precision rationals,
//! division of fractions — the migration is a one-line dep swap.
//!
//! # No operator overloads
//!
//! Addition and multiplication can fail at the `i64`/`u64` boundary,
//! so they are exposed only as [`checked_add`](Self::checked_add) and
//! [`checked_mul`](Self::checked_mul). Callers compose with `?` or
//! handle the `None` arm explicitly.
//!
//! [`Amount`]: u128

use core::{cmp::Ordering, fmt};

use crate::TimeError;

/// An integer rational `numerator / denominator`, stored in reduced
/// form (no common factor between `|numerator|` and denominator).
///
/// The numerator is signed; the denominator is always positive.
///
/// ```
/// use fasti::Fraction;
///
/// // Reducing happens at construction.
/// let half = Fraction::new(2, 4)?;
/// assert_eq!(half.parts(), (1, 2));
///
/// // Addition on a common denominator.
/// let third = Fraction::new(1, 3)?;
/// let sum = third.checked_add(third).expect("no overflow");
/// assert_eq!(sum.parts(), (2, 3));
///
/// // Negative numerator — sign is preserved through reduction.
/// assert_eq!(Fraction::new(-30, 360)?.parts(), (-1, 12));
/// # Ok::<(), fasti::TimeError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fraction {
    num: i64,
    den: u64,
}

impl Fraction {
    /// The zero fraction, `0 / 1`.
    ///
    /// ```
    /// use fasti::Fraction;
    /// assert_eq!(Fraction::ZERO.parts(), (0, 1));
    /// assert!(Fraction::ZERO.is_zero());
    /// ```
    pub const ZERO: Self = Self { num: 0, den: 1 };

    /// Construct a `Fraction` from a signed numerator and a
    /// positive denominator, reducing by `gcd(|num|, den)`.
    ///
    /// Returns [`TimeError::ZeroDenominator`] if `den == 0`.
    ///
    /// ```
    /// use fasti::{TimeError, Fraction};
    /// assert_eq!(Fraction::new(7, 360)?.parts(), (7, 360));
    /// assert_eq!(Fraction::new(0, 5)?.parts(), (0, 1));
    /// assert_eq!(Fraction::new(2, 4)?.parts(), (1, 2));
    /// assert_eq!(Fraction::new(-30, 360)?.parts(), (-1, 12));
    /// assert_eq!(Fraction::new(7, 0), Err(TimeError::ZeroDenominator));
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    pub fn new(num: i64, den: u64) -> Result<Self, TimeError> {
        if den == 0 {
            return Err(TimeError::ZeroDenominator);
        }
        // Reduce by gcd(|num|, den). Both factors fit `u64`, so the
        // gcd helper widens to `u128` and the result fits `u64` by
        // the bound `g ≤ min(|num|, den) ≤ u64::MAX`.
        let common_u128 = gcd(u128::from(num.unsigned_abs()), u128::from(den));
        // `common ≤ u64::MAX` (proved above), so the narrowing is
        // exact.
        #[allow(clippy::cast_possible_truncation)]
        let common = common_u128 as u64;
        let reduced_den = den / common;
        // `common` divides |num| exactly; do the signed division in
        // `i128` to handle the `num == i64::MIN` edge case
        // (unsigned_abs returns 2^63, which doesn't fit `i64`, but
        // `i128` covers both signs cleanly).
        let reduced_num_i128 = i128::from(num) / i128::from(common);
        // After reduction, |reduced_num| ≤ |num| ≤ |i64::MIN|, so the
        // result always fits `i64`.
        #[allow(clippy::cast_possible_truncation)]
        let reduced_num = reduced_num_i128 as i64;
        Ok(Self {
            num: reduced_num,
            den: reduced_den,
        })
    }

    /// The signed numerator of the reduced fraction.
    #[must_use]
    pub const fn numerator(self) -> i64 {
        self.num
    }

    /// The denominator of the reduced fraction. Always non-zero.
    #[must_use]
    pub const fn denominator(self) -> u64 {
        self.den
    }

    /// `(numerator, denominator)` of the reduced fraction.
    ///
    /// Callers that need to scale an amount by this fraction should
    /// use these values with checked integer arithmetic, ordering
    /// the multiplication before the division to retain precision:
    /// `amount.checked_mul(num)?.checked_div(den)?` (with sign
    /// handling on `num`).
    #[must_use]
    pub const fn parts(self) -> (i64, u64) {
        (self.num, self.den)
    }

    /// `true` iff the fraction is exactly zero.
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.num == 0
    }

    /// `true` iff the fraction is strictly negative.
    #[must_use]
    pub const fn is_negative(self) -> bool {
        self.num < 0
    }

    /// Negate the fraction. Returns [`None`] if the numerator is
    /// `i64::MIN`, which has no positive `i64` counterpart.
    ///
    /// ```
    /// use fasti::Fraction;
    /// let pos = Fraction::new(7, 360)?;
    /// let neg = pos.checked_neg().expect("non-MIN numerator");
    /// assert_eq!(neg.parts(), (-7, 360));
    /// // Round trip.
    /// assert_eq!(neg.checked_neg().expect("non-MIN numerator"), pos);
    /// // i64::MIN cannot be negated as i64.
    /// let edge = Fraction::new(i64::MIN, 1)?;
    /// assert_eq!(edge.checked_neg(), None);
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    #[must_use]
    pub const fn checked_neg(self) -> Option<Self> {
        match self.num.checked_neg() {
            Some(num) => Some(Self { num, den: self.den }),
            None => None,
        }
    }

    /// Add two fractions, returning [`None`] if any intermediate or
    /// the reduced result does not fit in `(i64, u64)`.
    ///
    /// `a/b + c/d = (a·d + c·b) / (b·d)`. Intermediates are computed
    /// in `i128` to avoid spurious overflow on inputs whose reduced
    /// sum still fits; the final result is returned as `Some` only
    /// if both numerator and denominator fit.
    ///
    /// ```
    /// use fasti::Fraction;
    /// let a = Fraction::new(1, 2)?;
    /// let b = Fraction::new(1, 4)?;
    /// assert_eq!(
    ///     a.checked_add(b).expect("no overflow").parts(),
    ///     (3, 4),
    /// );
    /// // Adding the negation cancels.
    /// let neg = Fraction::new(-1, 2)?;
    /// assert_eq!(a.checked_add(neg), Some(Fraction::ZERO));
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    #[must_use]
    pub fn checked_add(self, other: Self) -> Option<Self> {
        let lhs_num = i128::from(self.num);
        let rhs_num = i128::from(other.num);
        // `u64` denominators always fit `i128` non-negative.
        let lhs_den = i128::from(self.den);
        let rhs_den = i128::from(other.den);
        let num = lhs_num
            .checked_mul(rhs_den)?
            .checked_add(rhs_num.checked_mul(lhs_den)?)?;
        let den = lhs_den.checked_mul(rhs_den)?;
        // `den` is positive (both factors positive); the gcd helper
        // takes `u128`, so widen via `unsigned_abs`.
        let common_unsigned = gcd(num.unsigned_abs(), den.unsigned_abs());
        // `common ≤ den ≤ i128::MAX`, so the signed conversion
        // succeeds.
        let common = i128::try_from(common_unsigned).ok()?;
        let reduced_num = num / common;
        let reduced_den = den / common;
        Some(Self {
            num: i64::try_from(reduced_num).ok()?,
            den: u64::try_from(reduced_den).ok()?,
        })
    }

    /// Multiply two fractions: `(a/b) × (c/d) = (a·c) / (b·d)`. Returns
    /// [`None`] if any intermediate or the reduced result does not fit
    /// in `(i64, u64)`.
    ///
    /// Sign of the result is the product of the input signs;
    /// denominator stays positive. Result is reduced by
    /// `gcd(|num|, den)`.
    ///
    /// ```
    /// use fasti::Fraction;
    /// // 1/2 × 1/3 = 1/6.
    /// let half = Fraction::new(1, 2)?;
    /// let third = Fraction::new(1, 3)?;
    /// assert_eq!(half.checked_mul(third).expect("fits").parts(), (1, 6));
    /// // 10% × 1/4 = (1/10) × (1/4) = 1/40 (Bps lifted as 1_000/10_000).
    /// let ten_pct = Fraction::new(1_000, 10_000)?;
    /// let quarter = Fraction::new(90, 360)?;
    /// assert_eq!(ten_pct.checked_mul(quarter).expect("fits").parts(), (1, 40));
    /// // Sign composes: negative × positive = negative.
    /// let neg_half = Fraction::new(-1, 2)?;
    /// assert_eq!(neg_half.checked_mul(third).expect("fits").parts(), (-1, 6));
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    #[must_use]
    pub fn checked_mul(self, other: Self) -> Option<Self> {
        // Widen to i128 for both factors. The product of an i64 and a
        // u64 fits i128 with room to spare; the product of two such
        // products may not fit i64/u64, so we reduce before narrowing.
        let num = i128::from(self.num).checked_mul(i128::from(other.num))?;
        let den = i128::from(self.den).checked_mul(i128::from(other.den))?;
        // `den > 0` because both factors are u64 (so non-negative)
        // and at least one was non-zero (constructor invariant).
        let common_unsigned = gcd(num.unsigned_abs(), den.unsigned_abs());
        let common = i128::try_from(common_unsigned).ok()?;
        let reduced_num = num / common;
        let reduced_den = den / common;
        Some(Self {
            num: i64::try_from(reduced_num).ok()?,
            den: u64::try_from(reduced_den).ok()?,
        })
    }

    /// Compare two fractions by cross-multiplication.
    ///
    /// `a/b ?= c/d` becomes `a·d ?= c·b`. Both products are widened
    /// to `i128`, which always fits the product of an `i64` and a
    /// `u64` operand, so the comparison is total and never overflows.
    /// The denominator is positive on both sides, so cross-multiplication
    /// preserves the sign of the comparison.
    ///
    /// ```
    /// use core::cmp::Ordering;
    /// use fasti::Fraction;
    /// let third = Fraction::new(1, 3)?;
    /// let half = Fraction::new(1, 2)?;
    /// let neg_third = Fraction::new(-1, 3)?;
    /// assert_eq!(third.cmp_cross(half), Ordering::Less);
    /// assert_eq!(half.cmp_cross(half), Ordering::Equal);
    /// assert_eq!(neg_third.cmp_cross(third), Ordering::Less);
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    #[must_use]
    pub fn cmp_cross(self, other: Self) -> Ordering {
        let lhs = i128::from(self.num) * i128::from(other.den);
        let rhs = i128::from(other.num) * i128::from(self.den);
        lhs.cmp(&rhs)
    }
}

impl Default for Fraction {
    /// `Default` is [`Self::ZERO`], which lets day-count code use
    /// `Fraction::new(...).unwrap_or_default()` to fall back on
    /// the zero fraction when the constructor's only failure
    /// (`ZeroDenominator`) is unreachable in context.
    fn default() -> Self {
        Self::ZERO
    }
}

impl PartialOrd for Fraction {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Fraction {
    fn cmp(&self, other: &Self) -> Ordering {
        self.cmp_cross(*other)
    }
}

impl fmt::Display for Fraction {
    /// Formats as `numerator/denominator` in reduced form. A
    /// negative numerator displays with a leading `-`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.num, self.den)
    }
}

impl From<Fraction> for (i64, u64) {
    fn from(yf: Fraction) -> Self {
        yf.parts()
    }
}

// ---- helpers ------------------------------------------------------------

/// Greatest common divisor via Euclid's algorithm. `gcd(0, x) = x`
/// and `gcd(x, 0) = x`, which makes [`Fraction::new`]`(0, _)`
/// reduce to `(0, 1)` as expected.
///
/// Defined once over `u128` and reused for every call site by
/// widening — keeping a single implementation avoids drift between
/// per-width copies.
const fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

// ---- Tests --------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    extern crate alloc;

    use super::*;
    use proptest::prelude::*;

    #[test]
    fn rejects_zero_denominator() {
        assert_eq!(Fraction::new(0, 0), Err(TimeError::ZeroDenominator));
        assert_eq!(Fraction::new(7, 0), Err(TimeError::ZeroDenominator));
        assert_eq!(Fraction::new(-7, 0), Err(TimeError::ZeroDenominator));
    }

    #[test]
    fn reduces_on_construction() {
        assert_eq!(Fraction::new(2, 4).unwrap().parts(), (1, 2));
        assert_eq!(Fraction::new(0, 5).unwrap().parts(), (0, 1));
        assert_eq!(Fraction::new(7, 360).unwrap().parts(), (7, 360));
        assert_eq!(Fraction::new(360, 360).unwrap().parts(), (1, 1));
    }

    #[test]
    fn negative_numerator_round_trips() {
        // Sign lives on the numerator; the denominator stays positive.
        assert_eq!(Fraction::new(-30, 360).unwrap().parts(), (-1, 12));
        assert_eq!(Fraction::new(-7, 360).unwrap().parts(), (-7, 360));
    }

    #[test]
    fn negation_through_construction() {
        let pos = Fraction::new(7, 360).unwrap();
        let neg = Fraction::new(-7, 360).unwrap();
        assert_eq!(pos.numerator(), 7);
        assert_eq!(neg.numerator(), -7);
        assert_eq!(pos.denominator(), neg.denominator());
    }

    #[test]
    fn already_reduced_is_no_op() {
        let f = Fraction::new(7, 360).unwrap();
        let again = Fraction::new(f.numerator(), f.denominator()).unwrap();
        assert_eq!(f, again);
    }

    #[test]
    fn zero_constant() {
        assert_eq!(Fraction::ZERO, Fraction::new(0, 1).unwrap());
        assert_eq!(Fraction::ZERO, Fraction::new(0, 12345).unwrap());
        assert!(Fraction::ZERO.is_zero());
    }

    #[test]
    fn default_is_zero() {
        assert_eq!(Fraction::default(), Fraction::ZERO);
        // Critical for the `Result::unwrap_or_default` pattern in
        // `DayCount` impls — `Default` must agree with `ZERO`, not
        // the derived `(0, 0)` field default which would have a
        // zero denominator.
        let parts = Fraction::default().parts();
        assert_eq!(parts, (0, 1));
        assert_ne!(parts.1, 0);
    }

    #[test]
    fn is_negative_examples() {
        assert!(!Fraction::ZERO.is_negative());
        assert!(!Fraction::new(1, 2).unwrap().is_negative());
        assert!(Fraction::new(-1, 2).unwrap().is_negative());
    }

    #[test]
    fn unreduced_inputs_compare_equal() {
        // (1, 2) and (2, 4) reduce to the same canonical form.
        let half = Fraction::new(1, 2).unwrap();
        let two_quarters = Fraction::new(2, 4).unwrap();
        assert_eq!(half, two_quarters);
        assert_eq!(half.cmp_cross(two_quarters), Ordering::Equal);
    }

    #[test]
    fn cmp_cross_orders_correctly() {
        let third = Fraction::new(1, 3).unwrap();
        let half = Fraction::new(1, 2).unwrap();
        let two_thirds = Fraction::new(2, 3).unwrap();
        let neg_third = Fraction::new(-1, 3).unwrap();
        assert_eq!(third.cmp_cross(half), Ordering::Less);
        assert_eq!(half.cmp_cross(third), Ordering::Greater);
        assert_eq!(half.cmp_cross(two_thirds), Ordering::Less);
        assert_eq!(neg_third.cmp_cross(third), Ordering::Less);
        assert_eq!(neg_third.cmp_cross(Fraction::ZERO), Ordering::Less);
    }

    #[test]
    fn checked_add_basic_examples() {
        let third = Fraction::new(1, 3).unwrap();
        let two_thirds = third.checked_add(third).unwrap();
        assert_eq!(two_thirds.parts(), (2, 3));

        let a = Fraction::new(1, 2).unwrap();
        let b = Fraction::new(1, 4).unwrap();
        assert_eq!(a.checked_add(b).unwrap().parts(), (3, 4));
    }

    #[test]
    fn checked_add_zero_is_identity() {
        let f = Fraction::new(7, 360).unwrap();
        assert_eq!(f.checked_add(Fraction::ZERO), Some(f));
        assert_eq!(Fraction::ZERO.checked_add(f), Some(f));
    }

    #[test]
    fn checked_add_with_negation_cancels() {
        let f = Fraction::new(7, 360).unwrap();
        let neg_f = Fraction::new(-7, 360).unwrap();
        assert_eq!(f.checked_add(neg_f), Some(Fraction::ZERO));
    }

    #[test]
    fn checked_add_mixed_signs() {
        // 3/4 + (-1/2) = 1/4.
        let three_quarters = Fraction::new(3, 4).unwrap();
        let neg_half = Fraction::new(-1, 2).unwrap();
        assert_eq!(
            three_quarters.checked_add(neg_half).unwrap().parts(),
            (1, 4),
        );
    }

    #[test]
    fn checked_add_overflows_when_result_exceeds_i64() {
        // (i64::MAX / 2 + 1, 1) + (i64::MAX / 2 + 1, 1) — the reduced
        // numerator of the sum exceeds i64::MAX.
        let half_max = i64::MAX / 2 + 1;
        let a = Fraction::new(half_max, 1).unwrap();
        let b = Fraction::new(half_max, 1).unwrap();
        assert_eq!(a.checked_add(b), None);
    }

    #[test]
    fn display_renders_reduced_form() {
        assert_eq!(alloc::format!("{}", Fraction::new(2, 4).unwrap()), "1/2");
        assert_eq!(alloc::format!("{}", Fraction::ZERO), "0/1");
        assert_eq!(
            alloc::format!("{}", Fraction::new(7, 360).unwrap()),
            "7/360",
        );
        assert_eq!(
            alloc::format!("{}", Fraction::new(-30, 360).unwrap()),
            "-1/12",
        );
    }

    #[test]
    fn into_tuple_round_trips() {
        let f = Fraction::new(7, 360).unwrap();
        let parts: (i64, u64) = f.into();
        assert_eq!(parts, (7, 360));
        let neg = Fraction::new(-7, 360).unwrap();
        let neg_parts: (i64, u64) = neg.into();
        assert_eq!(neg_parts, (-7, 360));
    }

    #[test]
    fn ord_consistent_with_cmp_cross() {
        let a = Fraction::new(1, 3).unwrap();
        let b = Fraction::new(1, 2).unwrap();
        let c = Fraction::new(-1, 2).unwrap();
        assert!(a < b);
        assert!(b > a);
        assert!(c < a);
        assert_eq!(a.cmp(&a), Ordering::Equal);
    }

    #[test]
    fn checked_neg_round_trip() {
        let pos = Fraction::new(7, 360).unwrap();
        let neg = pos.checked_neg().unwrap();
        assert_eq!(neg.parts(), (-7, 360));
        assert_eq!(neg.checked_neg().unwrap(), pos);
        // ZERO negates to ZERO.
        assert_eq!(Fraction::ZERO.checked_neg(), Some(Fraction::ZERO));
    }

    #[test]
    fn checked_neg_returns_none_at_i64_min() {
        let edge = Fraction::new(i64::MIN, 1).unwrap();
        assert_eq!(edge.checked_neg(), None);
    }

    #[test]
    fn handles_i64_min_numerator() {
        // i64::MIN.unsigned_abs() = 2^63 doesn't fit i64; the
        // construction must still produce a valid reduced form.
        let yf = Fraction::new(i64::MIN, 1).unwrap();
        assert_eq!(yf.parts(), (i64::MIN, 1));
        // With a non-trivial gcd:
        let yf = Fraction::new(i64::MIN, 2).unwrap();
        // 2^63 / 2 = 2^62 = i64::MAX/2 + 1; reduced (-2^62, 1).
        assert_eq!(yf.parts(), (i64::MIN / 2, 1));
    }

    // ---- property tests ------------------------------------------------

    proptest! {
        /// New always produces a reduced form: feeding the result
        /// back through `new` is a no-op.
        #[test]
        fn new_is_idempotent_on_reduced_inputs(
            num in -10_000i64..=10_000,
            den in 1u64..=10_000,
        ) {
            let once = Fraction::new(num, den).unwrap();
            let twice = Fraction::new(once.numerator(), once.denominator()).unwrap();
            prop_assert_eq!(once, twice);
        }

        /// Equality matches cross-multiplication: `a == b` iff
        /// `a.cmp_cross(b) == Equal`.
        #[test]
        fn equality_matches_cross_multiplication(
            n1 in -10_000i64..=10_000, d1 in 1u64..=10_000,
            n2 in -10_000i64..=10_000, d2 in 1u64..=10_000,
        ) {
            let a = Fraction::new(n1, d1).unwrap();
            let b = Fraction::new(n2, d2).unwrap();
            prop_assert_eq!(a == b, a.cmp_cross(b) == Ordering::Equal);
        }

        /// Ord total ordering matches cross-multiplication.
        #[test]
        fn ord_matches_cross_multiplication(
            n1 in -10_000i64..=10_000, d1 in 1u64..=10_000,
            n2 in -10_000i64..=10_000, d2 in 1u64..=10_000,
        ) {
            let a = Fraction::new(n1, d1).unwrap();
            let b = Fraction::new(n2, d2).unwrap();
            prop_assert_eq!(a.cmp(&b), a.cmp_cross(b));
        }

        /// Adding zero is the identity.
        #[test]
        fn add_zero_identity(num in -10_000i64..=10_000, den in 1u64..=10_000) {
            let f = Fraction::new(num, den).unwrap();
            prop_assert_eq!(f.checked_add(Fraction::ZERO), Some(f));
            prop_assert_eq!(Fraction::ZERO.checked_add(f), Some(f));
        }

        /// A fraction plus its negation is zero.
        #[test]
        fn add_negation_cancels(num in -10_000i64..=10_000, den in 1u64..=10_000) {
            let f = Fraction::new(num, den).unwrap();
            let neg = Fraction::new(-num, den).unwrap();
            prop_assert_eq!(f.checked_add(neg), Some(Fraction::ZERO));
        }

        /// Addition is commutative whenever it is defined.
        #[test]
        fn add_is_commutative(
            n1 in -10_000i64..=10_000, d1 in 1u64..=10_000,
            n2 in -10_000i64..=10_000, d2 in 1u64..=10_000,
        ) {
            let a = Fraction::new(n1, d1).unwrap();
            let b = Fraction::new(n2, d2).unwrap();
            prop_assert_eq!(a.checked_add(b), b.checked_add(a));
        }

        /// Addition is associative whenever every intermediate
        /// addition succeeds. Smaller bounds keep the triple-product
        /// intermediates within the i128 scratch space when none of
        /// the partial sums alone would overflow.
        #[test]
        fn add_is_associative(
            n1 in -200i64..=200, d1 in 1u64..=200,
            n2 in -200i64..=200, d2 in 1u64..=200,
            n3 in -200i64..=200, d3 in 1u64..=200,
        ) {
            let a = Fraction::new(n1, d1).unwrap();
            let b = Fraction::new(n2, d2).unwrap();
            let c = Fraction::new(n3, d3).unwrap();
            let lhs = a.checked_add(b).and_then(|r| r.checked_add(c));
            let rhs = b.checked_add(c).and_then(|r| a.checked_add(r));
            prop_assert_eq!(lhs, rhs);
        }
    }
}
