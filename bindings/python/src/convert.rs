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

use crate::error::{err, invalid};

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

/// Coerce a Python object to a [`Date`]: a `datetime.date`, anything
/// deriving from it (`datetime.datetime`, `pandas.Timestamp` — the time
/// component is dropped), or an ISO `YYYY-MM-DD` string.
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
    if let Ok(s) = ob.cast::<PyString>() {
        let text = s.to_cow()?;
        return text.parse::<Date>().map_err(err);
    }
    Err(PyTypeError::new_err(format!(
        "expected a datetime.date, datetime.datetime, or an ISO 'YYYY-MM-DD' string, got {}",
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

/// A date-valued argument. Accepts everything [`to_date`] accepts.
#[derive(Debug, Clone, Copy)]
pub struct DateArg(pub Date);

impl FromPyObject<'_, '_> for DateArg {
    type Error = PyErr;

    fn extract(ob: Borrowed<'_, '_, PyAny>) -> Result<Self, Self::Error> {
        to_date(&ob).map(Self)
    }
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
