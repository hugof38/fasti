//! The small closed vocabularies — weekdays, conventions, generation
//! rules, frequencies, weekend-shift policies — and the coercions that
//! let a plain string stand in for any of them.
//!
//! Every enum is accepted as either the class member
//! (`BusinessDayConvention.MODIFIED_FOLLOWING`) or a string
//! (`"modified_following"`, `"ModifiedFollowing"`, `"mf"`). Matching
//! ignores case and punctuation, so a spelling that reads naturally in
//! a config file works too.
//!
//! Each vocabulary is a table: the members, the core-crate value each
//! stands for, the name it prints and pickles as, and the spellings it
//! answers to. `named_enum!` turns that into the conversions, the
//! argument type, the parser and the pickle support, so the table stays
//! the only part worth reading.

use pyo3::prelude::*;
use pyo3::{Borrowed, exceptions::PyTypeError};

use crate::convert::{normalize, type_name};
use crate::error::invalid;

/// Generate everything mechanical about a vocabulary enum.
///
/// Spellings are matched after [`normalize`], so they are written
/// lowercase and unpunctuated. `methods` is spliced into the generated
/// `#[pymethods]` block, because `PyO3` allows a type only one.
macro_rules! named_enum {
    (
        $py:ident => $core:path,
        arg: $arg:ident,
        rebuild: $rebuild:literal,
        expected: $expected:literal,
        members {
            $( $member:ident => $variant:ident, $canonical:literal, [$($spelling:literal),+] ; )+
        }
        $( methods { $($methods:tt)* } )?
        $( fallback: $fallback:expr, )?
    ) => {
        impl $py {
            /// The core-crate value this member stands for.
            pub const fn inner(self) -> $core {
                match self { $( Self::$member => <$core>::$variant, )+ }
            }

            /// The member standing for a core-crate value.
            pub const fn wrap(value: $core) -> Self {
                match value { $( <$core>::$variant => Self::$member, )+ }
            }

            /// The name this member prints and pickles as.
            pub const fn canonical(self) -> &'static str {
                match self { $( Self::$member => $canonical, )+ }
            }

            /// Resolve a spelling, ignoring case and punctuation.
            pub fn from_name(name: &str) -> Option<Self> {
                match normalize(name).as_str() {
                    $( $($spelling)|+ => Some(Self::$member), )+
                    _ => None,
                }
            }

            /// The canonical names, for an error message.
            fn spellings() -> String {
                [$($canonical),+].join(", ")
            }
        }

        #[pymethods]
        impl $py {
            $( $($methods)* )?

            /// Coerce a name — or a member — to a member.
            #[staticmethod]
            fn parse(value: $arg) -> Self {
                Self::wrap(value.0)
            }

            fn __str__(&self) -> &'static str {
                self.canonical()
            }

            fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<crate::pickle::Reduced<'py>> {
                crate::pickle::reduce(py, $rebuild, (self.canonical(),))
            }
        }

        #[doc = concat!("A ", $expected, ", as an argument: the member or a name.")]
        #[derive(Debug, Clone, Copy)]
        pub struct $arg(pub $core);

        impl FromPyObject<'_, '_> for $arg {
            type Error = PyErr;

            fn extract(ob: Borrowed<'_, '_, PyAny>) -> Result<Self, Self::Error> {
                if let Ok(member) = ob.extract::<$py>() {
                    return Ok(Self(member.inner()));
                }
                if let Ok(name) = ob.extract::<String>() {
                    return match <$py>::from_name(&name) {
                        Some(member) => Ok(Self(member.inner())),
                        None => Err(invalid(format!(
                            concat!("unknown ", $expected, ": {:?} (expected one of {})"),
                            name,
                            <$py>::spellings(),
                        ))),
                    };
                }
                $( if let Some(result) = ($fallback)(&ob) { return result.map(Self); } )?
                Err(PyTypeError::new_err(format!(
                    concat!("expected a ", $expected, ", got {}"),
                    type_name(&ob)
                )))
            }
        }
    };
}

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

named_enum! {
    Weekday => fasti::Weekday,
    arg: WeekdayArg,
    rebuild: "_rebuild_weekday",
    expected: "weekday",
    members {
        Mon => Mon, "Mon", ["mon", "monday", "mo"];
        Tue => Tue, "Tue", ["tue", "tuesday", "tues", "tu"];
        Wed => Wed, "Wed", ["wed", "wednesday", "we"];
        Thu => Thu, "Thu", ["thu", "thursday", "thur", "thurs", "th"];
        Fri => Fri, "Fri", ["fri", "friday", "fr"];
        Sat => Sat, "Sat", ["sat", "saturday", "sa"];
        Sun => Sun, "Sun", ["sun", "sunday", "su"];
    }
    methods {
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

        fn __int__(&self) -> u8 {
            self.inner().get()
        }
    }
    // A weekday is also spelled as its ISO number, which is what
    // `date.isoweekday()` hands you.
    fallback: |ob: &Borrowed<'_, '_, PyAny>| {
        let n = ob.extract::<i64>().ok()?;
        Some(match u8::try_from(n).ok().and_then(|n| fasti::Weekday::try_from_u8(n).ok()) {
            Some(weekday) => Ok(weekday),
            None => Err(invalid(format!(
                "weekday number must be 1..=7 (ISO: Mon=1, Sun=7 — as \
                 date.isoweekday(), not date.weekday()), got {n}"
            ))),
        })
    },
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

named_enum! {
    BusinessDayConvention => fasti::BusinessDayConvention,
    arg: ConventionArg,
    rebuild: "_rebuild_convention",
    expected: "business-day convention",
    members {
        Following => Following, "Following", ["following", "f", "succeeding"];
        ModifiedFollowing => ModifiedFollowing, "ModifiedFollowing",
            ["modifiedfollowing", "mf", "modfollowing"];
        Preceding => Preceding, "Preceding", ["preceding", "p", "previous"];
        ModifiedPreceding => ModifiedPreceding, "ModifiedPreceding",
            ["modifiedpreceding", "mp", "modpreceding"];
        Unadjusted => Unadjusted, "Unadjusted", ["unadjusted", "u", "none", "nil"];
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

named_enum! {
    DateGenerationRule => fasti::DateGenerationRule,
    arg: GenerationArg,
    rebuild: "_rebuild_generation",
    expected: "date generation rule",
    members {
        Forward => Forward, "forward", ["forward", "forwards"];
        Backward => Backward, "backward", ["backward", "backwards"];
        Zero => Zero, "zero", ["zero"];
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

named_enum! {
    Frequency => fasti::Frequency,
    arg: FrequencyArg,
    rebuild: "_rebuild_frequency",
    expected: "frequency",
    members {
        Annual => Annual, "Annual", ["annual", "annually", "yearly", "1y", "12m"];
        Semiannual => Semiannual, "Semiannual", ["semiannual", "semiannually", "halfyearly", "6m"];
        EveryFourthMonth => EveryFourthMonth, "EveryFourthMonth",
            ["everyfourthmonth", "triannual", "4m"];
        Quarterly => Quarterly, "Quarterly", ["quarterly", "quarter", "3m"];
        Bimonthly => Bimonthly, "Bimonthly", ["bimonthly", "2m"];
        Monthly => Monthly, "Monthly", ["monthly", "1m"];
        EveryFourthWeek => EveryFourthWeek, "EveryFourthWeek", ["everyfourthweek", "4w"];
        Biweekly => Biweekly, "Biweekly", ["biweekly", "fortnightly", "2w"];
        Weekly => Weekly, "Weekly", ["weekly", "1w"];
        Daily => Daily, "Daily", ["daily", "1d"];
    }
    methods {
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
    }
    // Any period that names a canonical frequency is one.
    fallback: |ob: &Borrowed<'_, '_, PyAny>| {
        let period = crate::period::PeriodArg::extract(*ob).ok()?;
        Some(
            fasti::Frequency::try_from(period.0)
                .map_err(|_| invalid(format!("{} does not name a canonical frequency", period.0))),
        )
    },
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

named_enum! {
    WeekendShift => fasti::WeekendShift,
    arg: ShiftArg,
    rebuild: "_rebuild_shift",
    expected: "weekend shift",
    members {
        None => None, "none", ["none", "lost"];
        Forward => Forward, "forward", ["forward", "uk"];
        SunForward => SunForward, "sun_forward", ["sunforward", "fed", "sifma"];
        SatBackSunForward => SatBackSunForward, "sat_back_sun_forward",
            ["satbacksunforward", "us", "federal"];
    }
}

/// Render a [`fasti::WeekendShift`] the way [`ShiftArg`] accepts it back.
pub fn shift_repr(shift: fasti::WeekendShift) -> &'static str {
    WeekendShift::wrap(shift).canonical()
}

// ---- Month --------------------------------------------------------------

/// A month-valued argument: a number 1..=12 or a name (`"jul"`, `"July"`).
#[derive(Debug, Clone, Copy)]
pub struct MonthArg(pub fasti::Month);

impl FromPyObject<'_, '_> for MonthArg {
    type Error = PyErr;

    fn extract(ob: Borrowed<'_, '_, PyAny>) -> Result<Self, Self::Error> {
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
        if let Ok(n) = ob.extract::<u8>() {
            return M::try_from_u8(n).map(Self).map_err(crate::error::err);
        }
        let Ok(name) = ob.extract::<String>() else {
            return Err(PyTypeError::new_err(format!(
                "expected a month number 1..=12 or a month name, got {}",
                type_name(&ob)
            )));
        };
        // A three-letter prefix is the usual spelling; anything shorter
        // is too ambiguous to guess at.
        let key = normalize(&name);
        MONTHS
            .iter()
            .find(|(full, _)| *full == key || (key.len() >= 3 && full.starts_with(&key)))
            .map(|(_, month)| Self(*month))
            .ok_or_else(|| invalid(format!("unknown month name: {name:?}")))
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
            days.push(item?.extract::<WeekdayArg>()?.0);
        }
        Ok(Self(W::from_weekdays(&days)))
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
        .map(|d| d.canonical().to_lowercase())
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
        match normalize(&name).as_str() {
            "western" | "gregorian" | "catholic" | "protestant" => Ok(Self(M::Western)),
            "orthodox" | "julian" | "eastern" => Ok(Self(M::Orthodox)),
            _ => Err(invalid(format!(
                "unknown Easter method: {name:?} (expected 'western' or 'orthodox')"
            ))),
        }
    }
}
