//! Behaviour freeze for the calendar-evaluation rework.
//!
//! The oracle below is a line-for-line copy of `Calendar::is_holiday` as
//! it stood on `main` before the rework (probe-every-rule-per-date), and
//! every built-in calendar is checked against it for every supported
//! date, 1901-01-01..=2199-12-31. This test must stay green through
//! every revision of the rework; it is deleted at the end (its oracle is
//! a copy of deleted code) and replaced with oracle-free invariants.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use fasti::{Calendar, Date, Rule, TimeError, Weekday, WeekendShift, calendars};

/// The rule's weekend-shift direction, as `main` computed it: only
/// `Rule::Fixed` carries one.
fn shift_of(rule: &Rule) -> WeekendShift {
    match rule {
        Rule::Fixed(f) => f.weekend_shift(),
        _ => WeekendShift::None,
    }
}

/// Copy of `WeekendShift::direction` from `main`.
fn direction(shift: WeekendShift, day: Weekday) -> Option<i32> {
    match (shift, day) {
        (WeekendShift::None, _) => None,
        (_, Weekday::Sun) | (WeekendShift::Forward, Weekday::Sat) => Some(1),
        (WeekendShift::SatBackSunForward, Weekday::Sat) => Some(-1),
        _ => None,
    }
}

fn is_natural_holiday(cal: &Calendar<'_>, date: Date) -> bool {
    cal.rules.iter().any(|r| r.is_holiday(date))
}

/// Copy of `Calendar::moves` from `main`.
fn moves(cal: &Calendar<'_>, day: Date, step: i32) -> bool {
    cal.is_weekend(day)
        && cal
            .rules
            .iter()
            .any(|r| direction(shift_of(r), day.weekday()) == Some(step) && r.is_holiday(day))
}

/// Copy of `Calendar::owed_by_weekend` from `main`.
fn owed_by_weekend(cal: &Calendar<'_>, saturday: Result<Date, TimeError>) -> usize {
    let Ok(saturday) = saturday else {
        return 0;
    };
    usize::from(moves(cal, saturday, 1))
        + usize::from(saturday.add_days(1).is_ok_and(|sun| moves(cal, sun, 1)))
}

/// Copy of `Calendar::is_substitute` from `main`.
fn is_substitute(cal: &Calendar<'_>, date: Date) -> bool {
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

/// Copy of `Calendar::is_holiday` from `main` — the oracle.
fn oracle_is_holiday(cal: &Calendar<'_>, date: Date) -> bool {
    is_natural_holiday(cal, date) || is_substitute(cal, date)
}

/// Exhaustive comparison over the whole supported range.
fn assert_equivalent(cal: &Calendar<'_>) {
    for serial in Date::MIN.serial()..=Date::MAX.serial() {
        let d = Date::from_serial(serial).unwrap();
        assert_eq!(
            cal.is_holiday(d),
            oracle_is_holiday(cal, d),
            "{}: is_holiday({d}) diverged from main's behaviour",
            cal.name,
        );
    }
}

macro_rules! equivalence {
    ($($test:ident => $cal:expr;)+) => {
        $(
            #[test]
            fn $test() {
                assert_equivalent(&$cal);
            }
        )+
    };
}

equivalence! {
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
