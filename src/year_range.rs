//! [`YearRange`]: the subset of years during which a holiday rule applies.
//!
//! Many holidays are defined only for certain years — US Juneteenth has
//! been observed federally only since 2021, MLK Day since 1986. A
//! [`YearRange`] captures that "active between year X and year Y"
//! constraint in a single `Copy` type with `const` constructors.
//!
//! Both bounds are concrete [`Year`]s (no `Option`). The crate already
//! fixes a hard upper and lower bound at [`Year::MIN`] and [`Year::MAX`],
//! so "unbounded above" collapses to `to = Year::MAX`, and "unbounded
//! below" collapses to `from = Year::MIN`. Using concrete bounds in both
//! directions is symmetric, keeps the struct to two `u16`s, and avoids
//! a phantom distinction between `Some(Year::MAX)` and `None`.

use crate::{TimeError, Year};

/// An inclusive range of [`Year`]s.
///
/// [`YearRange::ALWAYS`] covers every supported year;
/// [`YearRange::from_year`] covers `from..=Year::MAX`;
/// [`YearRange::through`] covers `Year::MIN..=to`;
/// [`YearRange::try_between`] covers an explicit `from..=to`.
///
/// ```
/// use fasti::{Year, YearRange};
/// let juneteenth = YearRange::from_year(Year::new(2021)?);
/// assert!(!juneteenth.contains(Year::new(2020)?));
/// assert!(juneteenth.contains(Year::new(2021)?));
/// assert!(juneteenth.contains(Year::MAX));
/// # Ok::<(), fasti::TimeError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct YearRange {
    from: Year,
    to: Year,
}

impl YearRange {
    /// A range covering every supported year, [`Year::MIN`]..=[`Year::MAX`].
    pub const ALWAYS: Self = Self {
        from: Year::MIN,
        to: Year::MAX,
    };

    /// A range covering `from..=Year::MAX`.
    ///
    /// ```
    /// use fasti::{Year, YearRange};
    /// let r = YearRange::from_year(Year::new(1986)?);
    /// assert_eq!(r.start(), Year::new(1986)?);
    /// assert_eq!(r.end(), Year::MAX);
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    #[must_use]
    pub const fn from_year(from: Year) -> Self {
        Self {
            from,
            to: Year::MAX,
        }
    }

    /// A range covering `Year::MIN..=to`.
    ///
    /// ```
    /// use fasti::{Year, YearRange};
    /// let r = YearRange::through(Year::new(2020)?);
    /// assert_eq!(r.start(), Year::MIN);
    /// assert!(r.contains(Year::new(2020)?));
    /// assert!(!r.contains(Year::new(2021)?));
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    #[must_use]
    pub const fn through(to: Year) -> Self {
        Self {
            from: Year::MIN,
            to,
        }
    }

    /// An inclusive range `from..=to`. Returns
    /// [`TimeError::InvalidYearRange`] if `to < from`.
    ///
    /// ```
    /// use fasti::{Year, YearRange};
    /// let r = YearRange::try_between(Year::new(1986)?, Year::new(2020)?)?;
    /// assert!(r.contains(Year::new(1986)?));
    /// assert!(r.contains(Year::new(2020)?));
    /// assert!(!r.contains(Year::new(2021)?));
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    pub const fn try_between(from: Year, to: Year) -> Result<Self, TimeError> {
        if to.get() < from.get() {
            Err(TimeError::InvalidYearRange)
        } else {
            Ok(Self { from, to })
        }
    }

    /// `Year::MIN..=to` from a compile-time literal. Companion to
    /// [`through`](Self::through) for use in `const` contexts.
    ///
    /// Validated at const-eval: an out-of-range `to` becomes a compile
    /// error via [`Year::literal`]. Do not call with a runtime value.
    ///
    /// ```
    /// use fasti::YearRange;
    /// const PRE_1971: YearRange = YearRange::literal_through(1970);
    /// ```
    #[must_use]
    pub const fn literal_through(to: u16) -> Self {
        Self {
            from: Year::MIN,
            to: Year::literal(to),
        }
    }

    /// Inclusive `from..=to` range from compile-time literals.
    /// Companion to [`try_between`](Self::try_between) for use in
    /// `const` contexts. Compile error if `to < from` or if either
    /// bound is outside the supported 1901..=2199 range.
    ///
    /// Like [`Year::literal`] this is intended for compile-time use
    /// only; calling with runtime values that violate the invariants
    /// would `assert!`-panic. All call sites in the crate pass
    /// compile-time constants.
    ///
    /// ```
    /// use fasti::YearRange;
    /// const INTERIM: YearRange = YearRange::literal_between(1971, 1977);
    /// ```
    ///
    /// ```compile_fail
    /// use fasti::YearRange;
    /// // Compile error: to < from.
    /// const BAD: YearRange = YearRange::literal_between(2020, 2019);
    /// ```
    #[must_use]
    #[allow(clippy::panic)]
    pub const fn literal_between(from: u16, to: u16) -> Self {
        let from = Year::literal(from);
        let to = Year::literal(to);
        // `assert!` panic surfaces as a compile error at const-eval
        // for bad literal inputs. Runtime callers violate the intended
        // use — see the method doc.
        assert!(
            to.get() >= from.get(),
            "YearRange::literal_between: to < from",
        );
        Self { from, to }
    }

    /// The lower bound (inclusive).
    #[must_use]
    pub const fn start(self) -> Year {
        self.from
    }

    /// The upper bound (inclusive).
    #[must_use]
    pub const fn end(self) -> Year {
        self.to
    }

    /// `true` iff `year` falls inside this range.
    #[must_use]
    pub const fn contains(&self, year: Year) -> bool {
        year.get() >= self.from.get() && year.get() <= self.to.get()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn always_covers_full_range() {
        let r = YearRange::ALWAYS;
        assert!(r.contains(Year::MIN));
        assert!(r.contains(Year::MAX));
    }

    #[test]
    fn from_year_is_unbounded_above() {
        let r = YearRange::from_year(Year::new(2021).unwrap());
        assert!(!r.contains(Year::new(2020).unwrap()));
        assert!(r.contains(Year::new(2021).unwrap()));
        assert!(r.contains(Year::MAX));
    }

    #[test]
    fn through_is_unbounded_below() {
        let r = YearRange::through(Year::new(2020).unwrap());
        assert!(r.contains(Year::MIN));
        assert!(r.contains(Year::new(2020).unwrap()));
        assert!(!r.contains(Year::new(2021).unwrap()));
    }

    #[test]
    fn try_between_enforces_ordering() {
        assert_eq!(
            YearRange::try_between(Year::new(2020).unwrap(), Year::new(2019).unwrap()),
            Err(TimeError::InvalidYearRange),
        );
        let r = YearRange::try_between(Year::new(2020).unwrap(), Year::new(2020).unwrap()).unwrap();
        assert!(r.contains(Year::new(2020).unwrap()));
        assert!(!r.contains(Year::new(2019).unwrap()));
        assert!(!r.contains(Year::new(2021).unwrap()));
    }
}
