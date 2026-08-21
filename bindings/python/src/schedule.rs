//! [`Schedule`] — the coupon-date grid between an effective and a
//! termination date.

use pyo3::exceptions::{PyIndexError, PyTypeError};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyType};

use crate::calendar::Calendar;
use crate::convert::{DateArg, DateOut};
use crate::enums::{ConventionArg, GenerationArg};
use crate::error::err;
use crate::period::{Period, PeriodArg};

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
#[pyclass(module = "fasti", from_py_object, frozen, eq)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schedule(pub fasti::Schedule);

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
    fn new(
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
        builder.build().map(Self).map_err(err)
    }

    /// Wrap an explicit list of dates — a term sheet's own schedule —
    /// without generating anything. The dates must strictly increase.
    #[classmethod]
    fn from_dates(_cls: &Bound<'_, PyType>, dates: Vec<DateArg>) -> PyResult<Self> {
        let dates: Vec<fasti::Date> = dates.into_iter().map(|d| d.0).collect();
        fasti::Schedule::try_from(dates).map(Self).map_err(err)
    }

    /// The adjusted coupon dates, ascending.
    #[getter]
    fn dates(&self) -> Vec<DateOut> {
        self.0.dates().iter().copied().map(DateOut).collect()
    }

    /// The regular period between coupons, or `None` for a schedule
    /// built from an explicit date list.
    #[getter]
    fn tenor(&self) -> Option<Period> {
        self.0.generation().map(|g| Period(g.tenor))
    }

    /// Whether generation snapped dates to month ends, or `None` for a
    /// schedule built from an explicit date list.
    #[getter]
    fn end_of_month(&self) -> Option<bool> {
        self.0.generation().map(|g| g.end_of_month)
    }

    /// The coupon periods as `(start, end)` pairs.
    fn periods(&self) -> Vec<(DateOut, DateOut)> {
        self.0
            .periods()
            .map(|p| (DateOut(p.start), DateOut(p.end)))
            .collect()
    }

    /// The reference period each coupon accrues against: the coupon
    /// period itself, except at a stub, where it is the notional
    /// quasi-coupon period one tenor from the adjacent coupon.
    fn reference_periods(&self) -> Vec<(DateOut, DateOut)> {
        self.0
            .reference_periods()
            .map(|p| (DateOut(p.start), DateOut(p.end)))
            .collect()
    }

    /// The latest schedule date strictly before `date`, or `None`.
    fn previous_date(&self, date: DateArg) -> Option<DateOut> {
        self.0.previous_date(date.0).map(DateOut)
    }

    /// The earliest schedule date strictly after `date`, or `None`.
    fn next_date(&self, date: DateArg) -> Option<DateOut> {
        self.0.next_date(date.0).map(DateOut)
    }

    /// The earliest schedule date on or after `date`, or `None`.
    fn lower_bound(&self, date: DateArg) -> Option<DateOut> {
        self.0.lower_bound(date.0).map(DateOut)
    }

    /// The part of this schedule at or after `cutoff`.
    fn after(&self, cutoff: DateArg) -> Self {
        Self(self.0.after(cutoff.0))
    }

    /// The part of this schedule at or before `cutoff`.
    fn until(&self, cutoff: DateArg) -> Self {
        Self(self.0.until(cutoff.0))
    }

    fn __len__(&self) -> usize {
        self.0.dates().len()
    }

    fn __getitem__(&self, py: Python<'_>, index: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let dates = self.0.dates();
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
        let list = PyList::new(py, self.0.dates().iter().copied().map(DateOut))?;
        Ok(list.as_any().try_iter()?.into_any())
    }

    fn __repr__(&self) -> String {
        let dates = self.0.dates();
        match (dates.first(), dates.last()) {
            (Some(first), Some(last)) => format!(
                "<fasti.Schedule {first}..{last}, {} periods>",
                dates.len().saturating_sub(1)
            ),
            _ => "<fasti.Schedule empty>".to_owned(),
        }
    }
}
