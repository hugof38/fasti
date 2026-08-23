//! [`HolidayCache`]: a caller-owned memo that resolves a [`Calendar`]'s
//! rules once per year instead of once per day.
//!
//! [`Calendar`] is a borrowed `Copy` view over `pub const` data, so it
//! cannot hold a cache of its own — no interior mutability, no lazy
//! init. This is where the memo lives instead: in a value the caller
//! creates, or inside the iterator returned by
//! [`Calendar::business_days`].

use crate::calendar::{Shape, Window, resolve};
use crate::{
    BusinessDayConvention, Calendar, Date, Month, Period, Rule, RuleDate, TimeError, WeekendShift,
    Year,
};

/// Days of padding kept before a cached year: a Monday of January 1 can
/// stand in for a Saturday three days into the previous December.
const PAD_BEFORE: u32 = 3;
/// Days of padding kept after it: a Friday of December 31 can stand in
/// for the Saturday of January 1.
const PAD_AFTER: u32 = 1;
/// Longest padded year: a leap year plus both pads.
const SPAN: usize = 366 + PAD_BEFORE as usize + PAD_AFTER as usize;
/// Words of bitmap per plane.
const WORDS: usize = SPAN.div_ceil(64);

/// One year's resolved rules, as three bit-planes over a padded span of
/// days. Every rule but [`Rule::Custom`] contributes here; an opaque
/// predicate is probed per day instead, so a calendar without one pays
/// nothing for the possibility.
#[derive(Debug, Clone, Copy)]
struct Planes {
    /// Serial of bit 0.
    base: u32,
    /// Days covered, including both pads.
    len: u32,
    /// Serial of the year's own first day — bit `PAD_BEFORE`, except
    /// where the pad ran off the start of the supported range.
    first: u32,
    /// Serial of the year's own last day.
    last: u32,
    /// A rule names this day outright.
    natural: [u64; WORDS],
    /// This day is a weekend day carrying a holiday that steps forwards.
    fwd: [u64; WORDS],
    /// ... that steps backwards.
    back: [u64; WORDS],
}

impl Planes {
    /// An unused slot: `first > last`, so it covers no date at all.
    const EMPTY: Self = Self {
        base: 0,
        len: 0,
        first: 1,
        last: 0,
        natural: [0; WORDS],
        fwd: [0; WORDS],
        back: [0; WORDS],
    };

    /// Resolve every non-opaque rule of `calendar` for `year`, together
    /// with the handful of days on either side a substitute can reach.
    fn build(calendar: Calendar<'_>, year: Year) -> Self {
        let Ok(jan1) = Date::from_ymd(year.get(), Month::Jan, 1) else {
            return Self::EMPTY;
        };
        let base = jan1.serial().saturating_sub(PAD_BEFORE);
        let last =
            (jan1.serial() + u32::from(year.length()) - 1 + PAD_AFTER).min(Date::MAX.serial());
        let mut planes = Self {
            base,
            len: last - base + 1,
            first: jan1.serial(),
            last: last.saturating_sub(PAD_AFTER),
            ..Self::EMPTY
        };
        // The pads belong to the neighbouring years, so their rules are
        // resolved too — a holiday's observed date can fall in a
        // different year from its natural one.
        let neighbours = [
            Year::new(year.get().saturating_sub(1)).ok(),
            Some(year),
            Year::new(year.get().saturating_add(1)).ok(),
        ];
        for named_year in neighbours.into_iter().flatten() {
            for rule in calendar.rules {
                if let RuleDate::On(named) = rule.natural_date(named_year) {
                    planes.mark(calendar, named, rule.weekend_shift());
                }
            }
        }
        planes
    }

    /// `true` iff `date` belongs to the year these planes resolve — and
    /// so iff every day a substitute decision for it reaches is inside
    /// the padded span. Two comparisons, where deriving the year is a
    /// search over three centuries.
    const fn covers(&self, date: Date) -> bool {
        date.serial() >= self.first && date.serial() <= self.last
    }

    /// Bit index of `date`, or [`None`] if it lies outside the span.
    const fn index(&self, date: Date) -> Option<usize> {
        let serial = date.serial();
        if serial < self.base || serial - self.base >= self.len {
            return None;
        }
        Some((serial - self.base) as usize)
    }

    /// Record a rule naming `date`, and which way it steps off it.
    fn mark(&mut self, calendar: Calendar<'_>, date: Date, shift: WeekendShift) {
        let Some(index) = self.index(date) else {
            return;
        };
        let (word, bit) = (index / 64, 1u64 << (index % 64));
        self.natural[word] |= bit;
        match calendar.steps(date, shift) {
            Some(1) => self.fwd[word] |= bit,
            Some(-1) => self.back[word] |= bit,
            _ => {}
        }
    }

    /// Read one plane for `date`; days outside the span are named by
    /// nothing, exactly as an out-of-range day is.
    fn get(&self, plane: &[u64; WORDS], date: Date) -> bool {
        self.index(date)
            .is_some_and(|i| plane[i / 64] & (1u64 << (i % 64)) != 0)
    }
}

/// A [`Calendar`]'s holidays, resolved a year at a time and reused.
///
/// Every rule but [`Rule::Custom`] names at most one date per year and
/// can compute it outright, so resolving a year up front turns each
/// later query into a bit test. Answers are identical to
/// [`Calendar::is_holiday`] for every date; only the cost differs.
///
/// Build one per hot loop and let it live as long as the loop does.
/// [`Calendar::business_days`] keeps one internally, so a range walk
/// needs nothing extra.
///
/// ```
/// use fasti::{Date, HolidayCache, Month, calendars};
///
/// let mut cache = HolidayCache::new(calendars::us::SETTLEMENT);
/// let mut open = 0;
/// let mut day = Date::from_ymd(2026, Month::Jan, 1)?;
/// let end = Date::from_ymd(2027, Month::Jan, 1)?;
/// while day < end {
///     if cache.is_business_day(day) {
///         open += 1;
///     }
///     day = day.add_days(1)?;
/// }
/// assert_eq!(open, 250);
/// # Ok::<(), fasti::TimeError>(())
/// ```
#[derive(Debug, Clone)]
pub struct HolidayCache<'a> {
    calendar: Calendar<'a>,
    /// Whether any rule steps off a weekend; if none does, no query ever
    /// looks at a neighbouring day.
    any_shift: bool,
    /// Whether any rule is opaque and so has to be probed per day.
    any_custom: bool,
    /// Two years live at once, so that walking a range backwards and
    /// forwards across a year boundary does not rebuild on every step.
    slots: [Planes; 2],
    /// Which slot the next miss evicts.
    next: usize,
}

impl<'a> HolidayCache<'a> {
    /// Build a cache for `calendar`. Nothing is resolved until the first
    /// query names a year.
    ///
    /// ```
    /// use fasti::{Date, HolidayCache, Month, calendars};
    /// let mut cache = HolidayCache::new(calendars::uk::SETTLEMENT);
    /// // Christmas 2021 fell on a Saturday; the Monday and Tuesday are
    /// // both granted, exactly as `Calendar::is_holiday` grants them.
    /// assert!(cache.is_holiday(Date::from_ymd(2021, Month::Dec, 27)?));
    /// assert!(cache.is_holiday(Date::from_ymd(2021, Month::Dec, 28)?));
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    #[must_use]
    pub fn new(calendar: Calendar<'a>) -> Self {
        Self {
            calendar,
            any_shift: calendar
                .rules
                .iter()
                .any(|r| !matches!(r.weekend_shift(), WeekendShift::None)),
            any_custom: calendar.rules.iter().any(|r| matches!(r, Rule::Custom(_))),
            slots: [Planes::EMPTY; 2],
            next: 0,
        }
    }

    /// The calendar this cache resolves.
    #[must_use]
    pub const fn calendar(&self) -> Calendar<'a> {
        self.calendar
    }

    /// The weekend configuration's verdict on `date` — a plain forward
    /// to [`Calendar::is_weekend`], which needs no memo.
    #[must_use]
    pub const fn is_weekend(&self, date: Date) -> bool {
        self.calendar.is_weekend(date)
    }

    /// As [`Calendar::is_holiday`], from the resolved year.
    pub fn is_holiday(&mut self, date: Date) -> bool {
        if self.calendar.rules.is_empty() {
            return false;
        }
        let weekday = date.weekday();
        let Some(shape) = Shape::of(weekday) else {
            return self.natural(date);
        };
        if self.calendar.is_weekend(date) || !self.any_shift {
            return self.natural(date);
        }
        let window = self.window(date, shape);
        resolve(weekday, window)
    }

    /// As [`Calendar::is_business_day`], from the resolved year.
    pub fn is_business_day(&mut self, date: Date) -> bool {
        !self.calendar.is_weekend(date) && !self.is_holiday(date)
    }

    /// As [`Calendar::next_business_day`], from the resolved year.
    pub fn next_business_day(&mut self, date: Date) -> Option<Date> {
        let mut d = date.add_days(1).ok()?;
        while !self.is_business_day(d) {
            d = d.add_days(1).ok()?;
        }
        Some(d)
    }

    /// As [`Calendar::prev_business_day`], from the resolved year.
    pub fn prev_business_day(&mut self, date: Date) -> Option<Date> {
        let mut d = date.add_days(-1).ok()?;
        while !self.is_business_day(d) {
            d = d.add_days(-1).ok()?;
        }
        Some(d)
    }

    /// As [`Calendar::adjust`], from the resolved year — the method that
    /// gains most, since a single adjustment evaluates the predicate
    /// three or four times.
    ///
    /// ```
    /// use fasti::{BusinessDayConvention, Date, HolidayCache, Month, calendars};
    /// let mut cache = HolidayCache::new(calendars::us::SETTLEMENT);
    /// // Sat Jul 4 2026 is observed on the Friday, so Following lands
    /// // on the Monday.
    /// let sat = Date::from_ymd(2026, Month::Jul, 4)?;
    /// assert_eq!(
    ///     cache.adjust(sat, BusinessDayConvention::Following)?,
    ///     Date::from_ymd(2026, Month::Jul, 6)?,
    /// );
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    pub fn adjust(
        &mut self,
        date: Date,
        convention: BusinessDayConvention,
    ) -> Result<Date, TimeError> {
        if self.is_business_day(date) {
            return Ok(date);
        }
        match convention {
            BusinessDayConvention::Unadjusted => Ok(date),
            BusinessDayConvention::Following => self
                .next_business_day(date)
                .ok_or(TimeError::DateOutOfRange),
            BusinessDayConvention::Preceding => self
                .prev_business_day(date)
                .ok_or(TimeError::DateOutOfRange),
            BusinessDayConvention::ModifiedFollowing => {
                let candidate = self
                    .next_business_day(date)
                    .ok_or(TimeError::DateOutOfRange)?;
                if candidate.month() == date.month() && candidate.year() == date.year() {
                    Ok(candidate)
                } else {
                    self.prev_business_day(date)
                        .ok_or(TimeError::DateOutOfRange)
                }
            }
            BusinessDayConvention::ModifiedPreceding => {
                let candidate = self
                    .prev_business_day(date)
                    .ok_or(TimeError::DateOutOfRange)?;
                if candidate.month() == date.month() && candidate.year() == date.year() {
                    Ok(candidate)
                } else {
                    self.next_business_day(date)
                        .ok_or(TimeError::DateOutOfRange)
                }
            }
        }
    }

    /// As [`Calendar::advance`], from the resolved year.
    pub fn advance(
        &mut self,
        date: Date,
        period: Period,
        convention: BusinessDayConvention,
        end_of_month: bool,
    ) -> Result<Date, TimeError> {
        self.adjust(date.advance(period, end_of_month)?, convention)
    }

    /// `true` iff a rule names `date` outright: a bit test, plus a probe
    /// of the opaque rules if the calendar has any.
    fn natural(&mut self, date: Date) -> bool {
        let slot = self.slot_for(date);
        let natural = self.slots[slot].get(&self.slots[slot].natural, date);
        natural || self.probe(date)
    }

    /// Probe the calendar's [`Rule::Custom`] predicates for `date`.
    fn probe(&self, date: Date) -> bool {
        self.any_custom
            && self.calendar.rules.iter().any(|rule| match rule {
                Rule::Custom(f) => f(date),
                _ => false,
            })
    }

    /// The window a substitute decision reads, from the resolved planes.
    fn window(&mut self, date: Date, shape: Shape) -> Window {
        let slot = self.slot_for(date);
        let planes = self.slots[slot];
        let mut window = Window::default();
        for i in 0..shape.len {
            // `shape.len` is at most 4, so neither cast can lose data.
            #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
            let (bit, offset) = (1u8 << (i as u8), shape.lo + i as i32);
            let Ok(day) = date.add_days(offset) else {
                continue;
            };
            if planes.get(&planes.natural, day)
                || (shape.natural_mask & bit != 0 && self.probe(day))
            {
                window.natural |= bit;
            }
            if planes.get(&planes.fwd, day) {
                window.fwd |= bit;
            }
            if planes.get(&planes.back, day) {
                window.back |= bit;
            }
        }
        window
    }

    /// The slot holding `date`'s year, building it if neither slot does.
    fn slot_for(&mut self, date: Date) -> usize {
        if self.slots[0].covers(date) {
            return 0;
        }
        if self.slots[1].covers(date) {
            return 1;
        }
        let year = date.year();
        let slot = self.next;
        self.next ^= 1;
        self.slots[slot] = Planes::build(self.calendar, year);
        slot
    }
}

impl<'a> From<Calendar<'a>> for HolidayCache<'a> {
    fn from(calendar: Calendar<'a>) -> Self {
        Self::new(calendar)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::{Weekend, calendars};

    fn ymd(y: u16, m: Month, d: u8) -> Date {
        Date::from_ymd(y, m, d).unwrap()
    }

    #[test]
    fn agrees_with_the_calendar_across_a_year_boundary() {
        // Jan 1 2022 was a Saturday, observed on Fri Dec 31 2021 — a
        // holiday whose observed date is in a different year from its
        // natural one, and so in a different cached year.
        let cal = calendars::us::SETTLEMENT;
        let mut cache = HolidayCache::new(cal);
        for offset in -10..=10 {
            let d = ymd(2022, Month::Jan, 1).add_days(offset).unwrap();
            assert_eq!(cache.is_holiday(d), cal.is_holiday(d), "{d}");
        }
        assert!(cache.is_holiday(ymd(2021, Month::Dec, 31)));
    }

    #[test]
    fn two_slots_survive_alternating_years() {
        // A DoubleEndedIterator consumed from both ends alternates
        // between two distant years; neither may evict the other.
        let cal = calendars::uk::SETTLEMENT;
        let mut cache = HolidayCache::new(cal);
        for i in 0..50 {
            let front = ymd(1950, Month::Jan, 1).add_days(i).unwrap();
            let back = ymd(2199, Month::Nov, 1).add_days(i).unwrap();
            assert_eq!(cache.is_business_day(front), cal.is_business_day(front));
            assert_eq!(cache.is_business_day(back), cal.is_business_day(back));
        }
    }

    #[test]
    fn custom_rules_are_probed_not_precomputed() {
        // NYSE's election-day and paperwork-crisis rules are opaque.
        let cal = calendars::us::NYSE;
        let mut cache = HolidayCache::new(cal);
        assert!(cache.any_custom);
        assert!(cache.is_holiday(ymd(1968, Month::Nov, 5))); // election day
        assert!(cache.is_holiday(ymd(1968, Month::Jun, 12))); // paperwork Wednesday
        assert!(!cache.is_holiday(ymd(1968, Month::Jun, 11)));
    }

    #[test]
    fn edges_of_the_supported_range_resolve() {
        let cal = calendars::us::SETTLEMENT;
        let mut cache = HolidayCache::new(cal);
        for d in [Date::MIN, Date::MIN.add_days(1).unwrap(), Date::MAX] {
            assert_eq!(cache.is_holiday(d), cal.is_holiday(d), "{d}");
        }
    }

    #[test]
    fn adjust_and_advance_mirror_the_calendar() {
        let cal = calendars::us::SETTLEMENT;
        let mut cache = HolidayCache::new(cal);
        let mut d = ymd(2020, Month::Jan, 1);
        let end = ymd(2021, Month::Jan, 1);
        while d < end {
            for conv in [
                BusinessDayConvention::Following,
                BusinessDayConvention::ModifiedFollowing,
                BusinessDayConvention::Preceding,
                BusinessDayConvention::ModifiedPreceding,
                BusinessDayConvention::Unadjusted,
            ] {
                assert_eq!(cache.adjust(d, conv), cal.adjust(d, conv), "{d} {conv:?}");
            }
            assert_eq!(
                cache.advance(
                    d,
                    Period::Months(3),
                    BusinessDayConvention::Following,
                    false
                ),
                cal.advance(
                    d,
                    Period::Months(3),
                    BusinessDayConvention::Following,
                    false
                ),
            );
            assert_eq!(cache.next_business_day(d), cal.next_business_day(d));
            assert_eq!(cache.prev_business_day(d), cal.prev_business_day(d));
            d = d.add_days(1).unwrap();
        }
    }

    #[test]
    fn from_calendar_and_accessors() {
        let cache: HolidayCache<'_> = calendars::TARGET.into();
        assert_eq!(cache.calendar().name, calendars::TARGET.name);
        assert!(cache.is_weekend(ymd(2026, Month::Jul, 4)));
        assert!(!cache.any_shift);
        assert_eq!(Weekend::SAT_SUN, cache.calendar().weekend);
    }
}
