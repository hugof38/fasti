//! Differential testing of substitute-day resolution against an
//! independent reference model.
//!
//! `Calendar::is_holiday` answers per-date via a sliding five-day
//! window. The model here answers per-weekend instead: walk every
//! weekend in the range, allocate the days off it owes in natural-date
//! order, and collect the resulting set. The two formulations share no
//! code path — the model probes rules with `Rule::is_holiday` (the
//! match side of the expand/match duality) and never touches the
//! window logic — so agreement pins both against each other.
//!
//! This replaces the temporary `tests/equivalence.rs` freeze test,
//! whose oracle was a copy of the pre-rework implementation and could
//! not outlive the code it duplicated.
//!
//! The model encodes the conventions as documented:
//! - `Forward` chains: first free day of {Monday, Tuesday}, where
//!   "free" means no natural holiday and no earlier substitute; the
//!   chain is bounded at the Tuesday.
//! - `SunForward` / `SatBackSunForward` take a single fixed step,
//!   whatever occupies the target.
//! - A day only moves if this calendar calls it a weekend, and a
//!   substitute never lands on a weekend day.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use fasti::{
    Calendar, CalendarBuilder, Date, FixedDate, Month, Rule, Weekday, Weekend, WeekendShift,
    calendars,
};

/// The shift variant of a rule, seen from outside the crate.
fn shift_of(rule: &Rule) -> WeekendShift {
    match rule {
        Rule::Fixed(f) => f.weekend_shift(),
        _ => WeekendShift::None,
    }
}

fn natural(cal: &Calendar<'_>, d: Date) -> bool {
    cal.rules.iter().any(|r| r.is_holiday(d))
}

/// Any rule whose natural date is `d` and whose variant matches `pred`.
fn mover(cal: &Calendar<'_>, d: Date, pred: impl Fn(WeekendShift) -> bool) -> bool {
    cal.is_weekend(d)
        && cal
            .rules
            .iter()
            .any(|r| pred(shift_of(r)) && r.is_holiday(d))
}

/// Every holiday (natural or substitute) in `lo..=hi` according to the
/// per-weekend reference model.
fn reference(cal: &Calendar<'_>, lo: Date, hi: Date) -> BTreeSet<Date> {
    let mut days = BTreeSet::new();
    for serial in lo.serial()..=hi.serial() {
        let d = Date::from_serial(serial).unwrap();
        if natural(cal, d) {
            days.insert(d);
        }
    }
    // Walk every ISO Saturday whose weekend can spill into the range.
    let spill = Date::from_serial(lo.serial().saturating_sub(7)).unwrap();
    let mut sat = spill.next_weekday(Weekday::Sat).unwrap();
    while sat.serial() <= hi.serial() {
        let sun = sat.add_days(1).unwrap();

        // Backward: a single fixed step from the Saturday.
        if mover(cal, sat, |s| matches!(s, WeekendShift::SatBackSunForward))
            && let Ok(fri) = sat.add_days(-1)
            && !cal.is_weekend(fri)
        {
            days.insert(fri);
        }

        // Forward movers, in natural-date order. Only `Forward` moves
        // a Saturday forwards; a Sunday moves under any forward
        // variant, chaining only under `Forward`.
        let sat_chain = mover(cal, sat, |s| matches!(s, WeekendShift::Forward));
        let sun_chain = mover(cal, sun, |s| matches!(s, WeekendShift::Forward));
        let sun_single = mover(cal, sun, |s| {
            matches!(
                s,
                WeekendShift::SunForward | WeekendShift::SatBackSunForward
            )
        });

        let mon = sat.add_days(2);
        let tue = sat.add_days(3);
        let mut insert = |slot: Result<Date, fasti::TimeError>| {
            if let Ok(day) = slot
                && !cal.is_weekend(day)
            {
                days.insert(day);
            }
        };
        let mut mon_taken = mon.is_ok_and(|m| natural(cal, m));
        for (chains, single) in [(sat_chain, false), (sun_chain, sun_single)] {
            if chains {
                if mon_taken {
                    // Pushed past the taken Monday; the chain is
                    // bounded at the Tuesday.
                    insert(tue);
                } else {
                    insert(mon);
                    mon_taken = true;
                }
            } else if single {
                // One fixed step, whatever already sits there.
                insert(mon);
            }
        }
        sat = match sat.add_days(7) {
            Ok(next) => next,
            Err(_) => break,
        };
    }
    days
}

/// Exhaustive agreement over `lo..=hi`.
fn assert_agrees(cal: &Calendar<'_>, lo: Date, hi: Date) {
    let expected = reference(cal, lo, hi);
    for serial in lo.serial()..=hi.serial() {
        let d = Date::from_serial(serial).unwrap();
        assert_eq!(
            cal.is_holiday(d),
            expected.contains(&d),
            "{}: is_holiday({d}) disagrees with the reference model",
            cal.name,
        );
    }
}

macro_rules! model_agreement {
    ($($test:ident => $cal:expr;)+) => {
        $(
            #[test]
            fn $test() {
                assert_agrees(&$cal, Date::MIN, Date::MAX);
            }
        )+
    };
}

model_agreement! {
    null_calendar => calendars::NULL_CALENDAR;
    weekends_only => calendars::WEEKENDS_ONLY;
    target => calendars::TARGET;
    france_settlement => calendars::france::SETTLEMENT;
    france_exchange => calendars::france::EXCHANGE;
    uk_settlement => calendars::uk::SETTLEMENT;
    us_settlement => calendars::us::SETTLEMENT;
    us_federal_reserve => calendars::us::FEDERAL_RESERVE;
    us_government_bond => calendars::us::GOVERNMENT_BOND;
    us_sofr => calendars::us::SOFR;
    us_nerc => calendars::us::NERC;
    us_nyse => calendars::us::NYSE;
}

/// The chaining shapes the built-ins never exercise: adjacent chained
/// pairs, mixed variants, blockers — over the full range.
#[test]
fn synthetic_chaining_calendars_agree_with_the_model() {
    let shapes: &[(&str, &[Rule])] = &[
        (
            "UK Christmas/Boxing Day pair",
            &[
                Rule::Fixed(FixedDate::new(Month::Dec, 25).shift(WeekendShift::Forward)),
                Rule::Fixed(FixedDate::new(Month::Dec, 26).shift(WeekendShift::Forward)),
            ],
        ),
        (
            "chained pair straddling the year boundary",
            &[
                Rule::Fixed(FixedDate::new(Month::Dec, 31).shift(WeekendShift::Forward)),
                Rule::Fixed(FixedDate::new(Month::Jan, 1).shift(WeekendShift::Forward)),
            ],
        ),
        (
            "chain blocked by a natural Monday-capable holiday",
            &[
                Rule::Fixed(FixedDate::new(Month::Jul, 4).shift(WeekendShift::Forward)),
                Rule::Fixed(FixedDate::new(Month::Jul, 5).shift(WeekendShift::Forward)),
                Rule::Fixed(FixedDate::new(Month::Jul, 6)),
            ],
        ),
        (
            "mixed variants sharing a weekend",
            &[
                Rule::Fixed(FixedDate::new(Month::Jul, 4).shift(WeekendShift::Forward)),
                Rule::Fixed(FixedDate::new(Month::Jul, 5).shift(WeekendShift::SunForward)),
            ],
        ),
        (
            "single step next to a blocker, plus a backward step",
            &[
                Rule::Fixed(FixedDate::new(Month::Jan, 1).shift(WeekendShift::SatBackSunForward)),
                Rule::Fixed(FixedDate::new(Month::Jan, 2)),
            ],
        ),
    ];
    for (name, rules) in shapes {
        let cal = Calendar {
            name,
            weekend: Weekend::SAT_SUN,
            rules,
        };
        assert_agrees(&cal, Date::MIN, Date::MAX);
    }
}

mod random_calendars {
    use super::*;
    use proptest::prelude::*;

    fn any_shift() -> impl Strategy<Value = WeekendShift> {
        prop_oneof![
            Just(WeekendShift::None),
            Just(WeekendShift::Forward),
            Just(WeekendShift::SunForward),
            Just(WeekendShift::SatBackSunForward),
        ]
    }

    fn any_fixed_rule() -> impl Strategy<Value = Rule> {
        (1u8..=12, 1u8..=31, any_shift()).prop_map(|(m, d, shift)| {
            Rule::Fixed(FixedDate::new(Month::try_from_u8(m).unwrap(), d).shift(shift))
        })
    }

    proptest! {
        /// Any pile of fixed-date rules, any shifts, checked against
        /// the model over a full year plus its straddling boundaries.
        #[test]
        fn random_rule_sets_agree_with_the_model(
            rules in proptest::collection::vec(any_fixed_rule(), 0..6),
            year in 1902u16..=2198,
        ) {
            let mut builder = CalendarBuilder::new("random", Weekend::SAT_SUN);
            for rule in rules {
                builder = builder.with_rule(rule);
            }
            let cal = builder.view();
            let lo = Date::from_ymd(year - 1, Month::Dec, 20).unwrap();
            let hi = Date::from_ymd(year + 1, Month::Jan, 10).unwrap();
            let expected = reference(&cal, lo, hi);
            for serial in lo.serial()..=hi.serial() {
                let d = Date::from_serial(serial).unwrap();
                prop_assert_eq!(
                    cal.is_holiday(d),
                    expected.contains(&d),
                    "is_holiday({}) disagrees with the reference model", d,
                );
            }
        }
    }
}
