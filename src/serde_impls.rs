//! Hand-written serde formats, behind the `serde` feature.
//!
//! Every type validates on the way in, so no input — however hostile —
//! deserializes into a value the rest of the crate would panic on:
//!
//! - [`Date`] is an ISO-8601 `"YYYY-MM-DD"` string. A bare integer is
//!   still read as a fasti serial (the 0.2 format), range-checked.
//! - [`Period`] is its `QuantLib` string, `"6M"`. The 0.2 map form,
//!   `{"Months": 6}`, is still read.
//! - [`Year`] is a number and [`Weekend`] its seven-bit mask; both are
//!   range-checked.

use core::fmt;

use serde::de::{self, Deserializer, MapAccess, Visitor};
use serde::{Deserialize, Serialize, Serializer};

use crate::{Date, Period, TimeError};

impl Serialize for Date {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Date {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct DateVisitor;

        impl Visitor<'_> for DateVisitor {
            type Value = Date;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a date string \"YYYY-MM-DD\" in 1901-01-01..=2199-12-31")
            }

            fn visit_str<E: de::Error>(self, s: &str) -> Result<Date, E> {
                s.parse().map_err(E::custom)
            }

            fn visit_u64<E: de::Error>(self, serial: u64) -> Result<Date, E> {
                u32::try_from(serial)
                    .map_err(|_| TimeError::DateOutOfRange)
                    .and_then(Date::from_serial)
                    .map_err(E::custom)
            }

            fn visit_i64<E: de::Error>(self, serial: i64) -> Result<Date, E> {
                u64::try_from(serial)
                    .map_err(|_| E::custom(TimeError::DateOutOfRange))
                    .and_then(|s| self.visit_u64(s))
            }
        }

        deserializer.deserialize_any(DateVisitor)
    }
}

impl Serialize for Period {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Period {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct PeriodVisitor;

        impl<'de> Visitor<'de> for PeriodVisitor {
            type Value = Period;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a period string such as \"6M\"")
            }

            fn visit_str<E: de::Error>(self, s: &str) -> Result<Period, E> {
                s.parse().map_err(E::custom)
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Period, A::Error> {
                let Some((unit, n)) = map.next_entry::<&str, i32>()? else {
                    return Err(de::Error::custom(TimeError::InvalidPeriodString));
                };
                if map.next_key::<&str>()?.is_some() {
                    return Err(de::Error::custom(TimeError::InvalidPeriodString));
                }
                match unit {
                    "Days" => Ok(Period::Days(n)),
                    "Weeks" => Ok(Period::Weeks(n)),
                    "Months" => Ok(Period::Months(n)),
                    "Years" => Ok(Period::Years(n)),
                    _ => Err(de::Error::custom(TimeError::InvalidPeriodString)),
                }
            }
        }

        deserializer.deserialize_any(PeriodVisitor)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use alloc::string::ToString;

    use crate::{
        Date, FixedDate, Fraction, Month, Period, Schedule, Weekend, Year, YearRange, calendars,
    };

    fn parse<T: serde::de::DeserializeOwned>(json: &str) -> Result<T, serde_json::Error> {
        serde_json::from_str(json)
    }

    #[test]
    fn dates_are_iso_strings_and_the_legacy_serial_is_range_checked() {
        let d = Date::from_ymd(2026, Month::Nov, 17).unwrap();
        assert_eq!(serde_json::to_string(&d).unwrap(), "\"2026-11-17\"");
        assert_eq!(parse::<Date>("\"2026-11-17\"").unwrap(), d);
        assert_eq!(parse::<Date>(&d.serial().to_string()).unwrap(), d);
        for bad in [
            "200000",
            "-1",
            "4294967296",
            "\"2026-02-30\"",
            "\"1900-12-31\"",
            "\"17/11/2026\"",
        ] {
            assert!(parse::<Date>(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn hostile_input_is_refused_before_it_can_reach_a_panic() {
        // Each of these used to deserialize, then panic on first use.
        assert!(parse::<Year>("5000").is_err());
        assert!(parse::<Weekend>("255").is_err());
        assert!(
            parse::<Schedule>(
                r#"{"dates":[45976,999999],"stubs":{"front":null,"back":null},"generation":null}"#
            )
            .is_err()
        );
        assert!(parse::<YearRange>(r#"{"from":2020,"to":1990}"#).is_err());
        assert!(
            parse::<FixedDate>(
                r#"{"month":"Feb","day":30,"shift":"None","years":{"from":1901,"to":2199}}"#
            )
            .is_err()
        );
        assert!(parse::<Fraction>(r#"{"num":1,"den":0}"#).is_err());
        // The valid neighbours still load.
        assert_eq!(parse::<Year>("2026").unwrap().get(), 2026);
        assert_eq!(parse::<Weekend>("96").unwrap(), Weekend::SAT_SUN);
        // The largest legacy serial is still a usable date.
        let last = parse::<Date>(&Date::MAX.serial().to_string()).unwrap();
        assert_eq!(last.to_string(), "2199-12-31");
        assert!(!calendars::us::SETTLEMENT.is_holiday(last));
    }

    #[test]
    fn periods_are_quantlib_strings_and_the_legacy_map_still_reads() {
        assert_eq!(serde_json::to_string(&Period::Months(6)).unwrap(), "\"6M\"");
        assert_eq!(parse::<Period>("\"6M\"").unwrap(), Period::Months(6));
        assert_eq!(
            parse::<Period>(r#"{"Months":6}"#).unwrap(),
            Period::Months(6)
        );
        assert!(parse::<Period>(r#"{"Fortnights":1}"#).is_err());
        assert!(parse::<Period>("\"6 months\"").is_err());
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
