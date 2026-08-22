//! The `datetime` boundary: every date crossing into or out of this
//! module is a `datetime.date`, and every year fraction is a
//! `fractions.Fraction`.
//!
//! Dates convert through the proleptic ordinal (`date.toordinal`) rather
//! than through year/month/day, which is one interpreter call instead of
//! three attribute reads and maps onto `fasti`'s serial representation by
//! a single subtraction. The `abi3` wheel has no access to `CPython`'s
//! datetime C API, so the `datetime` module is imported and cached.

use fasti::Date;
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyString, PyType};
use pyo3::{Borrowed, exceptions::PyTypeError, intern};

use crate::error::invalid;

/// `datetime.date(1901, 1, 1).toordinal()` — the ordinal of `fasti`
/// serial 0. Checked against Python in `tests/test_dates.py`.
const ORDINAL_EPOCH: i64 = 693_961;

fn date_type(py: Python<'_>) -> PyResult<&Bound<'_, PyType>> {
    static DATE: PyOnceLock<Py<PyType>> = PyOnceLock::new();
    DATE.import(py, "datetime", "date")
}

fn fraction_type(py: Python<'_>) -> PyResult<&Bound<'_, PyType>> {
    static FRACTION: PyOnceLock<Py<PyType>> = PyOnceLock::new();
    FRACTION.import(py, "fractions", "Fraction")
}

fn timedelta_type(py: Python<'_>) -> PyResult<&Bound<'_, PyType>> {
    static TIMEDELTA: PyOnceLock<Py<PyType>> = PyOnceLock::new();
    TIMEDELTA.import(py, "datetime", "timedelta")
}

/// `true` iff `ob` is a `datetime.timedelta`.
pub fn is_timedelta(ob: &Bound<'_, PyAny>) -> PyResult<bool> {
    ob.is_instance(timedelta_type(ob.py())?)
}

fn from_ordinal(ordinal: i64) -> Option<Date> {
    u32::try_from(ordinal - ORDINAL_EPOCH)
        .ok()
        .and_then(|serial| Date::from_serial(serial).ok())
}

/// Coerce a Python object to a [`Date`]. The only thing accepted is a
/// `datetime.date` — including anything deriving from it
/// (`datetime.datetime`, `pandas.Timestamp`), whose time component is
/// dropped. A date-shaped string is not a date: parsing one is
/// `datetime.date.fromisoformat`'s job, and doing it here would mean
/// this library owning a second date grammar and its failure modes.
pub fn to_date(ob: &Bound<'_, PyAny>) -> PyResult<Date> {
    let py = ob.py();
    if ob.is_instance(date_type(py)?)? {
        let ordinal = ob
            .call_method0(intern!(py, "toordinal"))?
            .extract::<i64>()?;
        return from_ordinal(ordinal).ok_or_else(|| {
            let shown = ob
                .str()
                .map_or_else(|_| "the given date".to_owned(), |s| s.to_string());
            invalid(format!(
                "date out of range: fasti supports 1901-01-01..=2199-12-31, got {shown}"
            ))
        });
    }
    // A string is the mistake worth naming, since it is the one people
    // reach for and the fix is a single call.
    if let Ok(text) = ob.cast::<PyString>() {
        return Err(PyTypeError::new_err(format!(
            "expected a datetime.date, got a str; parse it first, e.g. \
             datetime.date.fromisoformat({})",
            text.repr()
                .map_or_else(|_| "'...'".to_owned(), |r| r.to_string())
        )));
    }
    Err(PyTypeError::new_err(format!(
        "expected a datetime.date (or a datetime.datetime, whose time is dropped), got {}",
        type_name(ob)
    )))
}

/// Build a `datetime.date` from a [`Date`].
pub fn from_date(py: Python<'_>, date: Date) -> PyResult<Bound<'_, PyAny>> {
    let ordinal = i64::from(date.serial()) + ORDINAL_EPOCH;
    date_type(py)?.call_method1(intern!(py, "fromordinal"), (ordinal,))
}

/// Build a `fractions.Fraction` from a [`Fraction`](fasti::Fraction).
pub fn from_fraction(py: Python<'_>, value: fasti::Fraction) -> PyResult<Bound<'_, PyAny>> {
    let (num, den) = value.parts();
    fraction_type(py)?.call1((num, den))
}

/// The type name of `ob`, for error messages.
pub fn type_name(ob: &Bound<'_, PyAny>) -> String {
    ob.get_type()
        .name()
        .map_or_else(|_| "<unknown>".to_owned(), |n| n.to_string())
}

/// A date-valued argument: a `datetime.date`, and nothing else.
#[derive(Debug, Clone, Copy)]
pub struct DateArg(pub Date);

impl FromPyObject<'_, '_> for DateArg {
    type Error = PyErr;

    fn extract(ob: Borrowed<'_, '_, PyAny>) -> Result<Self, Self::Error> {
        to_date(&ob).map(Self)
    }
}

/// Accept either one date or an iterable of them.
///
/// Adding a single blackout day is the common case, and
/// `with_holidays(day)` is what a caller writes before
/// `with_holidays([day])` occurs to them. A `datetime.date` is not
/// iterable, so there is nothing to disambiguate.
pub fn dates(ob: &Bound<'_, PyAny>) -> PyResult<Vec<Date>> {
    if let Ok(one) = ob.extract::<DateArg>() {
        return Ok(vec![one.0]);
    }
    // A string is iterable, so without this it would be read as a
    // sequence of characters and complain about the first one instead of
    // about the string.
    if ob.is_instance_of::<PyString>() {
        return to_date(ob).map(|date| vec![date]);
    }
    let items = ob.try_iter().map_err(|_| {
        PyTypeError::new_err(format!(
            "expected a datetime.date or an iterable of them, got {}",
            type_name(ob)
        ))
    })?;
    // Element by element, so a bad entry reports what is wrong with it.
    let mut dates = Vec::new();
    for item in items {
        dates.push(item?.extract::<DateArg>()?.0);
    }
    Ok(dates)
}

/// A date-valued return. Always a `datetime.date`.
#[derive(Debug, Clone, Copy)]
pub struct DateOut(pub Date);

impl<'py> IntoPyObject<'py> for DateOut {
    type Target = PyAny;
    type Output = Bound<'py, PyAny>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        from_date(py, self.0)
    }
}

/// Normalize a convention/name string for matching: lowercase, with
/// separators and punctuation removed, so `"ACT/360"`, `"act 360"` and
/// `"Actual_360"` all land on the same key.
pub fn normalize(s: &str) -> String {
    s.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}
