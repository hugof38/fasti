//! [`Calendar`] — a weekend plus a set of holiday rules, and the
//! registry of built-in market calendars.

use pyo3::prelude::*;
use pyo3::types::PyType;

use crate::convert::{DateArg, DateOut, normalize};
use crate::enums::{ConventionArg, Weekday, WeekendArg, weekend_days, weekend_repr};
use crate::error::{err, invalid};
use crate::period::PeriodArg;
use crate::rules::Rule;

/// The built-in calendars, by canonical name.
const BUILTINS: &[(&str, fasti::Calendar<'static>)] = &[
    ("TARGET", fasti::calendars::TARGET),
    ("US.SETTLEMENT", fasti::calendars::us::SETTLEMENT),
    ("US.NYSE", fasti::calendars::us::NYSE),
    ("US.GOVERNMENT_BOND", fasti::calendars::us::GOVERNMENT_BOND),
    ("US.FEDERAL_RESERVE", fasti::calendars::us::FEDERAL_RESERVE),
    ("US.SOFR", fasti::calendars::us::SOFR),
    ("US.NERC", fasti::calendars::us::NERC),
    ("UK.SETTLEMENT", fasti::calendars::uk::SETTLEMENT),
    ("FRANCE.SETTLEMENT", fasti::calendars::france::SETTLEMENT),
    ("FRANCE.EXCHANGE", fasti::calendars::france::EXCHANGE),
    ("WEEKENDS_ONLY", fasti::calendars::WEEKENDS_ONLY),
    ("NULL", fasti::calendars::NULL_CALENDAR),
];

/// Short spellings that resolve to a canonical name.
const ALIASES: &[(&str, &str)] = &[
    ("us", "US.SETTLEMENT"),
    ("nyse", "US.NYSE"),
    ("sofr", "US.SOFR"),
    ("fed", "US.FEDERAL_RESERVE"),
    ("federalreserve", "US.FEDERAL_RESERVE"),
    ("governmentbond", "US.GOVERNMENT_BOND"),
    ("nerc", "US.NERC"),
    ("uk", "UK.SETTLEMENT"),
    ("gb", "UK.SETTLEMENT"),
    ("france", "FRANCE.SETTLEMENT"),
    ("fr", "FRANCE.SETTLEMENT"),
    ("eur", "TARGET"),
    ("nullcalendar", "NULL"),
    ("none", "NULL"),
    ("weekends", "WEEKENDS_ONLY"),
];

/// Look a built-in calendar up by name, ignoring case and punctuation.
/// Returns the canonical name alongside it, which is what a pickled
/// calendar records.
fn builtin(name: &str) -> Option<(&'static str, fasti::Calendar<'static>)> {
    let key = normalize(name);
    let canonical = ALIASES
        .iter()
        .find(|(alias, _)| *alias == key)
        .map_or(key.clone(), |(_, target)| normalize(target));
    BUILTINS
        .iter()
        .find(|(n, _)| normalize(n) == canonical)
        .map(|(n, cal)| (*n, *cal))
}

/// How a calendar was arrived at, so that pickling can arrive at it
/// again. Built-ins carry `Rule::Custom` predicates — fn pointers with
/// no data to serialize — so a built-in is recorded by name and rebuilt
/// from the registry, never rule by rule.
#[derive(Debug, Clone)]
pub enum Origin {
    /// A registry calendar, by canonical name.
    Builtin(&'static str),
    /// Built from scratch by `Calendar.custom`.
    Custom {
        name: String,
        weekend: fasti::Weekend,
        rules: Vec<Rule>,
    },
    /// `renamed`, `with_weekend`, `union`, and the rule-adding methods,
    /// each recorded against what they were applied to.
    Renamed(Box<Origin>, String),
    Weekend(Box<Origin>, fasti::Weekend),
    Union(Box<Origin>, Box<Origin>),
    Rules(Box<Origin>, Vec<Rule>),
}

impl Origin {
    /// Encode as nested tuples of picklable values. Rules and dates
    /// inside pickle themselves.
    pub fn encode<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let weekend = |w: fasti::Weekend| {
            weekend_days(w)
                .into_iter()
                .map(|d| d.inner().get())
                .collect::<Vec<_>>()
        };
        let object = match self {
            Self::Builtin(name) => ("builtin", *name).into_pyobject(py)?.into_any(),
            Self::Custom {
                name,
                weekend: w,
                rules,
            } => ("custom", name, weekend(*w), rules.clone())
                .into_pyobject(py)?
                .into_any(),
            Self::Renamed(inner, name) => ("renamed", inner.encode(py)?, name)
                .into_pyobject(py)?
                .into_any(),
            Self::Weekend(inner, w) => ("weekend", inner.encode(py)?, weekend(*w))
                .into_pyobject(py)?
                .into_any(),
            Self::Union(left, right) => ("union", left.encode(py)?, right.encode(py)?)
                .into_pyobject(py)?
                .into_any(),
            Self::Rules(inner, rules) => ("rules", inner.encode(py)?, rules.clone())
                .into_pyobject(py)?
                .into_any(),
        };
        Ok(object)
    }
}

/// Rebuild a calendar from what [`Origin::encode`] produced, by
/// replaying the same public constructors.
pub fn rebuild(spec: &Bound<'_, PyAny>) -> PyResult<Calendar> {
    let tag: String = spec.get_item(0)?.extract()?;
    let weekend = |item: Bound<'_, PyAny>| -> PyResult<WeekendArg> { item.extract() };
    match tag.as_str() {
        "builtin" => Calendar::py_new(&spec.get_item(1)?.extract::<String>()?),
        "custom" => Ok(Calendar::custom(
            &spec.get_item(1)?.extract::<String>()?,
            Some(weekend(spec.get_item(2)?)?),
            Some(spec.get_item(3)?.extract::<Vec<Rule>>()?),
            None,
        )),
        "renamed" => {
            Ok(rebuild(&spec.get_item(1)?)?.renamed(&spec.get_item(2)?.extract::<String>()?))
        }
        "weekend" => Ok(rebuild(&spec.get_item(1)?)?.with_weekend(weekend(spec.get_item(2)?)?)),
        "union" => Ok(rebuild(&spec.get_item(1)?)?.union(&rebuild(&spec.get_item(2)?)?)),
        "rules" => {
            Ok(rebuild(&spec.get_item(1)?)?.with_rules(spec.get_item(2)?.extract::<Vec<Rule>>()?))
        }
        _ => Err(invalid(format!("unknown calendar spec: {tag:?}"))),
    }
}

/// A holiday calendar: which weekdays are the weekend, and which days
/// are holidays.
///
/// Built-ins are loaded by name; `fasti.calendars` exposes each one as
/// a module attribute as well.
///
/// >>> import datetime, fasti
/// >>> nyse = fasti.Calendar.load("US.NYSE")
/// >>> nyse.is_business_day(datetime.date(2026, 7, 3))
/// False
/// >>> nyse.next_business_day(datetime.date(2026, 7, 3))
/// datetime.date(2026, 7, 6)
#[pyclass(module = "fasti", frozen)]
pub struct Calendar {
    builder: fasti::CalendarBuilder,
    origin: Origin,
}

impl Calendar {
    pub fn view(&self) -> fasti::Calendar<'_> {
        self.builder.view()
    }

    fn from_builder(builder: fasti::CalendarBuilder, origin: Origin) -> Self {
        Self { builder, origin }
    }

    /// How this calendar was arrived at — what a pickled schedule
    /// records so it can rebuild the calendar it was generated against.
    pub fn origin(&self) -> Origin {
        self.origin.clone()
    }
}

#[pymethods]
impl Calendar {
    /// Load a built-in calendar by name, e.g. `"US.SETTLEMENT"`,
    /// `"TARGET"`, `"nyse"`. Matching ignores case and punctuation.
    #[new]
    pub fn py_new(name: &str) -> PyResult<Self> {
        builtin(name)
            .map(|(canonical, cal)| {
                Self::from_builder(
                    fasti::CalendarBuilder::from_calendar(cal),
                    Origin::Builtin(canonical),
                )
            })
            .ok_or_else(|| {
                invalid(format!(
                    "unknown calendar {name:?}; Calendar.names() lists the built-ins"
                ))
            })
    }

    /// Load a built-in calendar by name — the same as `Calendar(name)`.
    #[classmethod]
    fn load(_cls: &Bound<'_, PyType>, name: &str) -> PyResult<Self> {
        Self::py_new(name)
    }

    /// The canonical names of every built-in calendar.
    #[staticmethod]
    fn names() -> Vec<String> {
        BUILTINS.iter().map(|(n, _)| (*n).to_owned()).collect()
    }

    /// Build a calendar from scratch: a name, a weekend, holiday
    /// `rules`, and any number of one-off `holidays`.
    ///
    /// >>> import fasti
    /// >>> from datetime import date
    /// >>> cal = fasti.Calendar.custom(
    /// ...     "Acme",
    /// ...     weekend=["sat", "sun"],
    /// ...     rules=[fasti.Rule.fixed("Jan", 1, shift="forward")],
    /// ...     holidays=[date(2026, 8, 15)],
    /// ... )
    /// >>> cal.is_holiday(date(2026, 8, 15))
    /// True
    #[staticmethod]
    #[pyo3(signature = (name, *, weekend=None, rules=None, holidays=None))]
    pub fn custom(
        name: &str,
        weekend: Option<WeekendArg>,
        rules: Option<Vec<Rule>>,
        holidays: Option<Vec<DateArg>>,
    ) -> Self {
        let weekend = weekend.map_or(fasti::Weekend::SAT_SUN, |w| w.0);
        // One-off holidays are rules; folding them in here keeps a
        // calendar's provenance a single list.
        let rules: Vec<Rule> = rules
            .into_iter()
            .flatten()
            .chain(
                holidays
                    .into_iter()
                    .flatten()
                    .map(|date| Rule::one_off_rule(date.0)),
            )
            .collect();
        let mut builder = fasti::CalendarBuilder::new(name, weekend);
        for rule in &rules {
            builder = builder.with_rule(rule.inner);
        }
        Self::from_builder(
            builder,
            Origin::Custom {
                name: name.to_owned(),
                weekend,
                rules,
            },
        )
    }

    /// The calendar's name.
    #[getter]
    fn name(&self) -> String {
        self.view().name.to_owned()
    }

    /// The weekdays this calendar treats as the weekend.
    #[getter]
    fn weekend(&self) -> Vec<Weekday> {
        weekend_days(self.view().weekend)
    }

    /// `True` iff `date` falls on this calendar's weekend.
    fn is_weekend(&self, date: DateArg) -> bool {
        self.view().is_weekend(date.0)
    }

    /// `True` iff `date` is a holiday — a rule's own date, or the
    /// substitute weekday a weekend holiday is observed on. Weekends
    /// themselves are not holidays.
    fn is_holiday(&self, date: DateArg) -> bool {
        self.view().is_holiday(date.0)
    }

    /// `True` iff `date` is neither a weekend nor a holiday.
    fn is_business_day(&self, date: DateArg) -> bool {
        self.view().is_business_day(date.0)
    }

    /// The business days in `[start, end)`, ascending. The end bound is
    /// excluded, as in `range()` and slicing.
    ///
    /// Walking a range is the one thing here that can take long enough
    /// to matter — a century of NYSE is tens of milliseconds of rule
    /// evaluation — so the interpreter is detached for it and other
    /// Python threads keep running.
    fn business_days(&self, py: Python<'_>, start: DateArg, end: DateArg) -> Vec<DateOut> {
        py.detach(|| {
            self.view()
                .business_days(start.0..end.0)
                .map(DateOut)
                .collect()
        })
    }

    /// How many business days are in `[start, end)`.
    fn count_business_days(&self, py: Python<'_>, start: DateArg, end: DateArg) -> usize {
        py.detach(|| self.view().business_days(start.0..end.0).count())
    }

    /// The holidays in `[start, end)`, ascending. Weekends are not
    /// included; substitute days are.
    fn holidays(&self, py: Python<'_>, start: DateArg, end: DateArg) -> Vec<DateOut> {
        py.detach(|| self.view().holidays(start.0..end.0).map(DateOut).collect())
    }

    /// The next business day strictly after `date`, or `None` past
    /// 2199-12-31.
    fn next_business_day(&self, date: DateArg) -> Option<DateOut> {
        self.view().next_business_day(date.0).map(DateOut)
    }

    /// The previous business day strictly before `date`, or `None`
    /// before 1901-01-01.
    fn prev_business_day(&self, date: DateArg) -> Option<DateOut> {
        self.view().prev_business_day(date.0).map(DateOut)
    }

    /// The first business day of `date`'s month, or `None` if it has
    /// none.
    fn first_business_day_of_month(&self, date: DateArg) -> Option<DateOut> {
        self.view().first_business_day_of_month(date.0).map(DateOut)
    }

    /// The last business day of `date`'s month, or `None` if it has
    /// none.
    fn last_business_day_of_month(&self, date: DateArg) -> Option<DateOut> {
        self.view().last_business_day_of_month(date.0).map(DateOut)
    }

    /// Roll `date` onto a business day under `convention` (default
    /// `"following"`). A business day is returned unchanged.
    #[pyo3(signature = (date, convention=None))]
    fn adjust(&self, date: DateArg, convention: Option<ConventionArg>) -> PyResult<DateOut> {
        let convention = convention.map_or(fasti::BusinessDayConvention::Following, |c| c.0);
        self.view()
            .adjust(date.0, convention)
            .map(DateOut)
            .map_err(err)
    }

    /// Step `date` by `period`, then roll onto a business day under
    /// `convention` (default `"following"`).
    ///
    /// With `end_of_month=True`, a month or year step from a month-end
    /// lands on the target month's end.
    #[pyo3(signature = (date, period, convention=None, *, end_of_month=false))]
    fn advance(
        &self,
        date: DateArg,
        period: PeriodArg,
        convention: Option<ConventionArg>,
        end_of_month: bool,
    ) -> PyResult<DateOut> {
        let convention = convention.map_or(fasti::BusinessDayConvention::Following, |c| c.0);
        self.view()
            .advance(date.0, period.0, convention, end_of_month)
            .map(DateOut)
            .map_err(err)
    }

    /// A calendar closed when either this one or `other` is, with the
    /// weekends unioned — `QuantLib`'s `JointCalendar`.
    pub fn union(&self, other: &Self) -> Self {
        Self::from_builder(
            self.builder.clone().union(other.view()),
            Origin::Union(
                Box::new(self.origin.clone()),
                Box::new(other.origin.clone()),
            ),
        )
    }

    /// This calendar plus extra one-off holidays.
    fn with_holidays(&self, holidays: Vec<DateArg>) -> Self {
        self.with_rules(
            holidays
                .into_iter()
                .map(|date| Rule::one_off_rule(date.0))
                .collect(),
        )
    }

    /// This calendar plus extra holiday rules.
    pub fn with_rules(&self, rules: Vec<Rule>) -> Self {
        let mut builder = self.builder.clone();
        for rule in &rules {
            builder = builder.with_rule(rule.inner);
        }
        Self::from_builder(builder, Origin::Rules(Box::new(self.origin.clone()), rules))
    }

    /// This calendar under a different name.
    pub fn renamed(&self, name: &str) -> Self {
        Self::from_builder(
            self.builder.clone().name(name),
            Origin::Renamed(Box::new(self.origin.clone()), name.to_owned()),
        )
    }

    /// This calendar with a different weekend.
    pub fn with_weekend(&self, weekend: WeekendArg) -> Self {
        Self::from_builder(
            self.builder.clone().with_weekend(weekend.0),
            Origin::Weekend(Box::new(self.origin.clone()), weekend.0),
        )
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<crate::pickle::Reduced<'py>> {
        crate::pickle::reduce(py, "_rebuild_calendar", (self.origin.encode(py)?,))
    }

    fn __repr__(&self) -> String {
        let view = self.view();
        format!(
            "<fasti.Calendar {:?} weekend={} rules={}>",
            view.name,
            weekend_repr(view.weekend),
            view.rules.len()
        )
    }
}
