//! [`DayCount`] — the conventions that measure elapsed time between two
//! dates, returning an exact `fractions.Fraction`.

use pyo3::prelude::*;

use crate::convert::{DateArg, DateOut, from_fraction, normalize};
use crate::enums::FrequencyArg;
use crate::error::{err, invalid};
use crate::schedule::Schedule;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Kind {
    Act360,
    Act365Fixed,
    ActActISDA,
    ActActICMA {
        frequency: fasti::Frequency,
        schedule: Option<Schedule>,
    },
    Thirty360Bond,
    Thirty360US,
    Thirty360European,
    Thirty360ISDA {
        termination: fasti::Date,
    },
}

/// A day-count convention.
///
/// Year fractions come back as `fractions.Fraction`, because that is
/// what they are: `fasti` computes them as reduced integer rationals and
/// never touches a float. Call `float()` on the result if that is what
/// you need downstream.
///
/// Recognized names (case and punctuation are ignored):
///
/// | Name | Aliases |
/// |---|---|
/// | `ACT/360` | `Actual/360` |
/// | `ACT/365F` | `ACT/365`, `Actual/365 Fixed` |
/// | `ACT/ACT ISDA` | `ACT/ACT`, `Actual/Actual (ISDA)` |
/// | `ACT/ACT ICMA` | `ACT/ACT ISMA` — needs `frequency=` or `schedule=` |
/// | `30/360` | `30/360 Bond Basis`, `Bond Basis` |
/// | `30/360 US` | `30U/360` |
/// | `30E/360` | `30/360 European`, `Eurobond Basis` |
/// | `30E/360 ISDA` | `30/360 German` — needs `termination=` |
///
/// >>> import fasti
/// >>> from datetime import date
/// >>> fasti.DayCount("ACT/360").year_fraction(date(2025, 1, 1), date(2025, 4, 1))
/// Fraction(1, 4)
/// >>> float(fasti.DayCount("ACT/365F").year_fraction(date(2025, 1, 1), date(2026, 1, 1)))
/// 1.0
#[pyclass(module = "fasti", from_py_object, frozen, eq, hash)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DayCount {
    kind: Kind,
}

impl DayCount {
    /// Hand the core-crate convention this stands for to `f`.
    ///
    /// Every method that measures anything goes through here, so the
    /// mapping from name to convention is written once. `ACT/ACT ICMA`
    /// is the only one that can fail: binding it to a schedule checks
    /// that the schedule's tenor matches its coupon frequency.
    fn with<R>(&self, f: impl FnOnce(&dyn fasti::DayCount) -> R) -> PyResult<R> {
        Ok(match &self.kind {
            Kind::Act360 => f(&fasti::Act360),
            Kind::Act365Fixed => f(&fasti::Act365Fixed),
            Kind::ActActISDA => f(&fasti::ActActISDA),
            Kind::ActActICMA {
                frequency,
                schedule,
            } => {
                let convention = fasti::ActActICMA::new(*frequency);
                match schedule {
                    Some(bound) => f(&convention.bind(&bound.inner).map_err(err)?),
                    None => f(&convention),
                }
            }
            Kind::Thirty360Bond => f(&fasti::Thirty360Bond),
            Kind::Thirty360US => f(&fasti::Thirty360US),
            Kind::Thirty360European => f(&fasti::Thirty360European),
            Kind::Thirty360ISDA { termination } => f(&fasti::Thirty360ISDA::new(*termination)),
        })
    }
}

#[pymethods]
impl DayCount {
    /// Build a convention by name.
    ///
    /// `ACT/ACT ICMA` needs the coupon `frequency`, and accrues against
    /// a `schedule` when one is given — without a schedule it treats
    /// every accrual as one whole coupon period. `30E/360 ISDA` needs
    /// the instrument's `termination` date, which its February rule
    /// depends on.
    #[new]
    #[pyo3(signature = (name, *, frequency=None, schedule=None, termination=None))]
    pub fn new(
        name: &str,
        frequency: Option<FrequencyArg>,
        schedule: Option<PyRef<'_, Schedule>>,
        termination: Option<DateArg>,
    ) -> PyResult<Self> {
        let schedule: Option<Schedule> = schedule.map(|s| s.clone());
        let kind = match normalize(name).as_str() {
            "act360" | "actual360" | "a360" => Kind::Act360,
            "act365f" | "act365" | "actual365" | "actual365fixed" | "a365f" | "a365" => {
                Kind::Act365Fixed
            }
            "actact" | "actactisda" | "actualactual" | "actualactualisda" => Kind::ActActISDA,
            "actacticma" | "actualactualicma" | "actactisma" | "actualactualisma" => {
                let frequency = match (frequency, schedule.as_ref()) {
                    (Some(f), _) => f.0,
                    (None, Some(s)) => s
                        .inner
                        .generation()
                        .and_then(|g| fasti::Frequency::try_from(g.tenor).ok())
                        .ok_or_else(|| {
                            invalid(
                                "ACT/ACT ICMA needs frequency=; the schedule's tenor does not \
                                 name a canonical one",
                            )
                        })?,
                    (None, None) => {
                        return Err(invalid(
                            "ACT/ACT ICMA needs the coupon frequency, e.g. \
                             DayCount('ACT/ACT ICMA', frequency='semiannual')",
                        ));
                    }
                };
                Kind::ActActICMA {
                    frequency,
                    schedule,
                }
            }
            "30360" | "30360bondbasis" | "bondbasis" | "thirty360" | "30360isda" => {
                Kind::Thirty360Bond
            }
            "30360us" | "30u360" | "30360usa" | "30360sia" => Kind::Thirty360US,
            "30e360" | "30360european" | "30360eurobond" | "eurobondbasis" | "30360icma" => {
                Kind::Thirty360European
            }
            "30e360isda" | "30360german" | "30360germanic" | "30e360german" => {
                let termination = termination.ok_or_else(|| {
                    invalid(
                        "30E/360 ISDA needs the instrument's maturity, e.g. \
                         DayCount('30E/360 ISDA', termination=date(2030, 1, 15))",
                    )
                })?;
                Kind::Thirty360ISDA {
                    termination: termination.0,
                }
            }
            _ => {
                return Err(invalid(format!(
                    "unknown day-count convention: {name:?} (expected one of ACT/360, \
                     ACT/365F, ACT/ACT ISDA, ACT/ACT ICMA, 30/360, 30/360 US, 30E/360, \
                     30E/360 ISDA)"
                )));
            }
        };
        Ok(Self { kind })
    }

    /// The convention's canonical name.
    #[getter]
    const fn name(&self) -> &'static str {
        match &self.kind {
            Kind::Act360 => "Actual/360",
            Kind::Act365Fixed => "Actual/365 (Fixed)",
            Kind::ActActISDA => "Actual/Actual (ISDA)",
            Kind::ActActICMA { .. } => "Actual/Actual (ICMA)",
            Kind::Thirty360Bond => "30/360 (Bond Basis)",
            Kind::Thirty360US => "30/360 (US)",
            Kind::Thirty360European => "30E/360",
            Kind::Thirty360ISDA { .. } => "30E/360 (ISDA)",
        }
    }

    /// The convention's day count between two dates, signed by
    /// direction. Calendar days for the ACT family; the 30/360 family
    /// counts its own way.
    pub fn day_count(&self, start: DateArg, end: DateArg) -> PyResult<i64> {
        self.with(|convention| convention.day_count(start.0, end.0))
    }

    /// The year fraction between two dates as an exact
    /// `fractions.Fraction`, signed by direction.
    pub fn year_fraction<'py>(
        &self,
        py: Python<'py>,
        start: DateArg,
        end: DateArg,
    ) -> PyResult<Bound<'py, PyAny>> {
        let fraction = self.with(|convention| convention.year_fraction(start.0, end.0))?;
        from_fraction(py, fraction)
    }

    /// The coupon frequency, for `ACT/ACT ICMA`; `None` otherwise.
    #[getter]
    fn frequency(&self) -> Option<crate::enums::Frequency> {
        match &self.kind {
            Kind::ActActICMA { frequency, .. } => Some(crate::enums::Frequency::wrap(*frequency)),
            _ => None,
        }
    }

    /// This convention bound to `schedule` — only `ACT/ACT ICMA` reads
    /// one; every other convention returns itself.
    fn bind(&self, schedule: PyRef<'_, Schedule>) -> Self {
        match &self.kind {
            Kind::ActActICMA { frequency, .. } => Self {
                kind: Kind::ActActICMA {
                    frequency: *frequency,
                    schedule: Some(schedule.clone()),
                },
            },
            _ => self.clone(),
        }
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<crate::pickle::Reduced<'py>> {
        let (frequency, schedule, termination) = match &self.kind {
            Kind::ActActICMA {
                frequency,
                schedule,
            } => (Some(frequency.to_string()), schedule.clone(), None),
            Kind::Thirty360ISDA { termination } => (None, None, Some(DateOut(*termination))),
            _ => (None, None, None),
        };
        crate::pickle::reduce(
            py,
            "_rebuild_daycount",
            (self.name(), frequency, schedule, termination),
        )
    }

    fn __repr__(&self) -> String {
        format!("DayCount({:?})", self.name())
    }

    fn __str__(&self) -> &'static str {
        self.name()
    }
}
