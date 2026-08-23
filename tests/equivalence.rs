//! Frozen-behaviour test: for every built-in calendar and every date in
//! 1901..=2199, `Calendar::is_holiday` and `Calendar::is_business_day`
//! must agree with the original rule-scanning algorithm, date for date.
//!
//! The oracle below is that original algorithm, transcribed from the
//! pre-optimisation `src/calendar.rs` and written against the published
//! API only — `Rule::is_holiday`, `FixedDate::weekend_shift`, and the
//! `WeekendShift` variants. It re-derives every date from scratch for
//! every rule, which is exactly what made it slow and exactly what makes
//! it a trustworthy reference: it shares no code with what it checks.
//!
//! If a date disagrees, the change under test is wrong, not this file.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use fasti::{
    Calendar, Date, EasterOffset, FixedDate, Month, OneOff, Rule, RuleDate, TimeError, Weekday,
    Weekend, WeekendShift, Year, calendars,
};

// ---- the oracle: the original implementation, verbatim in behaviour ----

/// `WeekendShift::direction`, which is crate-private; the table is the
/// public documentation of each variant.
fn direction(shift: WeekendShift, day: Weekday) -> Option<i32> {
    match (shift, day) {
        (WeekendShift::None, _) => None,
        (_, Weekday::Sun) | (WeekendShift::Forward, Weekday::Sat) => Some(1),
        (WeekendShift::SatBackSunForward, Weekday::Sat) => Some(-1),
        _ => None,
    }
}

/// `Rule::weekend_shift`, which is crate-private: only `FixedDate`
/// carries a shift.
fn shift_of(rule: &Rule) -> WeekendShift {
    match rule {
        Rule::Fixed(r) => r.weekend_shift(),
        _ => WeekendShift::None,
    }
}

fn is_natural_holiday(cal: Calendar<'_>, date: Date) -> bool {
    cal.rules.iter().any(|r| r.is_holiday(date))
}

fn moves(cal: Calendar<'_>, day: Date, step: i32) -> bool {
    cal.is_weekend(day)
        && cal
            .rules
            .iter()
            .any(|r| direction(shift_of(r), day.weekday()) == Some(step) && r.is_holiday(day))
}

fn owed_by_weekend(cal: Calendar<'_>, saturday: Result<Date, TimeError>) -> usize {
    let Ok(saturday) = saturday else {
        return 0;
    };
    usize::from(moves(cal, saturday, 1))
        + usize::from(saturday.add_days(1).is_ok_and(|sun| moves(cal, sun, 1)))
}

fn is_substitute(cal: Calendar<'_>, date: Date) -> bool {
    let shifts = |r: &Rule| !matches!(shift_of(r), WeekendShift::None);
    if cal.is_weekend(date) || !cal.rules.iter().any(shifts) {
        return false;
    }
    match date.weekday() {
        Weekday::Fri => date.add_days(1).is_ok_and(|sat| moves(cal, sat, -1)),
        Weekday::Mon => owed_by_weekend(cal, date.add_days(-2)) >= 1,
        Weekday::Tue => {
            let monday_already_a_holiday = date
                .add_days(-1)
                .is_ok_and(|mon| is_natural_holiday(cal, mon));
            match owed_by_weekend(cal, date.add_days(-3)) {
                0 => false,
                1 => monday_already_a_holiday,
                _ => true,
            }
        }
        _ => false,
    }
}

/// The original `Calendar::is_holiday`.
fn oracle_is_holiday(cal: Calendar<'_>, date: Date) -> bool {
    is_natural_holiday(cal, date) || is_substitute(cal, date)
}

/// The original `Calendar::is_business_day`.
fn oracle_is_business_day(cal: Calendar<'_>, date: Date) -> bool {
    !cal.is_weekend(date) && !oracle_is_holiday(cal, date)
}

// ---- the calendars under test ------------------------------------------

const BUILT_INS: [Calendar<'static>; 12] = [
    calendars::NULL_CALENDAR,
    calendars::WEEKENDS_ONLY,
    calendars::TARGET,
    calendars::france::SETTLEMENT,
    calendars::france::EXCHANGE,
    calendars::uk::SETTLEMENT,
    calendars::us::SETTLEMENT,
    calendars::us::FEDERAL_RESERVE,
    calendars::us::GOVERNMENT_BOND,
    calendars::us::SOFR,
    calendars::us::NERC,
    calendars::us::NYSE,
];

/// Every Monday — an opaque rule that collides with substitutes.
fn all_mondays(d: Date) -> bool {
    matches!(d.weekday(), Weekday::Mon)
}

const XMAS: Rule = Rule::Fixed(FixedDate::new(Month::Dec, 25).shift(WeekendShift::Forward));
const BOXING: Rule = Rule::Fixed(FixedDate::new(Month::Dec, 26).shift(WeekendShift::Forward));
const NEW_YEAR: Rule =
    Rule::Fixed(FixedDate::new(Month::Jan, 1).shift(WeekendShift::SatBackSunForward));

/// Shapes the built-ins do not reach, each named for what it pins.
const SYNTHETIC: [Calendar<'static>; 9] = [
    // A substitute reaching three days back over the year end.
    Calendar {
        name: "december 29",
        weekend: Weekend::SAT_SUN,
        // When December 29 is a Saturday, January 1 is the Tuesday
        // three days later, and it is granted the day off only
        // because December 31 is a holiday of its own that takes the
        // Monday. Resolving that Tuesday reads a day in the previous
        // year, three days before it.
        rules: &[
            Rule::Fixed(FixedDate::new(Month::Dec, 29).shift(WeekendShift::Forward)),
            Rule::Fixed(FixedDate::new(Month::Dec, 31)),
        ],
    },
    // A weekend owing two days across the year end.
    Calendar {
        name: "new year's eve and day",
        weekend: Weekend::SAT_SUN,
        // When December 31 is a Saturday, January 1 is the Sunday:
        // the Monday takes the first day off and the Tuesday the
        // second, so resolving that Tuesday has to reach back three
        // days into the previous year.
        rules: &[
            Rule::Fixed(FixedDate::new(Month::Dec, 31).shift(WeekendShift::Forward)),
            Rule::Fixed(FixedDate::new(Month::Jan, 1).shift(WeekendShift::Forward)),
        ],
    },
    // Fri/sat weekend, forward shifts.
    Calendar {
        name: "fri/sat",
        weekend: Weekend::FRI_SAT,
        rules: &[NEW_YEAR, XMAS, BOXING],
    },
    // Three shifted holidays in a row.
    Calendar {
        name: "queue overflow",
        weekend: Weekend::SAT_SUN,
        rules: &[
            Rule::Fixed(FixedDate::new(Month::Dec, 24).shift(WeekendShift::Forward)),
            XMAS,
            BOXING,
        ],
    },
    // The same day named twice.
    Calendar {
        name: "doubled",
        weekend: Weekend::SAT_SUN,
        rules: &[XMAS, XMAS],
    },
    // An opaque rule beside a shifted one.
    Calendar {
        name: "custom mondays",
        weekend: Weekend::SAT_SUN,
        rules: &[XMAS, BOXING, Rule::Custom(all_mondays)],
    },
    // A substitute blocked by a fixed holiday.
    Calendar {
        name: "blocked friday",
        weekend: Weekend::SAT_SUN,
        rules: &[
            Rule::Fixed(FixedDate::new(Month::Dec, 31)),
            NEW_YEAR,
            Rule::Fixed(FixedDate::new(Month::Jan, 2)),
        ],
    },
    // Easter and one-offs beside a shift.
    Calendar {
        name: "easter mix",
        weekend: Weekend::SAT_SUN,
        rules: &[
            Rule::Easter(EasterOffset::good_friday()),
            Rule::Easter(EasterOffset::easter_monday()),
            Rule::Easter(EasterOffset::new_orthodox(1)),
            Rule::OneOff(OneOff::new(Date::literal(2026, Month::Jul, 6))),
            Rule::Fixed(FixedDate::new(Month::Jul, 4).shift(WeekendShift::Forward)),
        ],
    },
    // An Easter offset that leaves its year.
    Calendar {
        name: "easter overflow",
        weekend: Weekend::SAT_SUN,
        // +300 lands in the next year, which `is_holiday` never
        // matches; the crate documents `Rule::Custom` for that.
        rules: &[Rule::Easter(EasterOffset::new(300)), XMAS],
    },
];

/// Every date the crate supports, 1901-01-01 through 2199-12-31.
fn every_supported_date() -> impl Iterator<Item = Date> {
    (0..=Date::MAX.serial()).filter_map(|s| Date::from_serial(s).ok())
}

#[test]
fn is_holiday_matches_the_original_algorithm_for_every_built_in() {
    for cal in BUILT_INS {
        let mut holidays = 0u32;
        for d in every_supported_date() {
            let expected = oracle_is_holiday(cal, d);
            assert_eq!(cal.is_holiday(d), expected, "{}: is_holiday({d})", cal.name);
            assert_eq!(
                cal.is_business_day(d),
                oracle_is_business_day(cal, d),
                "{}: is_business_day({d})",
                cal.name,
            );
            holidays += u32::from(expected);
        }
        // A calendar that suddenly has no holidays at all would satisfy
        // the comparison only if the oracle broke too; pin the shape.
        if !cal.rules.is_empty() {
            assert!(holidays > 0, "{}: no holidays in 1901..=2199", cal.name);
        }
    }
}

/// The iterators keep their memo in their own state and are double
/// ended, so both directions have to agree with the oracle as well.
#[test]
fn the_range_iterators_match_the_original_algorithm() {
    let range = Date::MIN..Date::MAX;
    for cal in BUILT_INS {
        let expected: Vec<Date> = every_supported_date()
            .filter(|d| *d < Date::MAX && oracle_is_business_day(cal, *d))
            .collect();
        let forwards: Vec<Date> = cal.business_days(range.clone()).collect();
        assert_eq!(forwards, expected, "{}: business_days forwards", cal.name);

        let mut backwards: Vec<Date> = cal.business_days(range.clone()).rev().collect();
        backwards.reverse();
        assert_eq!(backwards, expected, "{}: business_days backwards", cal.name);

        let holidays: Vec<Date> = cal.holidays(range.clone()).collect();
        let expected_holidays: Vec<Date> = every_supported_date()
            .filter(|d| *d < Date::MAX && oracle_is_holiday(cal, *d))
            .collect();
        assert_eq!(holidays, expected_holidays, "{}: holidays", cal.name);
    }
}

/// Every rule must name exactly the dates it claims: `natural_date` and
/// `is_holiday` are duals, and per-year resolution rests on that. The
/// synthetic rules are here too, because one of them — an Easter offset
/// landing in the next year — can only break this way: the calendar
/// never observes the difference, and a caller resolving a year would.
#[test]
fn natural_date_and_is_holiday_are_duals() {
    for cal in BUILT_INS.into_iter().chain(SYNTHETIC) {
        for (i, rule) in cal.rules.iter().enumerate() {
            if matches!(rule.natural_date(Year::MIN), RuleDate::Opaque) {
                // An opaque predicate names nothing in any year; that is
                // the whole of its contract.
                for year in Year::MIN.get()..=Year::MAX.get() {
                    let year = Year::new(year).unwrap();
                    assert!(
                        matches!(rule.natural_date(year), RuleDate::Opaque),
                        "{} rule {i}: Custom stopped being opaque in {year}",
                        cal.name,
                    );
                }
                continue;
            }
            // The date a rule names in a year is in that year — the
            // promise a caller resolving a year at a time relies on, and
            // the one an Easter offset running past December 31 would
            // break silently, since no calendar query can observe it.
            for year in Year::MIN.get()..=Year::MAX.get() {
                let year = Year::new(year).unwrap();
                if let RuleDate::On(named) = rule.natural_date(year) {
                    assert_eq!(named.year(), year, "{} rule {i}", cal.name);
                }
            }
            for d in every_supported_date() {
                let named = rule.natural_date(d.year()) == RuleDate::On(d);
                assert_eq!(rule.is_holiday(d), named, "{} rule {i}: {d}", cal.name);
            }
        }
    }
}

/// The same guarantee for calendar shapes no built-in has: other
/// weekends, shifts with nothing to step off, opaque rules beside
/// shifted ones, and substitutes landing on days already taken.
#[test]
fn synthetic_calendars_match_the_original_algorithm() {
    for cal in SYNTHETIC {
        for d in every_supported_date() {
            assert_eq!(
                cal.is_holiday(d),
                oracle_is_holiday(cal, d),
                "{}: is_holiday({d})",
                cal.name,
            );
            assert_eq!(
                cal.is_business_day(d),
                oracle_is_business_day(cal, d),
                "{}: is_business_day({d})",
                cal.name,
            );
        }
        // ... and through the range iterators, over the same range.
        let expected: Vec<Date> = every_supported_date()
            .filter(|d| *d < Date::MAX && oracle_is_holiday(cal, *d))
            .collect();
        assert_eq!(
            cal.holidays(Date::MIN..Date::MAX).collect::<Vec<_>>(),
            expected,
            "{}: holidays",
            cal.name,
        );
    }
}

// ---- randomised calendars ----------------------------------------------

/// A hand-picked calendar can only cover the shapes someone thought of.
/// These build calendars out of random rules, random year ranges and
/// random weekends, and hold all three paths — the oracle, the direct
/// one and the memo — to the same answer on random dates.
mod random {
    use super::{oracle_is_business_day, oracle_is_holiday};
    use fasti::{
        Calendar, Date, EasterMethod, EasterOffset, FixedDate, LastWeekday, Month, NthWeekday,
        OneOff, Ordinal, Rule, Weekday, Weekend, WeekendShift, Year, YearRange,
    };
    use proptest::prelude::*;

    /// Opaque predicates the generator can reach for. Each is cheap and
    /// names days that collide with substitutes.
    const CUSTOM: [fn(Date) -> bool; 3] = [
        |d| matches!(d.weekday(), Weekday::Mon),
        |d| d.day() == 1,
        |d| d.month() == Month::Dec && d.day() >= 24,
    ];

    fn any_month() -> impl Strategy<Value = Month> {
        (1u8..=12).prop_map(|m| Month::try_from_u8(m).unwrap_or(Month::Jan))
    }

    fn any_weekday() -> impl Strategy<Value = Weekday> {
        (1u8..=7).prop_map(|w| Weekday::try_from_u8(w).unwrap_or(Weekday::Mon))
    }

    fn any_shift() -> impl Strategy<Value = WeekendShift> {
        prop_oneof![
            Just(WeekendShift::None),
            Just(WeekendShift::Forward),
            Just(WeekendShift::SunForward),
            Just(WeekendShift::SatBackSunForward),
        ]
    }

    /// A range that is sometimes open, sometimes a narrow window, so
    /// rules switch on and off part-way through the walk.
    fn any_years() -> impl Strategy<Value = YearRange> {
        (1901u16..=2199, 0u16..=300).prop_map(|(from, span)| {
            let to = from.saturating_add(span).min(2199);
            YearRange::try_between(
                Year::new(from).unwrap_or(Year::MIN),
                Year::new(to).unwrap_or(Year::MAX),
            )
            .unwrap_or(YearRange::ALWAYS)
        })
    }

    fn any_rule() -> impl Strategy<Value = Rule> {
        prop_oneof![
            // Days beyond a month's length are deliberately reachable:
            // a rule naming February 31 names nothing, ever.
            (any_month(), 1u8..=31, any_shift(), any_years())
                .prop_map(|(m, d, s, y)| { Rule::Fixed(FixedDate::new(m, d).shift(s).years(y)) }),
            ((1u8..=5), any_weekday(), any_month(), any_years()).prop_map(|(n, w, m, y)| {
                Rule::NthWeekday(
                    NthWeekday::new(Ordinal::try_from_u8(n).unwrap_or(Ordinal::First), w, m)
                        .years(y),
                )
            }),
            (any_weekday(), any_month(), any_years())
                .prop_map(|(w, m, y)| Rule::LastWeekday(LastWeekday::new(w, m).years(y))),
            // Offsets past a year's end are included on purpose: they
            // name nothing, which is what the crate documents.
            (-70i16..=400, any::<bool>(), any_years()).prop_map(|(days, orthodox, y)| {
                let rule = if orthodox {
                    EasterOffset::new_orthodox(days)
                } else {
                    EasterOffset::new(days)
                };
                assert_eq!(
                    rule.method(),
                    if orthodox {
                        EasterMethod::Orthodox
                    } else {
                        EasterMethod::Western
                    },
                );
                Rule::Easter(rule.years(y))
            }),
            (0u32..=Date::MAX.serial())
                .prop_map(|s| Rule::OneOff(OneOff::new(Date::from_serial(s).unwrap_or(Date::MIN)))),
            (0usize..CUSTOM.len()).prop_map(|i| Rule::Custom(CUSTOM[i])),
        ]
    }

    fn any_weekend() -> impl Strategy<Value = Weekend> {
        prop::collection::vec(any_weekday(), 0..3).prop_map(|days| Weekend::from_weekdays(&days))
    }

    proptest! {
        /// One random calendar, one random date: all three paths agree.
        #[test]
        fn all_paths_agree_on_random_calendars(
            rules in prop::collection::vec(any_rule(), 0..7),
            weekend in any_weekend(),
            serial in 0u32..=Date::MAX.serial(),
        ) {
            let cal = Calendar { name: "random", weekend, rules: &rules };
            let date = Date::from_serial(serial).unwrap_or(Date::MIN);
            let expected = oracle_is_holiday(cal, date);
            prop_assert_eq!(cal.is_holiday(date), expected, "{}", date);
            prop_assert_eq!(cal.is_business_day(date), oracle_is_business_day(cal, date));
        }

        /// A run of days across a year boundary, where a substitute
        /// reaches back into the year before, read both ways.
        #[test]
        fn the_range_iterators_agree_across_a_year_boundary(
            rules in prop::collection::vec(any_rule(), 0..7),
            weekend in any_weekend(),
            year in 1902u16..=2198,
        ) {
            let cal = Calendar { name: "random", weekend, rules: &rules };
            let start = Date::from_ymd(year, Month::Dec, 20).unwrap_or(Date::MIN);
            let end = start.add_days(25).unwrap_or(Date::MAX);
            let expected: Vec<Date> = (start.serial()..end.serial())
                .filter_map(|s| Date::from_serial(s).ok())
                .filter(|d| oracle_is_holiday(cal, *d))
                .collect();
            prop_assert_eq!(cal.holidays(start..end).collect::<Vec<_>>(), expected.clone());
            let mut backwards: Vec<Date> = cal.holidays(start..end).rev().collect();
            backwards.reverse();
            prop_assert_eq!(backwards, expected);
        }
    }
}
