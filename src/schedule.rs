//! [`Schedule`] — payment-date generation with business-day
//! adjustment. Modeled on `QuantLib`'s `ql/time/schedule.hpp`.
//!
//! Candidates are `seed + i · tenor` fresh multiples (never chained,
//! avoiding the add-months clamp pathology), optionally end-of-month
//! snapped, then rolled through [`Calendar::adjust`]; the termination
//! date carries its own convention.

use alloc::vec;
use alloc::vec::Vec;

use crate::{BusinessDayConvention, Calendar, Date, Frequency, Period, TimeError};

/// One accrual window of a [`Schedule`] — the closed-open interval
/// from `start` to the next period's start (which is this period's
/// `end`).
///
/// Yielded by [`Schedule::periods`]. The two dates are always in
/// strictly ascending order because the parent schedule is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AccrualPeriod {
    start: Date,
    end: Date,
}

impl AccrualPeriod {
    /// The start (earlier) date of the accrual window.
    #[must_use]
    pub const fn start(self) -> Date {
        self.start
    }

    /// The end (later) date of the accrual window.
    #[must_use]
    pub const fn end(self) -> Date {
        self.end
    }
}

impl From<AccrualPeriod> for (Date, Date) {
    fn from(p: AccrualPeriod) -> Self {
        (p.start, p.end)
    }
}

/// How to walk the schedule grid between effective and termination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DateGenerationRule {
    /// Generate dates forward from `effective`. Any irregular
    /// (short/long) period lands at the back end.
    Forward,
    /// Generate dates backward from `termination`. Any irregular
    /// period lands at the front end.
    Backward,
    /// No interior dates — the schedule is just
    /// `[effective, termination]`.
    Zero,
}

/// A list of business-day-adjusted scheduled dates in chronological
/// order (length `periods + 1`), plus a parallel list of *reference*
/// dates.
///
/// The reference dates carry the regular coupon grid: for a fully
/// regular schedule the two lists are identical, and for short/long
/// stub periods the first/last reference date is the notional
/// (quasi-coupon) boundary one tenor from the adjacent coupon —
/// `QuantLib`'s quasi-payment reconciliation. Schedule-defined day
/// counts (ACT/ACT ICMA) read them via
/// [`reference_dates`](Self::reference_dates).
///
/// Serde round-trips both lists; deserialization re-validates, so a
/// deserialized `Schedule` is always valid.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(try_from = "ScheduleParts", into = "ScheduleParts")
)]
pub struct Schedule {
    dates: Vec<Date>,
    reference_dates: Vec<Date>,
}

/// Serde representation of a [`Schedule`]: both parallel lists.
#[cfg(feature = "serde")]
#[derive(serde::Serialize, serde::Deserialize)]
struct ScheduleParts {
    dates: Vec<Date>,
    reference_dates: Vec<Date>,
}

#[cfg(feature = "serde")]
impl TryFrom<ScheduleParts> for Schedule {
    type Error = TimeError;

    fn try_from(parts: ScheduleParts) -> Result<Self, Self::Error> {
        Self::from_parts(parts.dates, parts.reference_dates)
    }
}

#[cfg(feature = "serde")]
impl From<Schedule> for ScheduleParts {
    fn from(s: Schedule) -> Self {
        Self {
            dates: s.dates,
            reference_dates: s.reference_dates,
        }
    }
}

impl TryFrom<Vec<Date>> for Schedule {
    type Error = TimeError;

    /// Wrap a chronologically ordered date list, enforcing strict
    /// monotonicity; every period is treated as regular (reference
    /// dates = coupon dates).
    fn try_from(dates: Vec<Date>) -> Result<Self, Self::Error> {
        let reference_dates = dates.clone();
        Self::from_parts(dates, reference_dates)
    }
}

impl From<Schedule> for Vec<Date> {
    /// Unwrap the coupon date list.
    fn from(s: Schedule) -> Self {
        s.dates
    }
}

impl Schedule {
    /// Build from explicit coupon and reference date lists.
    ///
    /// Validates: coupon dates strictly monotonic; the lists have
    /// equal length; interior reference dates equal their coupon
    /// dates (only the first and last may differ, for stubs); the
    /// end reference periods are non-degenerate. Errors with
    /// [`TimeError::ScheduleNotMonotonic`] or
    /// [`TimeError::InvalidReferencePeriod`].
    pub fn from_parts(dates: Vec<Date>, reference_dates: Vec<Date>) -> Result<Self, TimeError> {
        for window in dates.windows(2) {
            if window[0] >= window[1] {
                return Err(TimeError::ScheduleNotMonotonic);
            }
        }
        if reference_dates.len() != dates.len() {
            return Err(TimeError::InvalidReferencePeriod);
        }
        let n = dates.len();
        if n > 2 {
            for i in 1..n - 1 {
                if reference_dates[i] != dates[i] {
                    return Err(TimeError::InvalidReferencePeriod);
                }
            }
        }
        if n >= 2
            && (reference_dates[0] >= reference_dates[1]
                || reference_dates[n - 1] <= reference_dates[n - 2])
        {
            return Err(TimeError::InvalidReferencePeriod);
        }
        Ok(Self {
            dates,
            reference_dates,
        })
    }

    /// Borrow the adjusted coupon dates as a slice.
    #[must_use]
    pub fn dates(&self) -> &[Date] {
        &self.dates
    }

    /// Borrow the reference (regular-grid) dates, parallel to
    /// [`dates`](Self::dates). Identical to the coupon dates except
    /// at stub ends, where they carry the notional quasi-coupon
    /// boundary.
    #[must_use]
    pub fn reference_dates(&self) -> &[Date] {
        &self.reference_dates
    }

    /// Number of dates (i.e., `periods + 1`).
    #[must_use]
    pub fn len(&self) -> usize {
        self.dates.len()
    }

    /// `true` iff the schedule has no dates. Cannot occur on a
    /// successfully built `Schedule`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.dates.is_empty()
    }

    /// First date — the adjusted effective.
    #[must_use]
    pub fn first(&self) -> Option<Date> {
        self.dates.first().copied()
    }

    /// Last date — the adjusted termination.
    #[must_use]
    pub fn last(&self) -> Option<Date> {
        self.dates.last().copied()
    }

    /// Iterate dates in chronological order.
    pub fn iter(&self) -> core::slice::Iter<'_, Date> {
        self.dates.iter()
    }

    /// Iterate accrual periods — adjacent `(start, end)` windows
    /// over the schedule's dates.
    pub fn periods(&self) -> impl Iterator<Item = AccrualPeriod> + '_ {
        self.dates.windows(2).map(|w| AccrualPeriod {
            start: w[0],
            end: w[1],
        })
    }

    /// Non-panicking date access.
    ///
    /// Use this in preference to indexing when the index might be
    /// out of bounds. Indexing via `schedule[i]` panics, matching
    /// slice semantics.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<Date> {
        self.dates.get(index).copied()
    }

    /// The largest schedule date strictly less than `ref_date`, if
    /// any.
    ///
    /// ```
    /// use fasti::{Date, Month, Period, ScheduleBuilder, calendars};
    /// let s = ScheduleBuilder::new(
    ///     Date::from_ymd(2025, Month::Jan, 15)?,
    ///     Date::from_ymd(2026, Month::Jan, 15)?,
    ///     Period::Months(3),
    ///     calendars::WEEKENDS_ONLY,
    /// )
    /// .forwards()
    /// .build()?;
    /// assert_eq!(
    ///     s.previous_date(Date::from_ymd(2025, Month::Aug, 1)?),
    ///     Some(Date::from_ymd(2025, Month::Jul, 15)?),
    /// );
    /// // Before the first date — no predecessor.
    /// assert_eq!(s.previous_date(Date::from_ymd(2025, Month::Jan, 1)?), None);
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    #[must_use]
    pub fn previous_date(&self, ref_date: Date) -> Option<Date> {
        // Index of the first date >= ref_date; the predecessor is
        // immediately before it.
        let idx = self.dates.partition_point(|d| *d < ref_date);
        if idx == 0 {
            None
        } else {
            Some(self.dates[idx - 1])
        }
    }

    /// The smallest schedule date strictly greater than `ref_date`,
    /// if any.
    #[must_use]
    pub fn next_date(&self, ref_date: Date) -> Option<Date> {
        let idx = self.dates.partition_point(|d| *d <= ref_date);
        self.dates.get(idx).copied()
    }

    /// The smallest schedule date `>= ref_date`, if any.
    ///
    /// Mirrors `std::lower_bound` on a sorted sequence: unlike
    /// [`next_date`](Self::next_date), this *includes* an exact
    /// match. Use this when "the next on-or-after coupon" is the
    /// natural query (e.g., asking "what is the current accrual
    /// period?" given today's date).
    #[must_use]
    pub fn lower_bound(&self, ref_date: Date) -> Option<Date> {
        let idx = self.dates.partition_point(|d| *d < ref_date);
        self.dates.get(idx).copied()
    }

    /// New schedule containing only dates `>= cutoff`.
    ///
    /// May be empty if `cutoff` is past the last date. Allocates a
    /// fresh `Vec`; the original is unchanged.
    #[must_use]
    pub fn after(&self, cutoff: Date) -> Self {
        let idx = self.dates.partition_point(|d| *d < cutoff);
        Self {
            dates: self.dates[idx..].to_vec(),
            reference_dates: self.reference_dates[idx..].to_vec(),
        }
    }

    /// New schedule containing only dates `<= cutoff`.
    ///
    /// May be empty if `cutoff` is before the first date. Allocates
    /// a fresh `Vec`; the original is unchanged.
    #[must_use]
    pub fn until(&self, cutoff: Date) -> Self {
        let idx = self.dates.partition_point(|d| *d <= cutoff);
        Self {
            dates: self.dates[..idx].to_vec(),
            reference_dates: self.reference_dates[..idx].to_vec(),
        }
    }
}

impl core::ops::Index<usize> for Schedule {
    type Output = Date;

    /// Panicking date access. Use [`Schedule::get`] for the
    /// non-panicking variant.
    fn index(&self, index: usize) -> &Self::Output {
        &self.dates[index]
    }
}

impl<'a> IntoIterator for &'a Schedule {
    type Item = &'a Date;
    type IntoIter = core::slice::Iter<'a, Date>;

    fn into_iter(self) -> Self::IntoIter {
        self.dates.iter()
    }
}

/// Builder for [`Schedule`]. Holds a borrowed [`Calendar`] for the
/// duration of the build call.
///
/// ```
/// use fasti::{
///     BusinessDayConvention, Date, DateGenerationRule, Month, Period,
///     ScheduleBuilder, calendars,
/// };
///
/// let effective = Date::from_ymd(2025, Month::Jan, 15)?;
/// let termination = Date::from_ymd(2026, Month::Jan, 15)?;
///
/// let schedule = ScheduleBuilder::new(
///     effective,
///     termination,
///     Period::Months(3),
///     calendars::WEEKENDS_ONLY,
/// )
/// .with_convention(BusinessDayConvention::ModifiedFollowing)
/// .with_rule(DateGenerationRule::Forward)
/// .build()?;
///
/// assert_eq!(schedule.len(), 5);
/// assert_eq!(schedule.first().unwrap(), effective);
/// assert_eq!(schedule.last().unwrap(), termination);
/// # Ok::<(), fasti::TimeError>(())
/// ```
#[derive(Debug, Clone)]
pub struct ScheduleBuilder<'cal> {
    effective: Date,
    termination: Date,
    tenor: Period,
    calendar: Calendar<'cal>,
    convention: BusinessDayConvention,
    termination_convention: BusinessDayConvention,
    rule: DateGenerationRule,
    end_of_month: bool,
    first_date: Option<Date>,
    next_to_last_date: Option<Date>,
}

impl<'cal> ScheduleBuilder<'cal> {
    /// Start a builder with the four required inputs. Defaults match
    /// common bond conventions: `ModifiedFollowing` for interior
    /// dates, `Unadjusted` for the termination date, `Backward`
    /// generation, `EoM`flag off, no stub dates.
    #[must_use]
    pub fn new(
        effective: Date,
        termination: Date,
        tenor: Period,
        calendar: Calendar<'cal>,
    ) -> Self {
        Self {
            effective,
            termination,
            tenor,
            calendar,
            convention: BusinessDayConvention::ModifiedFollowing,
            termination_convention: BusinessDayConvention::Unadjusted,
            rule: DateGenerationRule::Backward,
            end_of_month: false,
            first_date: None,
            next_to_last_date: None,
        }
    }

    /// Override the tenor with the [`Period`] canonical to the
    /// given [`Frequency`]. Convenient when the schedule's recurrence
    /// is more naturally expressed as "quarterly" / "semiannual" than
    /// as "every 3 months" / "every 6 months".
    #[must_use]
    pub fn with_frequency(mut self, frequency: Frequency) -> Self {
        self.tenor = Period::from(frequency);
        self
    }

    /// Override the business-day convention applied to interior
    /// dates and the effective date.
    #[must_use]
    pub fn with_convention(mut self, convention: BusinessDayConvention) -> Self {
        self.convention = convention;
        self
    }

    /// Override the business-day convention applied to the
    /// termination date specifically.
    #[must_use]
    pub fn with_termination_convention(mut self, convention: BusinessDayConvention) -> Self {
        self.termination_convention = convention;
        self
    }

    /// Override the date-generation rule.
    #[must_use]
    pub fn with_rule(mut self, rule: DateGenerationRule) -> Self {
        self.rule = rule;
        self
    }

    /// Toggle the EoM-preservation flag.
    #[must_use]
    pub fn with_end_of_month(mut self, end_of_month: bool) -> Self {
        self.end_of_month = end_of_month;
        self
    }

    /// Specify an explicit first regular date (front stub anchor).
    /// Must satisfy `effective < first_date < termination`.
    #[must_use]
    pub fn with_first_date(mut self, first_date: Date) -> Self {
        self.first_date = Some(first_date);
        self
    }

    /// Specify an explicit last regular date (back stub anchor).
    /// Must satisfy `effective < next_to_last_date < termination`.
    #[must_use]
    pub fn with_next_to_last_date(mut self, next_to_last_date: Date) -> Self {
        self.next_to_last_date = Some(next_to_last_date);
        self
    }

    /// Sugar for [`with_rule`](Self::with_rule)`(DateGenerationRule::Forward)`.
    #[must_use]
    pub fn forwards(self) -> Self {
        self.with_rule(DateGenerationRule::Forward)
    }

    /// Sugar for [`with_rule`](Self::with_rule)`(DateGenerationRule::Backward)`.
    #[must_use]
    pub fn backwards(self) -> Self {
        self.with_rule(DateGenerationRule::Backward)
    }

    /// Build the [`Schedule`]. Returns
    /// [`TimeError::EffectiveAfterTermination`],
    /// [`TimeError::ZeroTenor`], or [`TimeError::StubDateOutOfRange`]
    /// for inconsistent inputs, and
    /// [`TimeError::ScheduleNotMonotonic`] when business-day
    /// adjustment collapses adjacent dates onto the same business
    /// day.
    pub fn build(self) -> Result<Schedule, TimeError> {
        self.validate_inputs()?;
        let generated = match self.rule {
            // A Zero schedule's single period is its own reference.
            DateGenerationRule::Zero => Generated {
                dates: vec![self.effective, self.termination],
                front_notional: None,
                back_notional: None,
            },
            DateGenerationRule::Forward => Generator::forward(&self).generate()?,
            DateGenerationRule::Backward => Generator::backward(&self)?.generate()?,
        };
        let adjusted =
            BdcAdjuster::new(self.calendar, self.convention, self.termination_convention)
                .apply(generated.dates)?;
        let mut schedule = Schedule::try_from(adjusted)?;
        // Reconcile stub reference dates (quasi-coupon boundaries),
        // adjusted with the interior convention. Guards keep the
        // parallel-list invariants; a failed adjustment falls back to
        // treating the period as regular.
        let n = schedule.reference_dates.len();
        if let Some(front) = generated.front_notional
            && let Ok(adj) = self.calendar.adjust(front, self.convention)
            && n >= 2
            && adj < schedule.reference_dates[1]
        {
            schedule.reference_dates[0] = adj;
        }
        if let Some(back) = generated.back_notional
            && let Ok(adj) = self.calendar.adjust(back, self.convention)
            && n >= 2
            && adj > schedule.reference_dates[n - 2]
        {
            schedule.reference_dates[n - 1] = adj;
        }
        Ok(schedule)
    }

    fn validate_inputs(&self) -> Result<(), TimeError> {
        if self.effective >= self.termination {
            return Err(TimeError::EffectiveAfterTermination);
        }
        if !matches!(self.rule, DateGenerationRule::Zero) && self.tenor.is_zero() {
            return Err(TimeError::ZeroTenor);
        }
        if let Some(fd) = self.first_date
            && (fd <= self.effective || fd >= self.termination)
        {
            return Err(TimeError::StubDateOutOfRange);
        }
        if let Some(nld) = self.next_to_last_date
            && (nld <= self.effective || nld >= self.termination)
        {
            return Err(TimeError::StubDateOutOfRange);
        }
        if let (Some(fd), Some(nld)) = (self.first_date, self.next_to_last_date)
            && fd >= nld
        {
            return Err(TimeError::StubDateOutOfRange);
        }
        Ok(())
    }
}

/// Direction the unified date walk traverses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    Forward,
    Backward,
}

/// Stepping primitive: `seed + i · step`, with optional snap to the
/// end of the resulting month when the seed is itself end-of-month
/// and the step is in months or years.
///
/// This is the only place the `EoM`-preservation rule lives. The
/// step is signed; backward walks pass a negative `Period`.
#[derive(Debug, Clone, Copy)]
struct Stepper {
    seed: Date,
    step: Period,
    end_of_month: bool,
}

impl Stepper {
    const fn new(seed: Date, step: Period, end_of_month: bool) -> Self {
        Self {
            seed,
            step,
            end_of_month,
        }
    }

    /// Compute `seed + i · step`. Returns
    /// [`TimeError::DateOutOfRange`] on either step-scaling overflow
    /// or a date that escapes the supported range.
    fn step(&self, i: i32) -> Result<Date, TimeError> {
        let scaled = self.step.checked_mul(i).ok_or(TimeError::DateOutOfRange)?;
        let stepped = (self.seed + scaled)?;
        if self.end_of_month
            && self.seed.is_end_of_month()
            && matches!(self.step, Period::Months(_) | Period::Years(_))
        {
            Ok(stepped.end_of_month())
        } else {
            Ok(stepped)
        }
    }
}

/// Iterator over candidates produced by a [`Stepper`], halting at
/// (and never emitting) the first candidate that has reached or
/// passed `stop` in the configured direction.
///
/// The walk yields only the *interior* dates — anchors and stubs
/// are the [`Generator`]'s job.
struct Walk<'a> {
    stepper: &'a Stepper,
    stop: Date,
    direction: Direction,
    i: i32,
}

impl<'a> Walk<'a> {
    const fn new(stepper: &'a Stepper, stop: Date, direction: Direction) -> Self {
        Self {
            stepper,
            stop,
            direction,
            i: 1,
        }
    }

    fn past_stop(&self, candidate: Date) -> bool {
        match self.direction {
            Direction::Forward => candidate >= self.stop,
            Direction::Backward => candidate <= self.stop,
        }
    }
}

impl Iterator for Walk<'_> {
    type Item = Result<Date, TimeError>;

    fn next(&mut self) -> Option<Self::Item> {
        let candidate = match self.stepper.step(self.i) {
            Ok(d) => d,
            Err(e) => return Some(Err(e)),
        };
        if self.past_stop(candidate) {
            return None;
        }
        // `i` overflow is unreachable: the supported date range
        // (1901..=2199, ~109k days) means `Stepper::step(i)` errors
        // long before `i` itself approaches `i32::MAX`.
        self.i += 1;
        Some(Ok(candidate))
    }
}

/// Direction-bound date generator: owns the anchors, stubs, signed
/// step, and `EoM` flag, and emits the unadjusted, chronologically
/// ordered date list.
///
/// `Forward` and `Backward` produce *different* date sets — Forward
/// leaves irregular periods at the back end, Backward at the front.
/// They are not reverses of each other; the assembly is identical
/// modulo a final `reverse()` on backward.
struct Generator {
    start_anchor: Date,
    start_stub: Option<Date>,
    end_anchor: Date,
    end_stub: Option<Date>,
    step: Period,
    direction: Direction,
    end_of_month: bool,
}

impl Generator {
    fn forward(b: &ScheduleBuilder<'_>) -> Self {
        Self {
            start_anchor: b.effective,
            start_stub: b.first_date,
            end_anchor: b.termination,
            end_stub: b.next_to_last_date,
            step: b.tenor,
            direction: Direction::Forward,
            end_of_month: b.end_of_month,
        }
    }

    /// Negates the step so a `Backward` generator can drive the
    /// same [`Stepper`] / [`Walk`] machinery as `Forward`. Returns
    /// [`TimeError::DateOutOfRange`] only if the tenor's length is
    /// `i32::MIN` (no positive counterpart in `i32`).
    fn backward(b: &ScheduleBuilder<'_>) -> Result<Self, TimeError> {
        Ok(Self {
            start_anchor: b.termination,
            start_stub: b.next_to_last_date,
            end_anchor: b.effective,
            end_stub: b.first_date,
            step: b.tenor.checked_neg().ok_or(TimeError::DateOutOfRange)?,
            direction: Direction::Backward,
            end_of_month: b.end_of_month,
        })
    }

    fn generate(&self) -> Result<Generated, TimeError> {
        let seed = self.start_stub.unwrap_or(self.start_anchor);
        let stop = self.end_stub.unwrap_or(self.end_anchor);
        let stepper = Stepper::new(seed, self.step, self.end_of_month);

        let mut out: Vec<Date> = Vec::new();
        out.push(self.start_anchor);
        if let Some(s) = self.start_stub {
            out.push(s);
        }
        let mut walk = Walk::new(&stepper, stop, self.direction);
        for candidate in walk.by_ref() {
            out.push(candidate?);
        }
        // The walk halted on its first at-or-past-stop candidate; that
        // grid point is the quasi-coupon boundary of the stop-side
        // period. It differs from the anchor exactly when the period
        // is a stub.
        let stopping = stepper.step(walk.i).ok();
        if let Some(s) = self.end_stub {
            out.push(s);
        }
        out.push(self.end_anchor);

        // Notional boundary of the stop-side period: the halting grid
        // point, or one fresh tenor step from an explicit stub anchor.
        // A step escaping the supported range falls back to "regular".
        let far = match self.end_stub {
            None => stopping,
            Some(s) => Stepper::new(s, self.step, self.end_of_month).step(1).ok(),
        }
        .filter(|n| *n != self.end_anchor);
        // Likewise for an explicit stub at the seed side.
        let near = self.start_stub.and_then(|s| {
            Stepper::new(s, self.step, self.end_of_month)
                .step(-1)
                .ok()
                .filter(|n| *n != self.start_anchor)
        });

        let (front_notional, back_notional) = match self.direction {
            Direction::Forward => (near, far),
            Direction::Backward => {
                out.reverse();
                (far, near)
            }
        };
        Ok(Generated {
            dates: out,
            front_notional,
            back_notional,
        })
    }
}

/// A generator's output: the unadjusted chronological dates plus the
/// notional quasi-coupon boundaries of any stub periods.
struct Generated {
    dates: Vec<Date>,
    front_notional: Option<Date>,
    back_notional: Option<Date>,
}

/// Per-index business-day-convention pass. The last date uses the
/// terminal convention; every other date uses the interior one.
#[derive(Debug, Clone, Copy)]
struct BdcAdjuster<'a> {
    calendar: Calendar<'a>,
    interior: BusinessDayConvention,
    terminal: BusinessDayConvention,
}

impl<'a> BdcAdjuster<'a> {
    const fn new(
        calendar: Calendar<'a>,
        interior: BusinessDayConvention,
        terminal: BusinessDayConvention,
    ) -> Self {
        Self {
            calendar,
            interior,
            terminal,
        }
    }

    fn apply(&self, dates: Vec<Date>) -> Result<Vec<Date>, TimeError> {
        let Some(last_idx) = dates.len().checked_sub(1) else {
            return Ok(dates);
        };
        let mut adjusted = Vec::with_capacity(dates.len());
        for (i, d) in dates.into_iter().enumerate() {
            let conv = if i == last_idx {
                self.terminal
            } else {
                self.interior
            };
            adjusted.push(self.calendar.adjust(d, conv)?);
        }
        Ok(adjusted)
    }
}

// ---- Tests --------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::Month;
    use proptest::prelude::*;

    const WEEKENDS: Calendar<'static> = crate::calendars::WEEKENDS_ONLY;

    fn ymd(y: u16, m: Month, d: u8) -> Date {
        Date::from_ymd(y, m, d).unwrap()
    }

    // ---- Zero rule -----------------------------------------------------

    #[test]
    fn zero_rule_produces_two_dates() {
        let effective = ymd(2025, Month::Jan, 15);
        let termination = ymd(2026, Month::Jan, 15);
        let s = ScheduleBuilder::new(effective, termination, Period::ZERO, WEEKENDS)
            .with_rule(DateGenerationRule::Zero)
            .build()
            .unwrap();
        assert_eq!(s.dates(), &[effective, termination]);
    }

    #[test]
    fn rejects_equal_effective_and_termination() {
        // Strict effective < termination is required for every rule,
        // including Zero — a single-date schedule has no periods and
        // is not what users typically want.
        let d = ymd(2025, Month::Jan, 15);
        for rule in [
            DateGenerationRule::Zero,
            DateGenerationRule::Forward,
            DateGenerationRule::Backward,
        ] {
            let r = ScheduleBuilder::new(d, d, Period::Months(1), WEEKENDS)
                .with_rule(rule)
                .build();
            assert_eq!(
                r.unwrap_err(),
                TimeError::EffectiveAfterTermination,
                "{rule:?}"
            );
        }
    }

    // ---- Forward rule --------------------------------------------------

    #[test]
    fn forward_quarterly_no_stubs() {
        // Jan 15 2025 → Jan 15 2026, quarterly forward.
        // Expected: Jan 15, Apr 15, Jul 15, Oct 15, Jan 15 next year.
        let effective = ymd(2025, Month::Jan, 15);
        let termination = ymd(2026, Month::Jan, 15);
        let s = ScheduleBuilder::new(effective, termination, Period::Months(3), WEEKENDS)
            .with_rule(DateGenerationRule::Forward)
            .build()
            .unwrap();
        assert_eq!(
            s.dates(),
            &[
                ymd(2025, Month::Jan, 15),
                ymd(2025, Month::Apr, 15),
                ymd(2025, Month::Jul, 15),
                ymd(2025, Month::Oct, 15),
                ymd(2026, Month::Jan, 15),
            ],
        );
    }

    #[test]
    fn forward_with_back_stub() {
        // Jan 15 2025 (Wed) → Mar 1 2025 (Sat), monthly forward.
        // Unadjusted: Jan 15, Feb 15 (Sat), Mar 1 (Sat back stub).
        // ModFol on Feb 15: Following → Mon Feb 17 (still in Feb), so
        // ModFol keeps Feb 17. Termination uses default Unadjusted →
        // stays Sat Mar 1.
        let s = ScheduleBuilder::new(
            ymd(2025, Month::Jan, 15),
            ymd(2025, Month::Mar, 1),
            Period::Months(1),
            WEEKENDS,
        )
        .with_rule(DateGenerationRule::Forward)
        .build()
        .unwrap();
        assert_eq!(
            s.dates(),
            &[
                ymd(2025, Month::Jan, 15),
                ymd(2025, Month::Feb, 17),
                ymd(2025, Month::Mar, 1),
            ],
        );
    }

    #[test]
    fn forward_with_explicit_first_date() {
        // Effective Jan 5, first_date Jan 15, termination Apr 15, tenor 1M.
        // Front stub: Jan 5 → Jan 15. Then regular: Jan 15, Feb 15, Mar 15, Apr 15.
        let s = ScheduleBuilder::new(
            ymd(2025, Month::Jan, 5),
            ymd(2025, Month::Apr, 15),
            Period::Months(1),
            WEEKENDS,
        )
        .with_rule(DateGenerationRule::Forward)
        .with_first_date(ymd(2025, Month::Jan, 15))
        .with_convention(BusinessDayConvention::Unadjusted)
        .with_termination_convention(BusinessDayConvention::Unadjusted)
        .build()
        .unwrap();
        assert_eq!(
            s.dates(),
            &[
                ymd(2025, Month::Jan, 5),
                ymd(2025, Month::Jan, 15),
                ymd(2025, Month::Feb, 15),
                ymd(2025, Month::Mar, 15),
                ymd(2025, Month::Apr, 15),
            ],
        );
    }

    // ---- Backward rule -------------------------------------------------

    #[test]
    fn backward_quarterly_no_stubs() {
        // Jan 15 2025 → Jan 15 2026, quarterly backward.
        // Same result as forward when the period divides evenly.
        let s = ScheduleBuilder::new(
            ymd(2025, Month::Jan, 15),
            ymd(2026, Month::Jan, 15),
            Period::Months(3),
            WEEKENDS,
        )
        .with_rule(DateGenerationRule::Backward)
        .build()
        .unwrap();
        assert_eq!(
            s.dates(),
            &[
                ymd(2025, Month::Jan, 15),
                ymd(2025, Month::Apr, 15),
                ymd(2025, Month::Jul, 15),
                ymd(2025, Month::Oct, 15),
                ymd(2026, Month::Jan, 15),
            ],
        );
    }

    #[test]
    fn backward_with_front_stub() {
        // Jan 1 2025 → Apr 15 2025, monthly backward.
        // Backward from Apr 15: Apr 15, Mar 15, Feb 15, Jan 15. Then front stub Jan 1.
        let s = ScheduleBuilder::new(
            ymd(2025, Month::Jan, 1),
            ymd(2025, Month::Apr, 15),
            Period::Months(1),
            WEEKENDS,
        )
        .with_rule(DateGenerationRule::Backward)
        .with_convention(BusinessDayConvention::Unadjusted)
        .with_termination_convention(BusinessDayConvention::Unadjusted)
        .build()
        .unwrap();
        assert_eq!(
            s.dates(),
            &[
                ymd(2025, Month::Jan, 1),
                ymd(2025, Month::Jan, 15),
                ymd(2025, Month::Feb, 15),
                ymd(2025, Month::Mar, 15),
                ymd(2025, Month::Apr, 15),
            ],
        );
    }

    #[test]
    fn backward_with_explicit_next_to_last_date() {
        // Effective Jan 1, next_to_last Mar 15, termination Apr 15, tenor 1M.
        // Backward from Mar 15: Mar 15, Feb 15, Jan 15. Then front stub Jan 1.
        // Back stub: Mar 15 → Apr 15.
        let s = ScheduleBuilder::new(
            ymd(2025, Month::Jan, 1),
            ymd(2025, Month::Apr, 15),
            Period::Months(1),
            WEEKENDS,
        )
        .with_rule(DateGenerationRule::Backward)
        .with_next_to_last_date(ymd(2025, Month::Mar, 15))
        .with_convention(BusinessDayConvention::Unadjusted)
        .with_termination_convention(BusinessDayConvention::Unadjusted)
        .build()
        .unwrap();
        assert_eq!(
            s.dates(),
            &[
                ymd(2025, Month::Jan, 1),
                ymd(2025, Month::Jan, 15),
                ymd(2025, Month::Feb, 15),
                ymd(2025, Month::Mar, 15),
                ymd(2025, Month::Apr, 15),
            ],
        );
    }

    // ---- `EoM`flag ------------------------------------------------------

    #[test]
    fn eom_flag_preserves_eom_through_short_months() {
        // Seed Feb 28 (EoM in non-leap 2025). Monthly. Without EoM:
        // Feb 28, Mar 28, Apr 28, May 28. With EoM: Feb 28, Mar 31,
        // Apr 30, May 31, ... — every step lands on `EoM`of its month.
        let s = ScheduleBuilder::new(
            ymd(2025, Month::Feb, 28),
            ymd(2025, Month::Jun, 30),
            Period::Months(1),
            WEEKENDS,
        )
        .with_rule(DateGenerationRule::Forward)
        .with_end_of_month(true)
        .with_convention(BusinessDayConvention::Unadjusted)
        .with_termination_convention(BusinessDayConvention::Unadjusted)
        .build()
        .unwrap();
        assert_eq!(
            s.dates(),
            &[
                ymd(2025, Month::Feb, 28),
                ymd(2025, Month::Mar, 31),
                ymd(2025, Month::Apr, 30),
                ymd(2025, Month::May, 31),
                ymd(2025, Month::Jun, 30),
            ],
        );
    }

    #[test]
    fn eom_flag_inert_when_seed_is_not_eom() {
        // Seed Jan 15 (not EoM). `EoM`flag does nothing — same as off.
        let with_flag = ScheduleBuilder::new(
            ymd(2025, Month::Jan, 15),
            ymd(2025, Month::May, 15),
            Period::Months(1),
            WEEKENDS,
        )
        .with_rule(DateGenerationRule::Forward)
        .with_end_of_month(true)
        .with_convention(BusinessDayConvention::Unadjusted)
        .with_termination_convention(BusinessDayConvention::Unadjusted)
        .build()
        .unwrap();
        let without_flag = ScheduleBuilder::new(
            ymd(2025, Month::Jan, 15),
            ymd(2025, Month::May, 15),
            Period::Months(1),
            WEEKENDS,
        )
        .with_rule(DateGenerationRule::Forward)
        .with_convention(BusinessDayConvention::Unadjusted)
        .with_termination_convention(BusinessDayConvention::Unadjusted)
        .build()
        .unwrap();
        assert_eq!(with_flag.dates(), without_flag.dates());
    }

    // ---- BDC -----------------------------------------------------------

    #[test]
    fn termination_convention_distinct_from_interior() {
        // Sun Jan 5 2025 effective → ModFol moves to Mon Jan 6.
        // Sat Jan 17 2026 termination → Unadjusted keeps Sat Jan 17.
        // The two BDCs are applied differently: regular interior
        // dates (and effective) use the interior convention,
        // termination uses its own.
        let s = ScheduleBuilder::new(
            ymd(2025, Month::Jan, 5),
            ymd(2026, Month::Jan, 17),
            Period::Months(3),
            WEEKENDS,
        )
        .with_rule(DateGenerationRule::Forward)
        .with_convention(BusinessDayConvention::ModifiedFollowing)
        .with_termination_convention(BusinessDayConvention::Unadjusted)
        .build()
        .unwrap();
        assert_eq!(s.first().unwrap(), ymd(2025, Month::Jan, 6));
        assert_eq!(s.last().unwrap(), ymd(2026, Month::Jan, 17));
    }

    // ---- Periods iterator ---------------------------------------------

    #[test]
    fn periods_iterator_yields_adjacent_pairs() {
        let s = ScheduleBuilder::new(
            ymd(2025, Month::Jan, 15),
            ymd(2025, Month::Apr, 15),
            Period::Months(1),
            WEEKENDS,
        )
        .with_rule(DateGenerationRule::Forward)
        .with_convention(BusinessDayConvention::Unadjusted)
        .with_termination_convention(BusinessDayConvention::Unadjusted)
        .build()
        .unwrap();
        let pairs: Vec<(Date, Date)> = s.periods().map(Into::into).collect();
        assert_eq!(
            pairs,
            vec![
                (ymd(2025, Month::Jan, 15), ymd(2025, Month::Feb, 15)),
                (ymd(2025, Month::Feb, 15), ymd(2025, Month::Mar, 15)),
                (ymd(2025, Month::Mar, 15), ymd(2025, Month::Apr, 15)),
            ],
        );
    }

    #[test]
    fn accrual_period_exposes_start_and_end() {
        let s = ScheduleBuilder::new(
            ymd(2025, Month::Jan, 15),
            ymd(2025, Month::Mar, 15),
            Period::Months(1),
            WEEKENDS,
        )
        .with_rule(DateGenerationRule::Forward)
        .with_convention(BusinessDayConvention::Unadjusted)
        .with_termination_convention(BusinessDayConvention::Unadjusted)
        .build()
        .unwrap();
        let p = s.periods().next().unwrap();
        assert_eq!(p.start(), ymd(2025, Month::Jan, 15));
        assert_eq!(p.end(), ymd(2025, Month::Feb, 15));
    }

    // ---- Validation errors --------------------------------------------

    #[test]
    fn rejects_zero_tenor_for_non_zero_rule() {
        let r = ScheduleBuilder::new(
            ymd(2025, Month::Jan, 1),
            ymd(2026, Month::Jan, 1),
            Period::ZERO,
            WEEKENDS,
        )
        .with_rule(DateGenerationRule::Forward)
        .build();
        assert_eq!(r.unwrap_err(), TimeError::ZeroTenor);
    }

    #[test]
    fn rejects_termination_at_or_before_effective() {
        let r = ScheduleBuilder::new(
            ymd(2025, Month::Apr, 1),
            ymd(2025, Month::Jan, 1),
            Period::Months(1),
            WEEKENDS,
        )
        .build();
        assert_eq!(r.unwrap_err(), TimeError::EffectiveAfterTermination);
    }

    #[test]
    fn rejects_first_date_outside_range() {
        let r = ScheduleBuilder::new(
            ymd(2025, Month::Jan, 1),
            ymd(2025, Month::Apr, 1),
            Period::Months(1),
            WEEKENDS,
        )
        .with_first_date(ymd(2026, Month::Jan, 1)) // beyond termination
        .build();
        assert_eq!(r.unwrap_err(), TimeError::StubDateOutOfRange);
    }

    #[test]
    fn rejects_first_date_after_next_to_last_date() {
        let r = ScheduleBuilder::new(
            ymd(2025, Month::Jan, 1),
            ymd(2025, Month::Dec, 1),
            Period::Months(1),
            WEEKENDS,
        )
        .with_first_date(ymd(2025, Month::Jul, 1))
        .with_next_to_last_date(ymd(2025, Month::Jun, 1))
        .build();
        assert_eq!(r.unwrap_err(), TimeError::StubDateOutOfRange);
    }

    // ---- Query API ----------------------------------------------------

    fn quarterly_2025() -> Schedule {
        ScheduleBuilder::new(
            ymd(2025, Month::Jan, 15),
            ymd(2026, Month::Jan, 15),
            Period::Months(3),
            WEEKENDS,
        )
        .forwards()
        .with_convention(BusinessDayConvention::Unadjusted)
        .with_termination_convention(BusinessDayConvention::Unadjusted)
        .build()
        .unwrap()
    }

    #[test]
    fn index_returns_date_at_position() {
        let s = quarterly_2025();
        assert_eq!(s[0], ymd(2025, Month::Jan, 15));
        assert_eq!(s[2], ymd(2025, Month::Jul, 15));
        assert_eq!(s[s.len() - 1], ymd(2026, Month::Jan, 15));
    }

    #[test]
    #[should_panic(expected = "out of bounds")]
    fn index_panics_out_of_bounds() {
        let s = quarterly_2025();
        let _ = s[s.len()];
    }

    #[test]
    fn get_returns_some_or_none() {
        let s = quarterly_2025();
        assert_eq!(s.get(0), Some(ymd(2025, Month::Jan, 15)));
        assert_eq!(s.get(s.len()), None);
    }

    #[test]
    fn previous_date_examples() {
        let s = quarterly_2025();
        // Strictly less than: Aug 1 → previous is Jul 15.
        assert_eq!(
            s.previous_date(ymd(2025, Month::Aug, 1)),
            Some(ymd(2025, Month::Jul, 15)),
        );
        // ref_date == a schedule date → previous is the date BEFORE it.
        assert_eq!(
            s.previous_date(ymd(2025, Month::Jul, 15)),
            Some(ymd(2025, Month::Apr, 15)),
        );
        // Before the first → None.
        assert_eq!(s.previous_date(ymd(2025, Month::Jan, 15)), None);
        assert_eq!(s.previous_date(ymd(2024, Month::Dec, 1)), None);
        // After the last → returns the last.
        assert_eq!(
            s.previous_date(ymd(2027, Month::Jan, 1)),
            Some(ymd(2026, Month::Jan, 15)),
        );
    }

    #[test]
    fn next_date_examples() {
        let s = quarterly_2025();
        // Strictly greater than: Aug 1 → next is Oct 15.
        assert_eq!(
            s.next_date(ymd(2025, Month::Aug, 1)),
            Some(ymd(2025, Month::Oct, 15)),
        );
        // ref_date == a schedule date → next is the date AFTER it.
        assert_eq!(
            s.next_date(ymd(2025, Month::Jul, 15)),
            Some(ymd(2025, Month::Oct, 15)),
        );
        // Before the first → returns the first.
        assert_eq!(
            s.next_date(ymd(2024, Month::Dec, 1)),
            Some(ymd(2025, Month::Jan, 15)),
        );
        // After the last → None.
        assert_eq!(s.next_date(ymd(2026, Month::Jan, 15)), None);
        assert_eq!(s.next_date(ymd(2027, Month::Jan, 1)), None);
    }

    #[test]
    fn lower_bound_examples() {
        let s = quarterly_2025();
        // Strictly between Apr 15 and Jul 15 → first at-or-after is Jul 15.
        assert_eq!(
            s.lower_bound(ymd(2025, Month::May, 1)),
            Some(ymd(2025, Month::Jul, 15)),
        );
        // ref_date == a schedule date → that date is the lower bound (inclusive).
        // Distinguishes lower_bound from next_date, which would skip past it.
        assert_eq!(
            s.lower_bound(ymd(2025, Month::Jul, 15)),
            Some(ymd(2025, Month::Jul, 15)),
        );
        // Before the first → returns the first.
        assert_eq!(
            s.lower_bound(ymd(2024, Month::Dec, 1)),
            Some(ymd(2025, Month::Jan, 15)),
        );
        // Past the last → None.
        assert_eq!(s.lower_bound(ymd(2027, Month::Jan, 1)), None);
    }

    #[test]
    fn after_truncates_to_dates_at_or_after_cutoff() {
        let s = quarterly_2025();
        // Cutoff between Apr and Jul — keep Jul, Oct, Jan-next.
        let truncated = s.after(ymd(2025, Month::May, 1));
        assert_eq!(
            truncated.dates(),
            &[
                ymd(2025, Month::Jul, 15),
                ymd(2025, Month::Oct, 15),
                ymd(2026, Month::Jan, 15),
            ],
        );
        // Cutoff equal to a schedule date — that date is included.
        let truncated = s.after(ymd(2025, Month::Jul, 15));
        assert_eq!(truncated.first(), Some(ymd(2025, Month::Jul, 15)));
        // Cutoff past the last → empty.
        assert!(s.after(ymd(2027, Month::Jan, 1)).is_empty());
        // Cutoff before the first → all dates.
        let kept = s.after(ymd(2024, Month::Dec, 1));
        assert_eq!(kept.len(), s.len());
    }

    #[test]
    fn until_truncates_to_dates_at_or_before_cutoff() {
        let s = quarterly_2025();
        // Cutoff between Apr and Jul — keep Jan, Apr.
        let truncated = s.until(ymd(2025, Month::May, 1));
        assert_eq!(
            truncated.dates(),
            &[ymd(2025, Month::Jan, 15), ymd(2025, Month::Apr, 15)],
        );
        // Cutoff equal to a schedule date — that date is included.
        let truncated = s.until(ymd(2025, Month::Jul, 15));
        assert_eq!(truncated.last(), Some(ymd(2025, Month::Jul, 15)));
        // Cutoff before the first → empty.
        assert!(s.until(ymd(2024, Month::Dec, 1)).is_empty());
        // Cutoff past the last → all dates.
        let kept = s.until(ymd(2027, Month::Jan, 1));
        assert_eq!(kept.len(), s.len());
    }

    // ---- Property tests -----------------------------------------------

    proptest! {
        /// Schedule dates are strictly monotonically increasing, no
        /// matter the rule or stub configuration.
        #[test]
        fn dates_strictly_monotonic(
            x in 1u32..(Date::MAX.serial() - 5_000),
            span_days in 60u32..=2_000,
            forward in any::<bool>(),
        ) {
            let effective = Date::from_serial(x).unwrap();
            let termination = Date::from_serial(x + span_days).unwrap();
            let rule = if forward { DateGenerationRule::Forward } else { DateGenerationRule::Backward };
            let result = ScheduleBuilder::new(
                effective,
                termination,
                Period::Months(1),
                WEEKENDS,
            )
            .with_rule(rule)
            .with_convention(BusinessDayConvention::ModifiedFollowing)
            .with_termination_convention(BusinessDayConvention::Unadjusted)
            .build();
            if let Ok(s) = result {
                for w in s.dates().windows(2) {
                    prop_assert!(w[0] < w[1]);
                }
            }
        }

        /// First and last dates of the produced schedule are the
        /// adjusted effective and termination respectively.
        #[test]
        fn endpoints_are_adjusted_effective_and_termination(
            x in 1u32..(Date::MAX.serial() - 5_000),
            span_days in 60u32..=2_000,
        ) {
            let effective = Date::from_serial(x).unwrap();
            let termination = Date::from_serial(x + span_days).unwrap();
            let result = ScheduleBuilder::new(
                effective,
                termination,
                Period::Months(1),
                WEEKENDS,
            )
            .with_convention(BusinessDayConvention::ModifiedFollowing)
            .with_termination_convention(BusinessDayConvention::Unadjusted)
            .build();
            if let Ok(s) = result {
                let expected_first = WEEKENDS
                    .adjust(effective, BusinessDayConvention::ModifiedFollowing)
                    .unwrap();
                let expected_last = WEEKENDS
                    .adjust(termination, BusinessDayConvention::Unadjusted)
                    .unwrap();
                prop_assert_eq!(s.first().unwrap(), expected_first);
                prop_assert_eq!(s.last().unwrap(), expected_last);
            }
        }

        /// Interior dates are business days under any convention
        /// other than `Unadjusted`.
        #[test]
        fn interior_dates_are_business_days(
            x in 1u32..(Date::MAX.serial() - 5_000),
            span_days in 60u32..=2_000,
        ) {
            let effective = Date::from_serial(x).unwrap();
            let termination = Date::from_serial(x + span_days).unwrap();
            let result = ScheduleBuilder::new(
                effective,
                termination,
                Period::Months(1),
                WEEKENDS,
            )
            .with_convention(BusinessDayConvention::ModifiedFollowing)
            .with_termination_convention(BusinessDayConvention::ModifiedFollowing)
            .build();
            if let Ok(s) = result {
                for d in &s {
                    prop_assert!(WEEKENDS.is_business_day(*d));
                }
            }
        }
    }

    // ---- Stepper ------------------------------------------------------

    #[test]
    fn stepper_steps_forward_in_months() {
        let s = Stepper::new(ymd(2025, Month::Jan, 15), Period::Months(3), false);
        assert_eq!(s.step(1).unwrap(), ymd(2025, Month::Apr, 15));
        assert_eq!(s.step(2).unwrap(), ymd(2025, Month::Jul, 15));
        assert_eq!(s.step(4).unwrap(), ymd(2026, Month::Jan, 15));
    }

    #[test]
    fn stepper_steps_forward_in_days() {
        let s = Stepper::new(ymd(2025, Month::Jan, 1), Period::Days(7), false);
        assert_eq!(s.step(1).unwrap(), ymd(2025, Month::Jan, 8));
        assert_eq!(s.step(3).unwrap(), ymd(2025, Month::Jan, 22));
    }

    #[test]
    fn stepper_with_negative_step_walks_backward() {
        let s = Stepper::new(ymd(2025, Month::Apr, 15), Period::Months(-1), false);
        assert_eq!(s.step(1).unwrap(), ymd(2025, Month::Mar, 15));
        assert_eq!(s.step(3).unwrap(), ymd(2025, Month::Jan, 15));
    }

    #[test]
    fn stepper_eom_snaps_when_seed_is_eom_and_step_is_monthly() {
        // Feb 28 + 1M without `EoM`= Mar 28; with `EoM`= Mar 31.
        let s = Stepper::new(ymd(2025, Month::Feb, 28), Period::Months(1), true);
        assert_eq!(s.step(1).unwrap(), ymd(2025, Month::Mar, 31));
        assert_eq!(s.step(2).unwrap(), ymd(2025, Month::Apr, 30));
        assert_eq!(s.step(3).unwrap(), ymd(2025, Month::May, 31));
    }

    #[test]
    fn stepper_eom_inert_when_seed_is_not_eom() {
        let with_flag = Stepper::new(ymd(2025, Month::Jan, 15), Period::Months(1), true);
        let without_flag = Stepper::new(ymd(2025, Month::Jan, 15), Period::Months(1), false);
        for i in 1..=4 {
            assert_eq!(with_flag.step(i).unwrap(), without_flag.step(i).unwrap());
        }
    }

    #[test]
    fn stepper_eom_inert_for_day_step() {
        // Seed is EoM but step is in days — `EoM`rule does not apply.
        let s = Stepper::new(ymd(2025, Month::Feb, 28), Period::Days(1), true);
        assert_eq!(s.step(1).unwrap(), ymd(2025, Month::Mar, 1));
    }

    // ---- Walk ---------------------------------------------------------

    #[test]
    fn walk_forward_emits_interior_and_halts_before_stop() {
        let stepper = Stepper::new(ymd(2025, Month::Jan, 15), Period::Months(1), false);
        let stop = ymd(2025, Month::Apr, 15);
        let dates: Vec<Date> = Walk::new(&stepper, stop, Direction::Forward)
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            dates,
            vec![ymd(2025, Month::Feb, 15), ymd(2025, Month::Mar, 15)],
        );
    }

    #[test]
    fn walk_backward_emits_interior_and_halts_after_stop() {
        let stepper = Stepper::new(ymd(2025, Month::Apr, 15), Period::Months(-1), false);
        let stop = ymd(2025, Month::Jan, 15);
        let dates: Vec<Date> = Walk::new(&stepper, stop, Direction::Backward)
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            dates,
            vec![ymd(2025, Month::Mar, 15), ymd(2025, Month::Feb, 15)],
        );
    }

    #[test]
    fn walk_immediately_terminates_when_stop_equals_seed_plus_step() {
        // Forward: stop hit on first candidate → empty.
        let stepper = Stepper::new(ymd(2025, Month::Jan, 15), Period::Months(1), false);
        let stop = ymd(2025, Month::Feb, 15);
        let dates: Vec<Date> = Walk::new(&stepper, stop, Direction::Forward)
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(dates.is_empty());
    }

    // ---- Generator ----------------------------------------------------

    #[test]
    fn generator_forward_orders_anchors_walk_and_back_stub() {
        let b = ScheduleBuilder::new(
            ymd(2025, Month::Jan, 15),
            ymd(2025, Month::May, 1),
            Period::Months(1),
            WEEKENDS,
        )
        .with_next_to_last_date(ymd(2025, Month::Apr, 15));
        let dates = Generator::forward(&b).generate().unwrap().dates;
        assert_eq!(
            dates,
            vec![
                ymd(2025, Month::Jan, 15),
                ymd(2025, Month::Feb, 15),
                ymd(2025, Month::Mar, 15),
                ymd(2025, Month::Apr, 15),
                ymd(2025, Month::May, 1),
            ],
        );
    }

    #[test]
    fn generator_backward_reverses_into_chronological_order() {
        let b = ScheduleBuilder::new(
            ymd(2025, Month::Jan, 1),
            ymd(2025, Month::Apr, 15),
            Period::Months(1),
            WEEKENDS,
        );
        let dates = Generator::backward(&b).unwrap().generate().unwrap().dates;
        assert_eq!(
            dates,
            vec![
                ymd(2025, Month::Jan, 1),
                ymd(2025, Month::Jan, 15),
                ymd(2025, Month::Feb, 15),
                ymd(2025, Month::Mar, 15),
                ymd(2025, Month::Apr, 15),
            ],
        );
    }

    // ---- BdcAdjuster --------------------------------------------------

    #[test]
    fn bdc_adjuster_applies_interior_to_all_but_last() {
        // Sun Jan 5 2025 (interior, ModFol → Mon Jan 6).
        // Sat Jan 17 2026 (terminal, Unadjusted → stays Sat).
        let dates = vec![ymd(2025, Month::Jan, 5), ymd(2026, Month::Jan, 17)];
        let adjusted = BdcAdjuster::new(
            WEEKENDS,
            BusinessDayConvention::ModifiedFollowing,
            BusinessDayConvention::Unadjusted,
        )
        .apply(dates)
        .unwrap();
        assert_eq!(
            adjusted,
            vec![ymd(2025, Month::Jan, 6), ymd(2026, Month::Jan, 17)],
        );
    }

    #[test]
    fn bdc_adjuster_uses_terminal_only_on_last_index() {
        // Three dates, all on weekends. Interior=Following (forward to Mon),
        // terminal=Preceding (back to Fri). The last date should move to Fri,
        // the others to Mon — proves the index split.
        let dates = vec![
            ymd(2025, Month::Jan, 4), // Sat → Mon Jan 6
            ymd(2025, Month::Feb, 1), // Sat → Mon Feb 3
            ymd(2025, Month::Mar, 1), // Sat → Fri Feb 28
        ];
        let adjusted = BdcAdjuster::new(
            WEEKENDS,
            BusinessDayConvention::Following,
            BusinessDayConvention::Preceding,
        )
        .apply(dates)
        .unwrap();
        assert_eq!(
            adjusted,
            vec![
                ymd(2025, Month::Jan, 6),
                ymd(2025, Month::Feb, 3),
                ymd(2025, Month::Feb, 28),
            ],
        );
    }

    #[test]
    fn bdc_adjuster_returns_empty_for_empty_input() {
        let adjusted = BdcAdjuster::new(
            WEEKENDS,
            BusinessDayConvention::ModifiedFollowing,
            BusinessDayConvention::Unadjusted,
        )
        .apply(Vec::new())
        .unwrap();
        assert!(adjusted.is_empty());
    }

    // ---- with_frequency ----------------------------------------------

    #[test]
    fn with_frequency_overrides_tenor_with_canonical_period() {
        // Build the same quarterly schedule via tenor and via frequency
        // and check the dates match.
        let via_tenor = ScheduleBuilder::new(
            ymd(2025, Month::Jan, 15),
            ymd(2026, Month::Jan, 15),
            Period::Months(3),
            WEEKENDS,
        )
        .forwards()
        .with_convention(BusinessDayConvention::Unadjusted)
        .with_termination_convention(BusinessDayConvention::Unadjusted)
        .build()
        .unwrap();
        let via_freq = ScheduleBuilder::new(
            ymd(2025, Month::Jan, 15),
            ymd(2026, Month::Jan, 15),
            Period::Months(1), // gets overridden
            WEEKENDS,
        )
        .with_frequency(Frequency::Quarterly)
        .forwards()
        .with_convention(BusinessDayConvention::Unadjusted)
        .with_termination_convention(BusinessDayConvention::Unadjusted)
        .build()
        .unwrap();
        assert_eq!(via_tenor.dates(), via_freq.dates());
    }

    // ---- Schedule::try_from -------------------------------------------

    #[test]
    fn try_from_accepts_strictly_monotonic_dates() {
        let dates = vec![
            ymd(2025, Month::Jan, 15),
            ymd(2025, Month::Feb, 15),
            ymd(2025, Month::Mar, 15),
        ];
        let s = Schedule::try_from(dates.clone()).unwrap();
        assert_eq!(s.dates(), &dates[..]);
    }

    #[test]
    fn try_from_rejects_equal_adjacent() {
        let d = ymd(2025, Month::Jan, 15);
        let r = Schedule::try_from(vec![d, d]);
        assert_eq!(r.unwrap_err(), TimeError::ScheduleNotMonotonic);
    }

    #[test]
    fn try_from_rejects_decreasing() {
        let r = Schedule::try_from(vec![ymd(2025, Month::Feb, 15), ymd(2025, Month::Jan, 15)]);
        assert_eq!(r.unwrap_err(), TimeError::ScheduleNotMonotonic);
    }

    // ---- reference dates ----------------------------------------------

    #[test]
    fn regular_schedule_reference_dates_equal_coupon_dates() {
        let s = ScheduleBuilder::new(
            ymd(2025, Month::Jan, 15),
            ymd(2026, Month::Jan, 15),
            Period::Months(3),
            WEEKENDS,
        )
        .backwards()
        .with_convention(BusinessDayConvention::Unadjusted)
        .with_termination_convention(BusinessDayConvention::Unadjusted)
        .build()
        .unwrap();
        assert_eq!(s.reference_dates(), s.dates());
    }

    #[test]
    fn front_stub_reference_dates_carry_the_notional_boundary() {
        // Backward semiannual with a front stub: the first reference
        // date is one tenor before the first coupon.
        let s = ScheduleBuilder::new(
            ymd(2002, Month::Aug, 15),
            ymd(2004, Month::Jan, 15),
            Period::Months(6),
            WEEKENDS,
        )
        .backwards()
        .with_convention(BusinessDayConvention::Unadjusted)
        .with_termination_convention(BusinessDayConvention::Unadjusted)
        .build()
        .unwrap();
        assert_eq!(s.reference_dates()[0], ymd(2002, Month::Jul, 15));
        assert_eq!(&s.reference_dates()[1..], &s.dates()[1..]);
    }

    #[test]
    fn back_stub_reference_dates_carry_the_notional_boundary() {
        // Forward semiannual with a back stub: the last reference
        // date is one tenor after the last regular coupon.
        let s = ScheduleBuilder::new(
            ymd(2003, Month::Jan, 15),
            ymd(2004, Month::Jun, 30),
            Period::Months(6),
            WEEKENDS,
        )
        .forwards()
        .with_convention(BusinessDayConvention::Unadjusted)
        .with_termination_convention(BusinessDayConvention::Unadjusted)
        .build()
        .unwrap();
        let n = s.len();
        assert_eq!(s.reference_dates()[n - 1], ymd(2004, Month::Jul, 15));
        assert_eq!(&s.reference_dates()[..n - 1], &s.dates()[..n - 1]);
    }

    #[test]
    fn from_parts_validates_parallel_lists() {
        let dates = vec![ymd(2025, Month::Jan, 15), ymd(2025, Month::Jul, 15)];
        // Length mismatch.
        assert_eq!(
            Schedule::from_parts(dates.clone(), vec![ymd(2025, Month::Jan, 15)]),
            Err(TimeError::InvalidReferencePeriod),
        );
        // Interior reference date differing from its coupon date.
        let three = vec![
            ymd(2025, Month::Jan, 15),
            ymd(2025, Month::Jul, 15),
            ymd(2026, Month::Jan, 15),
        ];
        let mut bad_refs = three.clone();
        bad_refs[1] = ymd(2025, Month::Jul, 20);
        assert_eq!(
            Schedule::from_parts(three, bad_refs),
            Err(TimeError::InvalidReferencePeriod),
        );
        // Degenerate end reference period.
        assert_eq!(
            Schedule::from_parts(
                dates.clone(),
                vec![ymd(2025, Month::Jul, 15), ymd(2025, Month::Jul, 15)],
            ),
            Err(TimeError::InvalidReferencePeriod),
        );
        // Valid stub refs round-trip.
        let s = Schedule::from_parts(
            dates.clone(),
            vec![ymd(2025, Month::Jan, 1), ymd(2025, Month::Jul, 15)],
        )
        .unwrap();
        assert_eq!(s.dates(), &dates[..]);
    }
}
