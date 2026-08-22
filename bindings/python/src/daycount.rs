//! [`DayCount`] — the conventions that turn two dates into a year
//! fraction.
//!
//! The crate has one type per convention behind a trait. Python has one
//! class with a constructor per convention, since a Python caller picks
//! a convention at runtime and never writes an impl.

use fasti::{
    Act360, Act365Fixed, ActActICMA, ActActISDA, Date, DayCount, Thirty360Bond, Thirty360European,
    Thirty360ISDA, Thirty360US,
};
use pyo3::IntoPyObjectExt;
use pyo3::prelude::*;
use pyo3::types::{PyTuple, PyTupleMethods};

use crate::convert::{DateArg, FastiError, OrRaise, Reduction, date_out, fraction_out, hook};
use crate::rule::py_date_repr;
use crate::schedule::{PySchedule, ScheduleArg};
use crate::vocab::{FrequencyArg, PyFrequency};

/// Which convention, and the context it carries.
#[derive(Clone, PartialEq, Eq, Hash)]
enum Convention {
    Act360,
    Act365Fixed,
    ActActISDA,
    /// ACT/ACT ICMA, optionally bound to the schedule it accrues over.
    ActActICMA(PyFrequency, Option<Box<PySchedule>>),
    Thirty360Bond,
    Thirty360US,
    Thirty360European,
    Thirty360ISDA(Date),
}

/// A day-count convention: the map from a pair of dates to a year
/// fraction, as a `fractions.Fraction`. Never a float — the crate is
/// float-free and that survives the boundary.
///
/// >>> import datetime, fractions
/// >>> from fasti import DayCount
/// >>> q1 = (datetime.date(2025, 1, 1), datetime.date(2025, 4, 1))
/// >>> DayCount.act_360().year_fraction(*q1)
/// Fraction(1, 4)
/// >>> DayCount.act_365_fixed().name()
/// 'Actual/365 (Fixed)'
#[pyclass(
    frozen,
    eq,
    hash,
    skip_from_py_object,
    module = "fasti",
    name = "DayCount"
)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PyDayCount(Convention);

impl PyDayCount {
    /// Run `f` against the crate value this convention names.
    fn apply<R>(&self, f: impl FnOnce(&dyn DayCount) -> R) -> PyResult<R> {
        Ok(match &self.0 {
            Convention::Act360 => f(&Act360),
            Convention::Act365Fixed => f(&Act365Fixed),
            Convention::ActActISDA => f(&ActActISDA),
            Convention::ActActICMA(frequency, None) => f(&ActActICMA::new(frequency.0)),
            Convention::ActActICMA(frequency, Some(schedule)) => f(&ActActICMA::new(frequency.0)
                .bind(schedule.inner())
                .or_raise()?),
            Convention::Thirty360Bond => f(&Thirty360Bond),
            Convention::Thirty360US => f(&Thirty360US),
            Convention::Thirty360European => f(&Thirty360European),
            Convention::Thirty360ISDA(termination) => f(&Thirty360ISDA::new(*termination)),
        })
    }

    /// The constructor call that produced this, for `repr` and pickle.
    fn state(&self) -> (&'static str, Option<String>) {
        match &self.0 {
            Convention::Act360 => ("act_360", None),
            Convention::Act365Fixed => ("act_365_fixed", None),
            Convention::ActActISDA => ("act_act_isda", None),
            Convention::ActActICMA(frequency, _) => ("act_act_icma", Some(frequency.__repr__())),
            Convention::Thirty360Bond => ("thirty_360_bond", None),
            Convention::Thirty360US => ("thirty_360_us", None),
            Convention::Thirty360European => ("thirty_360_european", None),
            Convention::Thirty360ISDA(termination) => {
                ("thirty_360_isda", Some(py_date_repr(*termination)))
            }
        }
    }
}

#[pymethods]
impl PyDayCount {
    /// Actual days over a 360-day year.
    #[staticmethod]
    fn act_360() -> Self {
        Self(Convention::Act360)
    }

    /// Actual days over a fixed 365-day year.
    #[staticmethod]
    fn act_365_fixed() -> Self {
        Self(Convention::Act365Fixed)
    }

    /// ACT/ACT as ISDA defines it: each calendar year over its own length.
    #[staticmethod]
    fn act_act_isda() -> Self {
        Self(Convention::ActActISDA)
    }

    /// ACT/ACT as ICMA defines it, over a coupon grid of `frequency`.
    ///
    /// Call `bind` to accrue against a schedule's own reference periods,
    /// which is what a stub needs.
    #[staticmethod]
    fn act_act_icma(frequency: FrequencyArg) -> Self {
        Self(Convention::ActActICMA(frequency.0, None))
    }

    /// 30/360 Bond Basis.
    #[staticmethod]
    fn thirty_360_bond() -> Self {
        Self(Convention::Thirty360Bond)
    }

    /// 30/360 US (NASD).
    #[staticmethod]
    fn thirty_360_us() -> Self {
        Self(Convention::Thirty360US)
    }

    /// 30E/360, the European variant.
    #[staticmethod]
    fn thirty_360_european() -> Self {
        Self(Convention::Thirty360European)
    }

    /// 30E/360 ISDA, which needs the trade's termination date.
    #[staticmethod]
    fn thirty_360_isda(termination: DateArg) -> Self {
        Self(Convention::Thirty360ISDA(termination.0))
    }

    /// The convention's short name, as the crate spells it.
    fn name(&self) -> PyResult<&'static str> {
        self.apply(|dc| dc.name())
    }

    /// Days between `start` and `end`, signed by direction. The 30/360
    /// family counts its own way.
    fn day_count(&self, start: DateArg, end: DateArg) -> PyResult<i64> {
        self.apply(|dc| dc.day_count(start.0, end.0))
    }

    /// The year fraction between `start` and `end`, signed by direction.
    ///
    /// >>> import datetime
    /// >>> from fasti import DayCount
    /// >>> jan, jul = datetime.date(2025, 1, 1), datetime.date(2025, 7, 1)
    /// >>> DayCount.thirty_360_bond().year_fraction(jan, jul)
    /// Fraction(1, 2)
    fn year_fraction<'py>(
        &self,
        py: Python<'py>,
        start: DateArg,
        end: DateArg,
    ) -> PyResult<Bound<'py, PyAny>> {
        let fraction = self.apply(|dc| dc.year_fraction(start.0, end.0))?;
        fraction_out(py, fraction)
    }

    /// ACT/ACT ICMA bound to the schedule it accrues over, so that a
    /// stub accrues against its notional grid.
    ///
    /// Raises if the schedule's tenor disagrees with the convention's
    /// frequency — the alternative is silently accruing against the
    /// wrong grid — or if the convention is not a schedule-defined one.
    fn bind(&self, schedule: ScheduleArg) -> PyResult<Self> {
        let Convention::ActActICMA(frequency, _) = &self.0 else {
            return Err(FastiError::new_err(format!(
                "{} takes no schedule; only ACT/ACT ICMA binds to one",
                self.name()?
            )));
        };
        // Bind once here so a mismatched frequency is refused now.
        ActActICMA::new(frequency.0)
            .bind(schedule.0.inner())
            .or_raise()?;
        Ok(Self(Convention::ActActICMA(
            *frequency,
            Some(Box::new(schedule.0)),
        )))
    }

    /// The ACT/ACT ICMA year fraction against one explicit reference
    /// period — the manual escape hatch when the accrual has no
    /// schedule. Prefer `bind`.
    fn year_fraction_with_reference<'py>(
        &self,
        py: Python<'py>,
        start: DateArg,
        end: DateArg,
        ref_start: DateArg,
        ref_end: DateArg,
    ) -> PyResult<Bound<'py, PyAny>> {
        let Convention::ActActICMA(frequency, _) = &self.0 else {
            return Err(FastiError::new_err(format!(
                "{} has no reference periods; this is the ACT/ACT ICMA escape hatch",
                self.name()?
            )));
        };
        let fraction = ActActICMA::new(frequency.0)
            .year_fraction_with_reference(start.0, end.0, ref_start.0, ref_end.0)
            .or_raise()?;
        fraction_out(py, fraction)
    }

    fn __repr__(&self) -> String {
        let (name, argument) = self.state();
        let bound = match &self.0 {
            Convention::ActActICMA(_, Some(schedule)) => {
                format!(".bind({})", schedule.__repr__())
            }
            _ => String::new(),
        };
        format!("DayCount.{name}({}){bound}", argument.unwrap_or_default())
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<Reduction<'py, (Bound<'py, PyTuple>,)>> {
        let mut state: Vec<Bound<'py, PyAny>> = vec![self.state().0.into_bound_py_any(py)?];
        match &self.0 {
            Convention::ActActICMA(frequency, schedule) => {
                state.push(frequency.canonical().into_bound_py_any(py)?);
                if let Some(schedule) = schedule {
                    state.push((**schedule).clone().into_bound_py_any(py)?);
                }
            }
            Convention::Thirty360ISDA(termination) => {
                state.push(date_out(py, *termination)?);
            }
            _ => {}
        }
        Ok((hook(py, "_rebuild_daycount")?, (PyTuple::new(py, state)?,)))
    }
}

/// Replay a pickled day count.
#[pyfunction]
pub fn _rebuild_daycount(state: &Bound<'_, PyTuple>) -> PyResult<PyDayCount> {
    let at = |i: usize| state.get_item(i);
    let convention = match at(0)?.extract::<String>()?.as_str() {
        "act_360" => PyDayCount::act_360(),
        "act_365_fixed" => PyDayCount::act_365_fixed(),
        "act_act_isda" => PyDayCount::act_act_isda(),
        "act_act_icma" => {
            let icma = PyDayCount::act_act_icma(at(1)?.extract()?);
            return match state.len() {
                2 => Ok(icma),
                _ => icma.bind(at(2)?.extract()?),
            };
        }
        "thirty_360_bond" => PyDayCount::thirty_360_bond(),
        "thirty_360_us" => PyDayCount::thirty_360_us(),
        "thirty_360_european" => PyDayCount::thirty_360_european(),
        "thirty_360_isda" => PyDayCount::thirty_360_isda(at(1)?.extract()?),
        name => return Err(FastiError::new_err(format!("unknown day count {name:?}"))),
    };
    Ok(convention)
}
