//! [`Rule`] — a holiday rule, as the record of how it was written down.
//!
//! The crate spreads its rules over five builder types under one enum.
//! Python folds each variant into one constructor, because the builder
//! chain (`.shift(...)`, `.years(...)`) is what keyword arguments are.

use fasti::{
    Date, EasterOffset, FixedDate, LastWeekday, Month, NthWeekday, OneOff, Ordinal, Rule, Year,
    YearRange,
};
use pyo3::IntoPyObjectExt;
use pyo3::prelude::*;
use pyo3::types::{PyTuple, PyTupleMethods};

use crate::convert::{DateArg, FastiError, OrRaise, Reduction, date_out, hook, month, year};
use crate::vocab::{
    EasterMethodArg, PyEasterMethod, PyWeekday, PyWeekendShift, ShiftArg, WeekdayArg,
};

/// A date as the Python expression that builds it.
pub fn py_date_repr(date: Date) -> String {
    let (year, month, day) = date.to_ymd();
    format!("datetime.date({}, {}, {day})", year.get(), month.get())
}

/// The years a rule is active over, canonicalised to explicit bounds so
/// that `None` and the full supported range are the same rule.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Years(u16, u16);

impl Years {
    fn new(bounds: Option<(Option<u16>, Option<u16>)>) -> PyResult<Self> {
        let (from, to) = bounds.unwrap_or((None, None));
        let from = from.map_or(Ok(Year::MIN), year)?;
        let to = to.map_or(Ok(Year::MAX), year)?;
        YearRange::try_between(from, to).or_raise()?;
        Ok(Self(from.get(), to.get()))
    }

    fn range(self) -> YearRange {
        YearRange::literal_between(self.0, self.1)
    }

    /// The keyword argument that produced this, or nothing when it is
    /// the whole supported range.
    fn repr(self) -> String {
        if self == Self(Year::MIN.get(), Year::MAX.get()) {
            String::new()
        } else {
            format!(", years=({}, {})", self.0, self.1)
        }
    }
}

/// How a rule was written down. `Rule` cannot derive `PartialEq` in the
/// crate — `Rule::Custom` holds a function pointer — so equality and
/// hashing compare this record instead.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuleSpec {
    Fixed {
        month: Month,
        day: u8,
        shift: PyWeekendShift,
        years: Years,
    },
    NthWeekday {
        n: Ordinal,
        weekday: PyWeekday,
        month: Month,
        years: Years,
    },
    LastWeekday {
        weekday: PyWeekday,
        month: Month,
        years: Years,
    },
    Easter {
        days: i16,
        method: PyEasterMethod,
        years: Years,
    },
    OneOff {
        date: Date,
    },
}

impl RuleSpec {
    /// The crate rule this record describes.
    pub fn rule(self) -> Rule {
        match self {
            Self::Fixed {
                month,
                day,
                shift,
                years,
            } => Rule::Fixed(
                FixedDate::new(month, day)
                    .shift(shift.0)
                    .years(years.range()),
            ),
            Self::NthWeekday {
                n,
                weekday,
                month,
                years,
            } => Rule::NthWeekday(NthWeekday::new(n, weekday.0, month).years(years.range())),
            Self::LastWeekday {
                weekday,
                month,
                years,
            } => Rule::LastWeekday(LastWeekday::new(weekday.0, month).years(years.range())),
            Self::Easter {
                days,
                method,
                years,
            } => Rule::Easter(
                match method.0 {
                    fasti::EasterMethod::Western => EasterOffset::new(days),
                    fasti::EasterMethod::Orthodox => EasterOffset::new_orthodox(days),
                }
                .years(years.range()),
            ),
            Self::OneOff { date } => Rule::OneOff(OneOff::new(date)),
        }
    }
}

/// A holiday rule, matching a holiday's natural date.
///
/// A fixed-date rule matches that date even when it lands on a weekend;
/// the substitute day, if the shift grants one, is resolved by the
/// calendar, which alone can see what the other rules have taken.
///
/// `Rule.Custom` has no counterpart here: the crate's escape hatch is a
/// bare `fn(Date) -> bool` pointer, which no Python callable can be.
///
/// >>> import datetime
/// >>> from fasti import Rule
/// >>> independence_day = Rule.fixed(7, 4, shift="sat_back_sun_forward")
/// >>> independence_day.is_holiday(datetime.date(2026, 7, 4))
/// True
/// >>> thanksgiving = Rule.nth_weekday(4, "thu", 11)
/// >>> thanksgiving.is_holiday(datetime.date(2026, 11, 26))
/// True
#[pyclass(frozen, eq, hash, from_py_object, module = "fasti", name = "Rule")]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PyRule(pub RuleSpec);

#[pymethods]
impl PyRule {
    /// A holiday on the same month and day each year.
    ///
    /// `shift` says which way it moves when the natural date lands on a
    /// weekend; `years` restricts it to `(from, to)` inclusive, either
    /// end open.
    #[staticmethod]
    #[pyo3(signature = (month, day, *, shift=None, years=None))]
    fn fixed(
        month: u8,
        day: u8,
        shift: Option<ShiftArg>,
        years: Option<(Option<u16>, Option<u16>)>,
    ) -> PyResult<Self> {
        let month = self::month(month)?;
        // 2000 is a leap year, so this admits February 29 and no more.
        if day < 1 || day > month.length(Year::literal(2000)) {
            return Err(FastiError::new_err(format!(
                "day out of range for the given month and year: {month} has no day {day}"
            )));
        }
        Ok(Self(RuleSpec::Fixed {
            month,
            day,
            shift: shift.map_or(PyWeekendShift(fasti::WeekendShift::None), |s| s.0),
            years: Years::new(years)?,
        }))
    }

    /// A holiday on the `n`-th given weekday of a month, `n` in 1..=5.
    #[staticmethod]
    #[pyo3(signature = (n, weekday, month, *, years=None))]
    fn nth_weekday(
        n: u8,
        weekday: WeekdayArg,
        month: u8,
        years: Option<(Option<u16>, Option<u16>)>,
    ) -> PyResult<Self> {
        Ok(Self(RuleSpec::NthWeekday {
            n: Ordinal::try_from_u8(n).or_raise()?,
            weekday: weekday.0,
            month: self::month(month)?,
            years: Years::new(years)?,
        }))
    }

    /// A holiday on the last given weekday of a month.
    #[staticmethod]
    #[pyo3(signature = (weekday, month, *, years=None))]
    fn last_weekday(
        weekday: WeekdayArg,
        month: u8,
        years: Option<(Option<u16>, Option<u16>)>,
    ) -> PyResult<Self> {
        Ok(Self(RuleSpec::LastWeekday {
            weekday: weekday.0,
            month: self::month(month)?,
            years: Years::new(years)?,
        }))
    }

    /// A holiday a fixed number of days from Easter Sunday.
    ///
    /// Good Friday is -2, Easter Monday 1, Ascension 39, Whit Monday 50,
    /// Corpus Christi 60 — the crate's named constructors, as offsets.
    ///
    /// >>> import datetime
    /// >>> from fasti import Rule
    /// >>> Rule.easter(-2).is_holiday(datetime.date(2026, 4, 3))
    /// True
    #[staticmethod]
    #[pyo3(signature = (days, *, method=None, years=None))]
    fn easter(
        days: i16,
        method: Option<EasterMethodArg>,
        years: Option<(Option<u16>, Option<u16>)>,
    ) -> PyResult<Self> {
        Ok(Self(RuleSpec::Easter {
            days,
            method: method.map_or(PyEasterMethod(fasti::EasterMethod::Western), |m| m.0),
            years: Years::new(years)?,
        }))
    }

    /// A holiday on one specific date and no other.
    #[staticmethod]
    fn one_off(date: DateArg) -> Self {
        Self(RuleSpec::OneOff { date: date.0 })
    }

    /// True if this rule marks `date` as a holiday on its natural date.
    fn is_holiday(&self, date: DateArg) -> bool {
        self.0.rule().is_holiday(date.0)
    }

    pub fn __repr__(&self) -> String {
        match self.0 {
            RuleSpec::Fixed {
                month,
                day,
                shift,
                years,
            } => format!(
                "Rule.fixed({}, {day}, shift={}{})",
                month.get(),
                shift.__repr__(),
                years.repr()
            ),
            RuleSpec::NthWeekday {
                n,
                weekday,
                month,
                years,
            } => format!(
                "Rule.nth_weekday({}, {}, {}{})",
                n.get(),
                weekday.__repr__(),
                month.get(),
                years.repr()
            ),
            RuleSpec::LastWeekday {
                weekday,
                month,
                years,
            } => format!(
                "Rule.last_weekday({}, {}{})",
                weekday.__repr__(),
                month.get(),
                years.repr()
            ),
            RuleSpec::Easter {
                days,
                method,
                years,
            } => format!(
                "Rule.easter({days}, method={}{})",
                method.__repr__(),
                years.repr()
            ),
            RuleSpec::OneOff { date } => format!("Rule.one_off({})", py_date_repr(date)),
        }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<Reduction<'py, (Bound<'py, PyTuple>,)>> {
        let state: Vec<Bound<'py, PyAny>> = match self.0 {
            RuleSpec::Fixed {
                month,
                day,
                shift,
                years,
            } => vec![
                "fixed".into_bound_py_any(py)?,
                month.get().into_bound_py_any(py)?,
                day.into_bound_py_any(py)?,
                shift.canonical().into_bound_py_any(py)?,
                years.0.into_bound_py_any(py)?,
                years.1.into_bound_py_any(py)?,
            ],
            RuleSpec::NthWeekday {
                n,
                weekday,
                month,
                years,
            } => vec![
                "nth_weekday".into_bound_py_any(py)?,
                n.get().into_bound_py_any(py)?,
                weekday.canonical().into_bound_py_any(py)?,
                month.get().into_bound_py_any(py)?,
                years.0.into_bound_py_any(py)?,
                years.1.into_bound_py_any(py)?,
            ],
            RuleSpec::LastWeekday {
                weekday,
                month,
                years,
            } => vec![
                "last_weekday".into_bound_py_any(py)?,
                weekday.canonical().into_bound_py_any(py)?,
                month.get().into_bound_py_any(py)?,
                years.0.into_bound_py_any(py)?,
                years.1.into_bound_py_any(py)?,
            ],
            RuleSpec::Easter {
                days,
                method,
                years,
            } => vec![
                "easter".into_bound_py_any(py)?,
                days.into_bound_py_any(py)?,
                method.canonical().into_bound_py_any(py)?,
                years.0.into_bound_py_any(py)?,
                years.1.into_bound_py_any(py)?,
            ],
            RuleSpec::OneOff { date } => {
                vec!["one_off".into_bound_py_any(py)?, date_out(py, date)?]
            }
        };
        Ok((hook(py, "_rebuild_rule")?, (PyTuple::new(py, state)?,)))
    }
}

/// Replay a pickled rule from the arguments it was built with.
#[pyfunction]
pub fn _rebuild_rule(state: &Bound<'_, PyTuple>) -> PyResult<PyRule> {
    let at = |i: usize| state.get_item(i);
    let years = |i: usize| -> PyResult<Option<(Option<u16>, Option<u16>)>> {
        Ok(Some((Some(at(i)?.extract()?), Some(at(i + 1)?.extract()?))))
    };
    match at(0)?.extract::<String>()?.as_str() {
        "fixed" => PyRule::fixed(
            at(1)?.extract()?,
            at(2)?.extract()?,
            Some(at(3)?.extract()?),
            years(4)?,
        ),
        "nth_weekday" => PyRule::nth_weekday(
            at(1)?.extract()?,
            at(2)?.extract()?,
            at(3)?.extract()?,
            years(4)?,
        ),
        "last_weekday" => PyRule::last_weekday(at(1)?.extract()?, at(2)?.extract()?, years(3)?),
        "easter" => PyRule::easter(at(1)?.extract()?, Some(at(2)?.extract()?), years(3)?),
        "one_off" => Ok(PyRule::one_off(at(1)?.extract()?)),
        kind => Err(FastiError::new_err(format!("unknown rule kind {kind:?}"))),
    }
}
