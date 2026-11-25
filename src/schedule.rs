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

use crate::{
    AccrualPeriod, BusinessDayConvention, Calendar, Coupon, Date, Frequency, PaymentDate, Period,
    TimeError,
};

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

impl core::str::FromStr for DateGenerationRule {
    type Err = TimeError;

    /// Parse `"Forward"`, `"Backward"` or `"Zero"`, ignoring case and
    /// punctuation.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        crate::names::lookup(
            s,
            &[
                (Self::Forward, &["Forward"]),
                (Self::Backward, &["Backward"]),
                (Self::Zero, &["Zero"]),
            ],
        )
    }
}

/// A coupon schedule: its accrual dates and a payment date for each
/// period.
///
/// A `Schedule` derefs to its accrual dates. [`periods`](Self::periods)
/// yields [`AccrualPeriod`]s, [`coupons`](Self::coupons) pairs each with
/// its [`PaymentDate`], and [`accrual_period`](Self::accrual_period)
/// finds the period running on a settlement date. It owns its dates and
/// is independent of the [`Calendar`]s that built it; serde round-trips
/// it through the same checks as construction, so a deserialized one is valid.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "ScheduleData"))]
pub struct Schedule {
    dates: Vec<Date>,
    payments: Vec<PaymentDate>,
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
    /// The lattice of a `frequency` coupon grid, month-end snapping.
    pub(crate) fn of(frequency: Frequency) -> Self {
        Self {
            tenor: Period::from(frequency),
            end_of_month: true,
        }
    }

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
    /// coupon dates, say — as regular periods, each paid on
    /// its end date. A bare list names no coupon frequency, so ACT/ACT
    /// ICMA needs [`TryFrom<(Vec<Date>, Frequency)>`] instead.
    fn try_from(dates: Vec<Date>) -> Result<Self, Self::Error> {
        Self::from_parts(dates, None)
    }
}

impl TryFrom<(Vec<Date>, Frequency)> for Schedule {
    type Error = TimeError;

    /// As [`TryFrom<Vec<Date>>`], on a `frequency` coupon grid.
    fn try_from((dates, frequency): (Vec<Date>, Frequency)) -> Result<Self, Self::Error> {
        Self::from_parts(dates, Some(Generation::of(frequency)))
    }
}

impl From<Schedule> for Vec<Date> {
    /// Unwrap the accrual dates.
    fn from(s: Schedule) -> Self {
        s.dates
    }
}

/// Where a schedule departs from its regular lattice: the quasi-coupon
/// boundary of each end period that is a stub, adjusted like the coupon
/// date it stands in for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct StubReferences {
    front: Option<Date>,
    back: Option<Date>,
}

/// A [`Schedule`] as deserialized, before [`Schedule::new`] checks it.
#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
struct ScheduleData {
    dates: Vec<Date>,
    payments: Vec<PaymentDate>,
    stubs: StubReferences,
    generation: Option<Generation>,
}

#[cfg(feature = "serde")]
impl TryFrom<ScheduleData> for Schedule {
    type Error = TimeError;

    fn try_from(d: ScheduleData) -> Result<Self, Self::Error> {
        Self::new(d.dates, d.payments, d.stubs, d.generation)
    }
}

impl Schedule {
    /// The one constructor. Checks that there is a payment per period,
    /// that accrual dates strictly increase, and that a stub boundary
    /// lies outside the coupon it anchors, so every reference period is
    /// non-degenerate.
    fn new(
        dates: Vec<Date>,
        payments: Vec<PaymentDate>,
        stubs: StubReferences,
        generation: Option<Generation>,
    ) -> Result<Self, TimeError> {
        let aligned = payments.len() == dates.len().saturating_sub(1);
        if !aligned || dates.windows(2).any(|w| w[0] >= w[1]) {
            return Err(TimeError::ScheduleNotMonotonic);
        }
        let front_ok = stubs
            .front
            .is_none_or(|r| dates.get(1).is_some_and(|d| r < *d));
        let back_ok = stubs.back.is_none_or(|r| {
            let penultimate = dates.len().checked_sub(2);
            penultimate
                .and_then(|i| dates.get(i))
                .is_some_and(|d| r > *d)
        });
        if !front_ok || !back_ok {
            return Err(TimeError::InvalidReferencePeriod);
        }
        Ok(Self {
            dates,
            payments,
            stubs,
            generation,
        })
    }

    /// A date list as regular periods paid on their ends.
    fn from_parts(dates: Vec<Date>, generation: Option<Generation>) -> Result<Self, TimeError> {
        let payments = dates.iter().skip(1).map(|&d| PaymentDate::new(d)).collect();
        Self::new(dates, payments, StubReferences::default(), generation)
    }

    /// The accrual dates.
    #[must_use]
    pub fn dates(&self) -> &[Date] {
        &self.dates
    }

    /// The payment dates, one per period.
    #[must_use]
    pub fn payment_dates(&self) -> &[PaymentDate] {
        &self.payments
    }

    /// The lattice this schedule was generated on, absent for one built
    /// from a bare date list — mirroring `QuantLib`'s optional `tenor_`.
    #[must_use]
    pub const fn generation(&self) -> Option<Generation> {
        self.generation
    }

    /// The period between accrual dates `i` and `i + 1`, for
    /// `i < self.payments.len()`.
    fn period(&self, i: usize) -> AccrualPeriod {
        let (start, end) = (self.dates[i], self.dates[i + 1]);
        let ref_start = self.stubs.front.filter(|_| i == 0).unwrap_or(start);
        let ref_end = self
            .stubs
            .back
            .filter(|_| i + 1 == self.payments.len())
            .unwrap_or(end);
        AccrualPeriod::new(start..end, ref_start..ref_end, self.generation)
    }

    /// The accrual periods, in order.
    pub fn periods(&self) -> impl ExactSizeIterator<Item = AccrualPeriod> + '_ {
        (0..self.payments.len()).map(|i| self.period(i))
    }

    /// Each period with its payment date.
    pub fn coupons(&self) -> impl ExactSizeIterator<Item = Coupon> + '_ {
        self.periods()
            .zip(&self.payments)
            .map(|(accrual, &payment)| Coupon { accrual, payment })
    }

    /// The period whose interest is running on `date`:
    /// `start <= date < end`. On a coupon date that is the period
    /// starting there, so accrued interest to it is zero. `None` before
    /// the first accrual date and from the last one on.
    ///
    /// ```
    /// use fasti::{Date, Month, Period, ScheduleBuilder, calendars};
    /// let s = ScheduleBuilder::new(
    ///     Date::from_ymd(2025, Month::Jan, 15)?,
    ///     Date::from_ymd(2026, Month::Jan, 15)?,
    ///     Period::Months(6),
    ///     calendars::WEEKENDS_ONLY,
    /// )
    /// .build()?;
    /// let coupon_date = Date::from_ymd(2025, Month::Jul, 15)?;
    /// assert_eq!(s.accrual_period(coupon_date).map(|p| p.start()), Some(coupon_date));
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
    #[must_use]
    pub fn accrual_period(&self, date: Date) -> Option<AccrualPeriod> {
        let i = self.dates.partition_point(|d| *d <= date).checked_sub(1)?;
        (i < self.payments.len()).then(|| self.period(i))
    }

    /// The largest accrual date strictly before `ref_date`.
    #[must_use]
    pub fn previous_date(&self, ref_date: Date) -> Option<Date> {
        let idx = self.dates.partition_point(|d| *d < ref_date);
        idx.checked_sub(1).map(|i| self.dates[i])
    }

    /// The smallest accrual date strictly after `ref_date`.
    #[must_use]
    pub fn next_date(&self, ref_date: Date) -> Option<Date> {
        let idx = self.dates.partition_point(|d| *d <= ref_date);
        self.dates.get(idx).copied()
    }

    /// The smallest accrual date on or after `ref_date`.
    #[must_use]
    pub fn lower_bound(&self, ref_date: Date) -> Option<Date> {
        let idx = self.dates.partition_point(|d| *d < ref_date);
        self.dates.get(idx).copied()
    }

    /// The schedule from the first accrual date `>= cutoff` on; may be
    /// empty. A dropped front period takes its stub with it.
    #[must_use]
    pub fn after(&self, cutoff: Date) -> Self {
        let idx = self.dates.partition_point(|d| *d < cutoff);
        Self {
            dates: self.dates[idx..].to_vec(),
            payments: self.payments[idx.min(self.payments.len())..].to_vec(),
            stubs: StubReferences {
                front: self.stubs.front.filter(|_| idx == 0),
                ..self.stubs
            },
            generation: self.generation,
        }
    }

    /// The schedule up to the last accrual date `<= cutoff`; may be
    /// empty. A dropped back period takes its stub with it.
    #[must_use]
    pub fn until(&self, cutoff: Date) -> Self {
        let idx = self.dates.partition_point(|d| *d <= cutoff);
        Self {
            dates: self.dates[..idx].to_vec(),
            payments: self.payments[..idx.saturating_sub(1)].to_vec(),
            stubs: StubReferences {
                back: self.stubs.back.filter(|_| idx == self.dates.len()),
                ..self.stubs
            },
            generation: self.generation,
        }
    }
}

impl core::ops::Deref for Schedule {
    type Target = [Date];

    /// The accrual dates, so a `Schedule` indexes, slices, and iterates
    /// like the date list it is.
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
    payment_convention: BusinessDayConvention,
    payment_calendar: Option<Calendar<'cal>>,
    payment_lag: u32,
}

impl<'cal> ScheduleBuilder<'cal> {
    /// Start a builder with the four required inputs. Defaults are bond
    /// conventions, so a bond needs no further calls:
    ///
    /// - `Unadjusted` interior and effective dates, and `Unadjusted`
    ///   termination: a bond accrues on its stated coupon grid, and its
    ///   payments roll separately (see the crate's Treasury example).
    /// - `Backward` generation, anchored on maturity, so any odd period
    ///   is a front stub.
    /// - End-of-month on: a bond maturing on the last day of a month
    ///   pays on the last day of each coupon month (Feb 28 → Aug 31).
    /// - No stub dates.
    /// - Payment on the accrual end rolled `Following`, on the same
    ///   calendar, with no lag.
    ///
    /// Swap legs and other schedules that accrue on business days opt
    /// in with [`with_convention`](Self::with_convention), typically
    /// `ModifiedFollowing`, and [`with_end_of_month`](Self::with_end_of_month)
    /// as their terms say.
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
            convention: BusinessDayConvention::Unadjusted,
            termination_convention: BusinessDayConvention::Unadjusted,
            rule: DateGenerationRule::Backward,
            end_of_month: true,
            first_date: None,
            next_to_last_date: None,
            payment_convention: BusinessDayConvention::Following,
            payment_calendar: None,
            payment_lag: 0,
        }
    }

    /// Override the tenor with the [`Period`] canonical to the
    /// given [`Frequency`]. Convenient when the schedule's recurrence
    /// is more naturally expressed as "quarterly" / "semiannual" than
    /// as "every 3 months" / "every 6 months".
    ///
    /// The tenor passed to [`new`](Self::new) and this method set the
    /// same thing; whichever is called last wins.
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

    /// The convention rolling each accrual end onto its payment date
    /// (default `Following`). Applies when there is no payment lag; a lag
    /// counts business days and so always lands on one.
    #[must_use]
    pub fn with_payment_convention(mut self, convention: BusinessDayConvention) -> Self {
        self.payment_convention = convention;
        self
    }

    /// The calendar payments roll on (default: the schedule's).
    #[must_use]
    pub fn with_payment_calendar(mut self, calendar: Calendar<'cal>) -> Self {
        self.payment_calendar = Some(calendar);
        self
    }

    /// Pay this many business days after the accrual end, counted on the
    /// payment calendar (default 0). From a non-business day the first
    /// step lands on the next business day: one business day after a
    /// Saturday is the Monday.
    #[must_use]
    pub fn with_payment_lag(mut self, business_days: u32) -> Self {
        self.payment_lag = business_days;
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
    /// [`TimeError::ScheduleNotMonotonic`] if adjustment rolls the
    /// effective date onto or past the termination date.
    ///
    /// Dates that adjustment rolls onto one business day are merged, as
    /// in `QuantLib`: a daily `Following` schedule has one Friday-to-Monday
    /// period per weekend.
    ///
    /// ```
    /// use fasti::{BusinessDayConvention, Date, Frequency, Month, Period, ScheduleBuilder, calendars};
    /// let s = ScheduleBuilder::new(
    ///     Date::from_ymd(2025, Month::Jan, 3)?, // Friday
    ///     Date::from_ymd(2025, Month::Jan, 8)?, // Wednesday
    ///     Period::from(Frequency::Daily),
    ///     calendars::WEEKENDS_ONLY,
    /// )
    /// .with_convention(BusinessDayConvention::Following)
    /// .forwards()
    /// .build()?;
    /// // Saturday and Sunday both roll to Monday the 6th.
    /// let days: Vec<u8> = s.iter().map(|d| d.day()).collect();
    /// assert_eq!(days, [3, 6, 7, 8]);
    /// # Ok::<(), fasti::TimeError>(())
    /// ```
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
        let (dates, stubs) =
            BdcAdjuster::new(self.calendar, self.convention, self.termination_convention)
                .adjust_lattice(dates, stubs)?;
        let pay_calendar = self.payment_calendar.unwrap_or(self.calendar);
        let lag = i32::try_from(self.payment_lag).map_err(|_| TimeError::DateOutOfRange)?;
        // No lag: roll the accrual end under the payment convention. A
        // lag: count business days from the accrual end, which already
        // lands on a business day (QuantLib, Strata and rateslib agree).
        let payments = dates
            .iter()
            .skip(1)
            .map(|&end| {
                let paid = if lag == 0 {
                    pay_calendar.adjust(end, self.payment_convention)?
                } else {
                    pay_calendar.add_business_days(end, lag)?
                };
                Ok(PaymentDate::new(paid))
            })
            .collect::<Result<_, TimeError>>()?;
        let generation = (!matches!(self.rule, DateGenerationRule::Zero)).then(|| self.lattice());
        Schedule::new(dates, payments, stubs, generation)
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

/// Adjusts a generated lattice: the last date and back stub under the
/// terminal convention, every other date and the front stub under the
/// interior one.
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

    /// Adjust dates and stubs, then merge what adjustment collapsed.
    fn adjust_lattice(
        &self,
        dates: Vec<Date>,
        stubs: StubReferences,
    ) -> Result<(Vec<Date>, StubReferences), TimeError> {
        let roll = |d: Option<Date>, c| d.map(|d| self.calendar.adjust(d, c)).transpose();
        let stubs = StubReferences {
            front: roll(stubs.front, self.interior)?,
            back: roll(stubs.back, self.terminal)?,
        };
        Ok(Self::merge_collisions(self.apply(dates)?, stubs))
    }

    fn apply(&self, dates: Vec<Date>) -> Result<Vec<Date>, TimeError> {
        let last = dates.len().saturating_sub(1);
        dates
            .into_iter()
            .enumerate()
            .map(|(i, d)| {
                let convention = if i == last {
                    self.terminal
                } else {
                    self.interior
                };
                self.calendar.adjust(d, convention)
            })
            .collect()
    }

    /// Drop the dates adjustment rolled onto their predecessor or onto or
    /// past the last date, joining the periods around them (`QuantLib`
    /// skips them the same way). Interior dates share one monotone
    /// convention, so `dedup` catches every tie; only the terminal
    /// convention can roll a date strictly past the end.
    ///
    /// A front stub whose period collapsed to nothing goes with it. When
    /// dates past the end are dropped, the first of them is the lattice
    /// step the new last period accrues against. Crossed ends are left
    /// for [`Schedule::new`] to refuse.
    fn merge_collisions(dates: Vec<Date>, stubs: StubReferences) -> (Vec<Date>, StubReferences) {
        let (Some(&first), Some(&last)) = (dates.first(), dates.last()) else {
            return (dates, stubs);
        };
        if first >= last {
            return (dates, stubs);
        }
        let interior = &dates[1..dates.len() - 1];
        let front = stubs.front.filter(|_| interior.first() != Some(&first));
        let back = match interior.iter().find(|&&d| d >= last) {
            Some(&step) => (step != last).then_some(step),
            None => stubs.back,
        };
        let mut kept: Vec<Date> = dates.iter().copied().filter(|&d| d < last).collect();
        kept.dedup();
        kept.push(last);
        (kept, StubReferences { front, back })
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

    /// Accrual periods as ranges.
    fn ranges(s: &Schedule) -> Vec<Range<Date>> {
        s.periods().map(|p| p.range()).collect()
    }

    /// Each period's reference period.
    fn refs(s: &Schedule) -> Vec<Range<Date>> {
        s.periods().map(|p| p.reference()).collect()
    }

    /// A schedule of `dates` with the given stub references.
    fn stubbed(dates: Vec<Date>, stubs: StubReferences) -> Result<Schedule, TimeError> {
        let payments = dates.iter().skip(1).map(|&d| PaymentDate::new(d)).collect();
        Schedule::new(dates, payments, stubs, None)
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
        .with_convention(BusinessDayConvention::ModifiedFollowing)
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
        let pairs: Vec<(Date, Date)> = s.periods().map(|p| (p.start(), p.end())).collect();
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
        assert_eq!(p.reference(), p.range());
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
        assert_eq!(dates, Vec::<Date>::new());
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
        assert_eq!(adjusted, Vec::<Date>::new());
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
        let periods = ranges(&s);
        let references = refs(&s);
        assert_eq!(periods, references);
    }

    #[test]
    fn front_stub_reference_date_is_one_tenor_before_the_first_coupon() {
        let s = unadjusted(
            ymd(2002, Month::Aug, 15),
            ymd(2004, Month::Jan, 15),
            DateGenerationRule::Backward,
        );
        let references = refs(&s);
        assert_eq!(
            references[0],
            ymd(2002, Month::Jul, 15)..ymd(2003, Month::Jan, 15)
        );
        // Every later period is its own reference.
        assert_eq!(references[1..], ranges(&s)[1..]);
    }

    #[test]
    fn back_stub_reference_date_is_one_tenor_after_the_last_coupon() {
        let s = unadjusted(
            ymd(2003, Month::Jan, 15),
            ymd(2004, Month::Jun, 30),
            DateGenerationRule::Forward,
        );
        let references = refs(&s);
        let last = references.len() - 1;
        assert_eq!(references[last].end, ymd(2004, Month::Jul, 15));
        assert_eq!(references[..last], ranges(&s)[..last]);
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
        assert_eq!(ranges(&tail), refs(&tail),);
        // Truncating the tail keeps the front stub.
        let head = s.until(ymd(2003, Month::Jul, 15));
        assert_eq!(
            refs(&head).first().cloned().unwrap().start,
            ymd(2002, Month::Jul, 15),
        );
    }

    #[test]
    fn construction_rejects_degenerate_stub_references() {
        // A front stub boundary must sit before the coupon it anchors.
        assert_eq!(
            stubbed(
                alloc::vec![ymd(2025, Month::Jan, 15), ymd(2025, Month::Jul, 15)],
                StubReferences {
                    front: Some(ymd(2025, Month::Aug, 1)),
                    back: None,
                },
            ),
            Err(TimeError::InvalidReferencePeriod),
        );
        // A back stub boundary must sit after its coupon.
        assert_eq!(
            stubbed(
                alloc::vec![ymd(2025, Month::Jan, 15), ymd(2025, Month::Jul, 15)],
                StubReferences {
                    front: None,
                    back: Some(ymd(2025, Month::Jan, 1)),
                },
            ),
            Err(TimeError::InvalidReferencePeriod),
        );
        // A boundary outside its coupon is exactly what a stub needs.
        let s = stubbed(
            alloc::vec![ymd(2025, Month::Jan, 15), ymd(2025, Month::Jul, 15)],
            StubReferences {
                front: Some(ymd(2025, Month::Jan, 1)),
                back: None,
            },
        )
        .unwrap();
        assert_eq!(
            refs(&s).first().cloned().unwrap(),
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
        assert_eq!(ranges(&s), refs(&s),);
    }

    #[test]
    fn try_from_rejects_decreasing() {
        let r = Schedule::try_from(vec![ymd(2025, Month::Feb, 15), ymd(2025, Month::Jan, 15)]);
        assert_eq!(r.unwrap_err(), TimeError::ScheduleNotMonotonic);
    }

    // ---- Collisions after adjustment -----------------------------------

    const US: Calendar<'static> = crate::calendars::us::SETTLEMENT;

    const ROLLING: [BusinessDayConvention; 4] = [
        BusinessDayConvention::Following,
        BusinessDayConvention::ModifiedFollowing,
        BusinessDayConvention::Preceding,
        BusinessDayConvention::ModifiedPreceding,
    ];

    fn daily(effective: Date, termination: Date) -> ScheduleBuilder<'static> {
        ScheduleBuilder::new(effective, termination, Period::from(Frequency::Daily), US)
    }

    #[test]
    fn merge_collisions_drops_ties_and_dates_past_the_end() {
        let d = |day| ymd(2025, Month::Feb, day);
        let (dates, stubs) = BdcAdjuster::merge_collisions(
            vec![d(3), d(3), d(4), d(4), d(5), d(10), d(10), d(7)],
            StubReferences::default(),
        );
        assert_eq!(dates, vec![d(3), d(4), d(5), d(7)]);
        assert_eq!(stubs.back, Some(d(10)));
        assert_eq!(stubs.front, None);
        let crossed = vec![d(7), d(6), d(5)];
        assert_eq!(
            BdcAdjuster::merge_collisions(crossed.clone(), StubReferences::default()).0,
            crossed
        );
    }

    #[test]
    fn daily_schedule_is_the_business_days_between_its_ends() {
        // New Year's Day, MLK Day, four weekends; a Saturday maturity.
        let (effective, termination) = (ymd(2025, Month::Jan, 1), ymd(2025, Month::Feb, 1));
        for convention in ROLLING {
            for rule in [DateGenerationRule::Forward, DateGenerationRule::Backward] {
                let s = daily(effective, termination)
                    .with_convention(convention)
                    .with_rule(rule)
                    .build()
                    .unwrap();
                let first = US.adjust(effective, convention).unwrap();
                let mut expected = vec![first];
                expected.extend(US.business_days(first.add_days(1).unwrap()..termination));
                expected.push(termination);
                assert_eq!(s.dates(), &expected[..], "{convention:?} {rule:?}");
            }
        }
    }

    #[test]
    fn weekend_collapses_into_one_friday_to_monday_period() {
        let s = daily(ymd(2025, Month::Jan, 9), ymd(2025, Month::Jan, 15))
            .with_convention(BusinessDayConvention::Following)
            .build()
            .unwrap();
        let periods = ranges(&s);
        assert!(periods.contains(&(ymd(2025, Month::Jan, 10)..ymd(2025, Month::Jan, 13))));
        assert_eq!(periods.len(), 4);
    }

    #[test]
    fn penultimate_date_rolled_past_an_unadjusted_maturity_is_dropped() {
        // Saturday rolls to Monday, past the unadjusted Sunday maturity.
        let s = daily(ymd(2025, Month::Jan, 27), ymd(2025, Month::Feb, 2))
            .with_convention(BusinessDayConvention::Following)
            .with_termination_convention(BusinessDayConvention::Unadjusted)
            .forwards()
            .build()
            .unwrap();
        assert_eq!(s.dates().last(), Some(&ymd(2025, Month::Feb, 2)));
        assert_eq!(
            ranges(&s).last().cloned(),
            Some(ymd(2025, Month::Jan, 31)..ymd(2025, Month::Feb, 2)),
        );
        // It accrues against the step it absorbed: Friday to the dropped
        // Saturday, rolled to Monday.
        assert_eq!(
            refs(&s).last().cloned(),
            Some(ymd(2025, Month::Jan, 31)..ymd(2025, Month::Feb, 3)),
        );
    }

    #[test]
    fn penultimate_date_tying_with_maturity_leaves_a_regular_last_period() {
        // Saturday and the Sunday maturity both roll to Monday.
        let s = daily(ymd(2025, Month::Jan, 27), ymd(2025, Month::Feb, 2))
            .with_convention(BusinessDayConvention::Following)
            .with_termination_convention(BusinessDayConvention::Following)
            .forwards()
            .build()
            .unwrap();
        assert_eq!(
            ranges(&s).last().cloned(),
            Some(ymd(2025, Month::Jan, 31)..ymd(2025, Month::Feb, 3)),
        );
        assert_eq!(refs(&s).last(), ranges(&s).last());
    }

    #[test]
    fn front_stub_that_adjusts_onto_the_effective_date_vanishes() {
        // Saturday effective and Sunday stub date both roll to Monday.
        let s = ScheduleBuilder::new(
            ymd(2025, Month::Jan, 4),
            ymd(2025, Month::Feb, 2),
            Period::Weeks(1),
            US,
        )
        .with_convention(BusinessDayConvention::Following)
        .with_first_date(ymd(2025, Month::Jan, 5))
        .forwards()
        .build()
        .unwrap();
        assert_eq!(
            s.dates()[..2],
            [ymd(2025, Month::Jan, 6), ymd(2025, Month::Jan, 13)]
        );
        assert_eq!(refs(&s).first(), ranges(&s).first());
    }

    #[test]
    fn ends_that_meet_after_adjustment_are_still_refused() {
        // Saturday effective rolls past the unadjusted Sunday maturity.
        let r = daily(ymd(2025, Month::Jan, 4), ymd(2025, Month::Jan, 5))
            .with_convention(BusinessDayConvention::Following)
            .with_termination_convention(BusinessDayConvention::Unadjusted)
            .build();
        assert_eq!(r.unwrap_err(), TimeError::ScheduleNotMonotonic);
    }

    #[test]
    fn merged_daily_schedule_accrues_under_icma() {
        use crate::{AccrualDayCount, ActActICMA};
        let s = daily(ymd(2025, Month::Jan, 1), ymd(2025, Month::Feb, 1))
            .with_convention(BusinessDayConvention::Following)
            .build()
            .unwrap();
        for p in s.periods() {
            assert!(
                ActActICMA.accrual_fraction(&p).unwrap().parts().0 > 0,
                "{p:?}"
            );
        }
    }

    // ---- Payment dates and coupons ------------------------------------

    fn treasury() -> ScheduleBuilder<'static> {
        ScheduleBuilder::new(
            ymd(2025, Month::Jan, 15),
            ymd(2030, Month::Jan, 15),
            Period::Months(6),
            crate::calendars::us::GOVERNMENT_BOND,
        )
    }

    #[test]
    fn payments_roll_separately_from_accrual() {
        let s = treasury().build().unwrap();
        // Accrual stays on the 15th; Saturday Jan 15 2028 (MLK Monday
        // behind it) pays Tuesday the 18th.
        let c = s.coupons().nth(5).unwrap();
        assert_eq!(
            c.accrual.range(),
            ymd(2027, Month::Jul, 15)..ymd(2028, Month::Jan, 15)
        );
        assert_eq!(c.payment.date(), ymd(2028, Month::Jan, 18));
        assert_eq!(s.payment_dates().len(), s.periods().len());
        assert_eq!(s.coupons().count(), 10);
    }

    #[test]
    fn payment_lag_counts_business_days_from_the_accrual_end() {
        let lagged = treasury()
            .with_payment_lag(2)
            .with_payment_calendar(crate::calendars::TARGET)
            .build()
            .unwrap();
        // The lag counts from the accrual end, Saturday Jan 15 2028:
        // Monday 17th is one TARGET business day, Tuesday 18th two.
        assert_eq!(lagged.payment_dates()[5].date(), ymd(2028, Month::Jan, 18));
        let preceding = treasury()
            .with_payment_convention(BusinessDayConvention::Preceding)
            .build()
            .unwrap();
        assert_eq!(
            preceding.payment_dates()[5].date(),
            ymd(2028, Month::Jan, 14)
        );
        // Accrual is untouched by any of it.
        assert_eq!(lagged.dates(), treasury().build().unwrap().dates());
    }

    #[test]
    fn accrual_period_is_half_open_so_coupon_dates_accrue_nothing() {
        let s = treasury().build().unwrap();
        let coupon_date = ymd(2026, Month::Jul, 15);
        let p = s.accrual_period(coupon_date).unwrap();
        assert_eq!(p.start(), coupon_date);
        assert_eq!(
            p.until(coupon_date).unwrap().range(),
            coupon_date..coupon_date
        );
        assert_eq!(
            s.accrual_period(ymd(2026, Month::Jul, 14)).unwrap().end(),
            coupon_date
        );
        assert!(s.accrual_period(ymd(2025, Month::Jan, 14)).is_none());
        assert!(s.accrual_period(ymd(2030, Month::Jan, 15)).is_none());
        assert!(p.until(ymd(2026, Month::Jul, 14)).is_none());
    }

    #[test]
    fn slicing_keeps_payments_with_their_periods() {
        let s = treasury().build().unwrap();
        let tail = s.after(ymd(2027, Month::Jul, 15));
        assert_eq!(tail.payment_dates()[0].date(), ymd(2028, Month::Jan, 18));
        let head = s.until(ymd(2028, Month::Jan, 15));
        assert_eq!(
            head.payment_dates().last().map(|p| p.date()),
            Some(ymd(2028, Month::Jan, 18))
        );
        for part in [
            tail,
            head,
            s.after(ymd(2031, Month::Jan, 1)),
            s.until(ymd(2020, Month::Jan, 1)),
        ] {
            assert_eq!(
                part.payment_dates().len(),
                part.dates().len().saturating_sub(1)
            );
        }
    }

    #[test]
    fn construction_rejects_misaligned_lists() {
        let d = alloc::vec![ymd(2025, Month::Jan, 15), ymd(2025, Month::Jul, 15)];
        let pay = alloc::vec![PaymentDate::new(ymd(2025, Month::Jul, 15))];
        let none = StubReferences::default();
        assert!(Schedule::new(d.clone(), pay.clone(), none, None).is_ok());
        assert!(Schedule::new(d.clone(), Vec::new(), none, None).is_err());
        assert!(Schedule::new(d[..1].to_vec(), pay, none, None).is_err());
    }

    proptest! {
        /// Short tenors build whenever the ends survive adjustment, and
        /// merging keeps every schedule invariant.
        #[test]
        fn short_tenors_build_and_stay_valid(
            x in 1u32..(Date::MAX.serial() - 1_000),
            span_days in 1u32..=120,
            weeks in any::<bool>(),
            convention_idx in 0usize..4,
            terminal_idx in 0usize..5,
            forward in any::<bool>(),
        ) {
            let effective = Date::from_serial(x).unwrap();
            let termination = Date::from_serial(x + span_days).unwrap();
            let tenor = if weeks { Period::Weeks(1) } else { Period::Days(1) };
            let convention = ROLLING[convention_idx];
            let terminal = ROLLING.get(terminal_idx).copied()
                .unwrap_or(BusinessDayConvention::Unadjusted);
            let rule = if forward { DateGenerationRule::Forward } else { DateGenerationRule::Backward };
            let result = ScheduleBuilder::new(effective, termination, tenor, US)
                .with_convention(convention)
                .with_termination_convention(terminal)
                .with_rule(rule)
                .build();
            let first = US.adjust(effective, convention).unwrap();
            let last = US.adjust(termination, terminal).unwrap();
            if first >= last {
                prop_assert_eq!(result.unwrap_err(), TimeError::ScheduleNotMonotonic);
            } else {
                let s = result.unwrap();
                prop_assert_eq!(s.dates().first(), Some(&first));
                prop_assert_eq!(s.dates().last(), Some(&last));
                for w in s.dates().windows(2) {
                    prop_assert!(w[0] < w[1]);
                }
                for p in s.periods() {
                    prop_assert!(p.reference().start < p.reference().end);
                }
                prop_assert_eq!(s.payment_dates().len() + 1, s.dates().len());
                if !weeks {
                    // Nothing a daily schedule could land on is lost.
                    for b in US.business_days(first..last) {
                        prop_assert!(s.dates().binary_search(&b).is_ok(), "{b} missing");
                    }
                }
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod bond_default_tests {
    use super::*;
    use crate::Month;

    /// With no options set, a builder lays out a bond: unadjusted
    /// coupon dates on the maturity's grid, snapped to month end when
    /// maturity is one.
    #[test]
    fn defaults_are_bond_conventions() {
        let ymd = |y, m, d| Date::from_ymd(y, m, d).unwrap();
        // Maturity Thursday Feb 28 2030, a month end: coupons on the
        // last day of Aug and Feb, unadjusted even when that is a
        // weekend (Sat Aug 31 2024, Sun Aug 31 2025).
        let s = ScheduleBuilder::new(
            ymd(2024, Month::Feb, 29),
            ymd(2026, Month::Feb, 28),
            Period::Months(6),
            crate::calendars::us::GOVERNMENT_BOND,
        )
        .build()
        .unwrap();
        assert_eq!(
            s.dates(),
            &[
                ymd(2024, Month::Feb, 29),
                ymd(2024, Month::Aug, 31),
                ymd(2025, Month::Feb, 28),
                ymd(2025, Month::Aug, 31),
                ymd(2026, Month::Feb, 28),
            ],
        );
    }
}
