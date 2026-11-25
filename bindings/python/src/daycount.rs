//! [`DayCount`] — the conventions that turn two dates into a year
//! fraction.
//!
//! The crate has one type per convention behind a trait. Python has one
//! class with a constructor per convention, since a Python caller picks
//! a convention at runtime and never writes an impl.

use fasti::{
    AccrualDayCount, Act360, Act365Fixed, ActActICMA, ActActISDA, Date, DayCount, Thirty360Bond,
    Thirty360European, Thirty360ISDA, Thirty360US,
};
use pyo3::IntoPyObjectExt;
use pyo3::prelude::*;
use pyo3::types::{PyTuple, PyTupleMethods};

use crate::convert::{DateArg, FastiError, OrRaise, Reduction, date_out, fraction_out, hook};
use crate::rule::py_date_repr;
use crate::schedule::PyAccrualPeriod;
use crate::vocab::FrequencyArg;

/// Which convention, and the context it carries.
#[derive(Clone, PartialEq, Eq, Hash)]
enum Convention {
    Act360,
    Act365Fixed,
    ActActISDA,
    /// ACT/ACT ICMA, which measures a schedule's periods only.
    ActActICMA,
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
            Convention::ActActICMA => {
                return Err(FastiError::new_err(
                    "ACT/ACT ICMA measures a schedule's accrual periods, not two dates: \
                     use accrual_fraction(period), or year_fraction_with_reference",
                ));
            }
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
            Convention::ActActICMA => ("act_act_icma", None),
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

    /// ACT/ACT as ICMA defines it. It measures a schedule's accrual
    /// periods (`accrual_fraction`), taking the coupon frequency and any
    /// stub's reference period from the schedule.
    #[staticmethod]
    fn act_act_icma() -> Self {
        Self(Convention::ActActICMA)
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
        if let Convention::ActActICMA = &self.0 {
            return Ok(ActActICMA.name());
        }
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

    /// The year fraction accrued over a schedule's `period`. Every
    /// convention measures its start to end; ACT/ACT ICMA also counts
    /// against its reference period.
    ///
    /// >>> import datetime
    /// >>> from fasti import DayCount, Schedule
    /// >>> from fasti.calendars import NULL_CALENDAR
    /// >>> stub = Schedule(datetime.date(2002, 8, 15), datetime.date(2004, 1, 15),
    /// ...                 "semiannual", NULL_CALENDAR).periods()[0]
    /// >>> DayCount.act_act_icma().accrual_fraction(stub)
    /// Fraction(153, 368)
    fn accrual_fraction<'py>(
        &self,
        py: Python<'py>,
        period: PyRef<'py, PyAccrualPeriod>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let fraction = match &self.0 {
            Convention::ActActICMA => ActActICMA.accrual_fraction(&period.0),
            _ => self.apply(|dc| dc.accrual_fraction(&period.0))?,
        };
        fraction_out(py, fraction.or_raise()?)
    }

    /// The ACT/ACT ICMA year fraction against one explicit reference
    /// period of a `frequency` coupon grid — the escape hatch for an
    /// accrual with no schedule.
    fn year_fraction_with_reference<'py>(
        &self,
        py: Python<'py>,
        start: DateArg,
        end: DateArg,
        ref_start: DateArg,
        ref_end: DateArg,
        frequency: FrequencyArg,
    ) -> PyResult<Bound<'py, PyAny>> {
        let Convention::ActActICMA = &self.0 else {
            return Err(FastiError::new_err(format!(
                "{} has no reference periods; this is the ACT/ACT ICMA escape hatch",
                self.name()?
            )));
        };
        let fraction = ActActICMA
            .year_fraction_with_reference(start.0, end.0, ref_start.0, ref_end.0, frequency.0.0)
            .or_raise()?;
        fraction_out(py, fraction)
    }

    fn __repr__(&self) -> String {
        let (name, argument) = self.state();
        format!("DayCount.{name}({})", argument.unwrap_or_default())
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<Reduction<'py, (Bound<'py, PyTuple>,)>> {
        let mut state: Vec<Bound<'py, PyAny>> = vec![self.state().0.into_bound_py_any(py)?];
        if let Convention::Thirty360ISDA(termination) = &self.0 {
            state.push(date_out(py, *termination)?);
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
        "act_act_icma" => PyDayCount::act_act_icma(),
        "thirty_360_bond" => PyDayCount::thirty_360_bond(),
        "thirty_360_us" => PyDayCount::thirty_360_us(),
        "thirty_360_european" => PyDayCount::thirty_360_european(),
        "thirty_360_isda" => PyDayCount::thirty_360_isda(at(1)?.extract()?),
        name => return Err(FastiError::new_err(format!("unknown day count {name:?}"))),
    };
    Ok(convention)
}
