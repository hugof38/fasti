//! [`Calendar`] — a weekend and a set of holiday rules.
//!
//! `Calendar<'a>` is a borrowed view in the crate, so a Python calendar
//! owns a `CalendarBuilder` and hands out `.view()` per call. It also
//! keeps the record of how it was built: built-in calendars hold
//! function-pointer rules that no value can be compared against, so
//! equality, hashing and pickling all go through that record.

use fasti::{Calendar, CalendarBuilder, Date, Weekday, Weekend, calendars};
use pyo3::IntoPyObjectExt;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyTuple, PyTupleMethods};

use crate::convert::{DateArg, FastiError, OrRaise, ReplayReduction, date_out, dates_out, hook};
use crate::period::PeriodArg;
use crate::rule::{PyRule, RuleSpec};
use crate::vocab::{ConventionArg, PyWeekday, WeekdayArg};

/// Every built-in calendar, under the path it has in the crate.
const REGISTRY: &[(&str, Calendar<'static>)] = &[
    ("NULL_CALENDAR", calendars::NULL_CALENDAR),
    ("TARGET", calendars::TARGET),
    ("WEEKENDS_ONLY", calendars::WEEKENDS_ONLY),
    ("france::EXCHANGE", calendars::france::EXCHANGE),
    ("france::SETTLEMENT", calendars::france::SETTLEMENT),
    ("uk::SETTLEMENT", calendars::uk::SETTLEMENT),
    ("us::FEDERAL_RESERVE", calendars::us::FEDERAL_RESERVE),
    ("us::GOVERNMENT_BOND", calendars::us::GOVERNMENT_BOND),
    ("us::NERC", calendars::us::NERC),
    ("us::NYSE", calendars::us::NYSE),
    ("us::SETTLEMENT", calendars::us::SETTLEMENT),
    ("us::SOFR", calendars::us::SOFR),
];

fn lookup(name: &str) -> PyResult<(&'static str, Calendar<'static>)> {
    REGISTRY
        .iter()
        .find(|(key, _)| *key == name)
        .copied()
        .ok_or_else(|| FastiError::new_err(format!("no built-in calendar named {name:?}")))
}

/// Where a calendar started life.
#[derive(Clone, PartialEq, Eq, Hash)]
enum Origin {
    /// A `pub const Calendar<'static>`, travelling as its registry name.
    Builtin(&'static str),
    /// A calendar written out in Python.
    Fresh(String, Vec<PyWeekday>, Vec<RuleSpec>),
}

/// A derivation applied to a calendar, named as the Python method that
/// applies it.
#[derive(Clone, PartialEq, Eq, Hash)]
enum Op {
    Name(String),
    Weekend(Vec<PyWeekday>),
    Rule(RuleSpec),
    Union(CalendarSpec),
}

/// The record of how a calendar was built.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct CalendarSpec {
    origin: Origin,
    ops: Vec<Op>,
}

fn weekend_of(days: &[PyWeekday]) -> Weekend {
    let days: Vec<Weekday> = days.iter().map(|d| d.0).collect();
    Weekend::from_weekdays(&days)
}

impl CalendarSpec {
    /// Replay the record into the owned builder the view comes from.
    pub fn build(&self) -> PyResult<CalendarBuilder> {
        let mut builder = match &self.origin {
            Origin::Builtin(name) => CalendarBuilder::from_calendar(lookup(name)?.1),
            Origin::Fresh(name, weekend, rules) => {
                let mut builder = CalendarBuilder::new(name.clone(), weekend_of(weekend));
                for rule in rules {
                    builder = builder.with_rule(rule.rule());
                }
                builder
            }
        };
        for op in &self.ops {
            builder = match op {
                Op::Name(name) => builder.name(name.clone()),
                Op::Weekend(weekend) => builder.with_weekend(weekend_of(weekend)),
                Op::Rule(rule) => builder.with_rule(rule.rule()),
                // The other calendar's storage only has to outlive the
                // call: `union` copies the rules out of the view.
                Op::Union(other) => builder.union(other.build()?.view()),
            };
        }
        Ok(builder)
    }

    /// The record as the Python object it describes.
    pub fn py_calendar(&self) -> PyResult<PyCalendar> {
        PyCalendar::from_spec(self.clone())
    }

    fn repr(&self) -> String {
        let mut text = match &self.origin {
            Origin::Builtin(name) => format!("calendars.{}", name.replace("::", ".")),
            Origin::Fresh(name, weekend, rules) => format!(
                "Calendar({name:?}, [{}], [{}])",
                weekend
                    .iter()
                    .map(|d| d.__repr__())
                    .collect::<Vec<_>>()
                    .join(", "),
                rules
                    .iter()
                    .map(|r| PyRule(*r).__repr__())
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
        };
        for op in &self.ops {
            text = match op {
                Op::Name(name) => format!("{text}.with_name({name:?})"),
                Op::Weekend(weekend) => format!(
                    "{text}.with_weekend([{}])",
                    weekend
                        .iter()
                        .map(|d| d.__repr__())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                Op::Rule(rule) => format!("{text}.with_rule({})", PyRule(*rule).__repr__()),
                Op::Union(other) => format!("{text}.union({})", other.repr()),
            };
        }
        text
    }
}

/// A weekend and a set of holiday rules: what makes a day a business day.
///
/// The weekend is a set of weekdays; the rules name each holiday's
/// natural date, and the calendar resolves substitutes, since only it
/// can see what the other rules have already taken.
///
/// Equality is structural — two calendars are equal when they were
/// built the same way. It is not "these two agree on every date":
/// settling that means walking 1901 through 2199.
///
/// >>> import datetime
/// >>> from fasti import Calendar, Rule
/// >>> from fasti.calendars import us
/// >>> us.SETTLEMENT.is_holiday(datetime.date(2024, 7, 4))
/// True
/// >>> Calendar("Acme", ["sat", "sun"], [Rule.fixed(1, 1)]).name
/// 'Acme'
/// >>> shutdown = us.SETTLEMENT.with_rule(Rule.one_off(datetime.date(2026, 8, 3)))
/// >>> shutdown.is_business_day(datetime.date(2026, 8, 3))
/// False
#[pyclass(frozen, eq, hash, module = "fasti", name = "Calendar")]
pub struct PyCalendar {
    spec: CalendarSpec,
    builder: CalendarBuilder,
}

/// Structural equality: same construction, not "agrees on every date".
impl PartialEq for PyCalendar {
    fn eq(&self, other: &Self) -> bool {
        self.spec == other.spec
    }
}

impl Eq for PyCalendar {}

impl core::hash::Hash for PyCalendar {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.spec.hash(state);
    }
}

impl PyCalendar {
    fn from_spec(spec: CalendarSpec) -> PyResult<Self> {
        Ok(Self {
            builder: spec.build()?,
            spec,
        })
    }

    fn derived(&self, op: Op) -> PyResult<Self> {
        let mut spec = self.spec.clone();
        spec.ops.push(op);
        Self::from_spec(spec)
    }

    /// The borrowed view every crate method is reached through.
    pub fn view(&self) -> Calendar<'_> {
        self.builder.view()
    }

    /// The record this calendar was built from.
    pub fn spec(&self) -> &CalendarSpec {
        &self.spec
    }

    /// The weekend as the weekdays it contains, Monday first.
    fn weekend_days(&self) -> Vec<PyWeekday> {
        let weekend = self.view().weekend;
        [
            Weekday::Mon,
            Weekday::Tue,
            Weekday::Wed,
            Weekday::Thu,
            Weekday::Fri,
            Weekday::Sat,
            Weekday::Sun,
        ]
        .into_iter()
        .filter(|day| weekend.contains(*day))
        .map(PyWeekday)
        .collect()
    }
}

#[pymethods]
impl PyCalendar {
    /// A calendar from a name, a weekend, and holiday rules.
    ///
    /// PyO3 drops a constructor's doc comment, so the example lives on
    /// the class.
    #[new]
    #[pyo3(signature = (name, weekend=Vec::new(), rules=Vec::new()))]
    fn new(name: String, weekend: Vec<WeekdayArg>, rules: Vec<PyRule>) -> PyResult<Self> {
        Self::from_spec(CalendarSpec {
            origin: Origin::Fresh(
                name,
                weekend.into_iter().map(|d| d.0).collect(),
                rules.into_iter().map(|r| r.0).collect(),
            ),
            ops: Vec::new(),
        })
    }

    /// The calendar's human-readable name.
    #[getter]
    fn name(&self) -> String {
        self.view().name.to_owned()
    }

    /// The weekend, as the weekdays it contains, Monday first.
    ///
    /// The crate's `Weekend` is a bitmask; a tuple of weekdays says the
    /// same thing without a second name for it.
    ///
    /// >>> from fasti.calendars import us
    /// >>> us.SETTLEMENT.weekend
    /// (Weekday.SAT, Weekday.SUN)
    #[getter]
    fn weekend<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(py, self.weekend_days())
    }

    /// The business days in `[start, end)`, ascending.
    ///
    /// The crate takes a half-open range; Python takes its two ends.
    ///
    /// >>> import datetime
    /// >>> from fasti.calendars import us
    /// >>> july = (datetime.date(2024, 7, 1), datetime.date(2024, 8, 1))
    /// >>> len(us.SETTLEMENT.business_days(*july))
    /// 22
    fn business_days<'py>(
        &self,
        py: Python<'py>,
        start: DateArg,
        end: DateArg,
    ) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let view = self.view();
        // A century of one calendar is a few million predicate calls
        // and no Python at all.
        let days: Vec<Date> = py.detach(|| view.business_days(start.0..end.0).collect());
        dates_out(py, &days)
    }

    /// The same calendar under a different name. The crate spells this
    /// `CalendarBuilder::name`, which here is the read accessor.
    fn with_name(&self, name: String) -> PyResult<Self> {
        self.derived(Op::Name(name))
    }

    /// The same calendar with a different weekend.
    fn with_weekend(&self, weekend: Vec<WeekdayArg>) -> PyResult<Self> {
        self.derived(Op::Weekend(weekend.into_iter().map(|d| d.0).collect()))
    }

    /// The same calendar with one more holiday rule.
    fn with_rule(&self, rule: PyRule) -> PyResult<Self> {
        self.derived(Op::Rule(rule.0))
    }

    /// The joint calendar: closed when either side is closed.
    ///
    /// >>> import datetime
    /// >>> from fasti.calendars import france, us
    /// >>> joint = us.SETTLEMENT.union(france.SETTLEMENT)
    /// >>> joint.is_holiday(datetime.date(2026, 7, 14))
    /// True
    fn union(&self, other: PyRef<'_, Self>) -> PyResult<Self> {
        self.derived(Op::Union(other.spec.clone()))
    }

    /// True if `date` falls on a weekend day under this calendar.
    fn is_weekend(&self, date: DateArg) -> bool {
        self.view().is_weekend(date.0)
    }

    /// True if a rule names `date`, or if it is the substitute day a
    /// weekend holiday was granted. A holiday falling on a weekend is
    /// still one; only a substitute for it can be lost.
    fn is_holiday(&self, date: DateArg) -> bool {
        self.view().is_holiday(date.0)
    }

    /// True if `date` is neither a weekend day nor a holiday.
    fn is_business_day(&self, date: DateArg) -> bool {
        self.view().is_business_day(date.0)
    }

    /// The holidays in `[start, end)`, ascending.
    fn holidays<'py>(
        &self,
        py: Python<'py>,
        start: DateArg,
        end: DateArg,
    ) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let view = self.view();
        let days: Vec<Date> = py.detach(|| view.holidays(start.0..end.0).collect());
        dates_out(py, &days)
    }

    /// The first business day of `date`'s month, or None if it has none.
    fn first_business_day_of_month<'py>(
        &self,
        py: Python<'py>,
        date: DateArg,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.view()
            .first_business_day_of_month(date.0)
            .map(|d| date_out(py, d))
            .transpose()
    }

    /// The last business day of `date`'s month, or None if it has none.
    fn last_business_day_of_month<'py>(
        &self,
        py: Python<'py>,
        date: DateArg,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.view()
            .last_business_day_of_month(date.0)
            .map(|d| date_out(py, d))
            .transpose()
    }

    /// The first business day strictly after `date`, if it is in range.
    fn next_business_day<'py>(
        &self,
        py: Python<'py>,
        date: DateArg,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.view()
            .next_business_day(date.0)
            .map(|d| date_out(py, d))
            .transpose()
    }

    /// The last business day strictly before `date`, if it is in range.
    fn prev_business_day<'py>(
        &self,
        py: Python<'py>,
        date: DateArg,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.view()
            .prev_business_day(date.0)
            .map(|d| date_out(py, d))
            .transpose()
    }

    /// Roll `date` onto a business day under `convention`.
    ///
    /// >>> import datetime
    /// >>> from fasti.calendars import us
    /// >>> us.SETTLEMENT.adjust(datetime.date(2024, 7, 4), "following")
    /// datetime.date(2024, 7, 5)
    fn adjust<'py>(
        &self,
        py: Python<'py>,
        date: DateArg,
        convention: ConventionArg,
    ) -> PyResult<Bound<'py, PyAny>> {
        let adjusted = self.view().adjust(date.0, convention.get()).or_raise()?;
        date_out(py, adjusted)
    }

    /// Step `date` by `period`, then roll onto a business day.
    ///
    /// With `end_of_month`, a month or year step from a month end lands
    /// on the target month's end before rolling.
    #[pyo3(signature = (date, period, convention, end_of_month=false))]
    fn advance<'py>(
        &self,
        py: Python<'py>,
        date: DateArg,
        period: PeriodArg,
        convention: ConventionArg,
        end_of_month: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        let advanced = self
            .view()
            .advance(date.0, period.0, convention.get(), end_of_month)
            .or_raise()?;
        date_out(py, advanced)
    }

    fn __repr__(&self) -> String {
        self.spec.repr()
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<ReplayReduction<'py>> {
        Ok((
            hook(py, "_rebuild_calendar")?,
            (
                origin_out(py, &self.spec.origin)?,
                ops_out(py, &self.spec.ops)?,
            ),
        ))
    }
}

fn weekend_out<'py>(py: Python<'py>, days: &[PyWeekday]) -> PyResult<Bound<'py, PyList>> {
    PyList::new(py, days.iter().map(|d| d.canonical()))
}

fn origin_out<'py>(py: Python<'py>, origin: &Origin) -> PyResult<Bound<'py, PyAny>> {
    match origin {
        Origin::Builtin(name) => name.into_bound_py_any(py),
        Origin::Fresh(name, weekend, rules) => (
            name,
            weekend_out(py, weekend)?,
            PyList::new(py, rules.iter().map(|r| PyRule(*r)))?,
        )
            .into_bound_py_any(py),
    }
}

fn ops_out<'py>(py: Python<'py>, ops: &[Op]) -> PyResult<Bound<'py, PyList>> {
    let entries: Vec<Bound<'py, PyAny>> = ops
        .iter()
        .map(|op| match op {
            Op::Name(name) => ("with_name", name).into_bound_py_any(py),
            Op::Weekend(weekend) => {
                ("with_weekend", weekend_out(py, weekend)?).into_bound_py_any(py)
            }
            Op::Rule(rule) => ("with_rule", PyRule(*rule)).into_bound_py_any(py),
            Op::Union(other) => (
                "union",
                (origin_out(py, &other.origin)?, ops_out(py, &other.ops)?),
            )
                .into_bound_py_any(py),
        })
        .collect::<PyResult<_>>()?;
    PyList::new(py, entries)
}

fn spec_in(origin: &Bound<'_, PyAny>, ops: &Bound<'_, PyList>) -> PyResult<CalendarSpec> {
    let origin = if let Ok(name) = origin.extract::<String>() {
        Origin::Builtin(lookup(&name)?.0)
    } else {
        let (name, weekend, rules): (String, Vec<WeekdayArg>, Vec<PyRule>) = origin.extract()?;
        Origin::Fresh(
            name,
            weekend.into_iter().map(|d| d.0).collect(),
            rules.into_iter().map(|r| r.0).collect(),
        )
    };
    let mut spec = CalendarSpec {
        origin,
        ops: Vec::new(),
    };
    for entry in ops.iter() {
        let entry = entry.cast_into::<PyTuple>()?;
        let payload = entry.get_item(1)?;
        spec.ops
            .push(match entry.get_item(0)?.extract::<String>()?.as_str() {
                "with_name" => Op::Name(payload.extract()?),
                "with_weekend" => Op::Weekend(
                    payload
                        .extract::<Vec<WeekdayArg>>()?
                        .into_iter()
                        .map(|d| d.0)
                        .collect(),
                ),
                "with_rule" => Op::Rule(payload.extract::<PyRule>()?.0),
                "union" => {
                    let (origin, ops): (Bound<'_, PyAny>, Bound<'_, PyList>) = payload.extract()?;
                    Op::Union(spec_in(&origin, &ops)?)
                }
                name => return Err(FastiError::new_err(format!("unknown calendar op {name:?}"))),
            });
    }
    Ok(spec)
}

/// Look a built-in calendar up by its path in the crate.
#[pyfunction]
pub fn _builtin(name: &str) -> PyResult<PyCalendar> {
    PyCalendar::from_spec(CalendarSpec {
        origin: Origin::Builtin(lookup(name)?.0),
        ops: Vec::new(),
    })
}

/// Replay a pickled calendar: its origin, then the operations applied.
#[pyfunction]
pub fn _rebuild_calendar(
    origin: &Bound<'_, PyAny>,
    ops: &Bound<'_, PyList>,
) -> PyResult<PyCalendar> {
    PyCalendar::from_spec(spec_in(origin, ops)?)
}

/// A `Calendar` argument, kept as the record so a schedule can rebuild
/// the view it needs without borrowing the Python object.
pub struct CalendarArg(pub CalendarSpec);

impl<'py> FromPyObject<'_, 'py> for CalendarArg {
    type Error = PyErr;

    fn extract(obj: pyo3::Borrowed<'_, 'py, PyAny>) -> PyResult<Self> {
        Ok(Self(obj.cast::<PyCalendar>()?.get().spec.clone()))
    }
}
