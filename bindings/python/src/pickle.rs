//! Pickle support.
//!
//! Extension types are not picklable by default, which breaks the first
//! thing a user does with a calendar at scale: hand it to a
//! `multiprocessing` worker. Every type here is immutable, so the
//! recommended shape for frozen classes applies — `__reduce__` naming a
//! module-level function and the arguments that rebuild the value,
//! rather than `__setstate__` mutating one into place.
//!
//! Each type therefore remembers how it was made. That is cheap (a name,
//! a few numbers) and exact: rebuilding replays the same constructor
//! calls, so an unpickled value is not a copy of the internals but the
//! same construction run again.

use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyModule, PyTuple};

/// Look up one of the private `_rebuild_*` functions by name.
///
/// Pickle stores the callable by reference, so it has to be reachable
/// as a module attribute — a bound method or a closure would not do.
pub fn rebuilder<'py>(py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    static MODULE: PyOnceLock<Py<PyModule>> = PyOnceLock::new();
    MODULE
        .get_or_try_init(py, || py.import("fasti._fasti").map(Bound::unbind))?
        .bind(py)
        .getattr(name)
}

/// The `(callable, args)` pair `__reduce__` returns.
pub type Reduced<'py> = (Bound<'py, PyAny>, Bound<'py, PyTuple>);

/// Build a `__reduce__` result: the named rebuilder, plus its arguments.
pub fn reduce<'py, A>(py: Python<'py>, rebuilder_name: &str, args: A) -> PyResult<Reduced<'py>>
where
    A: IntoPyObject<'py, Target = PyTuple, Output = Bound<'py, PyTuple>>,
{
    let args = args.into_pyobject(py).map_err(Into::into)?;
    Ok((rebuilder(py, rebuilder_name)?, args))
}

// ---- Rebuilders ---------------------------------------------------------
//
// Private module-level functions, one per type, each taking exactly what
// that type's `__reduce__` hands back. They go through the ordinary
// public constructors, so an unpickled value cannot be built in a state
// the public API would refuse.

use crate::enums::{
    BusinessDayConvention, ConventionArg, DateGenerationRule, Frequency, FrequencyArg,
    GenerationArg, ShiftArg, Weekday, WeekdayArg, WeekendShift,
};
use crate::period::Period;

#[pyfunction]
pub fn _rebuild_weekday(value: WeekdayArg) -> Weekday {
    Weekday::wrap(value.0)
}

#[pyfunction]
pub fn _rebuild_convention(value: ConventionArg) -> BusinessDayConvention {
    BusinessDayConvention::wrap(value.0)
}

#[pyfunction]
pub fn _rebuild_generation(value: GenerationArg) -> DateGenerationRule {
    DateGenerationRule::wrap(value.0)
}

#[pyfunction]
pub fn _rebuild_frequency(value: FrequencyArg) -> Frequency {
    Frequency::wrap(value.0)
}

#[pyfunction]
pub fn _rebuild_shift(value: ShiftArg) -> WeekendShift {
    WeekendShift::wrap(value.0)
}

#[pyfunction]
pub fn _rebuild_period(spec: &str) -> PyResult<Period> {
    Period::parse(spec)
}

#[pyfunction]
pub fn _rebuild_schedule(
    py: Python<'_>,
    spec: &Bound<'_, PyAny>,
) -> PyResult<crate::schedule::Schedule> {
    crate::schedule::rebuild(py, spec)
}

#[pyfunction]
pub fn _rebuild_daycount(
    name: &str,
    frequency: Option<crate::enums::FrequencyArg>,
    schedule: Option<PyRef<'_, crate::schedule::Schedule>>,
    termination: Option<crate::convert::DateArg>,
) -> PyResult<crate::daycount::DayCount> {
    crate::daycount::DayCount::new(name, frequency, schedule, termination)
}

#[pyfunction]
pub fn _rebuild_calendar(spec: &Bound<'_, PyAny>) -> PyResult<crate::calendar::Calendar> {
    crate::calendar::rebuild(spec)
}

#[pyfunction]
pub fn _rebuild_rule(kind: &str, payload: &Bound<'_, PyDict>) -> PyResult<crate::rules::Rule> {
    crate::rules::rebuild(kind, payload)
}

/// Register the rebuilders on the module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(_rebuild_weekday, m)?)?;
    m.add_function(wrap_pyfunction!(_rebuild_convention, m)?)?;
    m.add_function(wrap_pyfunction!(_rebuild_generation, m)?)?;
    m.add_function(wrap_pyfunction!(_rebuild_frequency, m)?)?;
    m.add_function(wrap_pyfunction!(_rebuild_shift, m)?)?;
    m.add_function(wrap_pyfunction!(_rebuild_period, m)?)?;
    m.add_function(wrap_pyfunction!(_rebuild_rule, m)?)?;
    m.add_function(wrap_pyfunction!(_rebuild_calendar, m)?)?;
    m.add_function(wrap_pyfunction!(_rebuild_schedule, m)?)?;
    m.add_function(wrap_pyfunction!(_rebuild_daycount, m)?)?;
    Ok(())
}
