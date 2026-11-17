//! serde formats, behind the `serde` feature. [`Date`] is `"YYYY-MM-DD"`
//! and [`Period`] is `"6M"`, both through their `Display` / `FromStr`.
//! Every type validates on the way in, so no input deserializes into a
//! value the rest of the crate would panic on.

use alloc::string::{String, ToString};

use crate::{Date, Period, TimeError};

/// The string form [`Date`] and [`Period`] serialize through.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub(crate) struct Text(String);

impl From<Date> for Text {
    fn from(date: Date) -> Self {
        Self(date.to_string())
    }
}

impl TryFrom<Text> for Date {
    type Error = TimeError;

    fn try_from(text: Text) -> Result<Self, TimeError> {
        text.0.parse()
    }
}

impl From<Period> for Text {
    fn from(period: Period) -> Self {
        Self(period.to_string())
    }
}

impl TryFrom<Text> for Period {
    type Error = TimeError;

    fn try_from(text: Text) -> Result<Self, TimeError> {
        text.0.parse()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use crate::{Date, FixedDate, Fraction, Month, Period, Schedule, Weekend, Year, YearRange};

    fn parse<T: serde::de::DeserializeOwned>(json: &str) -> Result<T, serde_json::Error> {
        serde_json::from_str(json)
    }

    #[test]
    fn dates_and_periods_are_strings() {
        let d = Date::from_ymd(2026, Month::Nov, 17).unwrap();
        assert_eq!(serde_json::to_string(&d).unwrap(), "\"2026-11-17\"");
        assert_eq!(parse::<Date>("\"2026-11-17\"").unwrap(), d);
        assert_eq!(serde_json::to_string(&Period::Months(6)).unwrap(), "\"6M\"");
        assert_eq!(parse::<Period>("\"6M\"").unwrap(), Period::Months(6));
        for bad in [
            "\"2026-02-30\"",
            "\"1900-12-31\"",
            "\"17/11/2026\"",
            "45976",
        ] {
            assert!(parse::<Date>(bad).is_err(), "{bad}");
        }
        assert!(parse::<Period>("\"6 months\"").is_err());
    }

    #[test]
    fn hostile_input_is_refused_before_it_can_reach_a_panic() {
        // Each of these used to deserialize, then panic on first use.
        assert!(parse::<Year>("5000").is_err());
        assert!(parse::<Weekend>("255").is_err());
        let schedule = |dates: &str, payments: &str| {
            parse::<Schedule>(&alloc::format!(
                r#"{{"unadjusted":{dates},"dates":{dates},"payments":{payments},"stubs":{{"front":null,"back":null}},"generation":null}}"#
            ))
        };
        assert!(schedule(r#"["2026-01-01","2026-07-01"]"#, r#"["2026-07-01"]"#).is_ok());
        assert!(schedule(r#"["2026-01-01","2200-01-01"]"#, r#"["2200-01-01"]"#).is_err());
        assert!(schedule(r#"["2026-01-01","2026-07-01"]"#, "[]").is_err());
        assert!(parse::<YearRange>(r#"{"from":2020,"to":1990}"#).is_err());
        assert!(
            parse::<FixedDate>(
                r#"{"month":"Feb","day":30,"shift":"None","years":{"from":1901,"to":2199}}"#
            )
            .is_err()
        );
        assert!(parse::<Fraction>(r#"{"num":1,"den":0}"#).is_err());
        assert_eq!(parse::<Year>("2026").unwrap().get(), 2026);
        assert_eq!(parse::<Weekend>("96").unwrap(), Weekend::SAT_SUN);
    }

    #[test]
    fn fractions_round_trip_reduced() {
        let f = Fraction::new(2, 4).unwrap();
        let json = serde_json::to_string(&f).unwrap();
        assert_eq!(json, r#"{"num":1,"den":2}"#);
        assert_eq!(parse::<Fraction>(&json).unwrap(), f);
        assert_eq!(parse::<Fraction>(r#"{"num":2,"den":4}"#).unwrap(), f);
    }
}
