//! Dates and their building blocks: [`Date`], [`Year`], [`Month`],
//! [`Weekday`], and [`Ordinal`].
//!
//! [`Date`] is a newtype over [`u32`] counting days from 1901-01-01 (serial
//! zero); supported range 1901-01-01..=2199-12-31, else [`TimeError`].

use crate::{Period, TimeError};
use core::fmt;
use core::ops::{Add, Sub};

// ---- Range constants ----------------------------------------------------

const EPOCH_YEAR: u16 = 1901;
const END_YEAR: u16 = 2199;
const NUM_YEARS: u16 = END_YEAR - EPOCH_YEAR + 1;

const fn is_leap(year: u16) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
}

/// `CUMULATIVE[i]` = days from 1901-01-01 to (1901 + i)-01-01; the entry
/// at index `NUM_YEARS` is one past the last valid day.
const CUMULATIVE: [u32; NUM_YEARS as usize + 1] = {
    let mut out = [0u32; NUM_YEARS as usize + 1];
    let mut i: u16 = 0;
    while i < NUM_YEARS {
        let year = EPOCH_YEAR + i;
        let len: u32 = if is_leap(year) { 366 } else { 365 };
        out[i as usize + 1] = out[i as usize] + len;
        i += 1;
    }
    out
};

const MAX_SERIAL: u32 = CUMULATIVE[NUM_YEARS as usize] - 1;

/// 0-based day-of-year at the start of month `i + 1`, non-leap year;
/// entry 12 is a sentinel.
const MONTH_OFFSETS_NONLEAP: [u32; 13] =
    [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334, 365];

/// As [`MONTH_OFFSETS_NONLEAP`], for a leap year.
const MONTH_OFFSETS_LEAP: [u32; 13] = [0, 31, 60, 91, 121, 152, 182, 213, 244, 274, 305, 335, 366];

// ---- Year ---------------------------------------------------------------

/// A year in the range 1901..=2199.
///
/// ```
/// use fasti::Year;
/// let y = Year::new(2026)?;
/// assert_eq!(y.get(), 2026);
/// assert!(!y.is_leap());
/// assert!(Year::new(1900).is_err());
/// # Ok::<(), fasti::TimeError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct Year(u16);

impl Year {
    /// The earliest supported year, 1901.
    pub const MIN: Self = Self(EPOCH_YEAR);

    /// The latest supported year, 2199.
    pub const MAX: Self = Self(END_YEAR);

    /// Construct a [`Year`], refusing values outside `1901..=2199`.
    pub const fn new(year: u16) -> Result<Self, TimeError> {
        if year < EPOCH_YEAR || year > END_YEAR {
            Err(TimeError::YearOutOfRange)
        } else {
            Ok(Self(year))
        }
    }

    /// Construct a [`Year`] from a compile-time literal; an out-of-range
    /// value is a compile error, not a runtime panic.
    ///
    /// ```
    /// use fasti::Year;
    /// const MLK_FEDERAL_FROM: Year = Year::literal(1986);
    /// assert_eq!(MLK_FEDERAL_FROM.get(), 1986);
    /// ```
    ///
    /// ```compile_fail
    /// use fasti::Year;
    /// // Compile error: argument out of range.
    /// const BAD: Year = Year::literal(1800);
    /// ```
    #[must_use]
    #[allow(clippy::panic)]
    pub const fn literal(year: u16) -> Self {
        match Self::new(year) {
            Ok(y) => y,
            // Reached only at const-eval time — a compile error, not a runtime panic.
            Err(_) => panic!("Year::literal: argument must be in 1901..=2199"),
        }
    }

    /// Return the underlying year as a [`u16`].
    ///
    /// ```
    /// use fasti::Year;
    /// assert_eq!(Year::new(2026)?.get(), 2026);
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }

    /// `true` iff this is a Gregorian leap year.
    ///
    /// ```
    /// use fasti::Year;
    /// assert!(Year::new(2000)?.is_leap());   // div by 400
    /// assert!(!Year::new(2100)?.is_leap());  // div by 100 but not 400
    /// assert!(Year::new(2024)?.is_leap());   // div by 4 only
    /// assert!(!Year::new(2025)?.is_leap());
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    #[must_use]
    pub const fn is_leap(self) -> bool {
        is_leap(self.0)
    }

    /// Number of days in the year (365 or 366).
    ///
    /// ```
    /// use fasti::Year;
    /// assert_eq!(Year::new(2024)?.length(), 366);
    /// assert_eq!(Year::new(2025)?.length(), 365);
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    #[must_use]
    pub const fn length(self) -> u16 {
        if self.is_leap() { 366 } else { 365 }
    }
}

impl fmt::Display for Year {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ---- Month --------------------------------------------------------------

/// A month of the year, discriminant 1..=12.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum Month {
    /// January
    Jan = 1,
    /// February
    Feb = 2,
    /// March
    Mar = 3,
    /// April
    Apr = 4,
    /// May
    May = 5,
    /// June
    Jun = 6,
    /// July
    Jul = 7,
    /// August
    Aug = 8,
    /// September
    Sep = 9,
    /// October
    Oct = 10,
    /// November
    Nov = 11,
    /// December
    Dec = 12,
}

impl Month {
    /// Month number, `Jan => 1`, ..., `Dec => 12`.
    ///
    /// ```
    /// use fasti::Month;
    /// assert_eq!(Month::Jul.get(), 7);
    /// ```
    #[must_use]
    pub const fn get(self) -> u8 {
        self as u8
    }

    /// Construct a [`Month`] from a 1-based month number, refusing
    /// anything outside `1..=12`.
    ///
    /// ```
    /// use fasti::{Month, TimeError};
    /// assert_eq!(Month::try_from_u8(7)?, Month::Jul);
    /// assert_eq!(Month::try_from_u8(0), Err(TimeError::MonthOutOfRange));
    /// assert_eq!(Month::try_from_u8(13), Err(TimeError::MonthOutOfRange));
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    pub const fn try_from_u8(month: u8) -> Result<Self, TimeError> {
        match month {
            1 => Ok(Self::Jan),
            2 => Ok(Self::Feb),
            3 => Ok(Self::Mar),
            4 => Ok(Self::Apr),
            5 => Ok(Self::May),
            6 => Ok(Self::Jun),
            7 => Ok(Self::Jul),
            8 => Ok(Self::Aug),
            9 => Ok(Self::Sep),
            10 => Ok(Self::Oct),
            11 => Ok(Self::Nov),
            12 => Ok(Self::Dec),
            _ => Err(TimeError::MonthOutOfRange),
        }
    }

    /// Number of days in this month for the given [`Year`], with February
    /// returning 28 or 29 as appropriate.
    ///
    /// ```
    /// use fasti::{Month, Year};
    /// assert_eq!(Month::Feb.length(Year::new(2024)?), 29); // leap
    /// assert_eq!(Month::Feb.length(Year::new(2025)?), 28);
    /// assert_eq!(Month::Apr.length(Year::new(2025)?), 30);
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    #[must_use]
    pub const fn length(self, year: Year) -> u8 {
        match self {
            Self::Jan | Self::Mar | Self::May | Self::Jul | Self::Aug | Self::Oct | Self::Dec => 31,
            Self::Apr | Self::Jun | Self::Sep | Self::Nov => 30,
            Self::Feb => {
                if year.is_leap() {
                    29
                } else {
                    28
                }
            }
        }
    }
}

impl fmt::Display for Month {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Jan => "Jan",
            Self::Feb => "Feb",
            Self::Mar => "Mar",
            Self::Apr => "Apr",
            Self::May => "May",
            Self::Jun => "Jun",
            Self::Jul => "Jul",
            Self::Aug => "Aug",
            Self::Sep => "Sep",
            Self::Oct => "Oct",
            Self::Nov => "Nov",
            Self::Dec => "Dec",
        };
        f.write_str(name)
    }
}

// ---- Weekday ------------------------------------------------------------

/// Day of the week. Discriminants follow ISO 8601: Monday = 1 .. Sunday = 7.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum Weekday {
    /// Monday — ISO 1.
    Mon = 1,
    /// Tuesday — ISO 2.
    Tue = 2,
    /// Wednesday — ISO 3.
    Wed = 3,
    /// Thursday — ISO 4.
    Thu = 4,
    /// Friday — ISO 5.
    Fri = 5,
    /// Saturday — ISO 6.
    Sat = 6,
    /// Sunday — ISO 7.
    Sun = 7,
}

impl Weekday {
    /// The ISO 8601 weekday number: Monday = 1 .. Sunday = 7.
    ///
    /// ```
    /// use fasti::Weekday;
    /// assert_eq!(Weekday::Mon.get(), 1);
    /// assert_eq!(Weekday::Sun.get(), 7);
    /// ```
    #[must_use]
    pub const fn get(self) -> u8 {
        self as u8
    }

    /// Construct a [`Weekday`] from an ISO weekday number (`1..=7`),
    /// refusing anything outside that range.
    ///
    /// ```
    /// use fasti::{Weekday, TimeError};
    /// assert_eq!(Weekday::try_from_u8(1)?, Weekday::Mon);
    /// assert_eq!(Weekday::try_from_u8(7)?, Weekday::Sun);
    /// assert_eq!(Weekday::try_from_u8(0), Err(TimeError::WeekdayOutOfRange));
    /// assert_eq!(Weekday::try_from_u8(8), Err(TimeError::WeekdayOutOfRange));
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    pub const fn try_from_u8(weekday: u8) -> Result<Self, TimeError> {
        match weekday {
            1 => Ok(Self::Mon),
            2 => Ok(Self::Tue),
            3 => Ok(Self::Wed),
            4 => Ok(Self::Thu),
            5 => Ok(Self::Fri),
            6 => Ok(Self::Sat),
            7 => Ok(Self::Sun),
            _ => Err(TimeError::WeekdayOutOfRange),
        }
    }
}

impl fmt::Display for Weekday {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Mon => "Mon",
            Self::Tue => "Tue",
            Self::Wed => "Wed",
            Self::Thu => "Thu",
            Self::Fri => "Fri",
            Self::Sat => "Sat",
            Self::Sun => "Sun",
        };
        f.write_str(name)
    }
}

// ---- Ordinal ------------------------------------------------------------

/// An ordinal position within a month for nth-weekday rules. "First" =
/// first occurrence, "Fifth" = fifth (which may not exist in every month).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum Ordinal {
    /// 1st occurrence.
    First = 1,
    /// 2nd occurrence.
    Second = 2,
    /// 3rd occurrence.
    Third = 3,
    /// 4th occurrence.
    Fourth = 4,
    /// 5th occurrence (may not exist in all month/weekday pairs).
    Fifth = 5,
}

impl Ordinal {
    /// The underlying 1-based discriminant.
    ///
    /// ```
    /// use fasti::Ordinal;
    /// assert_eq!(Ordinal::Third.get(), 3);
    /// ```
    #[must_use]
    pub const fn get(self) -> u8 {
        self as u8
    }

    /// Construct an [`Ordinal`] from a 1-based value, refusing anything
    /// outside `1..=5`.
    ///
    /// ```
    /// use fasti::{Ordinal, TimeError};
    /// assert_eq!(Ordinal::try_from_u8(3)?, Ordinal::Third);
    /// assert_eq!(Ordinal::try_from_u8(0), Err(TimeError::OrdinalOutOfRange));
    /// assert_eq!(Ordinal::try_from_u8(6), Err(TimeError::OrdinalOutOfRange));
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    pub const fn try_from_u8(n: u8) -> Result<Self, TimeError> {
        match n {
            1 => Ok(Self::First),
            2 => Ok(Self::Second),
            3 => Ok(Self::Third),
            4 => Ok(Self::Fourth),
            5 => Ok(Self::Fifth),
            _ => Err(TimeError::OrdinalOutOfRange),
        }
    }
}

impl fmt::Display for Ordinal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::First => "First",
            Self::Second => "Second",
            Self::Third => "Third",
            Self::Fourth => "Fourth",
            Self::Fifth => "Fifth",
        };
        f.write_str(name)
    }
}

// ---- Date ---------------------------------------------------------------

/// A calendar date in the supported range 1901-01-01..=2199-12-31.
///
/// Internally a [`u32`] count of days since 1901-01-01 (inclusive).
///
/// ```
/// use fasti::{Date, Month, Weekday};
///
/// let d = Date::from_ymd(2026, Month::Jul, 4)?;
/// assert_eq!(d.year().get(), 2026);
/// assert_eq!(d.month(), Month::Jul);
/// assert_eq!(d.day(), 4);
/// assert_eq!(d.weekday(), Weekday::Sat);
/// # Ok::<(), fasti::TimeError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct Date(u32);

impl Date {
    /// The earliest representable date, 1901-01-01.
    pub const MIN: Self = Self(0);

    /// The latest representable date, 2199-12-31.
    pub const MAX: Self = Self(MAX_SERIAL);

    /// Construct a [`Date`] from year, month, and day. Refuses
    /// out-of-range years, zero days, and days exceeding the month length
    /// (accounting for leap years).
    pub const fn from_ymd(year: u16, month: Month, day: u8) -> Result<Self, TimeError> {
        let y = match Year::new(year) {
            Ok(y) => y,
            Err(e) => return Err(e),
        };
        let len = month.length(y);
        if day == 0 || day > len {
            return Err(TimeError::DayOutOfRange);
        }
        let year_idx = (year - EPOCH_YEAR) as usize;
        let year_start = CUMULATIVE[year_idx];
        let month_offset = if y.is_leap() {
            MONTH_OFFSETS_LEAP[(month.get() - 1) as usize]
        } else {
            MONTH_OFFSETS_NONLEAP[(month.get() - 1) as usize]
        };
        Ok(Self(year_start + month_offset + day as u32 - 1))
    }

    /// Construct a [`Date`] from compile-time year, month, and day
    /// literals; an invalid date is a compile error, not a runtime panic.
    ///
    /// ```
    /// use fasti::{Date, Month};
    /// const CARTER_FUNERAL: Date = Date::literal(2025, Month::Jan, 9);
    /// assert_eq!(CARTER_FUNERAL.year().get(), 2025);
    /// ```
    ///
    /// ```compile_fail
    /// use fasti::{Date, Month};
    /// // Compile error: Feb 30 does not exist.
    /// const BAD: Date = Date::literal(2025, Month::Feb, 30);
    /// ```
    #[must_use]
    #[allow(clippy::panic)]
    pub const fn literal(year: u16, month: Month, day: u8) -> Self {
        match Self::from_ymd(year, month, day) {
            Ok(d) => d,
            // Reached only at const-eval time — a compile error, not a runtime panic.
            Err(_) => panic!("Date::literal: invalid year/month/day"),
        }
    }

    /// Construct a [`Date`] from a serial day count relative to
    /// 1901-01-01 (serial 0). Refuses values outside the supported range.
    ///
    /// ```
    /// use fasti::{Date, Month};
    /// assert_eq!(Date::from_serial(0)?, Date::from_ymd(1901, Month::Jan, 1)?);
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    pub const fn from_serial(serial: u32) -> Result<Self, TimeError> {
        if serial > MAX_SERIAL {
            Err(TimeError::DateOutOfRange)
        } else {
            Ok(Self(serial))
        }
    }

    /// The underlying serial: days since 1901-01-01 inclusive (serial 0).
    #[must_use]
    pub const fn serial(self) -> u32 {
        self.0
    }

    /// The [`Year`] component.
    #[must_use]
    pub const fn year(self) -> Year {
        // Largest `idx` with `CUMULATIVE[idx] <= serial`; `lo`/`hi` are `u16` so the final add needs no cast.
        let serial = self.0;
        let mut lo: u16 = 0;
        let mut hi: u16 = NUM_YEARS;
        while hi - lo > 1 {
            let mid = lo + (hi - lo) / 2;
            if CUMULATIVE[mid as usize] <= serial {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        Year(EPOCH_YEAR + lo)
    }

    /// Decompose into `(year, month, day-of-month)`.
    ///
    /// ```
    /// use fasti::{Date, Month};
    /// let d = Date::from_ymd(2026, Month::Jul, 4)?;
    /// let (y, m, dom) = d.to_ymd();
    /// assert_eq!((y.get(), m, dom), (2026, Month::Jul, 4));
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    #[must_use]
    pub const fn to_ymd(self) -> (Year, Month, u8) {
        let y = self.year();
        let year_idx = (y.0 - EPOCH_YEAR) as usize;
        let doy = self.0 - CUMULATIVE[year_idx];
        let offsets = if y.is_leap() {
            &MONTH_OFFSETS_LEAP
        } else {
            &MONTH_OFFSETS_NONLEAP
        };
        let mut m: usize = 0;
        while m + 1 < 13 && offsets[m + 1] <= doy {
            m += 1;
        }
        let month = match m {
            0 => Month::Jan,
            1 => Month::Feb,
            2 => Month::Mar,
            3 => Month::Apr,
            4 => Month::May,
            5 => Month::Jun,
            6 => Month::Jul,
            7 => Month::Aug,
            8 => Month::Sep,
            9 => Month::Oct,
            10 => Month::Nov,
            _ => Month::Dec,
        };
        // `doy - offsets[m] + 1` is bounded 1..=31, so the `as u8` narrowing is safe.
        #[allow(clippy::cast_possible_truncation)]
        let day_of_month = (doy - offsets[m] + 1) as u8;
        (y, month, day_of_month)
    }

    /// The [`Month`] component.
    ///
    /// ```
    /// use fasti::{Date, Month};
    /// assert_eq!(Date::from_ymd(2026, Month::Jul, 4)?.month(), Month::Jul);
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    #[must_use]
    pub const fn month(self) -> Month {
        let (_, m, _) = self.to_ymd();
        m
    }

    /// The day-of-month component, `1..=31`.
    ///
    /// ```
    /// use fasti::{Date, Month};
    /// assert_eq!(Date::from_ymd(2026, Month::Jul, 4)?.day(), 4);
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    #[must_use]
    pub const fn day(self) -> u8 {
        let (_, _, d) = self.to_ymd();
        d
    }

    /// The 1-indexed day of the year, `1..=366`.
    ///
    /// ```
    /// use fasti::{Date, Month};
    /// assert_eq!(Date::from_ymd(2024, Month::Jan, 1)?.day_of_year(), 1);
    /// assert_eq!(Date::from_ymd(2024, Month::Dec, 31)?.day_of_year(), 366); // leap
    /// assert_eq!(Date::from_ymd(2025, Month::Dec, 31)?.day_of_year(), 365);
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    #[must_use]
    pub const fn day_of_year(self) -> u16 {
        let y = self.year();
        let year_idx = (y.0 - EPOCH_YEAR) as usize;
        // Result is 1..=366, so the `u32 -> u16` narrowing is safe.
        #[allow(clippy::cast_possible_truncation)]
        let doy = (self.0 - CUMULATIVE[year_idx] + 1) as u16;
        doy
    }

    /// The day of the week.
    ///
    /// ```
    /// use fasti::{Date, Month, Weekday};
    /// assert_eq!(
    ///     Date::from_ymd(2026, Month::Jul, 4)?.weekday(),
    ///     Weekday::Sat,
    /// );
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    #[must_use]
    pub const fn weekday(self) -> Weekday {
        // Serial 0 (1901-01-01) is Tuesday, so `(serial + 1) % 7` gives 0..=6 keyed Mon..Sun.
        match (self.0 + 1) % 7 {
            0 => Weekday::Mon,
            1 => Weekday::Tue,
            2 => Weekday::Wed,
            3 => Weekday::Thu,
            4 => Weekday::Fri,
            5 => Weekday::Sat,
            _ => Weekday::Sun,
        }
    }

    /// Add `n` days, returning [`TimeError::DateOutOfRange`] if the result
    /// would fall outside the supported range.
    ///
    /// ```
    /// use fasti::{Date, Month, TimeError};
    /// let d = Date::from_ymd(2026, Month::Feb, 28)?;
    /// assert_eq!(d.add_days(1)?, Date::from_ymd(2026, Month::Mar, 1)?);
    /// assert_eq!(Date::MAX.add_days(1), Err(TimeError::DateOutOfRange));
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    pub const fn add_days(self, n: i32) -> Result<Self, TimeError> {
        // Widen to `i64` so any `u32 + i32` sum fits and can be bounds-checked before narrowing.
        let target = self.0 as i64 + n as i64;
        if target < 0 || target > MAX_SERIAL as i64 {
            return Err(TimeError::DateOutOfRange);
        }
        // `target` is in `0..=MAX_SERIAL`, so the `i64 -> u32` narrowing is safe.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let serial = target as u32;
        Ok(Self(serial))
    }

    /// Signed difference `self - other` in days. Returns a negative value
    /// when `self` precedes `other`.
    ///
    /// ```
    /// use fasti::{Date, Month};
    /// let a = Date::from_ymd(2026, Month::Jan, 1)?;
    /// let b = Date::from_ymd(2026, Month::Jan, 31)?;
    /// assert_eq!(b.days_since(a), 30);
    /// assert_eq!(a.days_since(b), -30);
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    #[must_use]
    pub const fn days_since(self, other: Self) -> i32 {
        // Widen to `i64` to avoid `u32 - u32` underflow.
        let diff = self.0 as i64 - other.0 as i64;
        // Bounded by `|diff| <= MAX_SERIAL`; `i64 -> i32` is safe.
        #[allow(clippy::cast_possible_truncation)]
        let diff_i32 = diff as i32;
        diff_i32
    }

    /// Add `n` calendar months, clamping the day-of-month to the new
    /// month's length. Matches `QuantLib`'s `Date::advance` semantics.
    /// Returns [`TimeError::DateOutOfRange`] if the result is out of range.
    ///
    /// ```
    /// use fasti::{Date, Month};
    /// let jan31 = Date::from_ymd(2026, Month::Jan, 31)?;
    /// assert_eq!(jan31.add_months(1)?, Date::from_ymd(2026, Month::Feb, 28)?);
    /// let feb28_2024 = Date::from_ymd(2024, Month::Feb, 28)?;
    /// assert_eq!(feb28_2024.add_months(12)?, Date::from_ymd(2025, Month::Feb, 28)?);
    /// let apr30 = Date::from_ymd(2026, Month::Apr, 30)?;
    /// assert_eq!(apr30.add_months(-2)?, Date::from_ymd(2026, Month::Feb, 28)?);
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    pub const fn add_months(self, n: i32) -> Result<Self, TimeError> {
        let (year, month, day) = self.to_ymd();
        // Zero-based month index in `i32` — all in-range (year, month) pairs fit.
        let total_months = year.get() as i32 * 12 + (month.get() as i32 - 1);
        let Some(new_total) = total_months.checked_add(n) else {
            return Err(TimeError::DateOutOfRange);
        };
        // Euclidean div/rem stay correct if `new_total` is negative.
        let target_year_i32 = new_total.div_euclid(12);
        let new_month_idx = new_total.rem_euclid(12);
        if target_year_i32 < Year::MIN.get() as i32 || target_year_i32 > Year::MAX.get() as i32 {
            return Err(TimeError::DateOutOfRange);
        }
        // `target_year_i32` is bounded to 1901..=2199, a `u16` range; narrowing is safe.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let new_year_u16 = target_year_i32 as u16;
        // `new_month_idx` is bounded to 0..=11, a `u8` range.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let new_month = match Month::try_from_u8((new_month_idx as u8) + 1) {
            Ok(found) => found,
            Err(err) => return Err(err),
        };
        let target_year = match Year::new(new_year_u16) {
            Ok(found) => found,
            Err(err) => return Err(err),
        };
        let clamped_day = {
            let len = new_month.length(target_year);
            if day > len { len } else { day }
        };
        Self::from_ymd(new_year_u16, new_month, clamped_day)
    }

    /// Add `n` calendar years, clamping Feb 29 to Feb 28 when the
    /// target year is not a leap year.
    ///
    /// ```
    /// use fasti::{Date, Month};
    /// let leap_day = Date::from_ymd(2024, Month::Feb, 29)?;
    /// // 2025 is not a leap year — Feb 29 clamps to Feb 28.
    /// assert_eq!(leap_day.add_years(1)?, Date::from_ymd(2025, Month::Feb, 28)?);
    /// // 2028 is a leap year — Feb 29 preserved.
    /// assert_eq!(leap_day.add_years(4)?, Date::from_ymd(2028, Month::Feb, 29)?);
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    pub const fn add_years(self, n: i32) -> Result<Self, TimeError> {
        let Some(months) = n.checked_mul(12) else {
            return Err(TimeError::DateOutOfRange);
        };
        self.add_months(months)
    }

    /// `self + period`, preserving end-of-month when `end_of_month`
    /// is set and `self` is itself the last day of its month.
    ///
    /// This is the crate's one stepping rule: [`Calendar::advance`](crate::Calendar::advance)
    /// rolls its result onto a business day, and
    /// [`Generation::step`](crate::Generation::step) scales the tenor
    /// before calling it. The flag is inert for `Days` and `Weeks`
    /// periods, where end-of-month has no meaning. Semantics match
    /// `QuantLib`'s `Date::advance`.
    ///
    /// ```
    /// use fasti::{Date, Month, Period};
    /// let feb_end = Date::from_ymd(2025, Month::Feb, 28)?;
    /// assert_eq!(feb_end.advance(Period::Months(1), false)?, Date::from_ymd(2025, Month::Mar, 28)?);
    /// assert_eq!(feb_end.advance(Period::Months(1), true)?, Date::from_ymd(2025, Month::Mar, 31)?);
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    pub fn advance(self, period: Period, end_of_month: bool) -> Result<Self, TimeError> {
        let stepped = (self + period)?;
        Ok(
            if end_of_month
                && self.is_end_of_month()
                && matches!(period, Period::Months(_) | Period::Years(_))
            {
                stepped.end_of_month()
            } else {
                stepped
            },
        )
    }

    /// The last day of `self`'s month.
    ///
    /// ```
    /// use fasti::{Date, Month};
    /// let d = Date::from_ymd(2024, Month::Feb, 10)?;
    /// assert_eq!(d.end_of_month(), Date::from_ymd(2024, Month::Feb, 29)?);
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    #[must_use]
    pub const fn end_of_month(self) -> Self {
        let (year, month, _) = self.to_ymd();
        let last = month.length(year);
        // Serial arithmetic — month start plus (length - 1) — avoids an unreachable `from_ymd` error path.
        let month_start = self.0 - (self.day() as u32 - 1);
        Self(month_start + last as u32 - 1)
    }

    /// `true` iff `self` is the last day of its month.
    ///
    /// ```
    /// use fasti::{Date, Month};
    /// assert!(Date::from_ymd(2024, Month::Feb, 29)?.is_end_of_month()); // leap
    /// assert!(Date::from_ymd(2025, Month::Feb, 28)?.is_end_of_month()); // non-leap
    /// assert!(!Date::from_ymd(2025, Month::Feb, 27)?.is_end_of_month());
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    #[must_use]
    pub const fn is_end_of_month(self) -> bool {
        let (year, month, day) = self.to_ymd();
        day == month.length(year)
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Decompose once instead of three separate year lookups.
        let (y, m, d) = self.to_ymd();
        write!(f, "{:04}-{:02}-{:02}", y.get(), m.get(), d)
    }
}

impl core::str::FromStr for Date {
    type Err = TimeError;

    /// Parse a strict ISO-8601 `YYYY-MM-DD` date — the exact format
    /// [`Display`](fmt::Display) produces. Malformed strings return
    /// [`TimeError::InvalidDateString`]; range errors match [`Date::from_ymd`].
    ///
    /// ```
    /// use fasti::{Date, Month, TimeError};
    /// let d: Date = "2026-07-04".parse()?;
    /// assert_eq!(d, Date::from_ymd(2026, Month::Jul, 4)?);
    /// // Round trip through Display.
    /// assert_eq!("2026-07-04".parse::<Date>()?.to_string(), "2026-07-04");
    /// // Malformed strings are rejected.
    /// assert_eq!("2026-7-4".parse::<Date>(), Err(TimeError::InvalidDateString));
    /// // Well-formed but nonexistent dates surface the range error.
    /// assert_eq!("2026-02-30".parse::<Date>(), Err(TimeError::DayOutOfRange));
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        const fn digit(b: u8) -> Result<u16, TimeError> {
            if b.is_ascii_digit() {
                Ok((b - b'0') as u16)
            } else {
                Err(TimeError::InvalidDateString)
            }
        }
        let [y3, y2, y1, y0, h1, m1, m0, h2, d1, d0] = s.as_bytes() else {
            return Err(TimeError::InvalidDateString);
        };
        if *h1 != b'-' || *h2 != b'-' {
            return Err(TimeError::InvalidDateString);
        }
        let year = 1000 * digit(*y3)? + 100 * digit(*y2)? + 10 * digit(*y1)? + digit(*y0)?;
        let month_num = 10 * digit(*m1)? + digit(*m0)?;
        let day = 10 * digit(*d1)? + digit(*d0)?;
        // Both values are at most 99, so the u16 -> u8 narrowing is exact.
        #[allow(clippy::cast_possible_truncation)]
        let month = Month::try_from_u8(month_num as u8)?;
        #[allow(clippy::cast_possible_truncation)]
        let day = day as u8;
        Self::from_ymd(year, month, day)
    }
}

/// Step a [`Date`] forward by a [`Period`]. Returns
/// [`TimeError::DateOutOfRange`] for out-of-range results; `Months`/`Years`
/// clamp the day-of-month (see [`Date::add_months`]).
///
/// ```
/// use fasti::{Date, Month, Period};
/// let d = Date::from_ymd(2026, Month::Jan, 15)?;
/// assert_eq!((d + Period::Months(6))?, Date::from_ymd(2026, Month::Jul, 15)?);
/// assert_eq!((d + Period::Years(1))?, Date::from_ymd(2027, Month::Jan, 15)?);
/// assert_eq!((d + Period::Days(7))?, Date::from_ymd(2026, Month::Jan, 22)?);
/// // Negative periods step backward.
/// assert_eq!((d + (-Period::Months(1)))?, Date::from_ymd(2025, Month::Dec, 15)?);
/// # Ok::<(), fasti::TimeError>(())
/// ```
impl Add<Period> for Date {
    type Output = Result<Self, TimeError>;

    fn add(self, period: Period) -> Self::Output {
        match period {
            Period::Days(n) => self.add_days(n),
            Period::Weeks(n) => match n.checked_mul(7) {
                Some(days) => self.add_days(days),
                None => Err(TimeError::DateOutOfRange),
            },
            Period::Months(n) => self.add_months(n),
            Period::Years(n) => self.add_years(n),
        }
    }
}

/// Step a [`Date`] backward by a [`Period`]. Uses [`Period::checked_neg`],
/// surfacing `i32::MIN` overflow as [`TimeError::DateOutOfRange`].
///
/// ```
/// use fasti::{Date, Month, Period};
/// let d = Date::from_ymd(2026, Month::Jul, 15)?;
/// assert_eq!((d - Period::Months(6))?, Date::from_ymd(2026, Month::Jan, 15)?);
/// # Ok::<(), fasti::TimeError>(())
/// ```
impl Sub<Period> for Date {
    type Output = Result<Self, TimeError>;

    fn sub(self, period: Period) -> Self::Output {
        // `+` inside `Sub` is the deliberate factoring: delegate to `Add` after negating.
        #[allow(clippy::suspicious_arithmetic_impl)]
        match period.checked_neg() {
            Some(neg) => self + neg,
            None => Err(TimeError::DateOutOfRange),
        }
    }
}

// ---- Tests --------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    extern crate alloc;

    use super::*;
    use proptest::prelude::*;

    #[test]
    fn epoch_is_1901_01_01_tuesday() {
        let d = Date::MIN;
        assert_eq!(d.serial(), 0);
        assert_eq!(d.year().get(), 1901);
        assert_eq!(d.month(), Month::Jan);
        assert_eq!(d.day(), 1);
        assert_eq!(d.weekday(), Weekday::Tue);
    }

    #[test]
    fn max_is_2199_12_31() {
        let d = Date::MAX;
        assert_eq!(d.year().get(), 2199);
        assert_eq!(d.month(), Month::Dec);
        assert_eq!(d.day(), 31);
    }

    #[test]
    fn from_ymd_rejects_out_of_range_year() {
        assert_eq!(
            Date::from_ymd(1900, Month::Jan, 1),
            Err(TimeError::YearOutOfRange)
        );
        assert_eq!(
            Date::from_ymd(2200, Month::Jan, 1),
            Err(TimeError::YearOutOfRange)
        );
    }

    #[test]
    fn from_ymd_rejects_day_zero_and_overflow() {
        assert_eq!(
            Date::from_ymd(2026, Month::Jan, 0),
            Err(TimeError::DayOutOfRange)
        );
        assert_eq!(
            Date::from_ymd(2026, Month::Jan, 32),
            Err(TimeError::DayOutOfRange)
        );
        assert_eq!(
            Date::from_ymd(2026, Month::Apr, 31),
            Err(TimeError::DayOutOfRange)
        );
    }

    #[test]
    fn february_leap_year_behavior() {
        // 2000 is a leap year (divisible by 400).
        assert!(Date::from_ymd(2000, Month::Feb, 29).is_ok());
        // 2100 is NOT a leap year (divisible by 100, not 400).
        assert_eq!(
            Date::from_ymd(2100, Month::Feb, 29),
            Err(TimeError::DayOutOfRange)
        );
        // 2024 is a leap year (divisible by 4, not 100).
        assert!(Date::from_ymd(2024, Month::Feb, 29).is_ok());
        // 2026 is not a leap year.
        assert_eq!(
            Date::from_ymd(2026, Month::Feb, 29),
            Err(TimeError::DayOutOfRange)
        );
    }

    #[test]
    fn known_weekdays() {
        // Anchors independently verifiable.
        assert_eq!(
            Date::from_ymd(1901, Month::Jan, 1).unwrap().weekday(),
            Weekday::Tue,
        );
        assert_eq!(
            Date::from_ymd(2000, Month::Jan, 1).unwrap().weekday(),
            Weekday::Sat,
        );
        assert_eq!(
            Date::from_ymd(2026, Month::Jul, 4).unwrap().weekday(),
            Weekday::Sat,
        );
        assert_eq!(
            Date::from_ymd(2021, Month::Jun, 19).unwrap().weekday(),
            Weekday::Sat,
        );
        assert_eq!(
            Date::from_ymd(2199, Month::Dec, 31).unwrap().weekday(),
            Weekday::Tue,
        );
    }

    #[test]
    fn day_of_year_boundaries() {
        assert_eq!(
            Date::from_ymd(2024, Month::Jan, 1).unwrap().day_of_year(),
            1,
        );
        assert_eq!(
            Date::from_ymd(2024, Month::Dec, 31).unwrap().day_of_year(),
            366, // leap
        );
        assert_eq!(
            Date::from_ymd(2025, Month::Dec, 31).unwrap().day_of_year(),
            365,
        );
    }

    #[test]
    fn add_days_at_boundaries() {
        assert_eq!(Date::MIN.add_days(-1), Err(TimeError::DateOutOfRange));
        assert_eq!(Date::MAX.add_days(1), Err(TimeError::DateOutOfRange));
        let d = Date::from_ymd(2026, Month::Feb, 28).unwrap();
        assert_eq!(
            d.add_days(1).unwrap(),
            Date::from_ymd(2026, Month::Mar, 1).unwrap()
        );
        let leap = Date::from_ymd(2024, Month::Feb, 28).unwrap();
        assert_eq!(
            leap.add_days(1).unwrap(),
            Date::from_ymd(2024, Month::Feb, 29).unwrap()
        );
    }

    #[test]
    fn display_is_iso_8601() {
        let d = Date::from_ymd(2026, Month::Jul, 4).unwrap();
        assert_eq!(alloc::format!("{d}"), "2026-07-04");
    }

    #[test]
    fn weekday_iso_numbering() {
        assert_eq!(Weekday::Mon.get(), 1);
        assert_eq!(Weekday::Sun.get(), 7);
        assert_eq!(Weekday::try_from_u8(1).unwrap(), Weekday::Mon);
        assert_eq!(Weekday::try_from_u8(7).unwrap(), Weekday::Sun);
        assert_eq!(Weekday::try_from_u8(0), Err(TimeError::WeekdayOutOfRange));
        assert_eq!(Weekday::try_from_u8(8), Err(TimeError::WeekdayOutOfRange));
    }

    #[test]
    fn ordinal_display() {
        assert_eq!(alloc::format!("{}", Ordinal::First), "First");
        assert_eq!(alloc::format!("{}", Ordinal::Fifth), "Fifth");
    }

    #[test]
    fn from_str_parses_display_output() {
        for (y, m, d) in [
            (1901u16, Month::Jan, 1u8),
            (2026, Month::Jul, 4),
            (2024, Month::Feb, 29),
            (2199, Month::Dec, 31),
        ] {
            let date = Date::from_ymd(y, m, d).unwrap();
            let parsed: Date = alloc::format!("{date}").parse().unwrap();
            assert_eq!(parsed, date);
        }
    }

    #[test]
    fn from_str_rejects_malformed_strings() {
        for bad in [
            "",
            "2026",
            "2026-07",
            "2026-7-4",    // not zero-padded
            "26-07-04",    // two-digit year
            "2026/07/04",  // wrong separator
            "2026-07-04T", // trailing content
            " 2026-07-04", // leading whitespace
            "2026-07-04 ", // trailing whitespace
            "+026-07-04",  // sign
            "2026-0a-04",  // non-digit
            "٢٠٢٦-07-04",  // non-ASCII digits
        ] {
            assert_eq!(
                bad.parse::<Date>(),
                Err(TimeError::InvalidDateString),
                "{bad:?} should be rejected as malformed",
            );
        }
    }

    #[test]
    fn from_str_surfaces_range_errors_for_well_formed_input() {
        assert_eq!("1900-12-31".parse::<Date>(), Err(TimeError::YearOutOfRange));
        assert_eq!("2200-01-01".parse::<Date>(), Err(TimeError::YearOutOfRange));
        assert_eq!(
            "2026-13-01".parse::<Date>(),
            Err(TimeError::MonthOutOfRange)
        );
        assert_eq!(
            "2026-00-01".parse::<Date>(),
            Err(TimeError::MonthOutOfRange)
        );
        assert_eq!("2026-02-30".parse::<Date>(), Err(TimeError::DayOutOfRange));
        assert_eq!("2026-01-00".parse::<Date>(), Err(TimeError::DayOutOfRange));
    }

    #[test]
    fn to_ymd_matches_individual_accessors() {
        let d = Date::from_ymd(2026, Month::Jul, 4).unwrap();
        let (y, m, dom) = d.to_ymd();
        assert_eq!(y, d.year());
        assert_eq!(m, d.month());
        assert_eq!(dom, d.day());
    }

    // ---- property tests ------------------------------------------------

    /// Strategy: uniformly sample a valid (year, month, day) in range.
    fn any_ymd() -> impl Strategy<Value = (u16, Month, u8)> {
        (EPOCH_YEAR..=END_YEAR, 1u8..=12u8).prop_flat_map(|(y, m)| {
            let month = Month::try_from_u8(m).expect("1..=12");
            let year = Year::new(y).expect("in range");
            let max_day = month.length(year);
            (Just(y), Just(month), 1u8..=max_day)
        })
    }

    proptest! {
        #[test]
        fn from_ymd_round_trips(
            (year, month, day) in any_ymd()
        ) {
            let d = Date::from_ymd(year, month, day).expect("valid ymd");
            prop_assert_eq!(d.year().get(), year);
            prop_assert_eq!(d.month(), month);
            prop_assert_eq!(d.day(), day);
        }

        #[test]
        fn serial_round_trips(
            serial in 0u32..=MAX_SERIAL,
        ) {
            let d = Date::from_serial(serial).expect("in range");
            prop_assert_eq!(d.serial(), serial);
            let ymd = Date::from_ymd(d.year().get(), d.month(), d.day()).expect("valid");
            prop_assert_eq!(ymd.serial(), serial);
        }

        #[test]
        fn weekday_advances_by_one_per_day(
            serial in 0u32..MAX_SERIAL,
        ) {
            let today = Date::from_serial(serial).expect("in range");
            let tomorrow = today.add_days(1).expect("in range");
            let expected = match today.weekday() {
                Weekday::Mon => Weekday::Tue,
                Weekday::Tue => Weekday::Wed,
                Weekday::Wed => Weekday::Thu,
                Weekday::Thu => Weekday::Fri,
                Weekday::Fri => Weekday::Sat,
                Weekday::Sat => Weekday::Sun,
                Weekday::Sun => Weekday::Mon,
            };
            prop_assert_eq!(tomorrow.weekday(), expected);
        }

        #[test]
        fn add_days_is_inverse_of_days_since(
            a_serial in 0u32..=MAX_SERIAL,
            b_serial in 0u32..=MAX_SERIAL,
        ) {
            let a = Date::from_serial(a_serial).unwrap();
            let b = Date::from_serial(b_serial).unwrap();
            let diff = b.days_since(a);
            prop_assert_eq!(a.add_days(diff).unwrap(), b);
        }

        #[test]
        fn day_of_year_is_consistent(
            (year, month, day) in any_ymd()
        ) {
            let d = Date::from_ymd(year, month, day).expect("valid");
            let year_start = Date::from_ymd(year, Month::Jan, 1).expect("valid");
            prop_assert_eq!(
                u16::try_from(d.days_since(year_start) + 1).unwrap(),
                d.day_of_year(),
            );
        }

        #[test]
        fn month_try_from_u8_round_trips(m in 1u8..=12u8) {
            let parsed = Month::try_from_u8(m).expect("1..=12");
            prop_assert_eq!(parsed.get(), m);
        }

        #[test]
        fn weekday_try_from_u8_round_trips(n in 1u8..=7u8) {
            let parsed = Weekday::try_from_u8(n).expect("1..=7");
            prop_assert_eq!(parsed.get(), n);
        }

        #[test]
        fn ordinal_try_from_u8_round_trips(n in 1u8..=5u8) {
            let parsed = Ordinal::try_from_u8(n).expect("1..=5");
            prop_assert_eq!(parsed.get(), n);
        }

        #[test]
        fn add_days_accepts_iff_result_in_range(
            serial in 0u32..=MAX_SERIAL,
            n in i32::MIN..=i32::MAX,
        ) {
            let d = Date::from_serial(serial).expect("in range");
            let result = d.add_days(n);
            let target = i64::from(serial) + i64::from(n);
            let in_range = (0..=i64::from(MAX_SERIAL)).contains(&target);
            prop_assert_eq!(result.is_ok(), in_range);
            if in_range {
                prop_assert_eq!(
                    result.expect("in-range").serial(),
                    u32::try_from(target).expect("fits in u32"),
                );
            } else {
                prop_assert_eq!(result, Err(TimeError::DateOutOfRange));
            }
        }

        #[test]
        fn to_ymd_round_trips(serial in 0u32..=MAX_SERIAL) {
            let d = Date::from_serial(serial).expect("in range");
            let (y, m, dom) = d.to_ymd();
            let rebuilt = Date::from_ymd(y.get(), m, dom).expect("valid");
            prop_assert_eq!(rebuilt.serial(), serial);
        }

        /// Every date's `Display` output parses back to the same date.
        #[test]
        fn display_and_from_str_round_trip(serial in 0u32..=MAX_SERIAL) {
            let d = Date::from_serial(serial).expect("in range");
            let parsed: Date = alloc::format!("{d}").parse().expect("Display output is valid");
            prop_assert_eq!(parsed, d);
        }

        /// `add_months(n)` then `add_months(-n)` round-trips exactly for day ≤ 28 (never clamped).
        #[test]
        fn add_months_round_trip_on_safe_days(
            year in 1910u16..=2190,
            month in 1u8..=12,
            day in 1u8..=28,
            n in -500i32..=500,
        ) {
            let parsed_month = Month::try_from_u8(month).expect("1..=12");
            let start = Date::from_ymd(year, parsed_month, day).expect("valid");
            if let Ok(stepped) = start.add_months(n)
                && let Ok(restored) = stepped.add_months(-n)
            {
                prop_assert_eq!(restored, start);
            }
        }

        /// `add_months(n)` equals `add_years(n/12)` then `add_months(n%12)` for day ≤ 28;
        /// the year range keeps intermediates in range.
        #[test]
        fn add_months_decomposes_into_years_plus_months(
            year in 1921u16..=2179,
            month in 1u8..=12,
            day in 1u8..=28,
            whole_years in -20i32..=20,
            extra_months in -11i32..=11,
        ) {
            let parsed_month = Month::try_from_u8(month).expect("1..=12");
            let start = Date::from_ymd(year, parsed_month, day).expect("valid");
            let direct = start.add_months(whole_years * 12 + extra_months);
            let stepped = start
                .add_years(whole_years)
                .and_then(|x| x.add_months(extra_months));
            prop_assert_eq!(direct, stepped);
        }

        /// `add_months` never yields a day past the target month's length.
        #[test]
        fn add_months_never_exceeds_target_month_length(
            serial in 0u32..=MAX_SERIAL,
            n in -200i32..=200,
        ) {
            let d = Date::from_serial(serial).expect("in range");
            if let Ok(out) = d.add_months(n) {
                let (y, m, dom) = out.to_ymd();
                prop_assert!(dom <= m.length(y));
                prop_assert!(dom >= 1);
            }
        }

        /// `end_of_month` is idempotent.
        #[test]
        fn end_of_month_is_idempotent(serial in 0u32..=MAX_SERIAL) {
            let d = Date::from_serial(serial).expect("in range");
            prop_assert_eq!(d.end_of_month(), d.end_of_month().end_of_month());
            prop_assert!(d.end_of_month().is_end_of_month());
        }

        /// `date + Period::Days(n)` matches `date.add_days(n)`.
        #[test]
        fn add_period_days_matches_add_days(
            serial in 0u32..=MAX_SERIAL,
            n in -10_000i32..=10_000,
        ) {
            let start = Date::from_serial(serial).expect("in range");
            prop_assert_eq!(start + crate::Period::Days(n), start.add_days(n));
        }

        /// `date + Period::Weeks(n)` matches `date.add_days(n * 7)`
        /// (modulo overflow on the multiplication).
        #[test]
        fn add_period_weeks_equals_add_days_times_seven(
            serial in 0u32..=MAX_SERIAL,
            n in (i32::MIN / 7)..=(i32::MAX / 7),
        ) {
            let start = Date::from_serial(serial).expect("in range");
            prop_assert_eq!(start + crate::Period::Weeks(n), start.add_days(n * 7));
        }

        /// `date + Period::Months(n)` matches `date.add_months(n)`.
        #[test]
        fn add_period_months_matches_add_months(
            serial in 0u32..=MAX_SERIAL,
            n in -200i32..=200,
        ) {
            let start = Date::from_serial(serial).expect("in range");
            prop_assert_eq!(start + crate::Period::Months(n), start.add_months(n));
        }

        /// `date + Period::Years(n)` matches `date.add_years(n)`.
        #[test]
        fn add_period_years_matches_add_years(
            serial in 0u32..=MAX_SERIAL,
            n in -100i32..=100,
        ) {
            let start = Date::from_serial(serial).expect("in range");
            prop_assert_eq!(start + crate::Period::Years(n), start.add_years(n));
        }

        /// `(date - period)` equals `(date + (-period))` for every
        /// non-`i32::MIN` length.
        #[test]
        fn sub_period_equals_add_negated_period(
            serial in 0u32..=MAX_SERIAL,
            length in (i32::MIN + 1)..=i32::MAX,
            unit_idx in 0u8..=3,
        ) {
            let p = match unit_idx {
                0 => crate::Period::Days(length),
                1 => crate::Period::Weeks(length),
                2 => crate::Period::Months(length),
                _ => crate::Period::Years(length),
            };
            let start = Date::from_serial(serial).expect("in range");
            prop_assert_eq!(start - p, start + (-p));
        }
    }

    // ---- example-based tests for month/year arithmetic -----------------

    #[test]
    fn add_months_clamps_to_target_month_length() {
        let jan31 = Date::from_ymd(2026, Month::Jan, 31).unwrap();
        assert_eq!(
            jan31.add_months(1).unwrap(),
            Date::from_ymd(2026, Month::Feb, 28).unwrap()
        );
        // Leap year: Jan 31 2024 + 1M → Feb 29.
        let jan31_leap = Date::from_ymd(2024, Month::Jan, 31).unwrap();
        assert_eq!(
            jan31_leap.add_months(1).unwrap(),
            Date::from_ymd(2024, Month::Feb, 29).unwrap()
        );
        // May 31 + 1M → Jun 30 (no May 31 + 1M = Jun 31).
        let may31 = Date::from_ymd(2026, Month::May, 31).unwrap();
        assert_eq!(
            may31.add_months(1).unwrap(),
            Date::from_ymd(2026, Month::Jun, 30).unwrap()
        );
    }

    /// Clamp-on-add-months is not composable across end-of-month dates:
    /// `Jan 31 → Feb 28 → Mar 28` differs from `Jan 31 → Mar 31`.
    #[test]
    fn add_months_clamp_is_not_composable_across_eom() {
        let jan31 = Date::from_ymd(2026, Month::Jan, 31).unwrap();
        // Two single-month hops: 31 → 28 → 28. Day-of-month sticks at 28.
        let two_hops = jan31.add_months(1).unwrap().add_months(1).unwrap();
        assert_eq!(two_hops, Date::from_ymd(2026, Month::Mar, 28).unwrap());
        // One two-month hop: 31 → 31 (March has 31 days).
        let single_hop = jan31.add_months(2).unwrap();
        assert_eq!(single_hop, Date::from_ymd(2026, Month::Mar, 31).unwrap());
        // The two paths disagree by 3 days.
        assert_ne!(two_hops, single_hop);
    }

    #[test]
    fn add_months_crosses_year_boundaries() {
        let nov15 = Date::from_ymd(2026, Month::Nov, 15).unwrap();
        assert_eq!(
            nov15.add_months(3).unwrap(),
            Date::from_ymd(2027, Month::Feb, 15).unwrap()
        );
        assert_eq!(
            nov15.add_months(-11).unwrap(),
            Date::from_ymd(2025, Month::Dec, 15).unwrap()
        );
    }

    #[test]
    fn add_months_zero_is_identity() {
        let d = Date::from_ymd(2026, Month::Jul, 4).unwrap();
        assert_eq!(d.add_months(0).unwrap(), d);
    }

    #[test]
    fn add_months_refuses_out_of_range_result() {
        assert_eq!(Date::MAX.add_months(1), Err(TimeError::DateOutOfRange));
        assert_eq!(Date::MIN.add_months(-1), Err(TimeError::DateOutOfRange));
    }

    #[test]
    fn add_years_clamps_feb_29_in_non_leap_target() {
        let feb29 = Date::from_ymd(2024, Month::Feb, 29).unwrap();
        assert_eq!(
            feb29.add_years(1).unwrap(),
            Date::from_ymd(2025, Month::Feb, 28).unwrap()
        );
        assert_eq!(
            feb29.add_years(4).unwrap(),
            Date::from_ymd(2028, Month::Feb, 29).unwrap()
        );
    }

    #[test]
    fn end_of_month_examples() {
        // Jan 15 2024 → Jan 31 2024.
        let d = Date::from_ymd(2024, Month::Jan, 15).unwrap();
        assert_eq!(
            d.end_of_month(),
            Date::from_ymd(2024, Month::Jan, 31).unwrap()
        );
        // Feb 10 2024 (leap) → Feb 29 2024.
        let d = Date::from_ymd(2024, Month::Feb, 10).unwrap();
        assert_eq!(
            d.end_of_month(),
            Date::from_ymd(2024, Month::Feb, 29).unwrap()
        );
        // Feb 10 2025 (non-leap) → Feb 28 2025.
        let d = Date::from_ymd(2025, Month::Feb, 10).unwrap();
        assert_eq!(
            d.end_of_month(),
            Date::from_ymd(2025, Month::Feb, 28).unwrap()
        );
        // Dec 31 2199 (max) is already EoM.
        assert_eq!(Date::MAX.end_of_month(), Date::MAX);
        // Jan 1 1901 (min) → Jan 31 1901.
        assert_eq!(
            Date::MIN.end_of_month(),
            Date::from_ymd(1901, Month::Jan, 31).unwrap()
        );
    }

    #[test]
    fn is_end_of_month_examples() {
        assert!(
            Date::from_ymd(2024, Month::Feb, 29)
                .unwrap()
                .is_end_of_month()
        );
        assert!(
            !Date::from_ymd(2024, Month::Feb, 28)
                .unwrap()
                .is_end_of_month()
        );
        assert!(
            Date::from_ymd(2025, Month::Feb, 28)
                .unwrap()
                .is_end_of_month()
        );
        assert!(
            Date::from_ymd(2026, Month::Apr, 30)
                .unwrap()
                .is_end_of_month()
        );
        assert!(
            Date::from_ymd(2026, Month::May, 31)
                .unwrap()
                .is_end_of_month()
        );
    }

    #[test]
    fn add_period_dispatches_by_unit() {
        let start = Date::from_ymd(2026, Month::Jan, 15).unwrap();
        assert_eq!(
            (start + crate::Period::Days(1)).unwrap(),
            Date::from_ymd(2026, Month::Jan, 16).unwrap()
        );
        assert_eq!(
            (start + crate::Period::Weeks(2)).unwrap(),
            Date::from_ymd(2026, Month::Jan, 29).unwrap()
        );
        assert_eq!(
            (start + crate::Period::Months(3)).unwrap(),
            Date::from_ymd(2026, Month::Apr, 15).unwrap()
        );
        assert_eq!(
            (start + crate::Period::Years(1)).unwrap(),
            Date::from_ymd(2027, Month::Jan, 15).unwrap()
        );
    }

    #[test]
    fn sub_period_steps_backward() {
        let start = Date::from_ymd(2026, Month::Jul, 15).unwrap();
        assert_eq!(
            (start - crate::Period::Months(6)).unwrap(),
            Date::from_ymd(2026, Month::Jan, 15).unwrap(),
        );
        // Sub on a negative period steps forward.
        assert_eq!(
            (start - (-crate::Period::Months(6))).unwrap(),
            Date::from_ymd(2027, Month::Jan, 15).unwrap(),
        );
    }
}
