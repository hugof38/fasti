//! [`Calendar`]: a weekend definition + a sequence of holiday [`Rule`]s.
//!
//! `Calendar<'a>` is a borrowed view — the name is `&'a str`, the rules
//! live in `&'a [Rule]`. The lifetime lets built-in calendars live in
//! `pub const` values (with `'a = 'static`), and also lets a
//! [`CalendarBuilder`] produce calendars borrowing from its own
//! owned storage.

use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::vec::Vec;

use crate::{BusinessDayConvention, Date, Period, Rule, TimeError, Weekend};

/// A holiday calendar: a [`Weekend`] configuration plus a sequence of
/// [`Rule`]s. A date is a holiday iff at least one rule says so.
///
/// Calendars are borrowed views, `Copy`, and have no runtime cost
/// beyond three pointer-sized fields. Built-in calendars are declared
/// as `pub const Calendar<'static>`. User-built calendars come from
/// [`CalendarBuilder`].
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
    /// Holiday rules — evaluated in sequence; short-circuits on the
    /// first match.
    pub rules: &'a [Rule],
}

impl Calendar<'_> {
    /// `true` iff `date` falls on a weekend day under this calendar.
    #[must_use]
    pub const fn is_weekend(&self, date: Date) -> bool {
        self.weekend.contains(date.weekday())
    }

    /// `true` iff any rule in this calendar marks `date` as a holiday.
    /// Does not consider weekends — a Saturday can be a non-holiday
    /// non-business day.
    #[must_use]
    pub fn is_holiday(&self, date: Date) -> bool {
        self.rules.iter().any(|r| r.is_holiday(date))
    }

    /// `true` iff `date` is neither a weekend nor a holiday.
    #[must_use]
    pub fn is_business_day(&self, date: Date) -> bool {
        !self.is_weekend(date) && !self.is_holiday(date)
    }

    /// The next business day strictly after `date`.
    ///
    /// Returns [`None`] only if the search would run past [`Date::MAX`]
    /// — in practice this never happens for a calendar with any
    /// business days in the crate's supported range.
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

    /// Roll `date` onto a business day according to `convention`.
    ///
    /// If `date` is already a business day the input is returned
    /// unchanged for every convention. The returned error is
    /// [`TimeError::DateOutOfRange`], surfaced when the convention's
    /// search would run past [`Date::MIN`] or [`Date::MAX`] without
    /// finding a business day — in practice this only happens at the
    /// extreme boundary of the supported range.
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
    /// business day under `convention`.
    ///
    /// When `end_of_month` is `true` *and* `date` is the last day of
    /// its calendar month *and* `period` is in `Months` or `Years`
    /// units, the unadjusted result is snapped to the last day of its
    /// own month *before* the business-day adjustment runs. This is
    /// the `QuantLib` semantics — see
    /// [`ql/time/calendar.cpp`](https://github.com/lballabio/QuantLib/blob/master/ql/time/calendar.cpp).
    /// The flag is ignored for `Days` and `Weeks` periods, where the
    /// concept does not apply.
    ///
    /// Returns [`TimeError::DateOutOfRange`] if either the period
    /// arithmetic or the business-day search would step outside the
    /// supported range.
    ///
    /// Apr 30 2025 (a Wednesday, and the `EoM` of April) plus one month
    /// with `end_of_month = true` snaps to May 31 2025, which is a
    /// Saturday. `ModifiedFollowing` rolls back to Friday May 30 to
    /// stay within May:
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
        let stepped = (date + period)?;
        let target = if end_of_month
            && date.is_end_of_month()
            && matches!(period, Period::Months(_) | Period::Years(_))
        {
            stepped.end_of_month()
        } else {
            stepped
        };
        self.adjust(target, convention)
    }
}

/// Owned counterpart to [`Calendar`] — allocates heap storage for the
/// name and rule list, and exposes a [`view`](Self::view) method that
/// produces a borrowed [`Calendar`] referencing those allocations.
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

    /// Seed a builder from a [`Calendar`] — commonly a built-in like
    /// [`us::SETTLEMENT`](crate::calendars::us::SETTLEMENT) that the
    /// caller wants to extend with bespoke blackout days.
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

    /// Produce a borrowed [`Calendar`] backed by this builder's storage.
    ///
    /// The returned view lives as long as `&self`; use it immediately
    /// or clone fields if a longer lifetime is needed.
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
        // Sun Aug 31 2025 → Following: Mon Sep 1 (different month) → ModFol
        // falls back to Fri Aug 29.
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
        // Sat Mar 1 2025 → Preceding: Fri Feb 28 (different month) → ModPre
        // falls back to Mon Mar 3.
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
        // Sat Jul 6 2024 → Following: Mon Jul 8 (same month) → ModFol
        // returns the same date.
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
        // Sat Jul 6 with the same holiday: Preceding skips Fri Jul 5? No,
        // Fri Jul 5 is a business day — only Jul 4 itself is a holiday.
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
        // Apr 30 2025 (EoM, Wed) + 7 days = May 7 (Wed). EoM flag has
        // no effect for Days.
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
        // Apr 30 2025 (EoM) + 1M with EoM = May 31 2025; Unadjusted
        // returns May 31 even though it's a Saturday.
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
        // Apr 30 2025 (EoM, Wed) + 1M with EoM = unadjusted May 31 2025
        // (Sat) → ModFol falls back to May 30 (Fri) without crossing
        // the month boundary.
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
