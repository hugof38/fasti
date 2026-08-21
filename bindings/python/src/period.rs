//! [`Period`] — a signed length tagged by its calendar unit, and the
//! parsing that lets `"6M"`, `"6 months"`, `"semiannual"` and
//! `datetime.timedelta(days=7)` all stand in for one.

use pyo3::prelude::*;
use pyo3::{Borrowed, exceptions::PyTypeError, intern};

use crate::convert::{is_timedelta, normalize, type_name};
use crate::enums::Frequency;
use crate::error::invalid;

/// A signed duration tagged by its calendar unit: days, weeks, months,
/// or years.
///
/// Months and years are calendar units, not fixed day counts — adding
/// `Period("1M")` to January 31 lands on the end of February, not on
/// March 3.
///
/// >>> from fasti import Period
/// >>> Period("6M")
/// Period('6M')
/// >>> Period(months=6) == Period("6m") == Period("6 months")
/// True
/// >>> -Period("3M") * 2
/// Period('-6M')
#[pyclass(module = "fasti", from_py_object, frozen, eq, hash, str)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Period(pub fasti::Period);

#[pymethods]
impl Period {
    /// Build a period from a string (`"6M"`), a `datetime.timedelta`,
    /// another `Period`, or exactly one unit keyword.
    #[new]
    #[pyo3(signature = (spec=None, *, days=None, weeks=None, months=None, years=None))]
    fn new(
        spec: Option<PeriodArg>,
        days: Option<i32>,
        weeks: Option<i32>,
        months: Option<i32>,
        years: Option<i32>,
    ) -> PyResult<Self> {
        let units = [
            days.map(fasti::Period::Days),
            weeks.map(fasti::Period::Weeks),
            months.map(fasti::Period::Months),
            years.map(fasti::Period::Years),
        ];
        let mut given = units.into_iter().flatten();
        match (spec, given.next(), given.next()) {
            (Some(_), Some(_), _) | (_, Some(_), Some(_)) => Err(invalid(
                "give a period as one value or exactly one of days=/weeks=/months=/years=",
            )),
            (Some(p), None, _) => Ok(Self(p.0)),
            (None, Some(p), None) => Ok(Self(p)),
            (None, None, _) => Err(invalid(
                "Period() needs a value, e.g. Period('6M') or Period(months=6)",
            )),
        }
    }

    /// Parse a period string: `"6M"`, `"-3d"`, `"1 year"`, `"quarterly"`.
    #[staticmethod]
    fn parse(text: &str) -> PyResult<Self> {
        parse_period_str(text).map(Self)
    }

    /// A period of `n` calendar days.
    #[staticmethod]
    const fn days(n: i32) -> Self {
        Self(fasti::Period::Days(n))
    }

    /// A period of `n` weeks.
    #[staticmethod]
    const fn weeks(n: i32) -> Self {
        Self(fasti::Period::Weeks(n))
    }

    /// A period of `n` calendar months.
    #[staticmethod]
    const fn months(n: i32) -> Self {
        Self(fasti::Period::Months(n))
    }

    /// A period of `n` calendar years.
    #[staticmethod]
    const fn years(n: i32) -> Self {
        Self(fasti::Period::Years(n))
    }

    /// The signed length, without its unit: `Period("6M").length == 6`.
    #[getter]
    const fn length(&self) -> i32 {
        self.0.length()
    }

    /// The unit: `"days"`, `"weeks"`, `"months"`, or `"years"`.
    #[getter]
    const fn unit(&self) -> &'static str {
        match self.0 {
            fasti::Period::Days(_) => "days",
            fasti::Period::Weeks(_) => "weeks",
            fasti::Period::Months(_) => "months",
            fasti::Period::Years(_) => "years",
        }
    }

    /// `True` for a zero-length period, whatever its unit.
    #[getter]
    const fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    /// The canonical `Frequency` this period recurs at, or `None` if it
    /// is not one (`Period("5M")`).
    #[getter]
    fn frequency(&self) -> Option<Frequency> {
        fasti::Frequency::try_from(self.0).ok().map(Frequency::wrap)
    }

    /// The same period in its canonical unit: `12M` becomes `1Y`,
    /// `14D` becomes `2W`.
    const fn normalized(&self) -> Self {
        Self(self.0.normalized())
    }

    fn __neg__(&self) -> PyResult<Self> {
        self.0
            .checked_neg()
            .map(Self)
            .ok_or_else(|| invalid("period length overflowed on negation"))
    }

    fn __mul__(&self, n: i32) -> PyResult<Self> {
        self.0
            .checked_mul(n)
            .map(Self)
            .ok_or_else(|| invalid("period length overflowed on multiplication"))
    }

    fn __rmul__(&self, n: i32) -> PyResult<Self> {
        self.__mul__(n)
    }

    fn __repr__(&self) -> String {
        format!("Period('{}')", self.0)
    }
}

impl std::fmt::Display for Period {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Parse the period spellings this module accepts. Handles the sign
/// itself: `normalize` drops punctuation, so `-3D` has to be split
/// before the rest is folded.
fn parse_period_str(text: &str) -> PyResult<fasti::Period> {
    let trimmed = text.trim();
    let (negate, body) = match trimmed.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, trimmed.strip_prefix('+').unwrap_or(trimmed)),
    };
    let key = normalize(body);
    if key.is_empty() {
        return Err(invalid(format!(
            "cannot parse period {text:?}: it is empty"
        )));
    }
    // Frequency names first: "quarterly" is a period as much as "3M" is.
    if let Some(f) = frequency_by_name(&key) {
        let period = fasti::Period::from(f);
        return signed(period, negate, text);
    }
    // <digits><unit>, with the unit spelled long or short.
    let digits: String = key.chars().take_while(char::is_ascii_digit).collect();
    let unit = &key[digits.len()..];
    if digits.is_empty() {
        return Err(invalid(format!(
            "cannot parse period {text:?}: expected something like '6M', '2 weeks', or 'quarterly'"
        )));
    }
    let n: i32 = digits
        .parse()
        .map_err(|_| invalid(format!("period length out of range in {text:?}")))?;
    let period = match unit {
        "d" | "day" | "days" => fasti::Period::Days(n),
        "w" | "week" | "weeks" => fasti::Period::Weeks(n),
        "m" | "mo" | "month" | "months" => fasti::Period::Months(n),
        "y" | "yr" | "year" | "years" => fasti::Period::Years(n),
        "" => {
            return Err(invalid(format!(
                "period {text:?} has no unit: write '{n}D', '{n}W', '{n}M', or '{n}Y'"
            )));
        }
        _ => {
            return Err(invalid(format!(
                "unknown period unit {unit:?} in {text:?} (expected D, W, M, or Y)"
            )));
        }
    };
    signed(period, negate, text)
}

/// Apply a leading minus sign, refusing the one length that cannot be
/// negated.
fn signed(period: fasti::Period, negate: bool, text: &str) -> PyResult<fasti::Period> {
    if negate {
        period
            .checked_neg()
            .ok_or_else(|| invalid(format!("period length out of range in {text:?}")))
    } else {
        Ok(period)
    }
}

fn frequency_by_name(key: &str) -> Option<fasti::Frequency> {
    use fasti::Frequency as F;
    Some(match key {
        "annual" | "annually" | "yearly" => F::Annual,
        "semiannual" | "semiannually" | "halfyearly" => F::Semiannual,
        "everyfourthmonth" | "triannual" => F::EveryFourthMonth,
        "quarterly" | "quarter" => F::Quarterly,
        "bimonthly" => F::Bimonthly,
        "monthly" => F::Monthly,
        "everyfourthweek" => F::EveryFourthWeek,
        "biweekly" | "fortnightly" => F::Biweekly,
        "weekly" => F::Weekly,
        "daily" => F::Daily,
        _ => return None,
    })
}

/// A period-valued argument: a [`Period`], a string, a
/// `datetime.timedelta` of whole days, or a [`Frequency`].
#[derive(Debug, Clone, Copy)]
pub struct PeriodArg(pub fasti::Period);

impl FromPyObject<'_, '_> for PeriodArg {
    type Error = PyErr;

    fn extract(ob: Borrowed<'_, '_, PyAny>) -> Result<Self, Self::Error> {
        let py = ob.py();
        if let Ok(p) = ob.extract::<Period>() {
            return Ok(Self(p.0));
        }
        if let Ok(f) = ob.extract::<Frequency>() {
            return Ok(Self(fasti::Period::from(f.inner())));
        }
        if let Ok(text) = ob.extract::<String>() {
            return parse_period_str(&text).map(Self);
        }
        if is_timedelta(&ob)? {
            let days = ob.getattr(intern!(py, "days"))?.extract::<i64>()?;
            let seconds = ob.getattr(intern!(py, "seconds"))?.extract::<i64>()?;
            let micros = ob.getattr(intern!(py, "microseconds"))?.extract::<i64>()?;
            if seconds != 0 || micros != 0 {
                return Err(invalid(
                    "a timedelta used as a period must be a whole number of days",
                ));
            }
            let days =
                i32::try_from(days).map_err(|_| invalid("timedelta is too long to be a period"))?;
            return Ok(Self(fasti::Period::Days(days)));
        }
        Err(PyTypeError::new_err(format!(
            "expected a Period, a period string like '6M', a Frequency, or a \
             datetime.timedelta, got {}",
            type_name(&ob)
        )))
    }
}
