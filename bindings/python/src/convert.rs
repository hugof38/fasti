//! The Python boundary: dates in and out, year fractions out, errors.
//!
//! Dates cross as [`datetime.date`] and nothing else. A `datetime`
//! is refused because which day a moment falls on depends on a time
//! zone and fasti has none; a `str` is refused because parsing is the
//! caller's business. Year fractions cross as [`fractions.Fraction`]:
//! the crate is float-free and that has to survive the boundary.
//!
//! [`datetime.date`]: https://docs.python.org/3/library/datetime.html#date-objects
//! [`fractions.Fraction`]: https://docs.python.org/3/library/fractions.html

use fasti::{Date, Fraction, Month, TimeError, Year};
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyList, PyModule, PyString, PyType};
use pyo3::{Borrowed, create_exception};

create_exception!(
    fasti,
    FastiError,
    pyo3::exceptions::PyValueError,
    "Every refusal fasti's `TimeError` names, raised as a `ValueError`.\n\n\
     Type mistakes raise `TypeError` instead."
);

/// `datetime.date(1901, 1, 1).toordinal()` — the proleptic Gregorian
/// ordinal of fasti's serial 0, so serial and ordinal differ by a
/// constant and the conversion is one subtraction either way.
const EPOCH_ORDINAL: i64 = 693_961;

static DATE: PyOnceLock<Py<PyType>> = PyOnceLock::new();
static DATETIME: PyOnceLock<Py<PyType>> = PyOnceLock::new();
static FRACTION: PyOnceLock<Py<PyType>> = PyOnceLock::new();
static FROMORDINAL: PyOnceLock<Py<PyAny>> = PyOnceLock::new();
static EXTENSION: PyOnceLock<Py<PyModule>> = PyOnceLock::new();

/// abi3 exposes no datetime C API, so the types are imported once and
/// cached rather than reached through `PyDateTime_IMPORT`.
fn date_type(py: Python<'_>) -> PyResult<&Bound<'_, PyType>> {
    DATE.import(py, "datetime", "date")
}

fn datetime_type(py: Python<'_>) -> PyResult<&Bound<'_, PyType>> {
    DATETIME.import(py, "datetime", "datetime")
}

/// The name to show a caller who passed the wrong type.
fn type_name(obj: Borrowed<'_, '_, PyAny>) -> String {
    obj.get_type()
        .name()
        .map_or_else(|_| "<unknown>".to_owned(), |n| n.to_string())
}

/// A module-level `_rebuild_*` function, by name — what `__reduce__`
/// hands back to pickle along with the arguments to replay.
pub fn hook<'py>(py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    EXTENSION.import(py, "fasti", "_fasti")?.getattr(name)
}

/// What `__reduce__` hands pickle: the `_rebuild_*` hook, and `A`, the
/// arguments to replay.
pub type Reduction<'py, A> = (Bound<'py, PyAny>, A);

/// The reduction shape shared by the values that replay an origin plus
/// the operations applied to it.
pub type ReplayReduction<'py> = Reduction<'py, (Bound<'py, PyAny>, Bound<'py, PyList>)>;

/// A `datetime.date` argument, converted to a [`Date`] on extraction.
pub struct DateArg(pub Date);

impl<'py> FromPyObject<'_, 'py> for DateArg {
    type Error = PyErr;

    fn extract(obj: Borrowed<'_, 'py, PyAny>) -> PyResult<Self> {
        let py = obj.py();
        // datetime subclasses date, so it has to be checked first.
        if obj.is_instance(datetime_type(py)?.as_any())? {
            return Err(PyTypeError::new_err(
                "fasti takes a datetime.date, not a datetime.datetime: call .date() on it. \
                 Which day a moment falls on is a time-zone decision, and fasti has no time zones.",
            ));
        }
        if obj.is_instance(date_type(py)?.as_any())? {
            let ordinal: i64 = obj
                .call_method0(pyo3::intern!(py, "toordinal"))?
                .extract()?;
            return u32::try_from(ordinal - EPOCH_ORDINAL)
                .map_err(|_| TimeError::DateOutOfRange)
                .and_then(Date::from_serial)
                .map(Self)
                .or_raise();
        }
        if obj.is_instance_of::<PyString>() {
            return Err(PyTypeError::new_err(
                "fasti takes a datetime.date, not a str: \
                 parse it first with datetime.date.fromisoformat(...).",
            ));
        }
        Err(PyTypeError::new_err(format!(
            "fasti takes a datetime.date, got {}.",
            type_name(obj)
        )))
    }
}

/// A date operand for an operator: `None` when the object is not a date
/// at all, so the caller can return `NotImplemented` and let Python say
/// "unsupported operand type(s)" the way it says it everywhere else.
///
/// A `datetime.datetime` is not "not a date" — it is the near miss this
/// boundary exists to catch, so it is still refused by name.
pub fn date_operand(obj: &Bound<'_, PyAny>) -> PyResult<Option<Date>> {
    let py = obj.py();
    // The date branch also catches a datetime, and refuses it by name.
    if obj.is_instance(datetime_type(py)?.as_any())? || obj.is_instance(date_type(py)?.as_any())? {
        return DateArg::extract(obj.as_borrowed()).map(|date| Some(date.0));
    }
    Ok(None)
}

/// A [`Date`] as a `datetime.date`.
pub fn date_out(py: Python<'_>, date: Date) -> PyResult<Bound<'_, PyAny>> {
    FROMORDINAL
        .get_or_try_init(py, || {
            date_type(py)?.getattr("fromordinal").map(Bound::unbind)
        })?
        .bind(py)
        .call1((i64::from(date.serial()) + EPOCH_ORDINAL,))
}

/// A run of [`Date`]s as a `list[datetime.date]`.
pub fn dates_out<'py>(py: Python<'py>, dates: &[Date]) -> PyResult<Vec<Bound<'py, PyAny>>> {
    dates.iter().map(|d| date_out(py, *d)).collect()
}

/// A half-open `Range<Date>` as a `tuple[datetime.date, datetime.date]`.
pub fn period_out<'py>(
    py: Python<'py>,
    period: &core::ops::Range<Date>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    Ok((date_out(py, period.start)?, date_out(py, period.end)?))
}

/// A [`Fraction`] as a `fractions.Fraction`. Never a float.
pub fn fraction_out(py: Python<'_>, value: Fraction) -> PyResult<Bound<'_, PyAny>> {
    FRACTION
        .import(py, "fractions", "Fraction")?
        .call1((value.numerator(), value.denominator()))
}

/// A year number as a [`Year`].
pub fn year(value: u16) -> PyResult<Year> {
    Year::new(value).or_raise()
}

/// A month number as a [`Month`].
pub fn month(value: u8) -> PyResult<Month> {
    Month::try_from_u8(value).or_raise()
}

/// Turn a [`TimeError`] into the [`FastiError`] that names it.
pub trait OrRaise<T> {
    /// Raise `FastiError` carrying the `TimeError`'s own message.
    fn or_raise(self) -> PyResult<T>;
}

impl<T> OrRaise<T> for Result<T, TimeError> {
    fn or_raise(self) -> PyResult<T> {
        self.map_err(|e| FastiError::new_err(e.to_string()))
    }
}
