//! [`Schedule`] — the coupon-date grid between an effective and a
//! termination date.

use pyo3::exceptions::{PyIndexError, PyTypeError};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyType};

use crate::calendar::{Calendar, Origin};
use crate::convert::{DateArg, DateOut};
use crate::enums::{BusinessDayConvention, ConventionArg, DateGenerationRule, GenerationArg};
use crate::error::{err, invalid};
use crate::period::{Period, PeriodArg};
use crate::pickle::{Reduced, reduce};

/// What a schedule was built from. A generated schedule cannot be
/// rebuilt from its dates alone — the reference grid a stub accrues
/// against is not among them — so the generation arguments are kept and
/// replayed instead.
#[derive(Debug, Clone)]
enum Spec {
    Generated(Box<Generated>),
    FromDates(Vec<fasti::Date>),
    After(Box<Spec>, fasti::Date),
    Until(Box<Spec>, fasti::Date),
}

#[derive(Debug, Clone)]
struct Generated {
    effective: fasti::Date,
    termination: fasti::Date,
    tenor: fasti::Period,
    calendar: Option<Origin>,
    convention: Option<fasti::BusinessDayConvention>,
    termination_convention: Option<fasti::BusinessDayConvention>,
    rule: Option<fasti::DateGenerationRule>,
    end_of_month: bool,
    first_date: Option<fasti::Date>,
    next_to_last_date: Option<fasti::Date>,
}

impl Spec {
    fn encode<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let object = match self {
            Self::Generated(g) => {
                let payload = pyo3::types::PyDict::new(py);
                payload.set_item("effective", DateOut(g.effective))?;
                payload.set_item("termination", DateOut(g.termination))?;
                payload.set_item("tenor", g.tenor.to_string())?;
                payload.set_item(
                    "calendar",
                    g.calendar
                        .as_ref()
                        .map(|origin| origin.encode(py))
                        .transpose()?,
                )?;
                payload.set_item(
                    "convention",
                    g.convention
                        .map(|c| BusinessDayConvention::wrap(c).__str__()),
                )?;
                payload.set_item(
                    "termination_convention",
                    g.termination_convention
                        .map(|c| BusinessDayConvention::wrap(c).__str__()),
                )?;
                payload.set_item(
                    "rule",
                    g.rule.map(|r| DateGenerationRule::wrap(r).__str__()),
                )?;
                payload.set_item("end_of_month", g.end_of_month)?;
                payload.set_item("first_date", g.first_date.map(DateOut))?;
                payload.set_item("next_to_last_date", g.next_to_last_date.map(DateOut))?;
                ("generated", payload).into_pyobject(py)?.into_any()
            }
            Self::FromDates(dates) => (
                "from_dates",
                dates.iter().copied().map(DateOut).collect::<Vec<_>>(),
            )
                .into_pyobject(py)?
                .into_any(),
            Self::After(inner, cutoff) => ("after", inner.encode(py)?, DateOut(*cutoff))
                .into_pyobject(py)?
                .into_any(),
            Self::Until(inner, cutoff) => ("until", inner.encode(py)?, DateOut(*cutoff))
                .into_pyobject(py)?
                .into_any(),
        };
        Ok(object)
    }
}

/// Rebuild a schedule by replaying the generation it recorded.
pub fn rebuild(py: Python<'_>, spec: &Bound<'_, PyAny>) -> PyResult<Schedule> {
    let tag: String = spec.get_item(0)?.extract()?;
    match tag.as_str() {
        "generated" => {
            let payload = spec.get_item(1)?;
            let item = |key: &str| payload.get_item(key);
            let calendar = item("calendar")?;
            let calendar = if calendar.is_none() {
                None
            } else {
                Some(Py::new(py, crate::calendar::rebuild(&calendar)?)?)
            };
            Schedule::new(
                item("effective")?.extract()?,
                item("termination")?.extract()?,
                item("tenor")?.extract()?,
                calendar.as_ref().map(|c| c.borrow(py)),
                item("convention")?.extract()?,
                item("termination_convention")?.extract()?,
                item("rule")?.extract()?,
                item("end_of_month")?.extract()?,
                item("first_date")?.extract()?,
                item("next_to_last_date")?.extract()?,
            )
        }
        "from_dates" => Schedule::rebuild_from_dates(spec.get_item(1)?.extract()?),
        "after" => Ok(rebuild(py, &spec.get_item(1)?)?.after(spec.get_item(2)?.extract()?)),
        "until" => Ok(rebuild(py, &spec.get_item(1)?)?.until(spec.get_item(2)?.extract()?)),
        _ => Err(invalid(format!("unknown schedule spec: {tag:?}"))),
    }
}

/// A generated coupon schedule: business-day-adjusted dates in
/// chronological order, plus the reference grid a schedule-aware day
/// count (ACT/ACT ICMA) accrues against.
///
/// >>> import fasti
/// >>> from datetime import date
/// >>> s = fasti.Schedule(date(2025, 1, 15), date(2027, 1, 15), "6M",
/// ...                    fasti.calendars.US_SETTLEMENT)
/// >>> len(s)
/// 5
/// >>> s[0]
/// datetime.date(2025, 1, 15)
/// >>> [(a, b) for a, b in s.periods()][0]
/// (datetime.date(2025, 1, 15), datetime.date(2025, 7, 15))
#[pyclass(module = "fasti", from_py_object, frozen, eq, hash)]
#[derive(Debug, Clone)]
pub struct Schedule {
    pub inner: fasti::Schedule,
    spec: Spec,
}

/// Two schedules are equal when their dates and reference grid are —
/// how each was arrived at is not part of the value.
impl PartialEq for Schedule {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for Schedule {}

/// The dates are enough to hash on: equal schedules share them, and two
/// schedules that differ only in their stub reference grid are allowed
/// to collide.
impl std::hash::Hash for Schedule {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.inner.dates().hash(state);
    }
}

impl Schedule {
    /// Wrap an explicit date list, recording that this is where the
    /// schedule came from.
    pub fn rebuild_from_dates(dates: Vec<DateArg>) -> PyResult<Self> {
        let dates: Vec<fasti::Date> = dates.into_iter().map(|d| d.0).collect();
        fasti::Schedule::try_from(dates.clone())
            .map(|inner| Self {
                inner,
                spec: Spec::FromDates(dates),
            })
            .map_err(err)
    }
}

#[pymethods]
impl Schedule {
    /// Generate a schedule.
    ///
    /// `calendar` defaults to no calendar at all, which leaves every
    /// date unadjusted; pass one to roll the grid onto business days.
    /// Defaults otherwise follow bond convention: backward generation
    /// from termination, `"modified_following"` for interior dates,
    /// `"unadjusted"` for the termination date.
    #[new]
    #[pyo3(signature = (
        effective,
        termination,
        tenor,
        calendar=None,
        *,
        convention=None,
        termination_convention=None,
        rule=None,
        end_of_month=false,
        first_date=None,
        next_to_last_date=None,
    ))]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        effective: DateArg,
        termination: DateArg,
        tenor: PeriodArg,
        calendar: Option<PyRef<'_, Calendar>>,
        convention: Option<ConventionArg>,
        termination_convention: Option<ConventionArg>,
        rule: Option<GenerationArg>,
        end_of_month: bool,
        first_date: Option<DateArg>,
        next_to_last_date: Option<DateArg>,
    ) -> PyResult<Self> {
        let view = calendar
            .as_ref()
            .map_or(fasti::calendars::NULL_CALENDAR, |c| c.view());
        let mut builder = fasti::ScheduleBuilder::new(effective.0, termination.0, tenor.0, view);
        if let Some(c) = convention {
            builder = builder.with_convention(c.0);
        }
        if let Some(c) = termination_convention {
            builder = builder.with_termination_convention(c.0);
        }
        if let Some(r) = rule {
            builder = builder.with_rule(r.0);
        }
        if end_of_month {
            builder = builder.with_end_of_month(true);
        }
        if let Some(d) = first_date {
            builder = builder.with_first_date(d.0);
        }
        if let Some(d) = next_to_last_date {
            builder = builder.with_next_to_last_date(d.0);
        }
        let spec = Spec::Generated(Box::new(Generated {
            effective: effective.0,
            termination: termination.0,
            tenor: tenor.0,
            calendar: calendar.as_ref().map(|c| c.origin()),
            convention: convention.map(|c| c.0),
            termination_convention: termination_convention.map(|c| c.0),
            rule: rule.map(|r| r.0),
            end_of_month,
            first_date: first_date.map(|d| d.0),
            next_to_last_date: next_to_last_date.map(|d| d.0),
        }));
        builder
            .build()
            .map(|inner| Self { inner, spec })
            .map_err(err)
    }

    /// Wrap an explicit list of dates — a term sheet's own schedule —
    /// without generating anything. The dates must strictly increase.
    #[classmethod]
    fn from_dates(_cls: &Bound<'_, PyType>, dates: Vec<DateArg>) -> PyResult<Self> {
        Self::rebuild_from_dates(dates)
    }

    /// The adjusted coupon dates, ascending.
    #[getter]
    fn dates(&self) -> Vec<DateOut> {
        self.inner.dates().iter().copied().map(DateOut).collect()
    }

    /// The regular period between coupons, or `None` for a schedule
    /// built from an explicit date list.
    #[getter]
    fn tenor(&self) -> Option<Period> {
        self.inner.generation().map(|g| Period(g.tenor))
    }

    /// Whether generation snapped dates to month ends, or `None` for a
    /// schedule built from an explicit date list.
    #[getter]
    fn end_of_month(&self) -> Option<bool> {
        self.inner.generation().map(|g| g.end_of_month)
    }

    /// The coupon periods as `(start, end)` pairs.
    fn periods(&self) -> Vec<(DateOut, DateOut)> {
        self.inner
            .periods()
            .map(|p| (DateOut(p.start), DateOut(p.end)))
            .collect()
    }

    /// The reference period each coupon accrues against: the coupon
    /// period itself, except at a stub, where it is the notional
    /// quasi-coupon period one tenor from the adjacent coupon.
    fn reference_periods(&self) -> Vec<(DateOut, DateOut)> {
        self.inner
            .reference_periods()
            .map(|p| (DateOut(p.start), DateOut(p.end)))
            .collect()
    }

    /// The latest schedule date strictly before `date`, or `None`.
    fn previous_date(&self, date: DateArg) -> Option<DateOut> {
        self.inner.previous_date(date.0).map(DateOut)
    }

    /// The earliest schedule date strictly after `date`, or `None`.
    fn next_date(&self, date: DateArg) -> Option<DateOut> {
        self.inner.next_date(date.0).map(DateOut)
    }

    /// The earliest schedule date on or after `date`, or `None`.
    fn lower_bound(&self, date: DateArg) -> Option<DateOut> {
        self.inner.lower_bound(date.0).map(DateOut)
    }

    /// The part of this schedule at or after `cutoff`.
    fn after(&self, cutoff: DateArg) -> Self {
        Self {
            inner: self.inner.after(cutoff.0),
            spec: Spec::After(Box::new(self.spec.clone()), cutoff.0),
        }
    }

    /// The part of this schedule at or before `cutoff`.
    fn until(&self, cutoff: DateArg) -> Self {
        Self {
            inner: self.inner.until(cutoff.0),
            spec: Spec::Until(Box::new(self.spec.clone()), cutoff.0),
        }
    }

    fn __len__(&self) -> usize {
        self.inner.dates().len()
    }

    fn __getitem__(&self, py: Python<'_>, index: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let dates = self.inner.dates();
        if let Ok(i) = index.extract::<isize>() {
            let len = isize::try_from(dates.len()).unwrap_or(isize::MAX);
            let idx = if i < 0 { i + len } else { i };
            let date = usize::try_from(idx)
                .ok()
                .and_then(|i| dates.get(i))
                .ok_or_else(|| PyIndexError::new_err("schedule index out of range"))?;
            return DateOut(*date).into_pyobject(py).map(Bound::unbind);
        }
        if index.is_instance_of::<pyo3::types::PySlice>() {
            let all = PyList::new(py, dates.iter().copied().map(DateOut))?;
            return all.as_any().get_item(index).map(Bound::unbind);
        }
        Err(PyTypeError::new_err(
            "schedule indices must be integers or slices",
        ))
    }

    fn __iter__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let list = PyList::new(py, self.inner.dates().iter().copied().map(DateOut))?;
        Ok(list.as_any().try_iter()?.into_any())
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<Reduced<'py>> {
        reduce(py, "_rebuild_schedule", (self.spec.encode(py)?,))
    }

    fn __repr__(&self) -> String {
        let dates = self.inner.dates();
        match (dates.first(), dates.last()) {
            (Some(first), Some(last)) => format!(
                "<fasti.Schedule {first}..{last}, {} periods>",
                dates.len().saturating_sub(1)
            ),
            _ => "<fasti.Schedule empty>".to_owned(),
        }
    }
}
