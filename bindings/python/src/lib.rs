//! Python bindings for [`fasti`](https://github.com/hugof38/fasti):
//! dates, calendars, business-day conventions and day-count fractions
//! for financial code.
//!
//! The boundary is Python's own `datetime.date`: every date argument
//! takes one (or a `datetime.datetime`, whose time is dropped), and
//! every date result is one. Nothing else is accepted — a date-shaped
//! string is a string. Year fractions come back as
//! `fractions.Fraction`, matching the core crate's float-free
//! arithmetic exactly.

use pyo3::prelude::*;

mod calendar;
mod convert;
mod daycount;
mod enums;
mod error;
mod period;
mod pickle;
mod rules;
mod schedule;

use convert::{DateArg, from_date};
use daycount::DayCount;
use enums::{EasterMethodArg, FrequencyArg};
use error::err;

/// Resolve a convention argument that may be a name or a built
/// [`DayCount`].
fn resolve_day_count(
    convention: &Bound<'_, PyAny>,
    frequency: Option<FrequencyArg>,
    schedule: Option<PyRef<'_, schedule::Schedule>>,
    termination: Option<DateArg>,
) -> PyResult<DayCount> {
    if let Ok(dc) = convention.extract::<DayCount>() {
        return Ok(dc);
    }
    let name: String = convention.extract().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err(format!(
            "expected a DayCount or a convention name, got {}",
            convert::type_name(convention)
        ))
    })?;
    DayCount::new(&name, frequency, schedule, termination)
}

/// The year fraction between two dates under `convention`, as an exact
/// `fractions.Fraction`.
///
/// >>> import fasti
/// >>> from datetime import date
/// >>> fasti.year_fraction(date(2025, 1, 1), date(2025, 7, 1), "ACT/360")
/// Fraction(181, 360)
#[pyfunction]
#[pyo3(signature = (start, end, convention, *, frequency=None, schedule=None, termination=None))]
fn year_fraction<'py>(
    py: Python<'py>,
    start: DateArg,
    end: DateArg,
    convention: &Bound<'py, PyAny>,
    frequency: Option<FrequencyArg>,
    schedule: Option<PyRef<'py, schedule::Schedule>>,
    termination: Option<DateArg>,
) -> PyResult<Bound<'py, PyAny>> {
    resolve_day_count(convention, frequency, schedule, termination)?.year_fraction(py, start, end)
}

/// The day count between two dates under `convention`, signed by
/// direction.
///
/// >>> import fasti
/// >>> from datetime import date
/// >>> fasti.day_count(date(2025, 1, 31), date(2025, 2, 28), "30/360")
/// 28
#[pyfunction]
#[pyo3(signature = (start, end, convention, *, frequency=None, schedule=None, termination=None))]
fn day_count(
    start: DateArg,
    end: DateArg,
    convention: &Bound<'_, PyAny>,
    frequency: Option<FrequencyArg>,
    schedule: Option<PyRef<'_, schedule::Schedule>>,
    termination: Option<DateArg>,
) -> PyResult<i64> {
    Ok(resolve_day_count(convention, frequency, schedule, termination)?.day_count(start, end))
}

fn easter(
    py: Python<'_>,
    year: u16,
    method: Option<EasterMethodArg>,
    offset: i32,
) -> PyResult<Bound<'_, PyAny>> {
    let method = method.map_or(fasti::EasterMethod::Western, |m| m.0);
    let y = fasti::Year::new(year).map_err(err)?;
    let day_of_year = i32::from(fasti::easter_sunday(y, method));
    let jan_first = fasti::Date::from_ymd(year, fasti::Month::Jan, 1).map_err(err)?;
    let date = jan_first.add_days(day_of_year - 1 + offset).map_err(err)?;
    from_date(py, date)
}

/// Easter Sunday in `year`, under the Western (default) or Orthodox
/// computus.
///
/// >>> import fasti
/// >>> fasti.easter_sunday(2024)
/// datetime.date(2024, 3, 31)
/// >>> fasti.easter_sunday(2024, method="orthodox")
/// datetime.date(2024, 5, 5)
#[pyfunction]
#[pyo3(signature = (year, *, method=None))]
fn easter_sunday(
    py: Python<'_>,
    year: u16,
    method: Option<EasterMethodArg>,
) -> PyResult<Bound<'_, PyAny>> {
    easter(py, year, method, 0)
}

/// Easter Monday in `year` — the day after [`easter_sunday`].
#[pyfunction]
#[pyo3(signature = (year, *, method=None))]
fn easter_monday(
    py: Python<'_>,
    year: u16,
    method: Option<EasterMethodArg>,
) -> PyResult<Bound<'_, PyAny>> {
    easter(py, year, method, 1)
}

/// Dates, calendars, business-day conventions and day-count fractions.
///
/// The compiled core. Import the names from `fasti`, which re-exports
/// everything here and adds the `fasti.calendars` registry.
#[pymodule]
#[pyo3(name = "_fasti")]
fn fasti_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();

    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add("FastiError", py.get_type::<error::FastiError>())?;

    m.add_class::<calendar::Calendar>()?;
    m.add_class::<daycount::DayCount>()?;
    m.add_class::<enums::BusinessDayConvention>()?;
    m.add_class::<enums::DateGenerationRule>()?;
    m.add_class::<enums::Frequency>()?;
    m.add_class::<enums::Weekday>()?;
    m.add_class::<enums::WeekendShift>()?;
    m.add_class::<period::Period>()?;
    m.add_class::<rules::Rule>()?;
    m.add_class::<schedule::Schedule>()?;

    m.add_function(wrap_pyfunction!(year_fraction, m)?)?;
    m.add_function(wrap_pyfunction!(day_count, m)?)?;
    m.add_function(wrap_pyfunction!(easter_sunday, m)?)?;
    m.add_function(wrap_pyfunction!(easter_monday, m)?)?;
    pickle::register(m)?;

    // The supported range, as dates rather than as prose.
    m.add("MIN_DATE", from_date(py, fasti::Date::MIN)?)?;
    m.add("MAX_DATE", from_date(py, fasti::Date::MAX)?)?;

    Ok(())
}
