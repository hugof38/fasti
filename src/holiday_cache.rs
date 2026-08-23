//! The per-year memo behind [`Calendar::business_days`] and
//! [`Calendar::holidays`].
//!
//! `Calendar` is a borrowed `Copy` view over `pub const` data, so it can
//! hold no cache of its own — no interior mutability, no lazy init. The
//! memo lives in the iterator's state instead, resolving each year's
//! rules once into three bit-planes so a day costs a bit test. Callers
//! who want the same for their own loops have the same materials:
//! [`Rule::natural_date`] is what this is built on.

use crate::calendar::{Shape, Window, resolve};
use crate::{Calendar, Date, Month, Rule, RuleDate, WeekendShift, Year};

/// Days kept before a cached year: a Monday of January 1 can stand in
/// for a Saturday three days into the previous December.
const PAD_BEFORE: u32 = 3;
/// Days kept after it: a Friday of December 31 can stand in for the
/// Saturday of January 1.
const PAD_AFTER: u32 = 1;
/// Longest padded year: a leap year plus both pads.
const SPAN: usize = 366 + PAD_BEFORE as usize + PAD_AFTER as usize;
/// Words of bitmap per plane.
const WORDS: usize = SPAN.div_ceil(64);

/// One year's resolved rules as three bit-planes over a padded span.
/// Every rule but [`Rule::Custom`] contributes here; an opaque predicate
/// is probed per day instead, so a calendar without one pays nothing for
/// the possibility.
#[derive(Debug, Clone, Copy)]
struct Planes {
    /// Serial of bit 0.
    base: u32,
    /// Days covered, both pads included.
    len: u32,
    /// Serial of the year's own first day, and of its last: a date
    /// between them belongs to this year, which is the hit test.
    first: u32,
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

    /// Resolve every non-opaque rule of `calendar` for `year`, and for
    /// the days on either side a substitute can reach — a holiday's
    /// observed date can fall in a different year from its natural one.
    fn build(calendar: Calendar<'_>, year: Year) -> Self {
        let Ok(jan1) = Date::from_ymd(year.get(), Month::Jan, 1) else {
            return Self::EMPTY;
        };
        let base = jan1.serial().saturating_sub(PAD_BEFORE);
        let last = jan1.serial() + u32::from(year.length()) - 1;
        let span_last = (last + PAD_AFTER).min(Date::MAX.serial());
        let mut planes = Self {
            base,
            len: span_last - base + 1,
            first: jan1.serial(),
            last,
            ..Self::EMPTY
        };
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
    /// the padded span. Two comparisons, against a division and a table
    /// probe to derive the year.
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

    /// Read one plane; a day outside the span is named by nothing, as an
    /// out-of-range day is.
    fn get(&self, plane: &[u64; WORDS], date: Date) -> bool {
        self.index(date)
            .is_some_and(|i| plane[i / 64] & (1u64 << (i % 64)) != 0)
    }
}

/// A calendar's holidays, resolved a year at a time. Answers are
/// identical to [`Calendar::is_holiday`] for every date; only the cost
/// differs.
#[derive(Debug, Clone)]
pub(crate) struct HolidayCache<'a> {
    calendar: Calendar<'a>,
    /// Whether any rule steps off a weekend; if none does, no query ever
    /// looks at a neighbouring day.
    any_shift: bool,
    /// Whether any rule is opaque and so has to be probed per day.
    any_custom: bool,
    /// Two years live at once, so a walk consumed from both ends does
    /// not rebuild on every step.
    slots: [Planes; 2],
    /// Which slot the next miss evicts.
    next: usize,
}

impl<'a> HolidayCache<'a> {
    /// A cache for `calendar`. Nothing is resolved until a query names a
    /// year.
    pub(crate) fn new(calendar: Calendar<'a>) -> Self {
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

    /// As [`Calendar::is_holiday`], from the resolved year.
    pub(crate) fn is_holiday(&mut self, date: Date) -> bool {
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
    pub(crate) fn is_business_day(&mut self, date: Date) -> bool {
        !self.calendar.is_weekend(date) && !self.is_holiday(date)
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
        let mut window = Window::EMPTY;
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::calendars;

    fn ymd(y: u16, m: Month, d: u8) -> Date {
        Date::from_ymd(y, m, d).unwrap()
    }

    /// `tests/equivalence.rs` holds this to the original algorithm over
    /// the whole supported range; these pin the cases the structure
    /// exists for.
    #[test]
    fn agrees_with_the_calendar_across_a_year_boundary() {
        // Jan 1 2022 was a Saturday, observed on Fri Dec 31 2021 — an
        // observed date in a different year from its natural one, and so
        // in a different cached year.
        let cal = calendars::us::SETTLEMENT;
        let mut cache = HolidayCache::new(cal);
        for offset in -10..=10 {
            let d = ymd(2022, Month::Jan, 1).add_days(offset).unwrap();
            assert_eq!(cache.is_holiday(d), cal.is_holiday(d), "{d}");
        }
        assert!(cache.is_holiday(ymd(2021, Month::Dec, 31)));
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
        assert!(!HolidayCache::new(calendars::TARGET).any_custom);
    }
}
