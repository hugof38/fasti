//! [`Schedule`] — payment-date generation with business-day
//! adjustment.
//!
//! Modeled on `QuantLib`'s
//! [`ql/time/schedule.hpp`](https://github.com/lballabio/QuantLib/blob/master/ql/time/schedule.hpp).
//!
//! # Generation model
//!
//! A schedule is built from
//!
//! - an `effective` date and a `termination` date,
//! - a regular `tenor` ([`Period`]),
//! - a [`Calendar`] used for business-day adjustment,
//! - a [`DateGenerationRule`] (forward / backward / zero),
//! - optional explicit stub dates (`first_date`, `next_to_last_date`).
//!
//! The builder generates *unadjusted* candidate dates, then runs
//! each through [`Calendar::adjust`] with a chosen
//! [`BusinessDayConvention`]. The termination date can carry its
//! own convention (commonly [`Unadjusted`](BusinessDayConvention::Unadjusted))
//! so the literal maturity is preserved.
//!
//! # Stepping from a fresh seed
//!
//! Each candidate date is computed as `seed + i · tenor`, not by
//! chaining `add_months` from the previous candidate. The chaining
//! approach has a well-known clamp pathology: successive `+1M`
//! steps from `Jan 31` collapse to `Feb 28 → Mar 28 → Apr 28 → …`
//! and stay stuck at day 28, disagreeing with the single-hop
//! `Jan 31 → Mar 31`. The fresh-seed approach produces the natural
//! `Jan 31, Feb 28, Mar 31, Apr 30, May 31, …` instead.
//!
//! The optional `end_of_month` flag goes a step further: if the seed
//! is the last day of its month *and* the tenor is in months/years,
//! every generated date is snapped to the `EoM`of its own month
//! (rather than just clamped). This restores `EoM`after a clamp where
//! the natural day-of-month would not be `EoM`(e.g., `Feb 28 + 1M`
//! → `Mar 28` becomes `Mar 31` under `EoM`).

use alloc::vec;
use alloc::vec::Vec;

use core::ops::Range;

use crate::{BusinessDayConvention, Calendar, Date, Frequency, Period, TimeError};

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

/// Parallel lists of business-day-adjusted dates in chronological
/// order, each of length `periods + 1`: the coupon dates, and the
/// reference dates of the regular coupon grid.
///
/// The two are identical for a regular schedule. At a short or long
/// stub, the end reference date is instead the notional quasi-coupon
/// boundary one tenor from the adjacent coupon, which is what
/// schedule-defined day counts (ACT/ACT ICMA) accrue against.
///
/// `Schedule` owns its dates; once built, it is independent of the
/// [`Calendar`] used to construct it. Serde round-trips both lists
/// through [`TryFrom<(Vec<Date>, Vec<Date>)>`], so a deserialized
/// `Schedule` is always valid.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(try_from = "ScheduleData", into = "ScheduleData")
)]
pub struct Schedule {
    dates: Vec<Date>,
    stubs: StubReferences,
    generation: Option<Generation>,
}

/// The parameters a [`Schedule`] was generated from, retained so that
/// schedule-defined day counts can extend its grid — `QuantLib`'s
/// `Schedule` keeps the same information in its `tenor_` and
/// `endOfMonth_` members.
///
/// Absent on a schedule built from raw date lists, which carry no
/// generation history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Generation {
    /// The regular period between coupons.
    pub tenor: Period,
    /// Whether generation snapped dates to the end of their month.
    pub end_of_month: bool,
}

impl Generation {
    /// `anchor + i · tenor`, the lattice's own stepping rule.
    ///
    /// Computed as a fresh multiple rather than by chaining, which
    /// avoids the add-months clamp pathology: successive `+1M` steps
    /// from `Jan 31` stick at day 28, while `Jan 31 + i·1M` gives the
    /// natural `Jan 31, Feb 28, Mar 31, …`. When `end_of_month` is set
    /// and the anchor is itself month-end, the result snaps to the end
    /// of its own month.
    ///
    /// Negative `i` steps backward. Returns
    /// [`TimeError::DateOutOfRange`] on scaling overflow or a date
    /// outside the supported range.
    pub fn step(self, anchor: Date, i: i32) -> Result<Date, TimeError> {
        let scaled = self.tenor.checked_mul(i).ok_or(TimeError::DateOutOfRange)?;
        anchor.advance(scaled, self.end_of_month)
    }

    /// The notional coupon window `i` steps from `reference` along
    /// this lattice: window 0 is `reference` itself, negative windows
    /// precede it and positive ones follow.
    ///
    /// Schedule-defined day counts walk these when a stub period
    /// reaches beyond the reference period a schedule names for it.
    pub fn window(self, reference: &Range<Date>, i: i32) -> Result<Range<Date>, TimeError> {
        let (anchor, k) = match i {
            0 => return Ok(reference.clone()),
            i if i < 0 => (reference.start, i),
            i => (reference.end, i - 1),
        };
        Ok(self.step(anchor, k)?..self.step(anchor, k + 1)?)
    }
}

impl TryFrom<Vec<Date>> for Schedule {
    type Error = TimeError;

    /// Wrap a chronologically ordered date list — a term sheet's
    /// coupon dates, say. Every period is taken as regular, and the
    /// schedule carries no [`Generation`] because a bare date list
    /// names no lattice.
    fn try_from(dates: Vec<Date>) -> Result<Self, Self::Error> {
        Self::new(dates, StubReferences::default(), None)
    }
}

impl From<Schedule> for Vec<Date> {
    /// Unwrap the coupon date list.
    fn from(s: Schedule) -> Self {
        s.dates
    }
}

/// Where a schedule departs from its regular lattice: the notional
/// quasi-coupon boundary of each end period that is a stub.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct StubReferences {
    front: Option<Date>,
    back: Option<Date>,
}

/// Serde representation of a [`Schedule`], validated on the way back
/// in through [`Schedule::new`].
#[cfg(feature = "serde")]
#[derive(serde::Serialize, serde::Deserialize)]
struct ScheduleData {
    dates: Vec<Date>,
    stubs: StubReferences,
    generation: Option<Generation>,
}

#[cfg(feature = "serde")]
impl TryFrom<ScheduleData> for Schedule {
    type Error = TimeError;

    fn try_from(stored: ScheduleData) -> Result<Self, Self::Error> {
        Self::new(stored.dates, stored.stubs, stored.generation)
    }
}

#[cfg(feature = "serde")]
impl From<Schedule> for ScheduleData {
    fn from(s: Schedule) -> Self {
        Self {
            dates: s.dates,
            stubs: s.stubs,
            generation: s.generation,
        }
    }
}

impl Schedule {
    /// The one constructor: validates that the coupon dates strictly
    /// increase and that any stub boundary lies outside the coupon it
    /// anchors, so every reference period is non-degenerate.
    fn new(
        dates: Vec<Date>,
        stubs: StubReferences,
        generation: Option<Generation>,
    ) -> Result<Self, TimeError> {
        if dates.windows(2).any(|w| w[0] >= w[1]) {
            return Err(TimeError::ScheduleNotMonotonic);
        }
        let front_ok = stubs
            .front
            .is_none_or(|r| dates.get(1).is_some_and(|d| r < *d));
        let back_ok = stubs.back.is_none_or(|r| {
            dates
                .len()
                .checked_sub(2)
                .and_then(|i| dates.get(i))
                .is_some_and(|d| r > *d)
        });
        if !front_ok || !back_ok {
            return Err(TimeError::InvalidReferencePeriod);
        }
        Ok(Self {
            dates,
            stubs,
            generation,
        })
    }

    /// Borrow the adjusted coupon dates as a slice.
    #[must_use]
    pub fn dates(&self) -> &[Date] {
        &self.dates
    }

    /// The reference period each coupon period accrues against —
    /// equal to the coupon period unless it is a stub, where it is the
    /// notional quasi-coupon period one tenor from the adjacent
    /// coupon.
    pub fn reference_periods(&self) -> impl Iterator<Item = Range<Date>> + '_ {
        let last = self.dates.len().saturating_sub(2);
        self.dates.windows(2).enumerate().map(move |(i, w)| {
            let start = if i == 0 {
                self.stubs.front.unwrap_or(w[0])
            } else {
                w[0]
            };
            let end = if i == last {
                self.stubs.back.unwrap_or(w[1])
            } else {
                w[1]
            };
            start..end
        })
    }

    /// The lattice this schedule was generated on, absent when it was
    /// built from a bare date list that names no tenor — mirroring
    /// `QuantLib`'s optional `tenor_`. Only stub periods need it, and
    /// stubs only arise from generation, so a schedule without one is
    /// always entirely regular.
    #[must_use]
    pub const fn generation(&self) -> Option<Generation> {
        self.generation
    }

    /// Iterate accrual periods as half-open ranges — adjacent windows
    /// over the coupon dates.
    pub fn periods(&self) -> impl Iterator<Item = Range<Date>> + '_ {
        self.dates.windows(2).map(|w| w[0]..w[1])
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
            // Slicing off the front drops the front stub with it.
            stubs: StubReferences {
                front: (idx == 0).then_some(self.stubs.front).flatten(),
                ..self.stubs
            },
            generation: self.generation,
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
            // Likewise a truncated tail is no longer the back stub.
            stubs: StubReferences {
                back: (idx == self.dates.len())
                    .then_some(self.stubs.back)
                    .flatten(),
                ..self.stubs
            },
            generation: self.generation,
        }
    }
}

impl core::ops::Deref for Schedule {
    type Target = [Date];

    /// The coupon dates, so a `Schedule` indexes, slices, and iterates
    /// like the date list it is. Reference dates stay explicit.
    fn deref(&self) -> &Self::Target {
        &self.dates
    }
}

impl<'a> IntoIterator for &'a Schedule {
    type Item = &'a Date;
    type IntoIter = core::slice::Iter<'a, Date>;

    fn into_iter(self) -> Self::IntoIter {
        self.dates.iter()
    }
}

impl IntoIterator for Schedule {
    type Item = Date;
    type IntoIter = alloc::vec::IntoIter<Date>;

    fn into_iter(self) -> Self::IntoIter {
        self.dates.into_iter()
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
/// // A Schedule derefs to its coupon dates, so slice methods apply.
/// assert_eq!(schedule.len(), 5);
/// assert_eq!(schedule[0], effective);
/// assert_eq!(schedule.last().copied(), Some(termination));
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
        let (dates, stubs) = match self.rule {
            // A Zero schedule's single period is its own reference.
            DateGenerationRule::Zero => (
                vec![self.effective, self.termination],
                StubReferences::default(),
            ),
            DateGenerationRule::Forward => Generator::forward(&self).generate()?,
            DateGenerationRule::Backward => Generator::backward(&self).generate()?,
        };
        // Stub boundaries roll under the convention that governs their
        // side, matching the coupon date they stand in for.
        let stubs = StubReferences {
            front: stubs
                .front
                .map(|d| self.calendar.adjust(d, self.convention))
                .transpose()?,
            back: stubs
                .back
                .map(|d| self.calendar.adjust(d, self.termination_convention))
                .transpose()?,
        };
        let adjusted =
            BdcAdjuster::new(self.calendar, self.convention, self.termination_convention)
                .apply(dates)?;
        let generation = (!matches!(self.rule, DateGenerationRule::Zero)).then(|| self.lattice());
        Schedule::new(adjusted, stubs, generation)
    }

    /// The lattice this builder generates on.
    const fn lattice(&self) -> Generation {
        Generation {
            tenor: self.tenor,
            end_of_month: self.end_of_month,
        }
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

/// Iterator over lattice points from a seed, halting at (and never
/// emitting) the first candidate that has reached or passed `stop` in
/// the configured direction.
///
/// The walk yields only the *interior* dates — anchors and stubs
/// are the [`Generator`]'s job.
struct Walk {
    lattice: Generation,
    seed: Date,
    stop: Date,
    direction: Direction,
    i: i32,
}

impl Walk {
    const fn new(lattice: Generation, seed: Date, stop: Date, direction: Direction) -> Self {
        Self {
            lattice,
            seed,
            stop,
            direction,
            i: 1,
        }
    }

    /// The lattice point at index `i`, signed by direction.
    fn at(&self, i: i32) -> Result<Date, TimeError> {
        let step = match self.direction {
            Direction::Forward => i,
            Direction::Backward => -i,
        };
        self.lattice.step(self.seed, step)
    }

    fn past_stop(&self, candidate: Date) -> bool {
        match self.direction {
            Direction::Forward => candidate >= self.stop,
            Direction::Backward => candidate <= self.stop,
        }
    }
}

impl Iterator for Walk {
    type Item = Result<Date, TimeError>;

    fn next(&mut self) -> Option<Self::Item> {
        let candidate = match self.at(self.i) {
            Ok(d) => d,
            Err(e) => return Some(Err(e)),
        };
        if self.past_stop(candidate) {
            return None;
        }
        // `i` overflow is unreachable: the supported date range
        // (1901..=2199, ~109k days) means stepping errors
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
    lattice: Generation,
    direction: Direction,
}

impl Generator {
    fn forward(b: &ScheduleBuilder<'_>) -> Self {
        Self {
            start_anchor: b.effective,
            start_stub: b.first_date,
            end_anchor: b.termination,
            end_stub: b.next_to_last_date,
            lattice: b.lattice(),
            direction: Direction::Forward,
        }
    }

    /// The same machinery as `Forward`; `Walk` signs its steps by
    /// direction, so no negated tenor is needed.
    fn backward(b: &ScheduleBuilder<'_>) -> Self {
        Self {
            start_anchor: b.termination,
            start_stub: b.next_to_last_date,
            end_anchor: b.effective,
            end_stub: b.first_date,
            lattice: b.lattice(),
            direction: Direction::Backward,
        }
    }

    fn generate(&self) -> Result<(Vec<Date>, StubReferences), TimeError> {
        let seed = self.start_stub.unwrap_or(self.start_anchor);
        let stop = self.end_stub.unwrap_or(self.end_anchor);

        let mut out: Vec<Date> = Vec::new();
        out.push(self.start_anchor);
        if let Some(s) = self.start_stub {
            out.push(s);
        }
        let mut walk = Walk::new(self.lattice, seed, stop, self.direction);
        for candidate in walk.by_ref() {
            out.push(candidate?);
        }
        // The lattice point the walk halted on: the stop-side
        // quasi-coupon boundary, equal to the anchor exactly when that
        // period is regular.
        let stopping = walk.at(walk.i)?;
        if let Some(s) = self.end_stub {
            out.push(s);
        }
        out.push(self.end_anchor);

        // A stub's boundary steps away from the stub along the
        // lattice, one step *against* the walk's direction. `None`
        // marks a regular end, which is its own reference.
        let outward = match self.direction {
            Direction::Forward => -1,
            Direction::Backward => 1,
        };
        let seed_side = self
            .start_stub
            .map(|s| self.lattice.step(s, outward))
            .transpose()?;
        let stop_side = match self.end_stub {
            Some(s) => Some(self.lattice.step(s, -outward)?),
            None => (stopping != self.end_anchor).then_some(stopping),
        };

        let stubs = match self.direction {
            Direction::Forward => StubReferences {
                front: seed_side,
                back: stop_side,
            },
            Direction::Backward => {
                out.reverse();
                StubReferences {
                    front: stop_side,
                    back: seed_side,
                }
            }
        };
        Ok((out, stubs))
    }
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
        assert_eq!(s.first().copied().unwrap(), ymd(2025, Month::Jan, 6));
        assert_eq!(s.last().copied().unwrap(), ymd(2026, Month::Jan, 17));
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
        let pairs: Vec<(Date, Date)> = s.periods().map(|p| (p.start, p.end)).collect();
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
        assert_eq!(p.start, ymd(2025, Month::Jan, 15));
        assert_eq!(p.end, ymd(2025, Month::Feb, 15));
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
    fn slice_access_is_bounds_checked() {
        let s = quarterly_2025();
        assert_eq!(s.first().copied(), Some(ymd(2025, Month::Jan, 15)));
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
        assert_eq!(truncated.first().copied(), Some(ymd(2025, Month::Jul, 15)));
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
        assert_eq!(truncated.last().copied(), Some(ymd(2025, Month::Jul, 15)));
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
                prop_assert_eq!(s.first().copied().unwrap(), expected_first);
                prop_assert_eq!(s.last().copied().unwrap(), expected_last);
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

    // ---- Generation::step ---------------------------------------------

    fn lattice(tenor: Period, end_of_month: bool) -> Generation {
        Generation {
            tenor,
            end_of_month,
        }
    }

    #[test]
    fn step_advances_by_fresh_multiples() {
        let anchor = ymd(2025, Month::Jan, 15);
        let quarterly = lattice(Period::Months(3), false);
        assert_eq!(
            quarterly.step(anchor, 1).unwrap(),
            ymd(2025, Month::Apr, 15)
        );
        assert_eq!(
            quarterly.step(anchor, 2).unwrap(),
            ymd(2025, Month::Jul, 15)
        );
        assert_eq!(
            quarterly.step(anchor, 4).unwrap(),
            ymd(2026, Month::Jan, 15)
        );
        let weekly = lattice(Period::Days(7), false);
        let start = ymd(2025, Month::Jan, 1);
        assert_eq!(weekly.step(start, 1).unwrap(), ymd(2025, Month::Jan, 8));
        assert_eq!(weekly.step(start, 3).unwrap(), ymd(2025, Month::Jan, 22));
    }

    #[test]
    fn step_walks_backward_on_negative_indices() {
        let anchor = ymd(2025, Month::Apr, 15);
        let monthly = lattice(Period::Months(1), false);
        assert_eq!(monthly.step(anchor, -1).unwrap(), ymd(2025, Month::Mar, 15));
        assert_eq!(monthly.step(anchor, -3).unwrap(), ymd(2025, Month::Jan, 15));
    }

    #[test]
    fn step_snaps_to_month_end_only_when_configured() {
        // Feb 28 + 1M is Mar 28 plainly, Mar 31 under end-of-month.
        let anchor = ymd(2025, Month::Feb, 28);
        let snapping = lattice(Period::Months(1), true);
        assert_eq!(snapping.step(anchor, 1).unwrap(), ymd(2025, Month::Mar, 31));
        assert_eq!(snapping.step(anchor, 2).unwrap(), ymd(2025, Month::Apr, 30));
        assert_eq!(
            lattice(Period::Months(1), false).step(anchor, 1).unwrap(),
            ymd(2025, Month::Mar, 28)
        );
        // Inert when the anchor is not month-end.
        let mid = ymd(2025, Month::Jan, 15);
        assert_eq!(snapping.step(mid, 1).unwrap(), ymd(2025, Month::Feb, 15));
        // Inert for day-based tenors, where the concept does not apply.
        assert_eq!(
            lattice(Period::Days(1), true).step(anchor, 1).unwrap(),
            ymd(2025, Month::Mar, 1)
        );
    }

    #[test]
    fn step_refuses_to_leave_the_supported_range() {
        assert_eq!(
            lattice(Period::Years(1), false).step(Date::MAX, 1),
            Err(TimeError::DateOutOfRange),
        );
    }

    // ---- Walk ---------------------------------------------------------

    #[test]
    fn walk_forward_emits_interior_and_halts_before_stop() {
        let seed = ymd(2025, Month::Jan, 15);
        let monthly = lattice(Period::Months(1), false);
        let stop = ymd(2025, Month::Apr, 15);
        let dates: Vec<Date> = Walk::new(monthly, seed, stop, Direction::Forward)
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            dates,
            vec![ymd(2025, Month::Feb, 15), ymd(2025, Month::Mar, 15)],
        );
    }

    #[test]
    fn walk_backward_emits_interior_and_halts_after_stop() {
        let seed = ymd(2025, Month::Apr, 15);
        let monthly = lattice(Period::Months(1), false);
        let stop = ymd(2025, Month::Jan, 15);
        let dates: Vec<Date> = Walk::new(monthly, seed, stop, Direction::Backward)
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
        let seed = ymd(2025, Month::Jan, 15);
        let monthly = lattice(Period::Months(1), false);
        let stop = ymd(2025, Month::Feb, 15);
        let dates: Vec<Date> = Walk::new(monthly, seed, stop, Direction::Forward)
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
        let (dates, _) = Generator::forward(&b).generate().unwrap();
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
        let (dates, _) = Generator::backward(&b).generate().unwrap();
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

    // ---- reference dates ----------------------------------------------

    fn unadjusted(effective: Date, termination: Date, rule: DateGenerationRule) -> Schedule {
        ScheduleBuilder::new(effective, termination, Period::Months(6), WEEKENDS)
            .with_rule(rule)
            .with_convention(BusinessDayConvention::Unadjusted)
            .with_termination_convention(BusinessDayConvention::Unadjusted)
            .build()
            .unwrap()
    }

    #[test]
    fn regular_schedule_reference_periods_equal_coupon_periods() {
        let s = unadjusted(
            ymd(2025, Month::Jan, 15),
            ymd(2026, Month::Jan, 15),
            DateGenerationRule::Backward,
        );
        let periods: Vec<_> = s.periods().collect();
        let references: Vec<_> = s.reference_periods().collect();
        assert_eq!(periods, references);
    }

    #[test]
    fn front_stub_reference_date_is_one_tenor_before_the_first_coupon() {
        let s = unadjusted(
            ymd(2002, Month::Aug, 15),
            ymd(2004, Month::Jan, 15),
            DateGenerationRule::Backward,
        );
        let references: Vec<_> = s.reference_periods().collect();
        assert_eq!(
            references[0],
            ymd(2002, Month::Jul, 15)..ymd(2003, Month::Jan, 15)
        );
        // Every later period is its own reference.
        assert_eq!(references[1..], s.periods().skip(1).collect::<Vec<_>>()[..]);
    }

    #[test]
    fn back_stub_reference_date_is_one_tenor_after_the_last_coupon() {
        let s = unadjusted(
            ymd(2003, Month::Jan, 15),
            ymd(2004, Month::Jun, 30),
            DateGenerationRule::Forward,
        );
        let references: Vec<_> = s.reference_periods().collect();
        let last = references.len() - 1;
        assert_eq!(references[last].end, ymd(2004, Month::Jul, 15));
        assert_eq!(references[..last], s.periods().collect::<Vec<_>>()[..last]);
    }

    #[test]
    fn slicing_keeps_both_lists_parallel() {
        let s = unadjusted(
            ymd(2002, Month::Aug, 15),
            ymd(2004, Month::Jan, 15),
            DateGenerationRule::Backward,
        );
        // Slicing off the front drops the front stub: every remaining
        // period is regular.
        let tail = s.after(ymd(2003, Month::Jan, 15));
        assert_eq!(
            tail.periods().collect::<Vec<_>>(),
            tail.reference_periods().collect::<Vec<_>>(),
        );
        // Truncating the tail keeps the front stub.
        let head = s.until(ymd(2003, Month::Jul, 15));
        assert_eq!(
            head.reference_periods().next().unwrap().start,
            ymd(2002, Month::Jul, 15),
        );
    }

    #[test]
    fn construction_rejects_degenerate_stub_references() {
        // A front stub boundary must sit before the coupon it anchors.
        assert_eq!(
            Schedule::new(
                alloc::vec![ymd(2025, Month::Jan, 15), ymd(2025, Month::Jul, 15)],
                StubReferences {
                    front: Some(ymd(2025, Month::Aug, 1)),
                    back: None,
                },
                None,
            ),
            Err(TimeError::InvalidReferencePeriod),
        );
        // A back stub boundary must sit after its coupon.
        assert_eq!(
            Schedule::new(
                alloc::vec![ymd(2025, Month::Jan, 15), ymd(2025, Month::Jul, 15)],
                StubReferences {
                    front: None,
                    back: Some(ymd(2025, Month::Jan, 1)),
                },
                None,
            ),
            Err(TimeError::InvalidReferencePeriod),
        );
        // A boundary outside its coupon is exactly what a stub needs.
        let s = Schedule::new(
            alloc::vec![ymd(2025, Month::Jan, 15), ymd(2025, Month::Jul, 15)],
            StubReferences {
                front: Some(ymd(2025, Month::Jan, 1)),
                back: None,
            },
            None,
        )
        .unwrap();
        assert_eq!(
            s.reference_periods().next().unwrap(),
            ymd(2025, Month::Jan, 1)..ymd(2025, Month::Jul, 15),
        );
    }

    #[test]
    fn a_bare_date_list_is_all_regular_and_names_no_lattice() {
        let s = Schedule::try_from(alloc::vec![
            ymd(2025, Month::Jan, 15),
            ymd(2025, Month::Jul, 15),
            ymd(2026, Month::Jan, 15),
        ])
        .unwrap();
        assert!(s.generation().is_none());
        assert_eq!(
            s.periods().collect::<Vec<_>>(),
            s.reference_periods().collect::<Vec<_>>(),
        );
    }

    #[test]
    fn try_from_rejects_decreasing() {
        let r = Schedule::try_from(vec![ymd(2025, Month::Feb, 15), ymd(2025, Month::Jan, 15)]);
        assert_eq!(r.unwrap_err(), TimeError::ScheduleNotMonotonic);
    }
}
