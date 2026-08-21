//! [`Calendar`]: a weekend definition + a sequence of holiday [`Rule`]s,
//! and the substitute-day resolution that turns a rule's
//! [`WeekendShift`] into an observed date.
//!
//! `Calendar<'a>` is a borrowed view; built-in calendars are `pub const`
//! (`'a = 'static`), and [`CalendarBuilder`] produces calendars borrowing
//! from its own owned storage.

use alloc::borrow::ToOwned;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::ops::Range;

use crate::{
    BusinessDayConvention, Date, DateRange, Period, Rule, TimeError, Weekend, WeekendShift,
};

/// A holiday calendar: a [`Weekend`] configuration plus a sequence of
/// [`Rule`]s naming holidays' natural dates.
///
/// ```no_run
/// use fasti::{Calendar, Date, Month, Weekend};
///
/// // Trivial weekends-only calendar.
/// const WEEKENDS_ONLY: Calendar<'static> = Calendar {
///     name: "Weekends Only",
///     weekend: Weekend::SAT_SUN,
///     rules: &[],
/// };
///
/// let d = Date::from_ymd(2024, Month::Jan, 6).unwrap();     // Saturday
/// assert!(!WEEKENDS_ONLY.is_business_day(d));
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Calendar<'a> {
    /// Human-readable name for the calendar, e.g. `"US Federal"`.
    pub name: &'a str,
    /// The weekly weekend.
    pub weekend: Weekend,
    /// Holiday rules, each naming a holiday's natural date.
    pub rules: &'a [Rule],
}

impl Calendar<'_> {
    /// `true` iff `date` falls on a weekend day under this calendar.
    #[must_use]
    pub const fn is_weekend(&self, date: Date) -> bool {
        self.weekend.contains(date.weekday())
    }

    /// `true` iff `date` is a holiday — either a rule's natural date,
    /// or the substitute weekday granted to a holiday that fell on a
    /// weekend. Does not consider weekends.
    ///
    /// Rules name natural dates; a [`WeekendShift`] names only a
    /// direction. Turning that into a date is the calendar's job,
    /// because a substitute may not land on a day another holiday has
    /// already taken — the reason Christmas on a Saturday sends Boxing
    /// Day's substitute to the Tuesday.
    ///
    /// ```
    /// use fasti::{Date, Month, calendars};
    /// let uk = calendars::uk::SETTLEMENT;
    /// // Christmas 2021 fell on a Saturday. It keeps its natural date
    /// // and gains the Monday; Boxing Day is pushed on to the Tuesday.
    /// assert!(uk.is_holiday(Date::from_ymd(2021, Month::Dec, 25)?));
    /// assert!(uk.is_holiday(Date::from_ymd(2021, Month::Dec, 27)?));
    /// assert!(uk.is_holiday(Date::from_ymd(2021, Month::Dec, 28)?));
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    #[must_use]
    pub fn is_holiday(&self, date: Date) -> bool {
        self.is_natural_holiday(date) || self.is_substitute(date)
    }

    /// `true` iff a rule names `date` outright, before any shift.
    fn is_natural_holiday(&self, date: Date) -> bool {
        self.rules.iter().any(|r| r.is_holiday(date))
    }

    /// `true` iff `date` is the substitute day for a weekend holiday.
    ///
    /// Shifts are defined on Saturday and Sunday, so there is nothing
    /// to search for: forward substitutes queue off the weekend just
    /// behind `date`, a backward one off the Saturday just ahead, and
    /// both anchors are weekday arithmetic.
    fn is_substitute(&self, date: Date) -> bool {
        let shifts = |r: &Rule| !matches!(r.weekend_shift(), WeekendShift::None);
        if self.is_weekend(date) || !self.rules.iter().any(shifts) {
            return false;
        }
        // ISO weekday: Mon = 1 ..= Sun = 7.
        let w = i32::from(date.weekday().get());
        self.owed(date, -w, 1) || self.owed(date, 6 - w, -1)
    }

    /// `true` iff the weekend day `offset` days from `date` — with its
    /// neighbour one step further out — owes more substitutes than the
    /// free weekdays between them and `date` can absorb.
    ///
    /// A weekend is two days — the array below — so at most two
    /// substitutes queue, and `nth` never examines more free weekdays
    /// than are owed.
    fn owed(&self, date: Date, offset: i32, step: i32) -> bool {
        let Ok(near) = date.add_days(offset) else {
            return false;
        };
        let owed = [near.add_days(-step).ok(), Some(near)]
            .into_iter()
            .flatten()
            .filter(|d| self.moves(*d, step))
            .count();
        // `date` is served iff the movers outnumber the free weekdays
        // ahead of it in the queue.
        owed.checked_sub(1).is_some_and(|filled| {
            (1..offset.abs())
                .filter_map(|i| near.add_days(step * i).ok())
                .filter(|d| !self.is_weekend(*d) && !self.is_natural_holiday(*d))
                .nth(filled)
                .is_none()
        })
    }

    /// `true` iff `day` is a weekend day carrying a holiday that steps
    /// `step`.
    fn moves(&self, day: Date, step: i32) -> bool {
        self.is_weekend(day)
            && self.rules.iter().any(|r| {
                r.weekend_shift().direction(day.weekday()) == Some(step) && r.is_holiday(day)
            })
    }

    /// `true` iff `date` is neither a weekend nor a holiday.
    #[must_use]
    pub fn is_business_day(&self, date: Date) -> bool {
        !self.is_weekend(date) && !self.is_holiday(date)
    }

    /// The business days in `range`, ascending; the end bound is
    /// excluded. Count them with `.count()`, collect them with
    /// `.collect()`.
    ///
    /// ```
    /// use fasti::{Date, Month, calendars};
    /// // Jul 2024 has 23 weekdays.
    /// let jul = Date::from_ymd(2024, Month::Jul, 1)?..Date::from_ymd(2024, Month::Aug, 1)?;
    /// assert_eq!(calendars::WEEKENDS_ONLY.business_days(jul).count(), 23);
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    pub fn business_days(&self, range: Range<Date>) -> impl DoubleEndedIterator<Item = Date> {
        range.dates().filter(|d| self.is_business_day(*d))
    }

    /// The holidays in `range`, ascending. Weekends are excluded,
    /// matching [`is_holiday`](Self::is_holiday).
    pub fn holidays(&self, range: Range<Date>) -> impl DoubleEndedIterator<Item = Date> {
        range.dates().filter(|d| self.is_holiday(*d))
    }

    /// The first business day of `date`'s month, or [`None`] if the
    /// month has none.
    ///
    /// ```
    /// use fasti::{Date, Month, calendars};
    /// // Mar 2026 opens on a Sunday.
    /// let d = Date::from_ymd(2026, Month::Mar, 18)?;
    /// assert_eq!(
    ///     calendars::WEEKENDS_ONLY.first_business_day_of_month(d),
    ///     Some(Date::from_ymd(2026, Month::Mar, 2)?),
    /// );
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    #[must_use]
    pub fn first_business_day_of_month(&self, date: Date) -> Option<Date> {
        self.adjust(date.start_of_month(), BusinessDayConvention::Following)
            .ok()
            .filter(|d| d.month() == date.month())
    }

    /// The last business day of `date`'s month, or [`None`] if the
    /// month has none.
    #[must_use]
    pub fn last_business_day_of_month(&self, date: Date) -> Option<Date> {
        self.adjust(date.end_of_month(), BusinessDayConvention::Preceding)
            .ok()
            .filter(|d| d.month() == date.month())
    }

    /// The next business day strictly after `date`.
    ///
    /// Returns [`None`] if the search would run past [`Date::MAX`].
    #[must_use]
    pub fn next_business_day(&self, date: Date) -> Option<Date> {
        let mut d = date.add_days(1).ok()?;
        while !self.is_business_day(d) {
            d = d.add_days(1).ok()?;
        }
        Some(d)
    }

    /// The previous business day strictly before `date`.
    #[must_use]
    pub fn prev_business_day(&self, date: Date) -> Option<Date> {
        let mut d = date.add_days(-1).ok()?;
        while !self.is_business_day(d) {
            d = d.add_days(-1).ok()?;
        }
        Some(d)
    }

    /// Roll `date` onto a business day according to `convention`; a
    /// business day is returned unchanged. Returns
    /// [`TimeError::DateOutOfRange`] if the search leaves the supported range.
    ///
    /// ```
    /// use fasti::{BusinessDayConvention, Date, Month, calendars};
    ///
    /// // Sun Aug 31 2025: Following crosses into Sep, ModifiedFollowing
    /// // falls back to Fri Aug 29.
    /// let sun = Date::from_ymd(2025, Month::Aug, 31)?;
    /// assert_eq!(
    ///     calendars::WEEKENDS_ONLY.adjust(sun, BusinessDayConvention::Following)?,
    ///     Date::from_ymd(2025, Month::Sep, 1)?,
    /// );
    /// assert_eq!(
    ///     calendars::WEEKENDS_ONLY.adjust(sun, BusinessDayConvention::ModifiedFollowing)?,
    ///     Date::from_ymd(2025, Month::Aug, 29)?,
    /// );
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    pub fn adjust(&self, date: Date, convention: BusinessDayConvention) -> Result<Date, TimeError> {
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

    /// Step `date` forward (or backward) by `period`, then roll onto a
    /// business day under `convention`. Returns
    /// [`TimeError::DateOutOfRange`] if the arithmetic or search leaves the supported range.
    ///
    /// If `end_of_month` is set, a `Months`/`Years` step from a month-end snaps to the target month's end before adjusting. Matches `QuantLib`'s semantics.
    ///
    /// ```
    /// use fasti::{BusinessDayConvention, Date, Month, Period, calendars};
    /// let apr_eom = Date::from_ymd(2025, Month::Apr, 30)?;
    /// assert_eq!(
    ///     calendars::WEEKENDS_ONLY.advance(
    ///         apr_eom,
    ///         Period::Months(1),
    ///         BusinessDayConvention::ModifiedFollowing,
    ///         true,
    ///     )?,
    ///     Date::from_ymd(2025, Month::May, 30)?,
    /// );
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    pub fn advance(
        &self,
        date: Date,
        period: Period,
        convention: BusinessDayConvention,
        end_of_month: bool,
    ) -> Result<Date, TimeError> {
        self.adjust(date.advance(period, end_of_month)?, convention)
    }
}

/// Owned counterpart to [`Calendar`]; [`view`](Self::view) produces a
/// borrowed [`Calendar`] backed by this builder's storage.
///
/// ```
/// use fasti::{CalendarBuilder, Date, FixedDate, Month, OneOff, Rule, Weekend};
///
/// let blackout = CalendarBuilder::new("Acme Blackouts", Weekend::SAT_SUN)
///     .with_rule(Rule::OneOff(OneOff::new(Date::from_ymd(2026, Month::Aug, 15)?)))
///     .with_rule(Rule::OneOff(OneOff::new(Date::from_ymd(2026, Month::Dec, 24)?)));
///
/// let cal = blackout.view();
/// assert!(cal.is_holiday(Date::from_ymd(2026, Month::Aug, 15)?));
/// # Ok::<(), fasti::TimeError>(())
/// ```
#[derive(Debug, Clone)]
pub struct CalendarBuilder {
    name: String,
    weekend: Weekend,
    rules: Vec<Rule>,
}

impl CalendarBuilder {
    /// Start a builder with the given name and weekend. No rules yet.
    #[must_use]
    pub fn new(name: impl Into<String>, weekend: Weekend) -> Self {
        Self {
            name: name.into(),
            weekend,
            rules: Vec::new(),
        }
    }

    /// Seed a builder from a [`Calendar`], e.g. a built-in to extend
    /// with bespoke blackout days.
    #[must_use]
    pub fn from_calendar(cal: Calendar<'_>) -> Self {
        Self {
            name: cal.name.to_owned(),
            weekend: cal.weekend,
            rules: cal.rules.to_vec(),
        }
    }

    /// Rename the calendar.
    #[must_use]
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Replace the weekend configuration.
    #[must_use]
    pub fn with_weekend(mut self, weekend: Weekend) -> Self {
        self.weekend = weekend;
        self
    }

    /// Append a holiday rule.
    #[must_use]
    pub fn with_rule(mut self, rule: Rule) -> Self {
        self.rules.push(rule);
        self
    }

    /// Merge `other` in: a date is a holiday if either side says so,
    /// and the weekends union. `QuantLib`'s `JointCalendar` under
    /// `JoinHolidays`.
    ///
    /// ```
    /// use fasti::{CalendarBuilder, Date, Month, calendars};
    ///
    /// let joint = CalendarBuilder::from_calendar(calendars::us::SETTLEMENT)
    ///     .union(calendars::france::SETTLEMENT);
    /// let cal = joint.view();
    /// assert!(cal.is_holiday(Date::from_ymd(2026, Month::Nov, 26)?)); // Thanksgiving
    /// assert!(cal.is_holiday(Date::from_ymd(2026, Month::Jul, 14)?)); // Bastille Day
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    #[must_use]
    pub fn union(mut self, other: Calendar<'_>) -> Self {
        self.name = format!("{} + {}", self.name, other.name);
        self.weekend = self.weekend | other.weekend;
        self.rules.extend_from_slice(other.rules);
        self
    }

    /// Produce a borrowed [`Calendar`] backed by this builder's storage.
    /// The view lives as long as `&self`.
    #[must_use]
    pub fn view(&self) -> Calendar<'_> {
        Calendar {
            name: &self.name,
            weekend: self.weekend,
            rules: &self.rules,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::{FixedDate, Month, OneOff, WeekendShift};

    fn ymd(y: u16, m: Month, d: u8) -> Date {
        Date::from_ymd(y, m, d).unwrap()
    }

    #[test]
    fn weekends_only_calendar() {
        const CAL: Calendar<'static> = Calendar {
            name: "Weekends Only",
            weekend: Weekend::SAT_SUN,
            rules: &[],
        };
        assert!(CAL.is_business_day(ymd(2024, Month::Jan, 2))); // Tuesday
        assert!(!CAL.is_business_day(ymd(2024, Month::Jan, 6))); // Saturday
        assert!(!CAL.is_business_day(ymd(2024, Month::Jan, 7))); // Sunday
    }

    #[test]
    fn holiday_vs_weekend_distinct() {
        const CAL: Calendar<'static> = Calendar {
            name: "Test",
            weekend: Weekend::SAT_SUN,
            rules: &[Rule::Fixed(FixedDate::new(Month::Jul, 4))],
        };
        let independence = ymd(2024, Month::Jul, 4); // Thursday
        let sat = ymd(2024, Month::Jul, 6);
        assert!(CAL.is_holiday(independence));
        assert!(!CAL.is_weekend(independence));
        assert!(!CAL.is_holiday(sat));
        assert!(CAL.is_weekend(sat));
    }

    #[test]
    fn next_and_prev_business_day() {
        const CAL: Calendar<'static> = Calendar {
            name: "Test",
            weekend: Weekend::SAT_SUN,
            rules: &[Rule::Fixed(FixedDate::new(Month::Jul, 4))],
        };
        // Friday → Monday (skip weekend).
        let fri = ymd(2024, Month::Jul, 5);
        assert_eq!(CAL.next_business_day(fri), Some(ymd(2024, Month::Jul, 8)));
        // Friday July 5 → Wednesday July 3 (skip Thursday holiday).
        assert_eq!(CAL.prev_business_day(fri), Some(ymd(2024, Month::Jul, 3)));
        // Idempotent-like: prev of Monday after weekend goes to previous Friday.
        let mon = ymd(2024, Month::Jan, 8);
        assert_eq!(CAL.prev_business_day(mon), Some(ymd(2024, Month::Jan, 5)));
    }

    #[test]
    fn builder_extends_a_built_in_with_one_offs() {
        const BASE: Calendar<'static> = Calendar {
            name: "Base",
            weekend: Weekend::SAT_SUN,
            rules: &[Rule::Fixed(
                FixedDate::new(Month::Jul, 4).shift(WeekendShift::SatBackSunForward),
            )],
        };
        let blackout = ymd(2026, Month::Aug, 15);
        let builder = CalendarBuilder::from_calendar(BASE)
            .name("Base + Blackout")
            .with_rule(Rule::OneOff(OneOff::new(blackout)));
        let cal = builder.view();
        assert_eq!(cal.name, "Base + Blackout");
        assert!(cal.is_holiday(blackout));
        // Base rule still there.
        assert!(cal.is_holiday(ymd(2024, Month::Jul, 4)));
    }

    #[test]
    fn builder_view_lifetime() {
        let builder = CalendarBuilder::new("Test", Weekend::SAT_SUN);
        // The view borrows from the builder; usable within the same scope.
        let cal = builder.view();
        assert_eq!(cal.name, "Test");
        assert!(cal.rules.is_empty());
    }

    // ---- substitute days -------------------------------------------------

    /// Two adjacent holidays, both taking the next free weekday — the
    /// UK Christmas/Boxing Day shape.
    const PAIR: Calendar<'static> = Calendar {
        name: "Pair",
        weekend: Weekend::SAT_SUN,
        rules: &[
            Rule::Fixed(FixedDate::new(Month::Dec, 25).shift(WeekendShift::Forward)),
            Rule::Fixed(FixedDate::new(Month::Dec, 26).shift(WeekendShift::Forward)),
        ],
    };

    #[test]
    fn a_substitute_skips_a_day_an_earlier_one_took() {
        // 2021: Dec 25 Sat → Mon 27; Dec 26 Sun would also want Mon 27,
        // so it is pushed to Tue 28.
        assert!(PAIR.is_holiday(ymd(2021, Month::Dec, 27)));
        assert!(PAIR.is_holiday(ymd(2021, Month::Dec, 28)));
        assert!(PAIR.is_business_day(ymd(2021, Month::Dec, 29)));
    }

    #[test]
    fn two_rules_on_one_day_owe_one_substitute() {
        // Both rules name Dec 25 — the shape `CalendarBuilder::union`
        // produces when two calendars share a holiday. That is one
        // holiday owed one day off, not two, so Tuesday stays open.
        const DOUBLED: Calendar<'static> = Calendar {
            name: "Doubled",
            weekend: Weekend::SAT_SUN,
            rules: &[
                Rule::Fixed(FixedDate::new(Month::Dec, 25).shift(WeekendShift::Forward)),
                Rule::Fixed(FixedDate::new(Month::Dec, 25).shift(WeekendShift::Forward)),
            ],
        };
        // Dec 25 2022 was a Sunday.
        assert!(DOUBLED.is_holiday(ymd(2022, Month::Dec, 26)));
        assert!(DOUBLED.is_business_day(ymd(2022, Month::Dec, 27)));
    }

    #[test]
    fn a_substitute_skips_a_natural_holiday() {
        // 2022: Dec 25 Sun, Dec 26 Mon. Monday is already Boxing Day,
        // so Christmas lands on the Tuesday — after Boxing Day.
        assert!(PAIR.is_holiday(ymd(2022, Month::Dec, 26)));
        assert!(PAIR.is_holiday(ymd(2022, Month::Dec, 27)));
        assert!(PAIR.is_business_day(ymd(2022, Month::Dec, 28)));
    }

    #[test]
    fn the_natural_date_survives_alongside_its_substitute() {
        // Both are holidays; only the substitute is a day off, because
        // the natural date is a weekend anyway.
        let sat = ymd(2021, Month::Dec, 25);
        assert!(PAIR.is_holiday(sat));
        assert!(!PAIR.is_business_day(sat));
        assert!(PAIR.is_holiday(ymd(2021, Month::Dec, 27)));
    }

    #[test]
    fn two_substitutes_step_past_a_monday_that_is_itself_a_holiday() {
        // Jul 4 2026 is a Saturday, Jul 5 the Sunday, Jul 6 a Monday
        // holiday in its own right. The weekend still owes two days
        // off, so they land on the Tuesday and the Wednesday.
        const BLOCKED: Calendar<'static> = Calendar {
            name: "Blocked Monday",
            weekend: Weekend::SAT_SUN,
            rules: &[
                Rule::Fixed(FixedDate::new(Month::Jul, 4).shift(WeekendShift::Forward)),
                Rule::Fixed(FixedDate::new(Month::Jul, 5).shift(WeekendShift::Forward)),
                Rule::Fixed(FixedDate::new(Month::Jul, 6)),
            ],
        };
        assert!(BLOCKED.is_holiday(ymd(2026, Month::Jul, 7)));
        assert!(BLOCKED.is_holiday(ymd(2026, Month::Jul, 8)));
        // Two weekend days owe two substitutes, and no more.
        assert!(BLOCKED.is_business_day(ymd(2026, Month::Jul, 9)));
    }

    #[test]
    fn a_backward_substitute_steps_past_a_blocked_friday() {
        // Jul 4 2026 (Sat) steps back, but Jul 3 is already a holiday,
        // so it lands on Thursday Jul 2.
        const BLOCKED: Calendar<'static> = Calendar {
            name: "Blocked Friday",
            weekend: Weekend::SAT_SUN,
            rules: &[
                Rule::Fixed(FixedDate::new(Month::Jul, 3)),
                Rule::Fixed(FixedDate::new(Month::Jul, 4).shift(WeekendShift::SatBackSunForward)),
            ],
        };
        assert!(BLOCKED.is_holiday(ymd(2026, Month::Jul, 2)));
        assert!(BLOCKED.is_business_day(ymd(2026, Month::Jul, 1)));
    }

    #[test]
    fn each_shift_moves_the_right_way() {
        const SAT_BACK: Calendar<'static> = Calendar {
            name: "SatBack",
            weekend: Weekend::SAT_SUN,
            rules: &[Rule::Fixed(
                FixedDate::new(Month::Jul, 4).shift(WeekendShift::SatBackSunForward),
            )],
        };
        const SUN_ONLY: Calendar<'static> = Calendar {
            name: "SunForward",
            weekend: Weekend::SAT_SUN,
            rules: &[Rule::Fixed(
                FixedDate::new(Month::Jul, 4).shift(WeekendShift::SunForward),
            )],
        };
        const UNSHIFTED: Calendar<'static> = Calendar {
            name: "None",
            weekend: Weekend::SAT_SUN,
            rules: &[Rule::Fixed(FixedDate::new(Month::Jul, 4))],
        };
        // Jul 4 2026 is a Saturday; Jul 4 2021 is a Sunday.
        let (sat_year, sun_year) = (2026, 2021);
        assert!(SAT_BACK.is_holiday(ymd(sat_year, Month::Jul, 3)));
        assert!(SAT_BACK.is_holiday(ymd(sun_year, Month::Jul, 5)));
        // Sunday-forward grants nothing for a Saturday holiday.
        assert!(SUN_ONLY.is_business_day(ymd(sat_year, Month::Jul, 3)));
        assert!(SUN_ONLY.is_holiday(ymd(sun_year, Month::Jul, 5)));
        // No shift, no substitute either way.
        assert!(UNSHIFTED.is_business_day(ymd(sat_year, Month::Jul, 3)));
        assert!(UNSHIFTED.is_business_day(ymd(sun_year, Month::Jul, 5)));
    }

    #[test]
    fn a_substitute_may_cross_a_year_boundary() {
        const NEW_YEAR: Calendar<'static> = Calendar {
            name: "New Year",
            weekend: Weekend::SAT_SUN,
            rules: &[Rule::Fixed(
                FixedDate::new(Month::Jan, 1).shift(WeekendShift::SatBackSunForward),
            )],
        };
        // Jan 1 2022 was a Saturday → observed Friday Dec 31 2021.
        assert!(NEW_YEAR.is_holiday(ymd(2021, Month::Dec, 31)));
    }

    proptest! {
        /// A substitute is always a weekday, and never a date some rule
        /// already claims outright.
        #[test]
        fn substitutes_are_free_weekdays(serial in any_serial()) {
            let d = Date::from_serial(serial).unwrap();
            for cal in [PAIR, crate::calendars::uk::SETTLEMENT] {
                if cal.is_holiday(d) && !cal.rules.iter().any(|r| r.is_holiday(d)) {
                    prop_assert!(!cal.is_weekend(d), "{d} substitute on a weekend");
                }
            }
        }
    }

    // ---- ranges and month edges ----------------------------------------

    #[test]
    fn business_days_and_holidays_partition_the_weekdays() {
        const CAL: Calendar<'static> = Calendar {
            name: "Test",
            weekend: Weekend::SAT_SUN,
            rules: &[Rule::Fixed(FixedDate::new(Month::Jul, 4))],
        };
        // Jul 2024: 31 days, 23 weekdays, one of them the Jul 4 holiday.
        let jul = ymd(2024, Month::Jul, 1)..ymd(2024, Month::Aug, 1);
        assert_eq!(CAL.business_days(jul.clone()).count(), 22);
        assert_eq!(
            CAL.holidays(jul).collect::<Vec<_>>(),
            [ymd(2024, Month::Jul, 4)],
        );
    }

    #[test]
    fn empty_and_reversed_ranges_yield_nothing() {
        let day = ymd(2024, Month::Jul, 2);
        assert_eq!(WEEKENDS_ONLY.business_days(day..day).count(), 0);
        assert_eq!(
            WEEKENDS_ONLY
                .business_days(day..ymd(2024, Month::Jul, 1))
                .count(),
            0,
        );
    }

    #[test]
    fn month_edges_skip_weekends_and_holidays() {
        const CAL: Calendar<'static> = Calendar {
            name: "Test",
            weekend: Weekend::SAT_SUN,
            rules: &[Rule::Fixed(FixedDate::new(Month::Jul, 1))],
        };
        // Jul 2024 opens Mon Jul 1 (a holiday here) and closes Wed Jul 31.
        let mid = ymd(2024, Month::Jul, 15);
        assert_eq!(
            CAL.first_business_day_of_month(mid),
            Some(ymd(2024, Month::Jul, 2)),
        );
        assert_eq!(
            CAL.last_business_day_of_month(mid),
            Some(ymd(2024, Month::Jul, 31)),
        );
        // Mar 2026 opens Sun Mar 1 and closes Tue Mar 31.
        let mar = ymd(2026, Month::Mar, 18);
        assert_eq!(
            WEEKENDS_ONLY.first_business_day_of_month(mar),
            Some(ymd(2026, Month::Mar, 2)),
        );
        // Aug 2026 closes Mon Aug 31; May 2026 closes Sun May 31 → Fri May 29.
        assert_eq!(
            WEEKENDS_ONLY.last_business_day_of_month(ymd(2026, Month::May, 4)),
            Some(ymd(2026, Month::May, 29)),
        );
    }

    #[test]
    fn month_edges_agree_with_scanning_the_range() {
        for month in 1u8..=12 {
            let m = Month::try_from_u8(month).unwrap();
            let anchor = ymd(2026, m, 1);
            let range = anchor..anchor.end_of_month().add_days(1).unwrap();
            let mut days = WEEKENDS_ONLY.business_days(range);
            let (first, last) = (days.next(), days.next_back());
            assert_eq!(WEEKENDS_ONLY.first_business_day_of_month(anchor), first);
            assert_eq!(WEEKENDS_ONLY.last_business_day_of_month(anchor), last);
        }
    }

    #[test]
    fn month_with_no_business_day_is_none() {
        // A rule that blacks out every day of the month.
        const ALL_HOLIDAYS: Calendar<'static> = Calendar {
            name: "Closed",
            weekend: Weekend::SAT_SUN,
            rules: &[Rule::Custom(|d| d.month().get() == 7)],
        };
        assert_eq!(
            ALL_HOLIDAYS.first_business_day_of_month(ymd(2024, Month::Jul, 15)),
            None,
        );
        assert_eq!(
            ALL_HOLIDAYS.last_business_day_of_month(ymd(2024, Month::Jul, 15)),
            None,
        );
    }

    #[test]
    fn union_joins_holidays_and_weekends() {
        const A: Calendar<'static> = Calendar {
            name: "A",
            weekend: Weekend::SAT_SUN,
            rules: &[Rule::Fixed(FixedDate::new(Month::Jul, 4))],
        };
        const B: Calendar<'static> = Calendar {
            name: "B",
            weekend: Weekend::FRI_SAT,
            rules: &[Rule::Fixed(FixedDate::new(Month::Jul, 14))],
        };
        let joint = CalendarBuilder::from_calendar(A).union(B);
        let cal = joint.view();
        assert_eq!(cal.name, "A + B");
        assert!(cal.is_holiday(ymd(2024, Month::Jul, 4)));
        assert!(cal.is_holiday(ymd(2024, Month::Jul, 14)));
        // Fri, Sat and Sun are all weekend under the union.
        assert!(cal.is_weekend(ymd(2024, Month::Jul, 5)));
        assert!(cal.is_weekend(ymd(2024, Month::Jul, 6)));
        assert!(cal.is_weekend(ymd(2024, Month::Jul, 7)));
    }

    // ---- adjust ---------------------------------------------------------

    const WEEKENDS_ONLY: Calendar<'static> = crate::calendars::WEEKENDS_ONLY;

    #[test]
    fn adjust_business_day_returns_input_for_every_convention() {
        let tue = ymd(2024, Month::Jul, 2); // Tuesday
        for conv in [
            BusinessDayConvention::Unadjusted,
            BusinessDayConvention::Following,
            BusinessDayConvention::ModifiedFollowing,
            BusinessDayConvention::Preceding,
            BusinessDayConvention::ModifiedPreceding,
        ] {
            assert_eq!(WEEKENDS_ONLY.adjust(tue, conv).unwrap(), tue, "{conv:?}");
        }
    }

    #[test]
    fn adjust_unadjusted_passes_through_non_business_dates() {
        let sat = ymd(2024, Month::Jul, 6);
        assert_eq!(
            WEEKENDS_ONLY
                .adjust(sat, BusinessDayConvention::Unadjusted)
                .unwrap(),
            sat,
        );
    }

    #[test]
    fn adjust_following_rolls_forward() {
        // Sat Jul 6 2024 → Mon Jul 8.
        let sat = ymd(2024, Month::Jul, 6);
        assert_eq!(
            WEEKENDS_ONLY
                .adjust(sat, BusinessDayConvention::Following)
                .unwrap(),
            ymd(2024, Month::Jul, 8),
        );
    }

    #[test]
    fn adjust_preceding_rolls_backward() {
        // Sat Jul 6 2024 → Fri Jul 5.
        let sat = ymd(2024, Month::Jul, 6);
        assert_eq!(
            WEEKENDS_ONLY
                .adjust(sat, BusinessDayConvention::Preceding)
                .unwrap(),
            ymd(2024, Month::Jul, 5),
        );
    }

    #[test]
    fn adjust_modified_following_falls_back_when_crossing_month() {
        // Sun Aug 31 2025 → Following: Mon Sep 1; ModFol → Fri Aug 29.
        let sun = ymd(2025, Month::Aug, 31);
        assert_eq!(
            WEEKENDS_ONLY
                .adjust(sun, BusinessDayConvention::Following)
                .unwrap(),
            ymd(2025, Month::Sep, 1),
        );
        assert_eq!(
            WEEKENDS_ONLY
                .adjust(sun, BusinessDayConvention::ModifiedFollowing)
                .unwrap(),
            ymd(2025, Month::Aug, 29),
        );
    }

    #[test]
    fn adjust_modified_preceding_falls_back_when_crossing_month() {
        // Sat Mar 1 2025 → Preceding: Fri Feb 28; ModPre → Mon Mar 3.
        let sat = ymd(2025, Month::Mar, 1);
        assert_eq!(
            WEEKENDS_ONLY
                .adjust(sat, BusinessDayConvention::Preceding)
                .unwrap(),
            ymd(2025, Month::Feb, 28),
        );
        assert_eq!(
            WEEKENDS_ONLY
                .adjust(sat, BusinessDayConvention::ModifiedPreceding)
                .unwrap(),
            ymd(2025, Month::Mar, 3),
        );
    }

    #[test]
    fn adjust_modified_following_does_not_fall_back_within_month() {
        // Sat Jul 6 2024 → Mon Jul 8 (same month, no fallback).
        let sat = ymd(2024, Month::Jul, 6);
        assert_eq!(
            WEEKENDS_ONLY
                .adjust(sat, BusinessDayConvention::ModifiedFollowing)
                .unwrap(),
            ymd(2024, Month::Jul, 8),
        );
    }

    #[test]
    fn adjust_skips_holidays_and_weekends_together() {
        const CAL: Calendar<'static> = Calendar {
            name: "Test",
            weekend: Weekend::SAT_SUN,
            rules: &[Rule::Fixed(FixedDate::new(Month::Jul, 4))],
        };
        // Thu Jul 4 2024 is a holiday; Following → Fri Jul 5.
        let thu = ymd(2024, Month::Jul, 4);
        assert_eq!(
            CAL.adjust(thu, BusinessDayConvention::Following).unwrap(),
            ymd(2024, Month::Jul, 5),
        );
        // Sat Jul 6 → Preceding → Fri Jul 5 (only Jul 4 is a holiday).
        let sat = ymd(2024, Month::Jul, 6);
        assert_eq!(
            CAL.adjust(sat, BusinessDayConvention::Preceding).unwrap(),
            ymd(2024, Month::Jul, 5),
        );
    }

    #[test]
    fn adjust_is_idempotent() {
        let sun = ymd(2025, Month::Aug, 31);
        for conv in [
            BusinessDayConvention::Unadjusted,
            BusinessDayConvention::Following,
            BusinessDayConvention::ModifiedFollowing,
            BusinessDayConvention::Preceding,
            BusinessDayConvention::ModifiedPreceding,
        ] {
            let once = WEEKENDS_ONLY.adjust(sun, conv).unwrap();
            let twice = WEEKENDS_ONLY.adjust(once, conv).unwrap();
            assert_eq!(once, twice, "{conv:?}");
        }
    }

    // ---- adjust property tests -----------------------------------------

    use proptest::prelude::*;

    fn any_serial() -> impl Strategy<Value = u32> {
        // Stay well clear of Date::MIN/MAX so the search never runs out.
        7u32..(Date::MAX.serial() - 7)
    }

    // ---- advance --------------------------------------------------------

    #[test]
    fn advance_zero_period_equals_adjust() {
        let sun = ymd(2025, Month::Aug, 31);
        for conv in [
            BusinessDayConvention::Unadjusted,
            BusinessDayConvention::Following,
            BusinessDayConvention::ModifiedFollowing,
            BusinessDayConvention::Preceding,
            BusinessDayConvention::ModifiedPreceding,
        ] {
            assert_eq!(
                WEEKENDS_ONLY
                    .advance(sun, Period::ZERO, conv, false)
                    .unwrap(),
                WEEKENDS_ONLY.adjust(sun, conv).unwrap(),
                "{conv:?}",
            );
        }
    }

    #[test]
    fn advance_days_period_ignores_eom_flag() {
        // Apr 30 2025 + 7 days = May 7; EoM flag inert for Days.
        let apr_eom = ymd(2025, Month::Apr, 30);
        let with_flag = WEEKENDS_ONLY
            .advance(
                apr_eom,
                Period::Days(7),
                BusinessDayConvention::Following,
                true,
            )
            .unwrap();
        let without_flag = WEEKENDS_ONLY
            .advance(
                apr_eom,
                Period::Days(7),
                BusinessDayConvention::Following,
                false,
            )
            .unwrap();
        assert_eq!(with_flag, ymd(2025, Month::May, 7));
        assert_eq!(with_flag, without_flag);
    }

    #[test]
    fn advance_months_without_eom_clamps_via_add_months() {
        // Jan 31 2026 + 1M without EoM = Feb 28 2026 (clamp).
        let jan31 = ymd(2026, Month::Jan, 31);
        assert_eq!(
            WEEKENDS_ONLY
                .advance(
                    jan31,
                    Period::Months(1),
                    BusinessDayConvention::Unadjusted,
                    false,
                )
                .unwrap(),
            ymd(2026, Month::Feb, 28),
        );
    }

    #[test]
    fn advance_months_with_eom_snaps_to_target_eom() {
        // Apr 30 2025 (EoM) + 1M with EoM = Sat May 31 2025 (Unadjusted).
        let apr_eom = ymd(2025, Month::Apr, 30);
        assert_eq!(
            WEEKENDS_ONLY
                .advance(
                    apr_eom,
                    Period::Months(1),
                    BusinessDayConvention::Unadjusted,
                    true,
                )
                .unwrap(),
            ymd(2025, Month::May, 31),
        );
    }

    #[test]
    fn advance_eom_only_kicks_in_when_input_is_eom() {
        // Apr 15 2025 is not EoM; the flag is inert.
        let mid = ymd(2025, Month::Apr, 15);
        assert_eq!(
            WEEKENDS_ONLY
                .advance(
                    mid,
                    Period::Months(1),
                    BusinessDayConvention::Unadjusted,
                    true,
                )
                .unwrap(),
            ymd(2025, Month::May, 15),
        );
    }

    #[test]
    fn advance_applies_bdc_after_eom_snap() {
        // Apr 30 2025 + 1M with EoM = Sat May 31 → ModFol → Fri May 30.
        let apr_eom = ymd(2025, Month::Apr, 30);
        assert_eq!(
            WEEKENDS_ONLY
                .advance(
                    apr_eom,
                    Period::Months(1),
                    BusinessDayConvention::ModifiedFollowing,
                    true,
                )
                .unwrap(),
            ymd(2025, Month::May, 30),
        );
    }

    #[test]
    fn advance_negative_period_steps_backward() {
        // Wed Jan 1 2025 - 1M = Dec 1 2024 (Sun) → ModFol → Mon Dec 2.
        let jan1 = ymd(2025, Month::Jan, 1);
        assert_eq!(
            WEEKENDS_ONLY
                .advance(
                    jan1,
                    Period::Months(-1),
                    BusinessDayConvention::ModifiedFollowing,
                    false,
                )
                .unwrap(),
            ymd(2024, Month::Dec, 2),
        );
    }

    #[test]
    fn advance_propagates_period_overflow() {
        // Stepping past Date::MAX surfaces DateOutOfRange.
        let near_max = Date::MAX;
        assert_eq!(
            WEEKENDS_ONLY.advance(
                near_max,
                Period::Days(1),
                BusinessDayConvention::Unadjusted,
                false,
            ),
            Err(TimeError::DateOutOfRange),
        );
    }

    proptest! {
        #[test]
        fn advance_unadjusted_matches_period_arithmetic(
            serial in any_serial(),
            n in -100i32..=100,
        ) {
            let d = Date::from_serial(serial).unwrap();
            for period in [
                Period::Days(n),
                Period::Weeks(n / 7),
                Period::Months(n / 12),
                Period::Years(n / 144),
            ] {
                let direct = (d + period).ok();
                let advanced = WEEKENDS_ONLY
                    .advance(
                        d,
                        period,
                        BusinessDayConvention::Unadjusted,
                        false,
                    )
                    .ok();
                prop_assert_eq!(direct, advanced);
            }
        }

        #[test]
        fn advance_with_modified_following_lands_on_business_day(
            serial in any_serial(),
            n in -50i32..=50,
        ) {
            let d = Date::from_serial(serial).unwrap();
            for period in [Period::Days(n), Period::Months(n / 12)] {
                if let Ok(out) = WEEKENDS_ONLY.advance(
                    d,
                    period,
                    BusinessDayConvention::ModifiedFollowing,
                    false,
                ) {
                    prop_assert!(WEEKENDS_ONLY.is_business_day(out));
                }
            }
        }
    }

    proptest! {
        #[test]
        fn adjust_unadjusted_is_identity(serial in any_serial()) {
            let d = Date::from_serial(serial).unwrap();
            prop_assert_eq!(
                WEEKENDS_ONLY
                    .adjust(d, BusinessDayConvention::Unadjusted)
                    .unwrap(),
                d,
            );
        }

        #[test]
        fn adjust_produces_business_days_for_non_unadjusted(serial in any_serial()) {
            let d = Date::from_serial(serial).unwrap();
            for conv in [
                BusinessDayConvention::Following,
                BusinessDayConvention::ModifiedFollowing,
                BusinessDayConvention::Preceding,
                BusinessDayConvention::ModifiedPreceding,
            ] {
                let out = WEEKENDS_ONLY.adjust(d, conv).unwrap();
                prop_assert!(WEEKENDS_ONLY.is_business_day(out), "{conv:?} -> {out}");
            }
        }

        #[test]
        fn adjust_following_is_at_least_input(serial in any_serial()) {
            let d = Date::from_serial(serial).unwrap();
            let out = WEEKENDS_ONLY
                .adjust(d, BusinessDayConvention::Following)
                .unwrap();
            prop_assert!(out >= d);
        }

        #[test]
        fn adjust_preceding_is_at_most_input(serial in any_serial()) {
            let d = Date::from_serial(serial).unwrap();
            let out = WEEKENDS_ONLY
                .adjust(d, BusinessDayConvention::Preceding)
                .unwrap();
            prop_assert!(out <= d);
        }

        #[test]
        fn modified_following_stays_in_same_month(serial in any_serial()) {
            let d = Date::from_serial(serial).unwrap();
            let out = WEEKENDS_ONLY
                .adjust(d, BusinessDayConvention::ModifiedFollowing)
                .unwrap();
            prop_assert_eq!(out.year(), d.year());
            prop_assert_eq!(out.month(), d.month());
        }

        #[test]
        fn modified_preceding_stays_in_same_month(serial in any_serial()) {
            let d = Date::from_serial(serial).unwrap();
            let out = WEEKENDS_ONLY
                .adjust(d, BusinessDayConvention::ModifiedPreceding)
                .unwrap();
            prop_assert_eq!(out.year(), d.year());
            prop_assert_eq!(out.month(), d.month());
        }

        #[test]
        fn adjust_is_idempotent_for_all_conventions(serial in any_serial()) {
            let d = Date::from_serial(serial).unwrap();
            for conv in [
                BusinessDayConvention::Unadjusted,
                BusinessDayConvention::Following,
                BusinessDayConvention::ModifiedFollowing,
                BusinessDayConvention::Preceding,
                BusinessDayConvention::ModifiedPreceding,
            ] {
                let once = WEEKENDS_ONLY.adjust(d, conv).unwrap();
                let twice = WEEKENDS_ONLY.adjust(once, conv).unwrap();
                prop_assert_eq!(once, twice);
            }
        }
    }
}
