//! [`Rule`] — the holiday primitives a custom calendar is assembled
//! from: fixed dates, nth/last weekdays, Easter offsets, and one-offs.

use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::convert::{DateArg, DateOut};
use crate::enums::{EasterMethodArg, MonthArg, ShiftArg, WeekdayArg, shift_repr};
use crate::error::{err, invalid};
use crate::pickle::{Reduced, reduce};

/// What a rule was built from: enough to print it, and enough to build
/// it again. Holding the arguments rather than a rendered string keeps
/// `repr` and `__reduce__` from drifting apart, and keeps both honest
/// about what the rule actually is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Spec {
    Fixed {
        month: fasti::Month,
        day: u8,
        shift: crate::enums::WeekendShift,
    },
    NthWeekday {
        n: u8,
        weekday: fasti::Weekday,
        month: fasti::Month,
    },
    LastWeekday {
        weekday: fasti::Weekday,
        month: fasti::Month,
    },
    Easter {
        offset: i16,
        method: fasti::EasterMethod,
    },
    OneOff(fasti::Date),
}

impl Spec {
    const fn kind(self) -> &'static str {
        match self {
            Self::Fixed { .. } => "fixed",
            Self::NthWeekday { .. } => "nth_weekday",
            Self::LastWeekday { .. } => "last_weekday",
            Self::Easter { .. } => "easter",
            Self::OneOff(_) => "one_off",
        }
    }
}

const fn method_name(method: fasti::EasterMethod) -> &'static str {
    match method {
        fasti::EasterMethod::Western => "western",
        fasti::EasterMethod::Orthodox => "orthodox",
    }
}

/// A holiday rule, naming a holiday's *natural* date. Where a weekend
/// holiday is observed instead is the calendar's decision, driven by the
/// rule's `shift`.
///
/// >>> from datetime import date
/// >>> from fasti import Rule, Calendar
/// >>> juneteenth = Rule.fixed("Jun", 19, shift="us", from_year=2022)
/// >>> cal = Calendar.custom("Acme", rules=[juneteenth])
/// >>> cal.is_holiday(date(2027, 6, 18))  # 2027-06-19 is a Saturday
/// True
///
/// Rules compare by what they say, not by identity:
///
/// >>> Rule.fixed("Jul", 4) == Rule.fixed(7, 4)
/// True
/// >>> Rule.fixed("Jul", 4) == Rule.fixed("Jul", 4, from_year=1971)
/// False
#[pyclass(module = "fasti", from_py_object, frozen, eq, hash, str)]
#[derive(Debug, Clone)]
pub struct Rule {
    pub inner: fasti::Rule,
    spec: Spec,
    from_year: Option<u16>,
    to_year: Option<u16>,
}

/// Accept either one rule or an iterable of them, the way
/// [`crate::convert::dates`] does for dates.
pub fn rules(ob: &Bound<'_, PyAny>) -> PyResult<Vec<Rule>> {
    if let Ok(one) = ob.extract::<Rule>() {
        return Ok(vec![one]);
    }
    let items = ob.try_iter().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err(format!(
            "expected a Rule or an iterable of them, got {}",
            crate::convert::type_name(ob)
        ))
    })?;
    let mut rules = Vec::new();
    for item in items {
        rules.push(item?.extract::<Rule>()?);
    }
    Ok(rules)
}

/// Build the optional year range a rule is active over.
fn years(from_year: Option<u16>, to_year: Option<u16>) -> PyResult<Option<fasti::YearRange>> {
    let year = |y: u16| fasti::Year::new(y).map_err(err);
    Ok(match (from_year, to_year) {
        (None, None) => None,
        (Some(f), None) => Some(fasti::YearRange::from_year(year(f)?)),
        (None, Some(t)) => Some(fasti::YearRange::through(year(t)?)),
        (Some(f), Some(t)) => Some(fasti::YearRange::try_between(year(f)?, year(t)?).map_err(err)?),
    })
}

impl Rule {
    /// A one-off rule, for callers inside the crate that have a
    /// [`fasti::Date`] rather than a Python object.
    pub fn one_off_rule(date: fasti::Date) -> Self {
        Self::new(
            fasti::Rule::OneOff(fasti::OneOff::new(date)),
            Spec::OneOff(date),
            None,
            None,
        )
    }

    fn new(inner: fasti::Rule, spec: Spec, from_year: Option<u16>, to_year: Option<u16>) -> Self {
        Self {
            inner,
            spec,
            from_year,
            to_year,
        }
    }

    /// The year-range keywords, as they would be typed.
    fn years_repr(&self) -> String {
        match (self.from_year, self.to_year) {
            (None, None) => String::new(),
            (Some(f), None) => format!(", from_year={f}"),
            (None, Some(t)) => format!(", to_year={t}"),
            (Some(f), Some(t)) => format!(", from_year={f}, to_year={t}"),
        }
    }
}

#[pymethods]
impl Rule {
    /// A holiday on the same month and day every year.
    ///
    /// `shift` says what happens when the natural date is a weekend:
    /// `"none"` loses it, `"forward"` is the UK substitute day,
    /// `"sun_forward"` is the Fed/SIFMA rule, and
    /// `"sat_back_sun_forward"` (`"us"`) is the US federal rule.
    #[staticmethod]
    #[pyo3(signature = (month, day, *, shift=None, from_year=None, to_year=None))]
    fn fixed(
        month: MonthArg,
        day: u8,
        shift: Option<ShiftArg>,
        from_year: Option<u16>,
        to_year: Option<u16>,
    ) -> PyResult<Self> {
        // A day the month can never have would build a rule that simply
        // never matches, which is a typo, not a calendar.
        let longest = month.0.length(fasti::Year::literal(2024));
        if day == 0 || day > longest {
            return Err(invalid(format!(
                "{} has no day {day}: expected 1..={longest}",
                month.0
            )));
        }
        let shift = shift.map_or(fasti::WeekendShift::None, |s| s.0);
        let mut rule = fasti::FixedDate::new(month.0, day).shift(shift);
        let shift = crate::enums::WeekendShift::wrap(shift);
        if let Some(range) = years(from_year, to_year)? {
            rule = rule.years(range);
        }
        Ok(Self::new(
            fasti::Rule::Fixed(rule),
            Spec::Fixed {
                month: month.0,
                day,
                shift,
            },
            from_year,
            to_year,
        ))
    }

    /// The nth occurrence of a weekday in a month — `n` is 1..=5, e.g.
    /// `Rule.nth_weekday(3, "mon", "Jan")` for Martin Luther King Day.
    #[staticmethod]
    #[pyo3(signature = (n, weekday, month, *, from_year=None, to_year=None))]
    fn nth_weekday(
        n: u8,
        weekday: WeekdayArg,
        month: MonthArg,
        from_year: Option<u16>,
        to_year: Option<u16>,
    ) -> PyResult<Self> {
        let ordinal = fasti::Ordinal::try_from_u8(n).map_err(err)?;
        let weekday = weekday.0.inner();
        let mut rule = fasti::NthWeekday::new(ordinal, weekday, month.0);
        if let Some(range) = years(from_year, to_year)? {
            rule = rule.years(range);
        }
        Ok(Self::new(
            fasti::Rule::NthWeekday(rule),
            Spec::NthWeekday {
                n,
                weekday,
                month: month.0,
            },
            from_year,
            to_year,
        ))
    }

    /// The last occurrence of a weekday in a month, e.g.
    /// `Rule.last_weekday("mon", "May")` for Memorial Day.
    #[staticmethod]
    #[pyo3(signature = (weekday, month, *, from_year=None, to_year=None))]
    fn last_weekday(
        weekday: WeekdayArg,
        month: MonthArg,
        from_year: Option<u16>,
        to_year: Option<u16>,
    ) -> PyResult<Self> {
        let weekday = weekday.0.inner();
        let mut rule = fasti::LastWeekday::new(weekday, month.0);
        if let Some(range) = years(from_year, to_year)? {
            rule = rule.years(range);
        }
        Ok(Self::new(
            fasti::Rule::LastWeekday(rule),
            Spec::LastWeekday {
                weekday,
                month: month.0,
            },
            from_year,
            to_year,
        ))
    }

    /// A holiday a fixed number of days from Easter Sunday: `0` is
    /// Easter Sunday itself, `-2` Good Friday, `1` Easter Monday.
    #[staticmethod]
    #[pyo3(signature = (offset, *, method=None, from_year=None, to_year=None))]
    fn easter(
        offset: i16,
        method: Option<EasterMethodArg>,
        from_year: Option<u16>,
        to_year: Option<u16>,
    ) -> PyResult<Self> {
        let method = method.map_or(fasti::EasterMethod::Western, |m| m.0);
        // `EasterOffset` counts from Easter Sunday, which is how the
        // holidays are named, so the offset passes straight through.
        let mut rule = match method {
            fasti::EasterMethod::Western => fasti::EasterOffset::new(offset),
            fasti::EasterMethod::Orthodox => fasti::EasterOffset::new_orthodox(offset),
        };
        if let Some(range) = years(from_year, to_year)? {
            rule = rule.years(range);
        }
        Ok(Self::new(
            fasti::Rule::Easter(rule),
            Spec::Easter { offset, method },
            from_year,
            to_year,
        ))
    }

    /// Good Friday — two days before Easter Sunday.
    #[staticmethod]
    #[pyo3(signature = (*, method=None))]
    fn good_friday(method: Option<EasterMethodArg>) -> PyResult<Self> {
        Self::easter(-2, method, None, None)
    }

    /// Easter Monday — the day after Easter Sunday.
    #[staticmethod]
    #[pyo3(signature = (*, method=None))]
    fn easter_monday(method: Option<EasterMethodArg>) -> PyResult<Self> {
        Self::easter(1, method, None, None)
    }

    /// Ascension Day — 39 days after Easter Sunday.
    #[staticmethod]
    #[pyo3(signature = (*, method=None))]
    fn ascension(method: Option<EasterMethodArg>) -> PyResult<Self> {
        Self::easter(39, method, None, None)
    }

    /// Whit Monday — 50 days after Easter Sunday.
    #[staticmethod]
    #[pyo3(signature = (*, method=None))]
    fn whit_monday(method: Option<EasterMethodArg>) -> PyResult<Self> {
        Self::easter(50, method, None, None)
    }

    /// Corpus Christi — 60 days after Easter Sunday.
    #[staticmethod]
    #[pyo3(signature = (*, method=None))]
    fn corpus_christi(method: Option<EasterMethodArg>) -> PyResult<Self> {
        Self::easter(60, method, None, None)
    }

    /// A single date, observed once: a royal wedding, a market closure,
    /// a company blackout day.
    #[staticmethod]
    fn one_off(date: DateArg) -> Self {
        Self::one_off_rule(date.0)
    }

    /// `True` iff this rule names `date` as a holiday's natural date.
    /// Weekend substitutes are resolved by the calendar, not the rule,
    /// so ask `Calendar.is_holiday` for the observed day.
    fn is_holiday(&self, date: DateArg) -> bool {
        self.inner.is_holiday(date.0)
    }

    fn __repr__(&self) -> String {
        let years = self.years_repr();
        match self.spec {
            Spec::Fixed { month, day, shift } => {
                let shift = match shift.inner() {
                    fasti::WeekendShift::None => String::new(),
                    other => format!(", shift='{}'", shift_repr(other)),
                };
                format!("Rule.fixed('{month}', {day}{shift}{years})")
            }
            Spec::NthWeekday { n, weekday, month } => {
                format!("Rule.nth_weekday({n}, '{weekday}', '{month}'{years})")
            }
            Spec::LastWeekday { weekday, month } => {
                format!("Rule.last_weekday('{weekday}', '{month}'{years})")
            }
            Spec::Easter { offset, method } => {
                format!(
                    "Rule.easter({offset}, method='{}'{years})",
                    method_name(method)
                )
            }
            Spec::OneOff(date) => {
                let (year, month, day) = date.to_ymd();
                format!(
                    "Rule.one_off(datetime.date({}, {}, {day}))",
                    year.get(),
                    month.get()
                )
            }
        }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<Reduced<'py>> {
        let payload = PyDict::new(py);
        match self.spec {
            Spec::Fixed { month, day, shift } => {
                payload.set_item("month", month.get())?;
                payload.set_item("day", day)?;
                payload.set_item("shift", shift_repr(shift.inner()))?;
            }
            Spec::NthWeekday { n, weekday, month } => {
                payload.set_item("n", n)?;
                payload.set_item("weekday", weekday.get())?;
                payload.set_item("month", month.get())?;
            }
            Spec::LastWeekday { weekday, month } => {
                payload.set_item("weekday", weekday.get())?;
                payload.set_item("month", month.get())?;
            }
            Spec::Easter { offset, method } => {
                payload.set_item("offset", offset)?;
                payload.set_item("method", method_name(method))?;
            }
            Spec::OneOff(date) => payload.set_item("date", DateOut(date))?,
        }
        if let Some(from_year) = self.from_year {
            payload.set_item("from_year", from_year)?;
        }
        if let Some(to_year) = self.to_year {
            payload.set_item("to_year", to_year)?;
        }
        reduce(py, "_rebuild_rule", (self.spec.kind(), payload))
    }
}

/// Rebuild a rule from the arguments its `__reduce__` recorded.
pub fn rebuild(kind: &str, payload: &Bound<'_, PyDict>) -> PyResult<Rule> {
    let get = |key: &str| payload.get_item(key).ok().flatten();
    let extract =
        |key: &str| -> PyResult<Option<u16>> { get(key).map(|v| v.extract::<u16>()).transpose() };
    let from_year = extract("from_year")?;
    let to_year = extract("to_year")?;
    let required =
        |key: &str| get(key).ok_or_else(|| invalid(format!("rule payload is missing {key:?}")));
    match kind {
        "fixed" => Rule::fixed(
            required("month")?.extract()?,
            required("day")?.extract()?,
            Some(required("shift")?.extract()?),
            from_year,
            to_year,
        ),
        "nth_weekday" => Rule::nth_weekday(
            required("n")?.extract()?,
            required("weekday")?.extract()?,
            required("month")?.extract()?,
            from_year,
            to_year,
        ),
        "last_weekday" => Rule::last_weekday(
            required("weekday")?.extract()?,
            required("month")?.extract()?,
            from_year,
            to_year,
        ),
        "easter" => Rule::easter(
            required("offset")?.extract()?,
            Some(required("method")?.extract()?),
            from_year,
            to_year,
        ),
        "one_off" => Ok(Rule::one_off(required("date")?.extract()?)),
        _ => Err(invalid(format!("unknown rule kind: {kind:?}"))),
    }
}

/// A rule is exactly its arguments: `inner` is built from them, so it
/// has no say in whether two rules are the same.
impl PartialEq for Rule {
    fn eq(&self, other: &Self) -> bool {
        self.spec == other.spec
            && self.from_year == other.from_year
            && self.to_year == other.to_year
    }
}

impl Eq for Rule {}

impl std::hash::Hash for Rule {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.spec.hash(state);
        self.from_year.hash(state);
        self.to_year.hash(state);
    }
}

impl std::fmt::Display for Rule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.__repr__())
    }
}
