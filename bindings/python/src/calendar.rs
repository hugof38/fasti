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
fn builtin(name: &str) -> Option<fasti::Calendar<'static>> {
    let key = normalize(name);
    let canonical = ALIASES
        .iter()
        .find(|(alias, _)| *alias == key)
        .map_or(key.clone(), |(_, target)| normalize(target));
    BUILTINS
        .iter()
        .find(|(n, _)| normalize(n) == canonical)
        .map(|(_, cal)| *cal)
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
}

impl Calendar {
    pub fn view(&self) -> fasti::Calendar<'_> {
        self.builder.view()
    }

    fn from_builder(builder: fasti::CalendarBuilder) -> Self {
        Self { builder }
    }

    pub fn wrap(cal: fasti::Calendar<'_>) -> Self {
        Self::from_builder(fasti::CalendarBuilder::from_calendar(cal))
    }
}

#[pymethods]
impl Calendar {
    /// Load a built-in calendar by name, e.g. `"US.SETTLEMENT"`,
    /// `"TARGET"`, `"nyse"`. Matching ignores case and punctuation.
    #[new]
    fn py_new(name: &str) -> PyResult<Self> {
        builtin(name).map(Self::wrap).ok_or_else(|| {
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
    /// >>> cal = fasti.Calendar.custom(
    /// ...     "Acme",
    /// ...     weekend=["sat", "sun"],
    /// ...     rules=[fasti.Rule.fixed("Jan", 1, shift="forward")],
    /// ...     holidays=["2026-08-15"],
    /// ... )
    /// >>> cal.is_holiday("2026-08-15")
    /// True
    #[staticmethod]
    #[pyo3(signature = (name, *, weekend=None, rules=None, holidays=None))]
    fn custom(
        name: &str,
        weekend: Option<WeekendArg>,
        rules: Option<Vec<PyRef<'_, Rule>>>,
        holidays: Option<Vec<DateArg>>,
    ) -> Self {
        let weekend = weekend.map_or(fasti::Weekend::SAT_SUN, |w| w.0);
        let mut builder = fasti::CalendarBuilder::new(name, weekend);
        for rule in rules.into_iter().flatten() {
            builder = builder.with_rule(rule.inner);
        }
        for date in holidays.into_iter().flatten() {
            builder = builder.with_rule(fasti::Rule::OneOff(fasti::OneOff::new(date.0)));
        }
        Self::from_builder(builder)
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
    fn business_days(&self, start: DateArg, end: DateArg) -> Vec<DateOut> {
        self.view()
            .business_days(start.0..end.0)
            .map(DateOut)
            .collect()
    }

    /// How many business days are in `[start, end)`.
    fn count_business_days(&self, start: DateArg, end: DateArg) -> usize {
        self.view().business_days(start.0..end.0).count()
    }

    /// The holidays in `[start, end)`, ascending. Weekends are not
    /// included; substitute days are.
    fn holidays(&self, start: DateArg, end: DateArg) -> Vec<DateOut> {
        self.view().holidays(start.0..end.0).map(DateOut).collect()
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
    fn union(&self, other: &Self) -> Self {
        Self::from_builder(self.builder.clone().union(other.view()))
    }

    /// This calendar plus extra one-off holidays.
    fn with_holidays(&self, holidays: Vec<DateArg>) -> Self {
        let mut builder = self.builder.clone();
        for date in holidays {
            builder = builder.with_rule(fasti::Rule::OneOff(fasti::OneOff::new(date.0)));
        }
        Self::from_builder(builder)
    }

    /// This calendar plus extra holiday rules.
    fn with_rules(&self, rules: Vec<PyRef<'_, Rule>>) -> Self {
        let mut builder = self.builder.clone();
        for rule in rules {
            builder = builder.with_rule(rule.inner);
        }
        Self::from_builder(builder)
    }

    /// This calendar under a different name.
    fn renamed(&self, name: &str) -> Self {
        Self::from_builder(self.builder.clone().name(name))
    }

    /// This calendar with a different weekend.
    fn with_weekend(&self, weekend: WeekendArg) -> Self {
        Self::from_builder(self.builder.clone().with_weekend(weekend.0))
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
