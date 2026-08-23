//! [`Schedule`] — the coupon dates, and the regular grid they accrue
//! against.

use fasti::{Date, DateGenerationRule, Schedule, ScheduleBuilder};
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
    ConventionArg, GenerationRuleArg, PyBusinessDayConvention, PyDateGenerationRule,
};

/// What a schedule was generated from. A generated schedule cannot be
/// recovered from its dates alone — the stub reference grid is not in
/// them — so pickling replays the generation instead.
#[derive(Clone, PartialEq, Eq, Hash)]
enum Origin {
    Dates(Vec<Date>),
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
            Origin::Dates(dates) => Schedule::try_from(dates.clone()).or_raise()?,
            Origin::Generated(g) => {
                let calendar = g.calendar.build()?;
                let mut builder =
                    ScheduleBuilder::new(g.effective, g.termination, g.tenor.0, calendar.view())
                        .with_convention(g.convention.0)
                        .with_termination_convention(g.termination_convention.0)
                        .with_rule(g.rule.0)
                        .with_end_of_month(g.end_of_month);
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
/// and a calendar. The rest are keyword arguments whose defaults are
/// the crate's: `ModifiedFollowing` for interior dates, `Unadjusted`
/// for the termination date, `Backward` generation, no end-of-month
/// snapping, and no stub anchors.
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
        end_of_month=false, first_date=None, next_to_last_date=None,
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
    ) -> PyResult<Self> {
        Self::from_spec(ScheduleSpec {
            origin: Origin::Generated(Box::new(Generated {
                effective: effective.0,
                termination: termination.0,
                tenor: PyPeriod(tenor.0),
                calendar: calendar.0,
                convention: convention.map_or(
                    PyBusinessDayConvention(fasti::BusinessDayConvention::ModifiedFollowing),
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
            })),
            ops: Vec::new(),
        })
    }

    /// A schedule from a date list — a term sheet's dates, taken as
    /// given. It names no tenor, so it is entirely regular.
    #[staticmethod]
    fn from_dates(dates: Vec<DateArg>) -> PyResult<Self> {
        Self::from_spec(ScheduleSpec {
            origin: Origin::Dates(dates.into_iter().map(|d| d.0).collect()),
            ops: Vec::new(),
        })
    }

    /// The adjusted coupon dates.
    fn dates<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        dates_out(py, self.schedule.dates())
    }

    /// The accrual periods, as half-open `(start, end)` pairs.
    fn periods<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Vec<(Bound<'py, PyAny>, Bound<'py, PyAny>)>> {
        self.schedule
            .periods()
            .map(|p| period_out(py, &p))
            .collect()
    }

    /// The reference period each coupon period accrues against.
    fn reference_periods<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Vec<(Bound<'py, PyAny>, Bound<'py, PyAny>)>> {
        self.schedule
            .reference_periods()
            .map(|p| period_out(py, &p))
            .collect()
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
            Origin::Dates(dates) => PyList::new(py, dates_out(py, dates)?)?.into_any(),
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
    let mut schedule = if let Ok(dates) = origin.extract::<Vec<DateArg>>() {
        PySchedule::from_dates(dates)?
    } else {
        let g = origin.cast::<PyTuple>()?;
        let at = |i: usize| g.get_item(i);
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

/// A `Schedule` argument, cloned so a bound day count owns its grid.
pub struct ScheduleArg(pub PySchedule);

impl<'py> FromPyObject<'_, 'py> for ScheduleArg {
    type Error = PyErr;

    fn extract(obj: pyo3::Borrowed<'_, 'py, PyAny>) -> PyResult<Self> {
        Ok(Self(obj.cast::<PySchedule>()?.get().clone()))
    }
}
