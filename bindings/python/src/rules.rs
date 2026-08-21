//! [`Rule`] — the holiday primitives a custom calendar is assembled
//! from: fixed dates, nth/last weekdays, Easter offsets, and one-offs.

use pyo3::prelude::*;

use crate::convert::DateArg;
use crate::enums::{MonthArg, ShiftArg, WeekdayArg};
use crate::error::err;

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
#[pyclass(module = "fasti", from_py_object, frozen, str)]
#[derive(Debug, Clone)]
pub struct Rule {
    pub inner: fasti::Rule,
    description: String,
}

impl Rule {
    fn new(inner: fasti::Rule, description: String) -> Self {
        Self { inner, description }
    }
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

/// Render a year range for a rule's `repr`.
fn years_repr(from_year: Option<u16>, to_year: Option<u16>) -> String {
    match (from_year, to_year) {
        (None, None) => String::new(),
        (Some(f), None) => format!(", from_year={f}"),
        (None, Some(t)) => format!(", to_year={t}"),
        (Some(f), Some(t)) => format!(", from_year={f}, to_year={t}"),
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
            return Err(crate::error::invalid(format!(
                "{} has no day {day}: expected 1..={longest}",
                month.0
            )));
        }
        let shift = shift.map_or(fasti::WeekendShift::None, |s| s.0);
        let mut rule = fasti::FixedDate::new(month.0, day).shift(shift);
        if let Some(range) = years(from_year, to_year)? {
            rule = rule.years(range);
        }
        let shift_repr = match shift {
            fasti::WeekendShift::None => String::new(),
            other => format!(", shift='{}'", crate::enums::shift_repr(other)),
        };
        let description = format!(
            "Rule.fixed('{}', {day}{shift_repr}{})",
            month.0,
            years_repr(from_year, to_year)
        );
        Ok(Self::new(fasti::Rule::Fixed(rule), description))
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
        let mut rule = fasti::NthWeekday::new(ordinal, weekday.0.inner(), month.0);
        if let Some(range) = years(from_year, to_year)? {
            rule = rule.years(range);
        }
        let description = format!(
            "Rule.nth_weekday({n}, '{}', '{}'{})",
            weekday.0.inner(),
            month.0,
            years_repr(from_year, to_year)
        );
        Ok(Self::new(fasti::Rule::NthWeekday(rule), description))
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
        let mut rule = fasti::LastWeekday::new(weekday.0.inner(), month.0);
        if let Some(range) = years(from_year, to_year)? {
            rule = rule.years(range);
        }
        let description = format!(
            "Rule.last_weekday('{}', '{}'{})",
            weekday.0.inner(),
            month.0,
            years_repr(from_year, to_year)
        );
        Ok(Self::new(fasti::Rule::LastWeekday(rule), description))
    }

    /// A holiday a fixed number of days from Easter Sunday: `0` is
    /// Easter Sunday itself, `-2` Good Friday, `1` Easter Monday.
    #[staticmethod]
    #[pyo3(signature = (offset, *, method=None, from_year=None, to_year=None))]
    fn easter(
        offset: i16,
        method: Option<crate::enums::EasterMethodArg>,
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
        let method_name = match method {
            fasti::EasterMethod::Western => "western",
            fasti::EasterMethod::Orthodox => "orthodox",
        };
        let description = format!(
            "Rule.easter({offset}, method='{method_name}'{})",
            years_repr(from_year, to_year)
        );
        Ok(Self::new(fasti::Rule::Easter(rule), description))
    }

    /// Good Friday — two days before Easter Sunday.
    #[staticmethod]
    #[pyo3(signature = (*, method=None))]
    fn good_friday(method: Option<crate::enums::EasterMethodArg>) -> PyResult<Self> {
        Self::easter(-2, method, None, None)
    }

    /// Easter Monday — the day after Easter Sunday.
    #[staticmethod]
    #[pyo3(signature = (*, method=None))]
    fn easter_monday(method: Option<crate::enums::EasterMethodArg>) -> PyResult<Self> {
        Self::easter(1, method, None, None)
    }

    /// Ascension Day — 39 days after Easter Sunday.
    #[staticmethod]
    #[pyo3(signature = (*, method=None))]
    fn ascension(method: Option<crate::enums::EasterMethodArg>) -> PyResult<Self> {
        Self::easter(39, method, None, None)
    }

    /// Whit Monday — 50 days after Easter Sunday.
    #[staticmethod]
    #[pyo3(signature = (*, method=None))]
    fn whit_monday(method: Option<crate::enums::EasterMethodArg>) -> PyResult<Self> {
        Self::easter(50, method, None, None)
    }

    /// Corpus Christi — 60 days after Easter Sunday.
    #[staticmethod]
    #[pyo3(signature = (*, method=None))]
    fn corpus_christi(method: Option<crate::enums::EasterMethodArg>) -> PyResult<Self> {
        Self::easter(60, method, None, None)
    }

    /// A single date, observed once: a royal wedding, a market closure,
    /// a company blackout day.
    #[staticmethod]
    fn one_off(date: DateArg) -> Self {
        // Spelled as the call that would rebuild it, which now means a
        // date constructor rather than a string.
        let (year, month, day) = date.0.to_ymd();
        let description = format!(
            "Rule.one_off(datetime.date({}, {}, {day}))",
            year.get(),
            month.get()
        );
        Self::new(fasti::Rule::OneOff(fasti::OneOff::new(date.0)), description)
    }

    /// `True` iff this rule names `date` as a holiday's natural date.
    /// Weekend substitutes are resolved by the calendar, not the rule,
    /// so ask `Calendar.is_holiday` for the observed day.
    fn is_holiday(&self, date: DateArg) -> bool {
        self.inner.is_holiday(date.0)
    }

    fn __repr__(&self) -> String {
        self.description.clone()
    }
}

impl std::fmt::Display for Rule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.description)
    }
}
