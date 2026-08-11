//! Easter-Monday lookup tables: two 299-entry `[u8; 299]` arrays (Western
//! and Orthodox), indexed by `year - 1901`, storing the day-of-year of
//! Easter Monday.
//!
//! The tables are ported from
//! [QuantLib's `ql/time/calendar.cpp`](https://github.com/lballabio/QuantLib/blob/master/ql/time/calendar.cpp).
//! `QuantLib` is distributed under a permissive modified-BSD license; its
//! copyright notice and license text are reproduced in the repository's
//! `THIRD-PARTY-NOTICES` file. Every entry is validated against the
//! in-crate `#[cfg(test)]` computus implementations.

use crate::Year;

const EPOCH_YEAR: u16 = 1901;

/// Which computus to use for Easter dates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum EasterMethod {
    /// Gregorian computus — the Easter observed by Western churches.
    Western,
    /// Julian computus — the Easter observed by Orthodox churches,
    /// expressed in the Gregorian calendar.
    Orthodox,
}

/// Day-of-year of Easter Monday for `year` under `method`.
///
/// The returned value is `1`-indexed (January 1 is day 1), matching the
/// convention of [`Date::day_of_year`](crate::Date::day_of_year).
///
/// ```
/// use fasti::{easter_monday, EasterMethod, Year};
/// // 2024: Western Easter Sunday = March 31, so Easter Monday = April 1.
/// // Day-of-year of April 1 in leap year 2024 = 92.
/// assert_eq!(easter_monday(Year::new(2024)?, EasterMethod::Western), 92);
/// # Ok::<(), fasti::TimeError>(())
/// ```
#[must_use]
pub const fn easter_monday(year: Year, method: EasterMethod) -> u16 {
    let idx = (year.get() - EPOCH_YEAR) as usize;
    let raw = match method {
        EasterMethod::Western => WESTERN_EASTER_MONDAY[idx],
        EasterMethod::Orthodox => ORTHODOX_EASTER_MONDAY[idx],
    };
    raw as u16
}

/// Day-of-year of Easter Sunday for `year` under `method`. This is
/// [`easter_monday`] minus one.
#[must_use]
pub const fn easter_sunday(year: Year, method: EasterMethod) -> u16 {
    easter_monday(year, method) - 1
}

// ---- Lookup tables ------------------------------------------------------
//
// Ported verbatim from QuantLib ql/time/calendar.cpp; row layout matches
// upstream (first row 9 entries, then 10 per row). Index = year - 1901.

#[rustfmt::skip]
static WESTERN_EASTER_MONDAY: [u8; 299] = [
              98,  90, 103,  95, 114, 106,  91, 111, 102, // 1901-1909
     87, 107,  99,  83, 103,  95, 115,  99,  91, 111, // 1910-1919
     96,  87, 107,  92, 112, 103,  95, 108, 100,  91, // 1920-1929
    111,  96,  88, 107,  92, 112, 104,  88, 108, 100, // 1930-1939
     85, 104,  96, 116, 101,  92, 112,  97,  89, 108, // 1940-1949
    100,  85, 105,  96, 109, 101,  93, 112,  97,  89, // 1950-1959
    109,  93, 113, 105,  90, 109, 101,  86, 106,  97, // 1960-1969
     89, 102,  94, 113, 105,  90, 110, 101,  86, 106, // 1970-1979
     98, 110, 102,  94, 114,  98,  90, 110,  95,  86, // 1980-1989
    106,  91, 111, 102,  94, 107,  99,  90, 103,  95, // 1990-1999
    115, 106,  91, 111, 103,  87, 107,  99,  84, 103, // 2000-2009
     95, 115, 100,  91, 111,  96,  88, 107,  92, 112, // 2010-2019
    104,  95, 108, 100,  92, 111,  96,  88, 108,  92, // 2020-2029
    112, 104,  89, 108, 100,  85, 105,  96, 116, 101, // 2030-2039
     93, 112,  97,  89, 109, 100,  85, 105,  97, 109, // 2040-2049
    101,  93, 113,  97,  89, 109,  94, 113, 105,  90, // 2050-2059
    110, 101,  86, 106,  98,  89, 102,  94, 114, 105, // 2060-2069
     90, 110, 102,  86, 106,  98, 111, 102,  94, 114, // 2070-2079
     99,  90, 110,  95,  87, 106,  91, 111, 103,  94, // 2080-2089
    107,  99,  91, 103,  95, 115, 107,  91, 111, 103, // 2090-2099
     88, 108, 100,  85, 105,  96, 109, 101,  93, 112, // 2100-2109
     97,  89, 109,  93, 113, 105,  90, 109, 101,  86, // 2110-2119
    106,  97,  89, 102,  94, 113, 105,  90, 110, 101, // 2120-2129
     86, 106,  98, 110, 102,  94, 114,  98,  90, 110, // 2130-2139
     95,  86, 106,  91, 111, 102,  94, 107,  99,  90, // 2140-2149
    103,  95, 115, 106,  91, 111, 103,  87, 107,  99, // 2150-2159
     84, 103,  95, 115, 100,  91, 111,  96,  88, 107, // 2160-2169
     92, 112, 104,  95, 108, 100,  92, 111,  96,  88, // 2170-2179
    108,  92, 112, 104,  89, 108, 100,  85, 105,  96, // 2180-2189
    116, 101,  93, 112,  97,  89, 109, 100,  85, 105, // 2190-2199
];

#[rustfmt::skip]
static ORTHODOX_EASTER_MONDAY: [u8; 299] = [
             105, 118, 110, 102, 121, 106, 126, 118, 102, // 1901-1909
    122, 114,  99, 118, 110,  95, 115, 106, 126, 111, // 1910-1919
    103, 122, 107,  99, 119, 110, 123, 115, 107, 126, // 1920-1929
    111, 103, 123, 107,  99, 119, 104, 123, 115, 100, // 1930-1939
    120, 111,  96, 116, 108, 127, 112, 104, 124, 115, // 1940-1949
    100, 120, 112,  96, 116, 108, 128, 112, 104, 124, // 1950-1959
    109, 100, 120, 105, 125, 116, 101, 121, 113, 104, // 1960-1969
    117, 109, 101, 120, 105, 125, 117, 101, 121, 113, // 1970-1979
     98, 117, 109, 129, 114, 105, 125, 110, 102, 121, // 1980-1989
    106,  98, 118, 109, 122, 114, 106, 118, 110, 102, // 1990-1999
    122, 106, 126, 118, 103, 122, 114,  99, 119, 110, // 2000-2009
     95, 115, 107, 126, 111, 103, 123, 107,  99, 119, // 2010-2019
    111, 123, 115, 107, 127, 111, 103, 123, 108,  99, // 2020-2029
    119, 104, 124, 115, 100, 120, 112,  96, 116, 108, // 2030-2039
    128, 112, 104, 124, 116, 100, 120, 112,  97, 116, // 2040-2049
    108, 128, 113, 104, 124, 109, 101, 120, 105, 125, // 2050-2059
    117, 101, 121, 113, 105, 117, 109, 101, 121, 105, // 2060-2069
    125, 110, 102, 121, 113,  98, 118, 109, 129, 114, // 2070-2079
    106, 125, 110, 102, 122, 106,  98, 118, 110, 122, // 2080-2089
    114,  99, 119, 110, 102, 115, 107, 126, 118, 103, // 2090-2099
    123, 115, 100, 120, 112,  96, 116, 108, 128, 112, // 2100-2109
    104, 124, 109, 100, 120, 105, 125, 116, 108, 121, // 2110-2119
    113, 104, 124, 109, 101, 120, 105, 125, 117, 101, // 2120-2129
    121, 113,  98, 117, 109, 129, 114, 105, 125, 110, // 2130-2139
    102, 121, 113,  98, 118, 109, 129, 114, 106, 125, // 2140-2149
    110, 102, 122, 106, 126, 118, 103, 122, 114,  99, // 2150-2159
    119, 110, 102, 115, 107, 126, 111, 103, 123, 114, // 2160-2169
     99, 119, 111, 130, 115, 107, 127, 111, 103, 123, // 2170-2179
    108,  99, 119, 104, 124, 115, 100, 120, 112, 103, // 2180-2189
    116, 108, 128, 119, 104, 124, 116, 100, 120, 112, // 2190-2199
];

// ---- Tests --------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::{Date, Month, Weekday};

    /// Published Western Easter Monday dates used as the test oracle;
    /// each row's comment gives the preceding Easter Sunday.
    const WESTERN_ANCHORS: &[(u16, Month, u8)] = &[
        (1901, Month::Apr, 8),  // Sunday 1901-04-07
        (1913, Month::Mar, 24), // Sunday 1913-03-23 — early Easter
        (1943, Month::Apr, 26), // Sunday 1943-04-25 — late Easter
        (1950, Month::Apr, 10), // Sunday 1950-04-09
        (1976, Month::Apr, 19), // Sunday 1976-04-18 — leap year
        (2000, Month::Apr, 24), // Sunday 2000-04-23 — millennium, leap
        (2008, Month::Mar, 24), // Sunday 2008-03-23 — early, leap
        (2020, Month::Apr, 13), // Sunday 2020-04-12 — leap
        (2024, Month::Apr, 1),  // Sunday 2024-03-31 — leap
        (2025, Month::Apr, 21), // Sunday 2025-04-20 — coincides with Orthodox
        (2038, Month::Apr, 26), // Sunday 2038-04-25 — late
        (2100, Month::Mar, 29), // Sunday 2100-03-28 — century-shift, non-leap (per computus)
        (2199, Month::Apr, 15), // Sunday 2199-04-14 — far future (per computus)
    ];

    /// Published Orthodox Easter Monday dates (Gregorian calendar).
    const ORTHODOX_ANCHORS: &[(u16, Month, u8)] = &[
        (1901, Month::Apr, 15), // Orthodox Sunday 1901-04-14
        (2000, Month::May, 1),  // Orthodox Sunday 2000-04-30 — leap
        (2024, Month::May, 6),  // Orthodox Sunday 2024-05-05 — leap
        (2025, Month::Apr, 21), // Orthodox Sunday 2025-04-20 — coincides with Western
    ];

    fn check_anchors(method: EasterMethod, anchors: &[(u16, Month, u8)]) {
        for &(year, month, day) in anchors {
            let expected = Date::from_ymd(year, month, day).unwrap();
            let actual_doy = easter_monday(Year::new(year).unwrap(), method);
            let actual = Date::from_ymd(year, Month::Jan, 1)
                .unwrap()
                .add_days(i32::from(actual_doy) - 1)
                .unwrap();
            assert_eq!(
                actual, expected,
                "{method:?} Easter Monday {year}: expected {expected}, got {actual}",
            );
        }
    }

    #[test]
    fn western_table_matches_published_anchor_dates() {
        check_anchors(EasterMethod::Western, WESTERN_ANCHORS);
    }

    #[test]
    fn orthodox_table_matches_published_anchor_dates() {
        check_anchors(EasterMethod::Orthodox, ORTHODOX_ANCHORS);
    }

    /// Every Western entry must decode to a Monday — catches any
    /// transcription typo whose magnitude is not a multiple of seven.
    #[test]
    fn every_western_entry_lands_on_a_monday() {
        for year in 1901u16..=2199 {
            let doy = easter_monday(Year::new(year).unwrap(), EasterMethod::Western);
            let date = Date::from_ymd(year, Month::Jan, 1)
                .unwrap()
                .add_days(i32::from(doy) - 1)
                .unwrap();
            assert_eq!(
                date.weekday(),
                Weekday::Mon,
                "Western Easter Monday {year} (doy {doy}) landed on {:?}",
                date.weekday(),
            );
        }
    }

    /// Same structural invariant for Orthodox.
    #[test]
    fn every_orthodox_entry_lands_on_a_monday() {
        for year in 1901u16..=2199 {
            let doy = easter_monday(Year::new(year).unwrap(), EasterMethod::Orthodox);
            let date = Date::from_ymd(year, Month::Jan, 1)
                .unwrap()
                .add_days(i32::from(doy) - 1)
                .unwrap();
            assert_eq!(
                date.weekday(),
                Weekday::Mon,
                "Orthodox Easter Monday {year} (doy {doy}) landed on {:?}",
                date.weekday(),
            );
        }
    }

    /// Every Western entry must decode to a date in March or April —
    /// the only months Gregorian Easter can fall in.
    #[test]
    fn every_western_entry_falls_in_mar_or_apr() {
        for year in 1901u16..=2199 {
            let doy = easter_monday(Year::new(year).unwrap(), EasterMethod::Western);
            let date = Date::from_ymd(year, Month::Jan, 1)
                .unwrap()
                .add_days(i32::from(doy) - 1)
                .unwrap();
            assert!(
                matches!(date.month(), Month::Mar | Month::Apr),
                "Western Easter Monday {year} in unexpected month {:?}",
                date.month(),
            );
        }
    }

    /// Orthodox Easter (in Gregorian) can fall in April or May only.
    #[test]
    fn every_orthodox_entry_falls_in_apr_or_may() {
        for year in 1901u16..=2199 {
            let doy = easter_monday(Year::new(year).unwrap(), EasterMethod::Orthodox);
            let date = Date::from_ymd(year, Month::Jan, 1)
                .unwrap()
                .add_days(i32::from(doy) - 1)
                .unwrap();
            assert!(
                matches!(date.month(), Month::Apr | Month::May),
                "Orthodox Easter Monday {year} in unexpected month {:?}",
                date.month(),
            );
        }
    }

    /// Independent Anonymous/Meeus Gregorian computus; returns Easter
    /// *Sunday* as `(month, day)`.
    ///
    /// Single-letter variables are the algorithm's canonical notation,
    /// hence the lint allow.
    #[allow(clippy::many_single_char_names)]
    fn gregorian_computus_easter_sunday(year: u16) -> (u8, u8) {
        let y = i32::from(year);
        let a = y % 19;
        let b = y / 100;
        let c = y % 100;
        let d = b / 4;
        let e = b % 4;
        let f = (b + 8) / 25;
        let g = (b - f + 1) / 3;
        let h = (19 * a + b - d - g + 15) % 30;
        let i = c / 4;
        let k = c % 4;
        let l = (32 + 2 * e + 2 * i - h - k) % 7;
        let m = (a + 11 * h + 22 * l) / 451;
        let month = (h + l - 7 * m + 114) / 31;
        let day = ((h + l - 7 * m + 114) % 31) + 1;
        (u8::try_from(month).unwrap(), u8::try_from(day).unwrap())
    }

    /// Independent Meeus Julian computus for Orthodox Easter; returns
    /// Easter *Sunday* as a **Julian** `(month, day)`. Caller adds the
    /// Julian–Gregorian offset (13 days 1901..=2099, 14 days 2100..=2199).
    #[allow(clippy::many_single_char_names)]
    fn julian_computus_easter_sunday(year: u16) -> (u8, u8) {
        let y = i32::from(year);
        let a = y % 4;
        let b = y % 7;
        let c = y % 19;
        let d = (19 * c + 15) % 30;
        let e = (2 * a + 4 * b - d + 34) % 7;
        let month = (d + e + 114) / 31;
        let day = ((d + e + 114) % 31) + 1;
        (u8::try_from(month).unwrap(), u8::try_from(day).unwrap())
    }

    /// Every Western table entry must equal the Gregorian computus,
    /// computed from first principles.
    #[test]
    fn western_table_matches_gregorian_computus() {
        for year in 1901u16..=2199 {
            let (month, day) = gregorian_computus_easter_sunday(year);
            let sunday = Date::from_ymd(year, Month::try_from_u8(month).unwrap(), day).unwrap();
            let monday = sunday.add_days(1).unwrap();
            assert_eq!(
                easter_monday(Year::new(year).unwrap(), EasterMethod::Western),
                monday.day_of_year(),
                "Western table disagrees with Gregorian computus for {year}",
            );
        }
    }

    /// Every Orthodox table entry must equal the Julian computus plus
    /// the Julian→Gregorian calendar offset.
    #[test]
    fn orthodox_table_matches_julian_computus() {
        for year in 1901u16..=2199 {
            let (month, day) = julian_computus_easter_sunday(year);
            let offset = if year >= 2100 { 14 } else { 13 };
            // Julian Mar/Apr share month lengths with Gregorian, so the
            // Julian (y, m, d) builds as a Gregorian `Date` plus offset.
            let julian_as_serial =
                Date::from_ymd(year, Month::try_from_u8(month).unwrap(), day).unwrap();
            let sunday = julian_as_serial.add_days(offset).unwrap();
            let monday = sunday.add_days(1).unwrap();
            assert_eq!(
                easter_monday(Year::new(year).unwrap(), EasterMethod::Orthodox),
                monday.day_of_year(),
                "Orthodox table disagrees with Julian computus for {year}",
            );
        }
    }

    #[test]
    fn easter_sunday_is_one_day_before_monday() {
        for y in 1901u16..=2199 {
            let year = Year::new(y).unwrap();
            for m in [EasterMethod::Western, EasterMethod::Orthodox] {
                assert_eq!(easter_sunday(year, m) + 1, easter_monday(year, m));
            }
        }
    }
}
