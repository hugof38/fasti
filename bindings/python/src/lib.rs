//! Python bindings for the `fasti` crate: dates, calendars,
//! business-day conventions and day-count fractions.
//!
//! Every name here maps to one in the crate, with the same semantics
//! and the same argument order. Dates cross the boundary as
//! `datetime.date` and year fractions as `fractions.Fraction`.

mod calendar;
mod convert;
mod daycount;
mod period;
mod rule;
mod schedule;
mod vocab;

use fasti::{easter_monday as monday, easter_sunday as sunday};
use pyo3::prelude::*;

use crate::convert::{FastiError, year};
use crate::vocab::{EasterMethodArg, PyEasterMethod};

/// Day-of-year of Easter Sunday in `year`, 1-indexed as the crate is.
///
/// It is a day number rather than a date because that is what the
/// lookup tables hold.
///
/// >>> import datetime
/// >>> from fasti import easter_sunday
/// >>> easter_sunday(2024)
/// 91
/// >>> datetime.date(2024, 1, 1) + datetime.timedelta(days=easter_sunday(2024) - 1)
/// datetime.date(2024, 3, 31)
#[pyfunction]
#[pyo3(signature = (year, method=None))]
fn easter_sunday(year: u16, method: Option<EasterMethodArg>) -> PyResult<u16> {
    Ok(sunday(
        self::year(year)?,
        method.map_or(fasti::EasterMethod::Western, EasterMethodArg::get),
    ))
}

/// Day-of-year of Easter Monday in `year`, 1-indexed as the crate is.
///
/// >>> from fasti import easter_monday, easter_sunday
/// >>> easter_monday(2024) - easter_sunday(2024)
/// 1
/// >>> easter_monday(2024, "orthodox")
/// 127
#[pyfunction]
#[pyo3(signature = (year, method=None))]
fn easter_monday(year: u16, method: Option<EasterMethodArg>) -> PyResult<u16> {
    Ok(monday(
        self::year(year)?,
        method.map_or(fasti::EasterMethod::Western, EasterMethodArg::get),
    ))
}

#[pymodule(gil_used = false)]
fn _fasti(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add(
        "__doc__",
        "The compiled half of fasti; import `fasti` instead.",
    )?;
    m.add("FastiError", m.py().get_type::<FastiError>())?;

    m.add_class::<calendar::PyCalendar>()?;
    m.add_class::<daycount::PyDayCount>()?;
    m.add_class::<period::PyPeriod>()?;
    m.add_class::<rule::PyRule>()?;
    m.add_class::<schedule::PyGeneration>()?;
    m.add_class::<schedule::PySchedule>()?;
    m.add_class::<vocab::PyBusinessDayConvention>()?;
    m.add_class::<vocab::PyDateGenerationRule>()?;
    m.add_class::<PyEasterMethod>()?;
    m.add_class::<vocab::PyFrequency>()?;
    m.add_class::<vocab::PyWeekday>()?;
    m.add_class::<vocab::PyWeekendShift>()?;

    m.add_function(wrap_pyfunction!(easter_monday, m)?)?;
    m.add_function(wrap_pyfunction!(easter_sunday, m)?)?;

    // The registry lookup the calendars package is built from, and the
    // pickle hooks every value's __reduce__ names.
    m.add_function(wrap_pyfunction!(calendar::_builtin, m)?)?;
    m.add_function(wrap_pyfunction!(calendar::_rebuild_calendar, m)?)?;
    m.add_function(wrap_pyfunction!(daycount::_rebuild_daycount, m)?)?;
    m.add_function(wrap_pyfunction!(period::_rebuild_period, m)?)?;
    m.add_function(wrap_pyfunction!(rule::_rebuild_rule, m)?)?;
    m.add_function(wrap_pyfunction!(schedule::_rebuild_generation, m)?)?;
    m.add_function(wrap_pyfunction!(schedule::_rebuild_schedule, m)?)?;
    Ok(())
}
