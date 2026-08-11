//! [`Weekend`]: a bitmask over the seven weekdays.
//!
//! Most markets run a Saturday/Sunday weekend; traditional Gulf markets
//! observed Friday/Saturday; some markets have a single weekend day or
//! none. A [`Weekend`] captures any of these configurations as a 7-bit
//! mask that is `Copy` and const-constructible.

use crate::Weekday;

/// The set of weekdays a calendar treats as non-business days on
/// weekly cadence (independent of any holiday rule). Bit layout: bit
/// `(w.get() - 1)` is set iff `w` is a weekend day.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct Weekend(u8);

impl Weekend {
    const fn bit(w: Weekday) -> u8 {
        1u8 << (w.get() - 1)
    }

    /// No weekend — every weekday is a business day. Useful for markets
    /// that run seven days a week.
    pub const NONE: Self = Self(0);

    /// Saturday and Sunday — the default for most world markets.
    pub const SAT_SUN: Self = Self(Self::bit(Weekday::Sat) | Self::bit(Weekday::Sun));

    /// Friday and Saturday — historical Gulf-market configuration.
    pub const FRI_SAT: Self = Self(Self::bit(Weekday::Fri) | Self::bit(Weekday::Sat));

    /// Sunday only.
    pub const SUN_ONLY: Self = Self(Self::bit(Weekday::Sun));

    /// `true` iff `w` is a weekend day under this configuration.
    ///
    /// ```
    /// use fasti::{Weekday, Weekend};
    /// assert!(Weekend::SAT_SUN.contains(Weekday::Sat));
    /// assert!(Weekend::SAT_SUN.contains(Weekday::Sun));
    /// assert!(!Weekend::SAT_SUN.contains(Weekday::Mon));
    /// ```
    #[must_use]
    pub const fn contains(self, w: Weekday) -> bool {
        self.0 & Self::bit(w) != 0
    }

    /// Construct a weekend from an explicit set of weekdays.
    ///
    /// ```
    /// use fasti::{Weekday, Weekend};
    /// const MY_WEEKEND: Weekend = Weekend::from_weekdays(&[Weekday::Sun]);
    /// assert!(MY_WEEKEND.contains(Weekday::Sun));
    /// assert!(!MY_WEEKEND.contains(Weekday::Sat));
    /// ```
    #[must_use]
    pub const fn from_weekdays(weekdays: &[Weekday]) -> Self {
        let mut mask = 0u8;
        let mut i = 0;
        while i < weekdays.len() {
            mask |= Self::bit(weekdays[i]);
            i += 1;
        }
        Self(mask)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn sat_sun_contains_only_sat_and_sun() {
        for w in [
            Weekday::Mon,
            Weekday::Tue,
            Weekday::Wed,
            Weekday::Thu,
            Weekday::Fri,
        ] {
            assert!(!Weekend::SAT_SUN.contains(w), "{w:?} should not be weekend");
        }
        assert!(Weekend::SAT_SUN.contains(Weekday::Sat));
        assert!(Weekend::SAT_SUN.contains(Weekday::Sun));
    }

    #[test]
    fn fri_sat_configuration() {
        assert!(Weekend::FRI_SAT.contains(Weekday::Fri));
        assert!(Weekend::FRI_SAT.contains(Weekday::Sat));
        assert!(!Weekend::FRI_SAT.contains(Weekday::Sun));
    }

    #[test]
    fn none_configuration() {
        for w in [
            Weekday::Mon,
            Weekday::Tue,
            Weekday::Wed,
            Weekday::Thu,
            Weekday::Fri,
            Weekday::Sat,
            Weekday::Sun,
        ] {
            assert!(!Weekend::NONE.contains(w));
        }
    }

    #[test]
    fn from_weekdays_matches_explicit_constructions() {
        const SAT_SUN_VIA_ARRAY: Weekend = Weekend::from_weekdays(&[Weekday::Sat, Weekday::Sun]);
        assert_eq!(SAT_SUN_VIA_ARRAY, Weekend::SAT_SUN);
    }
}
