//! [`Period`] — a signed duration tagged by its calendar unit.

use fasti::{Frequency, Period, TimeError};
use pyo3::Borrowed;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;

use crate::convert::{OrRaise, Reduction, hook};
use crate::vocab::{FrequencyArg, PyFrequency};

/// A signed duration tagged by its calendar unit: days, weeks, months
/// or years. The unit is part of the value — three months is not
/// ninety days, and only a calendar can say what it is.
///
/// >>> from fasti import Frequency, Period
/// >>> Period.months(3)
/// Period.months(3)
/// >>> str(Period.months(12)), str(Period.months(12).normalized())
/// ('12M', '1Y')
/// >>> Period.from_frequency(Frequency.QUARTERLY) == Period.months(3)
/// True
#[pyclass(
    frozen,
    eq,
    hash,
    skip_from_py_object,
    module = "fasti",
    name = "Period"
)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PyPeriod(pub Period);

#[pymethods]
impl PyPeriod {
    /// The zero period, `0 Days`.
    #[classattr]
    #[allow(non_snake_case)]
    fn ZERO() -> Self {
        Self(Period::ZERO)
    }

    /// `n` calendar days.
    #[staticmethod]
    fn days(n: i32) -> Self {
        Self(Period::Days(n))
    }

    /// `n` weeks — seven calendar days, no calendar dependency.
    #[staticmethod]
    fn weeks(n: i32) -> Self {
        Self(Period::Weeks(n))
    }

    /// `n` calendar months, of variable length.
    #[staticmethod]
    fn months(n: i32) -> Self {
        Self(Period::Months(n))
    }

    /// `n` calendar years, of variable length.
    #[staticmethod]
    fn years(n: i32) -> Self {
        Self(Period::Years(n))
    }

    /// The period canonical to `frequency`. `Annual` maps to 12 months;
    /// call `normalized` for one year.
    ///
    /// >>> from fasti import Frequency, Period
    /// >>> str(Period.from_frequency("annual"))
    /// '12M'
    #[staticmethod]
    fn from_frequency(frequency: FrequencyArg) -> Self {
        Self(Period::from(frequency.get()))
    }

    /// The signed length component.
    fn length(&self) -> i32 {
        self.0.length()
    }

    /// The unit the length is counted in: the name of the enum variant,
    /// which Python cannot pattern-match its way to.
    #[getter]
    pub fn unit(&self) -> &'static str {
        match self.0 {
            Period::Days(_) => "days",
            Period::Weeks(_) => "weeks",
            Period::Months(_) => "months",
            Period::Years(_) => "years",
        }
    }

    /// True if the length is zero, whatever the unit.
    fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    /// The same duration in the largest unit that divides it exactly.
    fn normalized(&self) -> Self {
        Self(self.0.normalized())
    }

    /// The canonical frequency, or `FastiError` if the period is not
    /// one — five months names no frequency.
    ///
    /// >>> from fasti import Period
    /// >>> Period.months(6).frequency()
    /// Frequency.SEMIANNUAL
    fn frequency(&self) -> PyResult<PyFrequency> {
        Frequency::try_from(self.0).map(PyFrequency).or_raise()
    }

    /// Negate the length; raises on `i32` overflow, where the crate's
    /// `Neg` wraps and `checked_neg` reports.
    fn __neg__(&self) -> PyResult<Self> {
        self.0
            .checked_neg()
            .map(Self)
            .ok_or(TimeError::DateOutOfRange)
            .or_raise()
    }

    /// Scale the length, preserving the unit; raises on overflow.
    fn __mul__(&self, n: i32) -> PyResult<Self> {
        self.0
            .checked_mul(n)
            .map(Self)
            .ok_or(TimeError::DateOutOfRange)
            .or_raise()
    }

    fn __rmul__(&self, n: i32) -> PyResult<Self> {
        self.__mul__(n)
    }

    /// QuantLib's spelling: `3M`, `1Y`, `2W`, `14D`.
    fn __str__(&self) -> String {
        self.0.to_string()
    }

    pub fn __repr__(&self) -> String {
        format!("Period.{}({})", self.unit(), self.0.length())
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<Reduction<'py, (&'static str, i32)>> {
        Ok((hook(py, "_rebuild_period")?, (self.unit(), self.0.length())))
    }
}

/// A `Period` argument: a period, a frequency, or a spelling of one.
pub struct PeriodArg(pub Period);

impl<'py> FromPyObject<'_, 'py> for PeriodArg {
    type Error = PyErr;

    fn extract(obj: Borrowed<'_, 'py, PyAny>) -> PyResult<Self> {
        if let Ok(period) = obj.cast::<PyPeriod>() {
            return Ok(Self(period.get().0));
        }
        // A frequency names its canonical period, as `with_frequency` does.
        if let Ok(frequency) = obj.extract::<FrequencyArg>() {
            return Ok(Self(Period::from(frequency.get())));
        }
        Err(PyTypeError::new_err(
            "expected a Period, a Frequency, or a str naming a frequency.",
        ))
    }
}

/// Replay a pickled period.
#[pyfunction]
pub fn _rebuild_period(unit: &str, length: i32) -> PyResult<PyPeriod> {
    match unit {
        "days" => Ok(PyPeriod::days(length)),
        "weeks" => Ok(PyPeriod::weeks(length)),
        "months" => Ok(PyPeriod::months(length)),
        "years" => Ok(PyPeriod::years(length)),
        _ => Err(PyTypeError::new_err(format!(
            "unknown period unit {unit:?}"
        ))),
    }
}
