//! [`Schedule`] — the coupon dates, and the regular grid they accrue
//! against.

use fasti::{AccrualPeriod, Date, DateGenerationRule, Schedule, ScheduleBuilder};
use pyo3::IntoPyObjectExt;
use pyo3::prelude::*;
use pyo3::types::{PyIterator, PyList, PySlice, PyTuple, PyTupleMethods};

use crate::calendar::{CalendarArg, CalendarSpec};
use crate::convert::{
    DateArg, FastiError, OrRaise, Reduction, ReplayReduction, date_out, dates_out, hook, period_out,
};
use crate::period::{PeriodArg, PyPeriod};
use crate::rule::py_date_repr;
use crate::vocab::{
    ConventionArg, FrequencyArg, GenerationRuleArg, PyBusinessDayConvention, PyDateGenerationRule,
    PyFrequency,
};

/// What a schedule was generated from. A generated schedule cannot be
/// recovered from its dates alone — the stub reference grid is not in
/// them — so pickling replays the generation instead.
#[derive(Clone, PartialEq, Eq, Hash)]
enum Origin {
    Dates(Vec<Date>, Option<PyFrequency>),
    Generated(Box<Generated>),
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct Generated {
    effective: Date,
    termination: Date,
    tenor: PyPeriod,
    calendar: CalendarSpec,
    convention: PyBusinessDayConvention,
    termination_convention: PyBusinessDayConvention,
    rule: PyDateGenerationRule,
    end_of_month: bool,
    first_date: Option<Date>,
    next_to_last_date: Option<Date>,
    payment_convention: PyBusinessDayConvention,
    payment_calendar: Option<CalendarSpec>,
    payment_lag: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Op {
    After(Date),
    Until(Date),
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct ScheduleSpec {
    origin: Origin,
    ops: Vec<Op>,
}

impl ScheduleSpec {
    fn build(&self) -> PyResult<Schedule> {
        let mut schedule = match &self.origin {
            Origin::Dates(dates, None) => Schedule::try_from(dates.clone()).or_raise()?,
            Origin::Dates(dates, Some(frequency)) => {
                Schedule::try_from((dates.clone(), frequency.0)).or_raise()?
            }
            Origin::Generated(g) => {
                let calendar = g.calendar.build()?;
                let payment_calendar = g
                    .payment_calendar
                    .as_ref()
                    .map(CalendarSpec::build)
                    .transpose()?;
                let mut builder =
                    ScheduleBuilder::new(g.effective, g.termination, g.tenor.0, calendar.view())
                        .with_convention(g.convention.0)
                        .with_termination_convention(g.termination_convention.0)
                        .with_rule(g.rule.0)
                        .with_end_of_month(g.end_of_month)
                        .with_payment_convention(g.payment_convention.0)
                        .with_payment_lag(g.payment_lag);
                if let Some(payment_calendar) = &payment_calendar {
                    builder = builder.with_payment_calendar(payment_calendar.view());
                }
                if let Some(date) = g.first_date {
                    builder = builder.with_first_date(date);
                }
                if let Some(date) = g.next_to_last_date {
                    builder = builder.with_next_to_last_date(date);
                }
                builder.build().or_raise()?
            }
        };
        for op in &self.ops {
            schedule = match op {
                Op::After(cutoff) => schedule.after(*cutoff),
                Op::Until(cutoff) => schedule.until(*cutoff),
            };
        }
        Ok(schedule)
    }
}

/// The parameters a generated schedule was built on, kept so that
/// schedule-defined day counts can extend its grid.
///
/// >>> import datetime
/// >>> from fasti import Schedule
/// >>> from fasti.calendars import WEEKENDS_ONLY
/// >>> s = Schedule(datetime.date(2025, 1, 15), datetime.date(2026, 1, 15),
/// ...              "semiannual", WEEKENDS_ONLY)
/// >>> str(s.generation().tenor)
/// '6M'
#[pyclass(
    frozen,
    eq,
    hash,
    skip_from_py_object,
    module = "fasti",
    name = "Generation"
)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PyGeneration(fasti::Generation);

// `Generation` does not derive `Hash` in the crate; its two fields do.
impl core::hash::Hash for PyGeneration {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        core::hash::Hash::hash(&self.0.tenor, state);
        core::hash::Hash::hash(&self.0.end_of_month, state);
    }
}

#[pymethods]
impl PyGeneration {
    /// The lattice a schedule was generated on.
    #[new]
    fn new(tenor: PeriodArg, end_of_month: bool) -> Self {
        Self(fasti::Generation {
            tenor: tenor.0,
            end_of_month,
        })
    }

    /// The regular period between coupons.
    #[getter]
    fn tenor(&self) -> PyPeriod {
        PyPeriod(self.0.tenor)
    }

    /// Whether generation snapped dates to the end of their month.
    #[getter]
    fn end_of_month(&self) -> bool {
        self.0.end_of_month
    }

    pub fn __repr__(&self) -> String {
        format!(
            "Generation(tenor={}, end_of_month={})",
            PyPeriod(self.0.tenor).__repr__(),
            if self.0.end_of_month { "True" } else { "False" },
        )
    }

    fn __reduce__<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Reduction<'py, (&'static str, i32, bool)>> {
        let tenor = PyPeriod(self.0.tenor);
        Ok((
            hook(py, "_rebuild_generation")?,
            (tenor.unit(), self.0.tenor.length(), self.0.end_of_month),
        ))
    }
}

/// Coupon dates in chronological order, and the reference periods they
/// accrue against.
///
/// The two differ only at a stub, where the reference period is the
/// notional quasi-coupon period one tenor from the adjacent coupon —
/// which is what ACT/ACT ICMA accrues against.
///
/// A schedule indexes and iterates as its coupon dates.
///
/// Generating one takes the effective and termination dates, a tenor
/// and a calendar. The rest are keyword arguments whose defaults are the
/// crate's bond conventions: unadjusted accrual dates, `Backward`
/// generation, end of month on, no stub anchors, and payment on the
/// accrual end rolled `Following` on the same calendar with no lag.
///
/// >>> import datetime
/// >>> from fasti import Schedule
/// >>> from fasti.calendars import us
/// >>> s = Schedule(datetime.date(2025, 1, 31), datetime.date(2026, 1, 31),
/// ...              "quarterly", us.GOVERNMENT_BOND)
/// >>> len(s), s[0], s[-1]
/// (5, datetime.date(2025, 1, 31), datetime.date(2026, 1, 31))
#[pyclass(
    frozen,
    eq,
    hash,
    skip_from_py_object,
    module = "fasti",
    name = "Schedule"
)]
pub struct PySchedule {
    spec: ScheduleSpec,
    schedule: Schedule,
}

/// Two schedules are equal when their dates, stubs and generation are:
/// unlike a calendar, a schedule is its own value.
impl PartialEq for PySchedule {
    fn eq(&self, other: &Self) -> bool {
        self.schedule == other.schedule
    }
}

impl Eq for PySchedule {}

impl core::hash::Hash for PySchedule {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.schedule.dates().hash(state);
    }
}

impl Clone for PySchedule {
    fn clone(&self) -> Self {
        Self {
            spec: self.spec.clone(),
            schedule: self.schedule.clone(),
        }
    }
}

impl PySchedule {
    fn from_spec(spec: ScheduleSpec) -> PyResult<Self> {
        Ok(Self {
            schedule: spec.build()?,
            spec,
        })
    }

    /// The schedule itself, for the day counts that bind to one.
    pub fn inner(&self) -> &Schedule {
        &self.schedule
    }

    fn sliced(&self, op: Op) -> PyResult<Self> {
        let mut spec = self.spec.clone();
        spec.ops.push(op);
        Self::from_spec(spec)
    }
}

#[pymethods]
impl PySchedule {
    /// Generate a schedule between two dates on a tenor. PyO3 drops a
    /// constructor's doc comment, so this is documented on the class.
    #[new]
    #[pyo3(signature = (
        effective, termination, tenor, calendar, *,
        convention=None, termination_convention=None, rule=None,
        end_of_month=true, first_date=None, next_to_last_date=None,
        payment_convention=None, payment_calendar=None, payment_lag=0,
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        effective: DateArg,
        termination: DateArg,
        tenor: PeriodArg,
        calendar: CalendarArg,
        convention: Option<ConventionArg>,
        termination_convention: Option<ConventionArg>,
        rule: Option<GenerationRuleArg>,
        end_of_month: bool,
        first_date: Option<DateArg>,
        next_to_last_date: Option<DateArg>,
        payment_convention: Option<ConventionArg>,
        payment_calendar: Option<CalendarArg>,
        payment_lag: u32,
    ) -> PyResult<Self> {
        Self::from_spec(ScheduleSpec {
            origin: Origin::Generated(Box::new(Generated {
                effective: effective.0,
                termination: termination.0,
                tenor: PyPeriod(tenor.0),
                calendar: calendar.0,
                convention: convention.map_or(
                    PyBusinessDayConvention(fasti::BusinessDayConvention::Unadjusted),
                    |c| c.0,
                ),
                termination_convention: termination_convention.map_or(
                    PyBusinessDayConvention(fasti::BusinessDayConvention::Unadjusted),
                    |c| c.0,
                ),
                rule: rule.map_or(PyDateGenerationRule(DateGenerationRule::Backward), |r| r.0),
                end_of_month,
                first_date: first_date.map(|d| d.0),
                next_to_last_date: next_to_last_date.map(|d| d.0),
                payment_convention: payment_convention.map_or(
                    PyBusinessDayConvention(fasti::BusinessDayConvention::Following),
                    |c| c.0,
                ),
                payment_calendar: payment_calendar.map(|c| c.0),
                payment_lag,
            })),
            ops: Vec::new(),
        })
    }

    /// A schedule from a date list — a term sheet's dates, taken as
    /// given, each period paid on its end. Pass the coupon `frequency`
    /// for ACT/ACT ICMA to be able to measure it.
    #[staticmethod]
    #[pyo3(signature = (dates, frequency=None))]
    fn from_dates(dates: Vec<DateArg>, frequency: Option<FrequencyArg>) -> PyResult<Self> {
        Self::from_spec(ScheduleSpec {
            origin: Origin::Dates(
                dates.into_iter().map(|d| d.0).collect(),
                frequency.map(|f| f.0),
            ),
            ops: Vec::new(),
        })
    }

    /// The accrual dates.
    fn dates<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        dates_out(py, self.schedule.dates())
    }

    /// The payment dates, one per period.
    fn payment_dates<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let dates: Vec<Date> = self
            .schedule
            .payment_dates()
            .iter()
            .map(|p| p.date())
            .collect();
        dates_out(py, &dates)
    }

    /// The accrual periods.
    fn periods(&self) -> Vec<PyAccrualPeriod> {
        self.schedule.periods().map(PyAccrualPeriod).collect()
    }

    /// Each accrual period with its payment date.
    fn coupons(&self) -> Vec<PyCoupon> {
        self.schedule
            .coupons()
            .map(|c| PyCoupon {
                accrual: PyAccrualPeriod(c.accrual),
                payment: c.payment.date(),
            })
            .collect()
    }

    /// The period whose interest is running on `date` (start included,
    /// end excluded), or None outside the schedule. On a coupon date it
    /// is the period starting there, so accrued interest is zero.
    ///
    /// >>> import datetime
    /// >>> from fasti import DayCount, Schedule
    /// >>> from fasti.calendars import us
    /// >>> s = Schedule(datetime.date(2025, 1, 15), datetime.date(2030, 1, 15),
    /// ...              "semiannual", us.GOVERNMENT_BOND)
    /// >>> settle = datetime.date(2026, 11, 18)
    /// >>> DayCount.act_act_icma().accrual_fraction(s.accrual_period(settle).until(settle))
    /// Fraction(63, 184)
    fn accrual_period(&self, date: DateArg) -> Option<PyAccrualPeriod> {
        self.schedule.accrual_period(date.0).map(PyAccrualPeriod)
    }

    /// The lattice this schedule was generated on, or None when it was
    /// built from a bare date list.
    fn generation(&self) -> Option<PyGeneration> {
        self.schedule.generation().map(PyGeneration)
    }

    /// The largest schedule date strictly before `ref_date`, if any.
    fn previous_date<'py>(
        &self,
        py: Python<'py>,
        ref_date: DateArg,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.schedule
            .previous_date(ref_date.0)
            .map(|d| date_out(py, d))
            .transpose()
    }

    /// The smallest schedule date strictly after `ref_date`, if any.
    fn next_date<'py>(
        &self,
        py: Python<'py>,
        ref_date: DateArg,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.schedule
            .next_date(ref_date.0)
            .map(|d| date_out(py, d))
            .transpose()
    }

    /// The smallest schedule date on or after `ref_date`, if any —
    /// unlike `next_date`, an exact match counts.
    fn lower_bound<'py>(
        &self,
        py: Python<'py>,
        ref_date: DateArg,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.schedule
            .lower_bound(ref_date.0)
            .map(|d| date_out(py, d))
            .transpose()
    }

    /// The schedule restricted to dates on or after `cutoff`.
    fn after(&self, cutoff: DateArg) -> PyResult<Self> {
        self.sliced(Op::After(cutoff.0))
    }

    /// The schedule restricted to dates on or before `cutoff`.
    fn until(&self, cutoff: DateArg) -> PyResult<Self> {
        self.sliced(Op::Until(cutoff.0))
    }

    /// Iterate the coupon dates, as `&Schedule` does in the crate.
    fn __iter__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyIterator>> {
        PyList::new(py, self.dates(py)?)?.into_any().try_iter()
    }

    /// Iterate the coupon dates backwards. Without the sequence slots —
    /// which a slice cannot pass through — `reversed` needs saying.
    fn __reversed__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyIterator>> {
        let mut dates = self.schedule.dates().to_vec();
        dates.reverse();
        PyList::new(py, dates_out(py, &dates)?)?
            .into_any()
            .try_iter()
    }

    fn __len__(&self) -> usize {
        self.schedule.dates().len()
    }

    /// One coupon date by index, or a `list` of them by slice — the
    /// crate's schedule derefs to its dates, and a slice of those is what
    /// `&schedule[1..3]` gives there.
    ///
    /// >>> import datetime
    /// >>> from fasti import Schedule
    /// >>> from fasti.calendars import WEEKENDS_ONLY
    /// >>> s = Schedule(datetime.date(2025, 1, 15), datetime.date(2026, 1, 15),
    /// ...              "quarterly", WEEKENDS_ONLY)
    /// >>> s[1:3]
    /// [datetime.date(2025, 4, 15), datetime.date(2025, 7, 15)]
    /// >>> s[::-1] == list(reversed(s))
    /// True
    fn __getitem__<'py>(
        &self,
        py: Python<'py>,
        index: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let dates = self.schedule.dates();
        if let Ok(slice) = index.cast::<PySlice>() {
            let indices = slice.indices(isize::try_from(dates.len()).unwrap_or(isize::MAX))?;
            let mut picked = Vec::with_capacity(indices.slicelength);
            let mut at = indices.start;
            for _ in 0..indices.slicelength {
                if let Some(date) = usize::try_from(at).ok().and_then(|i| dates.get(i)) {
                    picked.push(*date);
                }
                at += indices.step;
            }
            return Ok(PyList::new(py, dates_out(py, &picked)?)?.into_any());
        }
        let index: isize = index.extract()?;
        let length = isize::try_from(dates.len()).unwrap_or(isize::MAX);
        let resolved = if index < 0 { index + length } else { index };
        usize::try_from(resolved)
            .ok()
            .and_then(|i| dates.get(i))
            .ok_or_else(|| pyo3::exceptions::PyIndexError::new_err("schedule index out of range"))
            .and_then(|d| date_out(py, *d))
    }

    pub fn __repr__(&self) -> String {
        let dates = self.schedule.dates();
        match (dates.first(), dates.last()) {
            (Some(first), Some(last)) => format!(
                "Schedule([{} .. {}], {} dates)",
                py_date_repr(*first),
                py_date_repr(*last),
                dates.len()
            ),
            _ => "Schedule([], 0 dates)".to_owned(),
        }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<ReplayReduction<'py>> {
        let origin: Bound<'py, PyAny> = match &self.spec.origin {
            Origin::Dates(dates, frequency) => (
                PyList::new(py, dates_out(py, dates)?)?,
                frequency.map(|f| f.canonical()),
            )
                .into_bound_py_any(py)?,
            Origin::Generated(g) => (
                date_out(py, g.effective)?,
                date_out(py, g.termination)?,
                g.tenor,
                g.calendar.py_calendar()?,
                g.convention.canonical(),
                g.termination_convention.canonical(),
                g.rule.canonical(),
                g.end_of_month,
                g.first_date.map(|d| date_out(py, d)).transpose()?,
                g.next_to_last_date.map(|d| date_out(py, d)).transpose()?,
                (
                    g.payment_convention.canonical(),
                    g.payment_calendar
                        .as_ref()
                        .map(CalendarSpec::py_calendar)
                        .transpose()?,
                    g.payment_lag,
                ),
            )
                .into_bound_py_any(py)?,
        };
        let ops: Vec<(&str, Bound<'py, PyAny>)> = self
            .spec
            .ops
            .iter()
            .map(|op| match op {
                Op::After(cutoff) => Ok(("after", date_out(py, *cutoff)?)),
                Op::Until(cutoff) => Ok(("until", date_out(py, *cutoff)?)),
            })
            .collect::<PyResult<_>>()?;
        Ok((
            hook(py, "_rebuild_schedule")?,
            (origin, PyList::new(py, ops)?),
        ))
    }
}

/// Replay a pickled generation lattice.
#[pyfunction]
pub fn _rebuild_generation(unit: &str, length: i32, end_of_month: bool) -> PyResult<PyGeneration> {
    Ok(PyGeneration(fasti::Generation {
        tenor: crate::period::_rebuild_period(unit, length)?.0,
        end_of_month,
    }))
}

/// Replay a pickled schedule: its generation arguments, then the slices.
#[pyfunction]
pub fn _rebuild_schedule(
    origin: &Bound<'_, PyAny>,
    ops: &Bound<'_, PyList>,
) -> PyResult<PySchedule> {
    let g = origin.cast::<PyTuple>()?;
    let at = |i: usize| g.get_item(i);
    let mut schedule = if g.len() == 2 {
        PySchedule::from_dates(at(0)?.extract()?, at(1)?.extract()?)?
    } else {
        let payment = at(10)?.cast_into::<PyTuple>()?;
        PySchedule::new(
            at(0)?.extract()?,
            at(1)?.extract()?,
            at(2)?.extract()?,
            at(3)?.extract()?,
            Some(at(4)?.extract()?),
            Some(at(5)?.extract()?),
            Some(at(6)?.extract()?),
            at(7)?.extract()?,
            at(8)?.extract()?,
            at(9)?.extract()?,
            Some(payment.get_item(0)?.extract()?),
            payment.get_item(1)?.extract()?,
            payment.get_item(2)?.extract()?,
        )?
    };
    for entry in ops.iter() {
        let entry = entry.cast_into::<PyTuple>()?;
        let cutoff: DateArg = entry.get_item(1)?.extract()?;
        schedule = match entry.get_item(0)?.extract::<String>()?.as_str() {
            "after" => schedule.after(cutoff)?,
            "until" => schedule.until(cutoff)?,
            name => return Err(FastiError::new_err(format!("unknown schedule op {name:?}"))),
        };
    }
    Ok(schedule)
}

/// One accrual period of a schedule: interest runs from `start` to
/// `end` (excluded). `reference` is the regular coupon period ACT/ACT
/// ICMA counts it against.
///
/// >>> import datetime
/// >>> from fasti import Schedule
/// >>> from fasti.calendars import NULL_CALENDAR
/// >>> stub = Schedule(datetime.date(2002, 8, 15), datetime.date(2004, 1, 15),
/// ...                 "semiannual", NULL_CALENDAR).periods()[0]
/// >>> stub.reference
/// (datetime.date(2002, 7, 15), datetime.date(2003, 1, 15))
#[pyclass(
    frozen,
    eq,
    skip_from_py_object,
    module = "fasti",
    name = "AccrualPeriod"
)]
#[derive(Clone, PartialEq, Eq)]
pub struct PyAccrualPeriod(pub AccrualPeriod);

#[pymethods]
impl PyAccrualPeriod {
    /// The first day of accrual.
    #[getter]
    fn start<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_out(py, self.0.start())
    }

    /// The day accrual stops, excluded.
    #[getter]
    fn end<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_out(py, self.0.end())
    }

    /// The regular coupon period it accrues against, as `(start, end)`.
    #[getter]
    fn reference<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
        period_out(py, &self.0.reference())
    }

    /// True if `date` is in `start <= date < end`.
    fn __contains__(&self, date: DateArg) -> bool {
        self.0.contains(date.0)
    }

    /// The period cut short at `date`, for accrued interest; None unless
    /// `start <= date <= end`.
    fn until(&self, date: DateArg) -> Option<Self> {
        self.0.until(date.0).map(Self)
    }

    fn __repr__(&self) -> String {
        format!(
            "AccrualPeriod({} .. {})",
            py_date_repr(self.0.start()),
            py_date_repr(self.0.end())
        )
    }
}

/// A coupon: its accrual period and its payment date.
#[pyclass(frozen, eq, skip_from_py_object, module = "fasti", name = "Coupon")]
#[derive(Clone, PartialEq, Eq)]
pub struct PyCoupon {
    accrual: PyAccrualPeriod,
    payment: Date,
}

#[pymethods]
impl PyCoupon {
    /// The period interest accrues over.
    #[getter]
    fn accrual(&self) -> PyAccrualPeriod {
        self.accrual.clone()
    }

    /// When the coupon is paid.
    #[getter]
    fn payment<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_out(py, self.payment)
    }

    fn __repr__(&self) -> String {
        format!(
            "Coupon({}, payment={})",
            self.accrual.__repr__(),
            py_date_repr(self.payment)
        )
    }
}
