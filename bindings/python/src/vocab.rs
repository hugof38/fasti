//! The four-plus-two vocabularies: one table per crate enum, and a
//! macro that turns each table into a frozen class, the argument type
//! that accepts it, the parser, and the pickle hook.
//!
//! Matching ignores case and punctuation, so `"ModifiedFollowing"`,
//! `"modified_following"` and `"modified following"` are one spelling.
//! The canonical spelling — the crate's own — is what prints, pickles
//! and appears in errors; aliases are convenience only, and two
//! spellings a reader would call synonyms never mean different things.

use fasti::{
    BusinessDayConvention, DateGenerationRule, EasterMethod, Frequency, Weekday, WeekendShift,
};
use pyo3::Borrowed;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;

use crate::convert::FastiError;

/// Case- and punctuation-insensitive matching key.
fn normalize(text: &str) -> String {
    text.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

macro_rules! vocabulary {
    (
        $(#[$meta:meta])*
        $py:ident : $core:ty, $arg:ident, $name:literal,
        [ $( $konst:ident => $variant:ident, $canon:literal $(| $alias:literal)* ; )+ ]
        $( methods { $($methods:tt)* } )?
    ) => {
        $(#[$meta])*
        #[pyclass(frozen, eq, hash, skip_from_py_object, module = "fasti", name = $name)]
        #[derive(Clone, Copy, PartialEq, Eq)]
        pub struct $py(pub $core);

        impl $py {
            /// The canonical spelling: what prints, pickles and appears in errors.
            pub fn canonical(self) -> &'static str {
                match self.0 { $( <$core>::$variant => $canon, )+ }
            }

            /// The class-attribute name, so `repr` round-trips through `eval`.
            fn attr(self) -> &'static str {
                match self.0 { $( <$core>::$variant => stringify!($konst), )+ }
            }

            /// Every accepted spelling, canonical first.
            pub fn parse(text: &str) -> PyResult<Self> {
                let key = normalize(text);
                $(
                    if key == normalize($canon) $( || key == normalize($alias) )* {
                        return Ok(Self(<$core>::$variant));
                    }
                )+
                Err(FastiError::new_err(format!(
                    "unknown {} '{text}'; expected one of {}",
                    $name,
                    [$($canon),+].join(", "),
                )))
            }
        }

        // The core enums do not all derive Hash, and the canonical name
        // is one-to-one with the variant, so it stands in for it.
        impl core::hash::Hash for $py {
            fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
                core::hash::Hash::hash(self.canonical(), state);
            }
        }

        // PyO3 allows exactly one #[pymethods] block per type, so the
        // per-vocabulary extras are spliced in here rather than added
        // in a block of their own.
        #[pymethods]
        impl $py {
            #[new]
            fn new(spelling: $arg) -> Self {
                spelling.0
            }

            $(
                #[classattr]
                #[allow(non_snake_case)]
                fn $konst() -> Self {
                    Self(<$core>::$variant)
                }
            )+

            pub fn __repr__(&self) -> String {
                format!("{}.{}", $name, self.attr())
            }

            pub fn __str__(&self) -> &'static str {
                self.canonical()
            }

            fn __reduce__<'py>(
                slf: &Bound<'py, Self>,
            ) -> (Bound<'py, PyAny>, (&'static str,)) {
                (slf.get_type().into_any(), (slf.get().canonical(),))
            }

            $( $($methods)* )?
        }

        #[doc = concat!("A `", $name, "` argument: the class, or any spelling of one.")]
        pub struct $arg(pub $py);

        impl $arg {
            /// The crate value behind the argument. Some call sites
            /// keep the wrapper instead, so this is not always used.
            #[allow(dead_code)]
            pub fn get(self) -> $core {
                (self.0).0
            }
        }

        impl<'py> FromPyObject<'_, 'py> for $arg {
            type Error = PyErr;

            fn extract(obj: Borrowed<'_, 'py, PyAny>) -> PyResult<Self> {
                if let Ok(value) = obj.cast::<$py>() {
                    return Ok(Self(*value.get()));
                }
                if let Ok(text) = obj.extract::<std::borrow::Cow<'_, str>>() {
                    return $py::parse(&text).map(Self);
                }
                Err(PyTypeError::new_err(format!(
                    "expected a {} or a str naming one, got {}.",
                    $name,
                    obj.get_type().name().map_or_else(
                        |_| "<unknown>".to_owned(), |n| n.to_string()),
                )))
            }
        }
    };
}

vocabulary! {
    /// How to roll a date that is not a business day onto one that is.
    ///
    /// >>> from fasti import BusinessDayConvention
    /// >>> BusinessDayConvention("modified following")
    /// BusinessDayConvention.MODIFIED_FOLLOWING
    /// >>> str(BusinessDayConvention.PRECEDING)
    /// 'Preceding'
    PyBusinessDayConvention : BusinessDayConvention, ConventionArg, "BusinessDayConvention",
    [
        UNADJUSTED => Unadjusted, "Unadjusted";
        FOLLOWING => Following, "Following";
        MODIFIED_FOLLOWING => ModifiedFollowing, "ModifiedFollowing" | "modfollowing";
        PRECEDING => Preceding, "Preceding";
        MODIFIED_PRECEDING => ModifiedPreceding, "ModifiedPreceding" | "modpreceding";
    ]
}

vocabulary! {
    /// A day of the week, ISO numbered — Monday is 1, Sunday is 7.
    ///
    /// >>> from fasti import Weekday
    /// >>> Weekday("saturday").get()
    /// 6
    PyWeekday : Weekday, WeekdayArg, "Weekday",
    [
        MON => Mon, "Mon" | "Monday";
        TUE => Tue, "Tue" | "Tuesday";
        WED => Wed, "Wed" | "Wednesday";
        THU => Thu, "Thu" | "Thursday";
        FRI => Fri, "Fri" | "Friday";
        SAT => Sat, "Sat" | "Saturday";
        SUN => Sun, "Sun" | "Sunday";
    ]
    methods {
        /// The ISO weekday number, 1 (Monday) through 7 (Sunday).
        fn get(&self) -> u8 {
            self.0.get()
        }
    }
}

vocabulary! {
    /// How often a schedule recurs in a year.
    ///
    /// >>> from fasti import Frequency
    /// >>> Frequency.QUARTERLY.per_year()
    /// 4
    PyFrequency : Frequency, FrequencyArg, "Frequency",
    [
        ANNUAL => Annual, "Annual" | "yearly";
        SEMIANNUAL => Semiannual, "Semiannual" | "halfyearly";
        EVERY_FOURTH_MONTH => EveryFourthMonth, "EveryFourthMonth";
        QUARTERLY => Quarterly, "Quarterly";
        BIMONTHLY => Bimonthly, "Bimonthly";
        MONTHLY => Monthly, "Monthly";
        EVERY_FOURTH_WEEK => EveryFourthWeek, "EveryFourthWeek";
        BIWEEKLY => Biweekly, "Biweekly" | "fortnightly";
        WEEKLY => Weekly, "Weekly";
        DAILY => Daily, "Daily";
    ]
    methods {
        /// The number of recurrences per year. Always positive.
        fn per_year(&self) -> u16 {
            self.0.per_year()
        }
    }
}

vocabulary! {
    /// Which way a fixed-date holiday moves when it lands on a weekend.
    ///
    /// `"fed"` is a spelling of `SUN_FORWARD` — the Federal Reserve and
    /// SIFMA convention. `"federal"` is a spelling of nothing: the US
    /// federal convention is `SAT_BACK_SUN_FORWARD`, and one word
    /// standing for two different answers would be a trap.
    ///
    /// >>> from fasti import WeekendShift
    /// >>> WeekendShift("fed")
    /// WeekendShift.SUN_FORWARD
    PyWeekendShift : WeekendShift, ShiftArg, "WeekendShift",
    [
        NONE => None, "None";
        FORWARD => Forward, "Forward";
        SUN_FORWARD => SunForward, "SunForward" | "fed" | "sifma";
        SAT_BACK_SUN_FORWARD => SatBackSunForward, "SatBackSunForward";
    ]
}

vocabulary! {
    /// Which computus dates Easter.
    ///
    /// >>> from fasti import EasterMethod
    /// >>> EasterMethod("julian")
    /// EasterMethod.ORTHODOX
    PyEasterMethod : EasterMethod, EasterMethodArg, "EasterMethod",
    [
        WESTERN => Western, "Western" | "gregorian";
        ORTHODOX => Orthodox, "Orthodox" | "julian";
    ]
}

vocabulary! {
    /// How to walk the schedule grid between effective and termination.
    ///
    /// >>> from fasti import DateGenerationRule
    /// >>> DateGenerationRule("backward")
    /// DateGenerationRule.BACKWARD
    PyDateGenerationRule : DateGenerationRule, GenerationRuleArg, "DateGenerationRule",
    [
        FORWARD => Forward, "Forward";
        BACKWARD => Backward, "Backward";
        ZERO => Zero, "Zero";
    ]
}
