//! The small closed vocabularies — weekdays, conventions, generation
//! rules, frequencies, weekend-shift policies — and the coercions that
//! let a plain string stand in for any of them.
//!
//! Every enum is accepted as either the class member
//! (`BusinessDayConvention.MODIFIED_FOLLOWING`) or a string
//! (`"modified_following"`, `"ModifiedFollowing"`, `"mf"`). Matching
//! ignores case and punctuation, so a spelling that reads naturally in
//! a config file works too.

use pyo3::prelude::*;
use pyo3::{Borrowed, exceptions::PyTypeError};

use crate::convert::{normalize, type_name};
use crate::error::invalid;

// ---- Weekday ------------------------------------------------------------

/// A day of the week, numbered as `datetime.date.isoweekday()`:
/// Monday is 1, Sunday is 7.
#[pyclass(module = "fasti", from_py_object, frozen, eq, eq_int, hash, ord)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Weekday {
    #[pyo3(name = "MON")]
    Mon = 1,
    #[pyo3(name = "TUE")]
    Tue = 2,
    #[pyo3(name = "WED")]
    Wed = 3,
    #[pyo3(name = "THU")]
    Thu = 4,
    #[pyo3(name = "FRI")]
    Fri = 5,
    #[pyo3(name = "SAT")]
    Sat = 6,
    #[pyo3(name = "SUN")]
    Sun = 7,
}

#[pymethods]
impl Weekday {
    /// The ISO weekday number, matching `datetime.date.isoweekday()`.
    #[getter]
    const fn isoweekday(&self) -> u8 {
        self.inner().get()
    }

    /// The `datetime.date.weekday()` number, where Monday is 0.
    #[getter]
    const fn weekday(&self) -> u8 {
        self.inner().get() - 1
    }

    /// Coerce a weekday name (`"mon"`, `"Monday"`), an ISO number
    /// (1–7), or a `Weekday` to a `Weekday`.
    #[staticmethod]
    fn parse(value: WeekdayArg) -> Self {
        value.0
    }

    fn __str__(&self) -> String {
        self.inner().to_string()
    }

    fn __int__(&self) -> u8 {
        self.inner().get()
    }
}

impl Weekday {
    pub const fn inner(self) -> fasti::Weekday {
        match self {
            Self::Mon => fasti::Weekday::Mon,
            Self::Tue => fasti::Weekday::Tue,
            Self::Wed => fasti::Weekday::Wed,
            Self::Thu => fasti::Weekday::Thu,
            Self::Fri => fasti::Weekday::Fri,
            Self::Sat => fasti::Weekday::Sat,
            Self::Sun => fasti::Weekday::Sun,
        }
    }

    pub const fn wrap(w: fasti::Weekday) -> Self {
        match w {
            fasti::Weekday::Mon => Self::Mon,
            fasti::Weekday::Tue => Self::Tue,
            fasti::Weekday::Wed => Self::Wed,
            fasti::Weekday::Thu => Self::Thu,
            fasti::Weekday::Fri => Self::Fri,
            fasti::Weekday::Sat => Self::Sat,
            fasti::Weekday::Sun => Self::Sun,
        }
    }
}

/// A weekday-valued argument: a [`Weekday`], a name, or an ISO number.
#[derive(Debug, Clone, Copy)]
pub struct WeekdayArg(pub Weekday);

impl FromPyObject<'_, '_> for WeekdayArg {
    type Error = PyErr;

    fn extract(ob: Borrowed<'_, '_, PyAny>) -> Result<Self, Self::Error> {
        if let Ok(w) = ob.extract::<Weekday>() {
            return Ok(Self(w));
        }
        if let Ok(name) = ob.extract::<String>() {
            let key = normalize(&name);
            let day = match key.as_str() {
                "mon" | "monday" | "mo" => Weekday::Mon,
                "tue" | "tuesday" | "tues" | "tu" => Weekday::Tue,
                "wed" | "wednesday" | "we" => Weekday::Wed,
                "thu" | "thursday" | "thur" | "thurs" | "th" => Weekday::Thu,
                "fri" | "friday" | "fr" => Weekday::Fri,
                "sat" | "saturday" | "sa" => Weekday::Sat,
                "sun" | "sunday" | "su" => Weekday::Sun,
                _ => return Err(invalid(format!("unknown weekday name: {name:?}"))),
            };
            return Ok(Self(day));
        }
        if let Ok(n) = ob.extract::<i64>() {
            let day = match n {
                1 => Weekday::Mon,
                2 => Weekday::Tue,
                3 => Weekday::Wed,
                4 => Weekday::Thu,
                5 => Weekday::Fri,
                6 => Weekday::Sat,
                7 => Weekday::Sun,
                _ => {
                    return Err(invalid(format!(
                        "weekday number must be 1..=7 (ISO: Mon=1, Sun=7 — as \
                         date.isoweekday(), not date.weekday()), got {n}"
                    )));
                }
            };
            return Ok(Self(day));
        }
        Err(PyTypeError::new_err(format!(
            "expected a Weekday, a weekday name, or an ISO number 1..=7, got {}",
            type_name(&ob)
        )))
    }
}

// ---- Month --------------------------------------------------------------

/// A month-valued argument: a number 1..=12 or a name (`"jul"`, `"July"`).
#[derive(Debug, Clone, Copy)]
pub struct MonthArg(pub fasti::Month);

impl FromPyObject<'_, '_> for MonthArg {
    type Error = PyErr;

    fn extract(ob: Borrowed<'_, '_, PyAny>) -> Result<Self, Self::Error> {
        if let Ok(n) = ob.extract::<u8>() {
            return fasti::Month::try_from_u8(n)
                .map(Self)
                .map_err(crate::error::err);
        }
        if let Ok(name) = ob.extract::<String>() {
            use fasti::Month as M;
            const MONTHS: [(&str, M); 12] = [
                ("january", M::Jan),
                ("february", M::Feb),
                ("march", M::Mar),
                ("april", M::Apr),
                ("may", M::May),
                ("june", M::Jun),
                ("july", M::Jul),
                ("august", M::Aug),
                ("september", M::Sep),
                ("october", M::Oct),
                ("november", M::Nov),
                ("december", M::Dec),
            ];
            // A three-letter prefix is the usual spelling; anything
            // shorter is too ambiguous to guess at.
            let key = normalize(&name);
            let found = MONTHS
                .iter()
                .find(|(full, _)| *full == key || (key.len() >= 3 && full.starts_with(&key)));
            if let Some((_, month)) = found {
                return Ok(Self(*month));
            }
            return Err(invalid(format!("unknown month name: {name:?}")));
        }
        Err(PyTypeError::new_err(format!(
            "expected a month number 1..=12 or a month name, got {}",
            type_name(&ob)
        )))
    }
}

// ---- Business-day convention -------------------------------------------

/// What to do when an adjusted date is not a business day.
#[pyclass(module = "fasti", from_py_object, frozen, eq, eq_int, hash)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BusinessDayConvention {
    /// The next business day.
    #[pyo3(name = "FOLLOWING")]
    Following,
    /// The next business day, unless that crosses into the next month,
    /// in which case the previous one.
    #[pyo3(name = "MODIFIED_FOLLOWING")]
    ModifiedFollowing,
    /// The previous business day.
    #[pyo3(name = "PRECEDING")]
    Preceding,
    /// The previous business day, unless that crosses into the previous
    /// month, in which case the next one.
    #[pyo3(name = "MODIFIED_PRECEDING")]
    ModifiedPreceding,
    /// Leave the date alone.
    #[pyo3(name = "UNADJUSTED")]
    Unadjusted,
}

#[pymethods]
impl BusinessDayConvention {
    /// Coerce a convention name to a `BusinessDayConvention`.
    #[staticmethod]
    fn parse(value: ConventionArg) -> Self {
        Self::wrap(value.0)
    }

    fn __str__(&self) -> String {
        self.inner().to_string()
    }
}

impl BusinessDayConvention {
    pub const fn inner(self) -> fasti::BusinessDayConvention {
        use fasti::BusinessDayConvention as C;
        match self {
            Self::Following => C::Following,
            Self::ModifiedFollowing => C::ModifiedFollowing,
            Self::Preceding => C::Preceding,
            Self::ModifiedPreceding => C::ModifiedPreceding,
            Self::Unadjusted => C::Unadjusted,
        }
    }

    pub const fn wrap(c: fasti::BusinessDayConvention) -> Self {
        use fasti::BusinessDayConvention as C;
        match c {
            C::Following => Self::Following,
            C::ModifiedFollowing => Self::ModifiedFollowing,
            C::Preceding => Self::Preceding,
            C::ModifiedPreceding => Self::ModifiedPreceding,
            C::Unadjusted => Self::Unadjusted,
        }
    }
}

/// A convention-valued argument: the enum or a name.
#[derive(Debug, Clone, Copy)]
pub struct ConventionArg(pub fasti::BusinessDayConvention);

impl FromPyObject<'_, '_> for ConventionArg {
    type Error = PyErr;

    fn extract(ob: Borrowed<'_, '_, PyAny>) -> Result<Self, Self::Error> {
        use fasti::BusinessDayConvention as C;
        if let Ok(c) = ob.extract::<BusinessDayConvention>() {
            return Ok(Self(c.inner()));
        }
        let Ok(name) = ob.extract::<String>() else {
            return Err(PyTypeError::new_err(format!(
                "expected a BusinessDayConvention or a convention name, got {}",
                type_name(&ob)
            )));
        };
        let convention = match normalize(&name).as_str() {
            "following" | "f" | "succeeding" => C::Following,
            "modifiedfollowing" | "mf" | "modfollowing" => C::ModifiedFollowing,
            "preceding" | "p" | "previous" => C::Preceding,
            "modifiedpreceding" | "mp" | "modpreceding" => C::ModifiedPreceding,
            "unadjusted" | "u" | "none" | "nil" => C::Unadjusted,
            _ => {
                return Err(invalid(format!(
                    "unknown business-day convention: {name:?} (expected one of \
                     following, modified_following, preceding, modified_preceding, unadjusted)"
                )));
            }
        };
        Ok(Self(convention))
    }
}

// ---- Date generation ----------------------------------------------------

/// Which end of a schedule the regular grid is anchored to.
#[pyclass(module = "fasti", from_py_object, frozen, eq, eq_int, hash)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DateGenerationRule {
    /// Step forward from the effective date; any stub lands at the back.
    #[pyo3(name = "FORWARD")]
    Forward,
    /// Step backward from termination; any stub lands at the front.
    #[pyo3(name = "BACKWARD")]
    Backward,
    /// No interior dates: effective and termination only.
    #[pyo3(name = "ZERO")]
    Zero,
}

#[pymethods]
impl DateGenerationRule {
    /// Coerce a rule name to a `DateGenerationRule`.
    #[staticmethod]
    fn parse(value: GenerationArg) -> Self {
        Self::wrap(value.0)
    }
}

impl DateGenerationRule {
    pub const fn inner(self) -> fasti::DateGenerationRule {
        use fasti::DateGenerationRule as R;
        match self {
            Self::Forward => R::Forward,
            Self::Backward => R::Backward,
            Self::Zero => R::Zero,
        }
    }

    pub const fn wrap(r: fasti::DateGenerationRule) -> Self {
        use fasti::DateGenerationRule as R;
        match r {
            R::Forward => Self::Forward,
            R::Backward => Self::Backward,
            R::Zero => Self::Zero,
        }
    }
}

/// A generation-rule argument: the enum or a name.
#[derive(Debug, Clone, Copy)]
pub struct GenerationArg(pub fasti::DateGenerationRule);

impl FromPyObject<'_, '_> for GenerationArg {
    type Error = PyErr;

    fn extract(ob: Borrowed<'_, '_, PyAny>) -> Result<Self, Self::Error> {
        use fasti::DateGenerationRule as R;
        if let Ok(r) = ob.extract::<DateGenerationRule>() {
            return Ok(Self(r.inner()));
        }
        let Ok(name) = ob.extract::<String>() else {
            return Err(PyTypeError::new_err(format!(
                "expected a DateGenerationRule or a rule name, got {}",
                type_name(&ob)
            )));
        };
        let rule = match normalize(&name).as_str() {
            "forward" | "forwards" => R::Forward,
            "backward" | "backwards" => R::Backward,
            "zero" => R::Zero,
            _ => {
                return Err(invalid(format!(
                    "unknown date generation rule: {name:?} (expected forward, backward, or zero)"
                )));
            }
        };
        Ok(Self(rule))
    }
}

// ---- Frequency ----------------------------------------------------------

/// A coupon frequency — how many times a year a payment recurs.
#[pyclass(module = "fasti", from_py_object, frozen, eq, eq_int, hash, ord)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Frequency {
    #[pyo3(name = "ANNUAL")]
    Annual = 1,
    #[pyo3(name = "SEMIANNUAL")]
    Semiannual = 2,
    #[pyo3(name = "EVERY_FOURTH_MONTH")]
    EveryFourthMonth = 3,
    #[pyo3(name = "QUARTERLY")]
    Quarterly = 4,
    #[pyo3(name = "BIMONTHLY")]
    Bimonthly = 6,
    #[pyo3(name = "MONTHLY")]
    Monthly = 12,
    #[pyo3(name = "EVERY_FOURTH_WEEK")]
    EveryFourthWeek = 13,
    #[pyo3(name = "BIWEEKLY")]
    Biweekly = 26,
    #[pyo3(name = "WEEKLY")]
    Weekly = 52,
    #[pyo3(name = "DAILY")]
    Daily = 365,
}

#[pymethods]
impl Frequency {
    /// Recurrences per year.
    #[getter]
    const fn per_year(&self) -> u16 {
        self.inner().per_year()
    }

    /// The canonical `Period` for this frequency, e.g. `Period("6M")`
    /// for `SEMIANNUAL`.
    #[getter]
    fn period(&self) -> crate::period::Period {
        crate::period::Period(fasti::Period::from(self.inner()))
    }

    /// Coerce a frequency name, a `Period`, or a `Frequency`.
    #[staticmethod]
    fn parse(value: FrequencyArg) -> Self {
        Self::wrap(value.0)
    }

    fn __str__(&self) -> String {
        self.inner().to_string()
    }
}

impl Frequency {
    pub const fn inner(self) -> fasti::Frequency {
        use fasti::Frequency as F;
        match self {
            Self::Annual => F::Annual,
            Self::Semiannual => F::Semiannual,
            Self::EveryFourthMonth => F::EveryFourthMonth,
            Self::Quarterly => F::Quarterly,
            Self::Bimonthly => F::Bimonthly,
            Self::Monthly => F::Monthly,
            Self::EveryFourthWeek => F::EveryFourthWeek,
            Self::Biweekly => F::Biweekly,
            Self::Weekly => F::Weekly,
            Self::Daily => F::Daily,
        }
    }

    pub const fn wrap(f: fasti::Frequency) -> Self {
        use fasti::Frequency as F;
        match f {
            F::Annual => Self::Annual,
            F::Semiannual => Self::Semiannual,
            F::EveryFourthMonth => Self::EveryFourthMonth,
            F::Quarterly => Self::Quarterly,
            F::Bimonthly => Self::Bimonthly,
            F::Monthly => Self::Monthly,
            F::EveryFourthWeek => Self::EveryFourthWeek,
            F::Biweekly => Self::Biweekly,
            F::Weekly => Self::Weekly,
            F::Daily => Self::Daily,
        }
    }
}

/// A frequency-valued argument: the enum, a name, or a `Period`
/// (`"6M"` is `SEMIANNUAL`).
#[derive(Debug, Clone, Copy)]
pub struct FrequencyArg(pub fasti::Frequency);

impl FromPyObject<'_, '_> for FrequencyArg {
    type Error = PyErr;

    fn extract(ob: Borrowed<'_, '_, PyAny>) -> Result<Self, Self::Error> {
        use fasti::Frequency as F;
        if let Ok(f) = ob.extract::<Frequency>() {
            return Ok(Self(f.inner()));
        }
        if let Ok(name) = ob.extract::<String>() {
            let key = normalize(&name);
            let frequency = match key.as_str() {
                "annual" | "annually" | "yearly" | "1y" | "12m" => Some(F::Annual),
                "semiannual" | "semiannually" | "halfyearly" | "6m" => Some(F::Semiannual),
                "everyfourthmonth" | "4m" | "triannual" => Some(F::EveryFourthMonth),
                "quarterly" | "quarter" | "3m" => Some(F::Quarterly),
                "bimonthly" | "2m" => Some(F::Bimonthly),
                "monthly" | "1m" => Some(F::Monthly),
                "everyfourthweek" | "4w" => Some(F::EveryFourthWeek),
                "biweekly" | "fortnightly" | "2w" => Some(F::Biweekly),
                "weekly" | "1w" => Some(F::Weekly),
                "daily" | "1d" => Some(F::Daily),
                _ => None,
            };
            if let Some(f) = frequency {
                return Ok(Self(f));
            }
        }
        // Anything Period accepts, if it maps onto a canonical frequency.
        let period = crate::period::PeriodArg::extract(ob).map_err(|_| {
            PyTypeError::new_err(format!(
                "expected a Frequency, a frequency name, or a Period, got {}",
                type_name(&ob)
            ))
        })?;
        F::try_from(period.0)
            .map(Self)
            .map_err(|_| invalid(format!("{} does not name a canonical frequency", period.0)))
    }
}

// ---- Weekend shift ------------------------------------------------------

/// Which way a fixed-date holiday moves when it falls on a weekend.
#[pyclass(module = "fasti", from_py_object, frozen, eq, eq_int, hash)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WeekendShift {
    /// Neither weekend day moves; the holiday is lost (France, TARGET).
    #[pyo3(name = "NONE")]
    None,
    /// Both weekend days move forward (UK and Commonwealth).
    #[pyo3(name = "FORWARD")]
    Forward,
    /// Sunday moves forward, Saturday stays (Fed, SIFMA).
    #[pyo3(name = "SUN_FORWARD")]
    SunForward,
    /// Saturday moves back, Sunday forward (US federal).
    #[pyo3(name = "SAT_BACK_SUN_FORWARD")]
    SatBackSunForward,
}

#[pymethods]
impl WeekendShift {
    /// Coerce a shift name to a `WeekendShift`.
    #[staticmethod]
    fn parse(value: ShiftArg) -> Self {
        Self::wrap(value.0)
    }
}

impl WeekendShift {
    pub const fn inner(self) -> fasti::WeekendShift {
        use fasti::WeekendShift as S;
        match self {
            Self::None => S::None,
            Self::Forward => S::Forward,
            Self::SunForward => S::SunForward,
            Self::SatBackSunForward => S::SatBackSunForward,
        }
    }

    pub const fn wrap(s: fasti::WeekendShift) -> Self {
        use fasti::WeekendShift as S;
        match s {
            S::None => Self::None,
            S::Forward => Self::Forward,
            S::SunForward => Self::SunForward,
            S::SatBackSunForward => Self::SatBackSunForward,
        }
    }
}

/// A weekend-shift argument: the enum or a name.
#[derive(Debug, Clone, Copy)]
pub struct ShiftArg(pub fasti::WeekendShift);

impl FromPyObject<'_, '_> for ShiftArg {
    type Error = PyErr;

    fn extract(ob: Borrowed<'_, '_, PyAny>) -> Result<Self, Self::Error> {
        use fasti::WeekendShift as S;
        if let Ok(s) = ob.extract::<WeekendShift>() {
            return Ok(Self(s.inner()));
        }
        let Ok(name) = ob.extract::<String>() else {
            return Err(PyTypeError::new_err(format!(
                "expected a WeekendShift or a shift name, got {}",
                type_name(&ob)
            )));
        };
        let shift = match normalize(&name).as_str() {
            "none" | "lost" => S::None,
            "forward" | "uk" => S::Forward,
            "sunforward" | "fed" | "sifma" => S::SunForward,
            "satbacksunforward" | "us" | "federal" => S::SatBackSunForward,
            _ => {
                return Err(invalid(format!(
                    "unknown weekend shift: {name:?} (expected none, forward, \
                     sun_forward, or sat_back_sun_forward)"
                )));
            }
        };
        Ok(Self(shift))
    }
}

// ---- Weekend ------------------------------------------------------------

/// A weekend-valued argument: a name (`"sat_sun"`, `"fri_sat"`,
/// `"sun"`, `"none"`) or any iterable of weekdays.
#[derive(Debug, Clone, Copy)]
pub struct WeekendArg(pub fasti::Weekend);

impl FromPyObject<'_, '_> for WeekendArg {
    type Error = PyErr;

    fn extract(ob: Borrowed<'_, '_, PyAny>) -> Result<Self, Self::Error> {
        use fasti::Weekend as W;
        if let Ok(name) = ob.extract::<String>() {
            let weekend = match normalize(&name).as_str() {
                "satsun" | "saturdaysunday" | "weekend" | "default" => W::SAT_SUN,
                "frisat" | "fridaysaturday" => W::FRI_SAT,
                "sun" | "sunonly" | "sunday" => W::SUN_ONLY,
                "none" | "nil" | "sevenday" => W::NONE,
                _ => {
                    return Err(invalid(format!(
                        "unknown weekend: {name:?} (expected sat_sun, fri_sat, sun, none, \
                         or a list of weekdays)"
                    )));
                }
            };
            return Ok(Self(weekend));
        }
        let items = ob.try_iter().map_err(|_| {
            PyTypeError::new_err(format!(
                "expected a weekend name or an iterable of weekdays, got {}",
                type_name(&ob)
            ))
        })?;
        // Extract element by element so that a bad weekday reports what
        // is wrong with *it* rather than "this is not a weekend".
        let mut days = Vec::new();
        for item in items {
            days.push(item?.extract::<WeekdayArg>()?.0.inner());
        }
        Ok(Self(W::from_weekdays(&days)))
    }
}

/// Render a [`fasti::WeekendShift`] the way [`ShiftArg`] accepts it back.
pub fn shift_repr(shift: fasti::WeekendShift) -> &'static str {
    use fasti::WeekendShift as S;
    match shift {
        S::None => "none",
        S::Forward => "forward",
        S::SunForward => "sun_forward",
        S::SatBackSunForward => "sat_back_sun_forward",
    }
}

/// Enumerate the weekdays a [`fasti::Weekend`] contains.
pub fn weekend_days(weekend: fasti::Weekend) -> Vec<Weekday> {
    use fasti::Weekday as D;
    [D::Mon, D::Tue, D::Wed, D::Thu, D::Fri, D::Sat, D::Sun]
        .into_iter()
        .filter(|d| weekend.contains(*d))
        .map(Weekday::wrap)
        .collect()
}

/// Render a [`fasti::Weekend`] the way [`WeekendArg`] accepts it back.
pub fn weekend_repr(weekend: fasti::Weekend) -> String {
    let days = weekend_days(weekend);
    if days.is_empty() {
        return "none".to_owned();
    }
    days.iter()
        .map(|d| d.inner().to_string().to_lowercase())
        .collect::<Vec<_>>()
        .join("_")
}

// ---- Easter method ------------------------------------------------------

/// An Easter-computus argument: `"western"` or `"orthodox"`.
#[derive(Debug, Clone, Copy)]
pub struct EasterMethodArg(pub fasti::EasterMethod);

impl FromPyObject<'_, '_> for EasterMethodArg {
    type Error = PyErr;

    fn extract(ob: Borrowed<'_, '_, PyAny>) -> Result<Self, Self::Error> {
        use fasti::EasterMethod as M;
        let Ok(name) = ob.extract::<String>() else {
            return Err(PyTypeError::new_err(format!(
                "expected 'western' or 'orthodox', got {}",
                type_name(&ob)
            )));
        };
        let method = match normalize(&name).as_str() {
            "western" | "gregorian" | "catholic" | "protestant" => M::Western,
            "orthodox" | "julian" | "eastern" => M::Orthodox,
            _ => {
                return Err(invalid(format!(
                    "unknown Easter method: {name:?} (expected 'western' or 'orthodox')"
                )));
            }
        };
        Ok(Self(method))
    }
}
