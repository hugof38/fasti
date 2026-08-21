//! Integration tests: fasti as an external consumer sees it.
//!
//! The unit tests inside `src/` can reach anything in the crate, so they
//! cannot tell whether an item is actually re-exported from the crate
//! root — a type reachable at `crate::foo::Bar` but never re-exported
//! compiles fine in-crate and is unusable from outside. This file is
//! compiled as a separate crate against the published surface only, so
//! a missing `pub use` is a hard compile error here.
//!
//! It also pins the ergonomics: if a realistic workflow needs a helper
//! that is `pub(crate)`, that shows up as an awkward test rather than as
//! a downstream bug report.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use core::str::FromStr;

use fasti::{
    Act360, Act365Fixed, ActActICMA, ActActISDA, BusinessDayConvention, Calendar, CalendarBuilder,
    Date, DateRange, DayCount, EasterMethod, EasterOffset, FixedDate, Fraction, Frequency,
    Generation, LastWeekday, Month, NthWeekday, OneOff, Ordinal, Period, Rule, Schedule,
    ScheduleBuilder, Thirty360Bond, Thirty360European, Thirty360ISDA, Thirty360US, TimeError,
    Weekday, Weekend, Year, YearRange, calendars, easter_monday, easter_sunday,
};

/// A realistic coupon-accrual workflow, the way a downstream crate would
/// write it: build a schedule on a real calendar, walk its periods,
/// accrue each one, and scale a notional by the result without ever
/// touching a float.
#[test]
fn end_to_end_coupon_accrual() -> Result<(), TimeError> {
    let schedule = ScheduleBuilder::new(
        Date::from_ymd(2025, Month::Jan, 15)?,
        Date::from_ymd(2027, Month::Jan, 15)?,
        Period::Months(6),
        calendars::us::SETTLEMENT,
    )
    .backwards()
    .with_convention(BusinessDayConvention::ModifiedFollowing)
    .with_termination_convention(BusinessDayConvention::Unadjusted)
    .build()?;

    assert_eq!(schedule.dates().len(), 5);
    assert_eq!(schedule.periods().count(), 4);

    // Every accrual fraction is a positive rational, and the periods
    // tile the schedule without gaps.
    let mut total = Fraction::new(0, 1).expect("0/1 is a valid fraction");
    let mut cursor = schedule.dates()[0];
    for period in schedule.periods() {
        assert_eq!(period.start, cursor);
        let yf = ActActISDA.year_fraction(period.start, period.end);
        assert!(!yf.is_negative() && !yf.is_zero());
        total = total.checked_add(yf).expect("accruals do not overflow i64");
        cursor = period.end;
    }
    assert_eq!(cursor, *schedule.dates().last().expect("non-empty"));

    // Two years of semiannual coupons accrue to two years of ACT/ACT.
    let whole = ActActISDA.year_fraction(schedule.dates()[0], cursor);
    assert_eq!(total.cmp_cross(whole), core::cmp::Ordering::Equal);

    // Scaling a notional by an accrual stays exact: 10_000_000 * n/d.
    let (num, den) = whole.parts();
    // $10,000,000.00 in cents.
    let notional: i128 = 1_000_000_000;
    assert_eq!(
        notional * i128::from(num) / i128::from(den),
        notional * 2 // exactly two years
    );
    Ok(())
}

/// Every day-count convention is constructible and callable from
/// outside, including the two that carry parameters.
#[test]
fn every_day_count_is_reachable() -> Result<(), TimeError> {
    let start = Date::from_ymd(2025, Month::Jan, 1)?;
    let end = Date::from_ymd(2025, Month::Jul, 1)?;

    let simple: [&dyn DayCount; 6] = [
        &Act360,
        &Act365Fixed,
        &ActActISDA,
        &Thirty360Bond,
        &Thirty360US,
        &Thirty360European,
    ];
    for dc in simple {
        assert!(!dc.name().is_empty(), "every convention names itself");
        let yf = dc.year_fraction(start, end);
        assert!(!yf.is_negative());
        // Half a year, give or take the convention's own day count.
        assert!(yf.numerator() > 0 && yf.denominator() > 0);
    }

    // Thirty360ISDA binds the termination date at construction.
    let isda = Thirty360ISDA::new(Date::from_ymd(2030, Month::Jan, 1)?);
    assert_eq!(isda.termination(), Date::from_ymd(2030, Month::Jan, 1)?);
    assert!(!isda.year_fraction(start, end).is_negative());

    // ActActICMA binds to a coupon schedule.
    let schedule = ScheduleBuilder::new(
        start,
        Date::from_ymd(2026, Month::Jan, 1)?,
        Period::Months(6),
        calendars::WEEKENDS_ONLY,
    )
    .build()?;
    let icma = ActActICMA::new(Frequency::Semiannual);
    assert_eq!(icma.frequency(), Frequency::Semiannual);
    let bound = icma.bind(&schedule)?;
    assert_eq!(bound.schedule().dates(), schedule.dates());
    assert_eq!(bound.year_fraction(start, end).parts(), (1, 2));
    Ok(())
}

/// Each built-in calendar is a `pub const` usable without setup, and
/// each one disagrees with the bare weekend baseline somewhere — which
/// is the only thing that makes it worth shipping.
#[test]
fn built_in_calendars_are_const_and_distinct() -> Result<(), TimeError> {
    const CALENDARS: [(&str, Calendar<'static>); 11] = [
        ("TARGET", calendars::TARGET),
        ("uk::SETTLEMENT", calendars::uk::SETTLEMENT),
        ("us::SETTLEMENT", calendars::us::SETTLEMENT),
        ("us::NYSE", calendars::us::NYSE),
        ("us::FEDERAL_RESERVE", calendars::us::FEDERAL_RESERVE),
        ("us::GOVERNMENT_BOND", calendars::us::GOVERNMENT_BOND),
        ("us::SOFR", calendars::us::SOFR),
        ("us::NERC", calendars::us::NERC),
        ("france::SETTLEMENT", calendars::france::SETTLEMENT),
        ("france::EXCHANGE", calendars::france::EXCHANGE),
        ("WEEKENDS_ONLY", calendars::WEEKENDS_ONLY),
    ];

    let year = Date::from_ymd(2026, Month::Jan, 1)?..Date::from_ymd(2027, Month::Jan, 1)?;
    for (name, cal) in CALENDARS {
        let holidays = cal.holidays(year.clone()).count();
        if name == "WEEKENDS_ONLY" {
            assert_eq!(holidays, 0, "the baseline has no holidays by definition");
        } else {
            assert!(holidays > 0, "{name} declared no 2026 holidays");
        }
    }

    // NULL_CALENDAR has no weekend either: every day is a business day.
    assert_eq!(calendars::NULL_CALENDAR.holidays(year.clone()).count(), 0);
    assert_eq!(
        calendars::NULL_CALENDAR.business_days(year.clone()).count(),
        365
    );
    Ok(())
}

/// A calendar built at runtime out of every rule variant, exercising the
/// builder path rather than the `const` one.
#[test]
fn calendar_builder_composes_every_rule_variant() -> Result<(), TimeError> {
    fn is_leap_day(d: Date) -> bool {
        d.month() == Month::Feb && d.day() == 29
    }

    let owned = CalendarBuilder::new("Test Market", Weekend::SAT_SUN)
        .with_rule(Rule::Fixed(FixedDate::new(Month::Jan, 1)))
        .with_rule(Rule::NthWeekday(NthWeekday::new(
            Ordinal::Third,
            Weekday::Mon,
            Month::Jan,
        )))
        .with_rule(Rule::LastWeekday(LastWeekday::new(
            Weekday::Mon,
            Month::May,
        )))
        .with_rule(Rule::Easter(EasterOffset::new(0)))
        .with_rule(Rule::OneOff(OneOff::new(Date::from_ymd(
            2026,
            Month::Sep,
            14,
        )?)))
        .with_rule(Rule::Custom(is_leap_day))
        .union(calendars::us::SETTLEMENT);

    let cal = owned.view();
    // `union` names the joint calendar after both sides.
    assert_eq!(cal.name, "Test Market + US settlement");
    assert!(cal.is_holiday(Date::from_ymd(2026, Month::Jan, 1)?));
    assert!(cal.is_holiday(Date::from_ymd(2026, Month::Jan, 19)?)); // 3rd Mon
    assert!(cal.is_holiday(Date::from_ymd(2026, Month::May, 25)?)); // last Mon
    assert!(cal.is_holiday(Date::from_ymd(2026, Month::Sep, 14)?)); // one-off
    assert!(cal.is_holiday(Date::from_ymd(2028, Month::Feb, 29)?)); // custom
    // Inherited from the union with US settlement.
    assert!(cal.is_holiday(Date::from_ymd(2026, Month::Jul, 3)?));
    Ok(())
}

/// Business-day adjustment and advancement over a real calendar.
#[test]
fn business_day_conventions_round_a_holiday() -> Result<(), TimeError> {
    let cal = calendars::us::SETTLEMENT;
    // Sat 2026-07-04 is observed on Fri 2026-07-03.
    let saturday = Date::from_ymd(2026, Month::Jul, 4)?;
    assert_eq!(saturday.weekday(), Weekday::Sat);
    assert!(cal.is_holiday(Date::from_ymd(2026, Month::Jul, 3)?));

    assert_eq!(
        cal.adjust(saturday, BusinessDayConvention::Following)?,
        Date::from_ymd(2026, Month::Jul, 6)?
    );
    assert_eq!(
        cal.adjust(saturday, BusinessDayConvention::Preceding)?,
        Date::from_ymd(2026, Month::Jul, 2)?
    );
    // ModifiedFollowing may not cross into August, but July has room.
    assert_eq!(
        cal.adjust(saturday, BusinessDayConvention::ModifiedFollowing)?,
        Date::from_ymd(2026, Month::Jul, 6)?
    );
    assert_eq!(
        cal.adjust(saturday, BusinessDayConvention::ModifiedPreceding)?,
        Date::from_ymd(2026, Month::Jul, 2)?
    );
    assert_eq!(
        cal.adjust(saturday, BusinessDayConvention::Unadjusted)?,
        saturday
    );

    let advanced = cal.advance(
        Date::from_ymd(2026, Month::Jul, 1)?,
        Period::Days(2),
        BusinessDayConvention::Following,
        false,
    )?;
    assert!(cal.is_business_day(advanced));
    Ok(())
}

/// Date primitives, parsing, and the arithmetic a caller reaches for first.
#[test]
fn date_primitives_from_outside() -> Result<(), TimeError> {
    let d = Date::from_str("2026-02-28")?;
    assert_eq!(d, Date::from_ymd(2026, Month::Feb, 28)?);
    assert_eq!(d.year(), Year::new(2026)?);
    assert_eq!(d.month(), Month::Feb);
    assert_eq!(d.day(), 28);
    assert_eq!(d.weekday(), Weekday::Sat);
    assert!(d.is_end_of_month());
    assert_eq!(d.add_days(1)?, Date::from_ymd(2026, Month::Mar, 1)?);
    assert_eq!(d.add_months(1)?, Date::from_ymd(2026, Month::Mar, 28)?);
    // `advance` carries the end-of-month flag that `add_months` does not.
    assert_eq!(
        d.advance(Period::Months(1), true)?,
        Date::from_ymd(2026, Month::Mar, 31)?
    );

    // Out-of-range inputs are errors, never panics.
    assert!(Date::from_ymd(1900, Month::Dec, 31).is_err());
    assert!(Date::from_ymd(2026, Month::Feb, 29).is_err());
    assert!(Date::from_str("not-a-date").is_err());
    assert_eq!(Year::new(2200), Err(TimeError::YearOutOfRange));

    // The `Range<Date>` extension trait.
    let range = Date::from_ymd(2026, Month::Jan, 1)?..Date::from_ymd(2026, Month::Feb, 1)?;
    assert_eq!(range.days(), 31);
    assert_eq!(range.dates().count(), 31);
    let overlap = Date::from_ymd(2026, Month::Jan, 20)?..Date::from_ymd(2026, Month::Mar, 1)?;
    assert_eq!(
        range.intersect(&overlap),
        Some(Date::from_ymd(2026, Month::Jan, 20)?..Date::from_ymd(2026, Month::Feb, 1)?)
    );
    Ok(())
}

/// Easter, period arithmetic, weekends and year ranges — the smaller
/// exports, checked for reachability and one anchored value each.
#[test]
fn supporting_types_are_reachable() -> Result<(), TimeError> {
    // Both return a day-of-year, matching `Date::day_of_year`. Western
    // Easter Sunday 2026 is April 5; Easter Monday is April 6.
    assert_eq!(
        easter_sunday(Year::new(2026)?, EasterMethod::Western),
        Date::from_ymd(2026, Month::Apr, 5)?.day_of_year()
    );
    assert_eq!(
        easter_monday(Year::new(2026)?, EasterMethod::Western),
        Date::from_ymd(2026, Month::Apr, 6)?.day_of_year()
    );
    // Orthodox Easter 2026 falls on the same day; 2024 does not.
    assert_ne!(
        easter_sunday(Year::new(2024)?, EasterMethod::Western),
        easter_sunday(Year::new(2024)?, EasterMethod::Orthodox)
    );

    assert_eq!(Period::Months(12).normalized(), Period::Years(1));
    assert_eq!(Period::Months(6).checked_mul(2), Some(Period::Months(12)));
    assert_eq!(Frequency::Quarterly.per_year(), 4);

    assert!(Weekend::SAT_SUN.contains(Weekday::Sat));
    assert!(!Weekend::SAT_SUN.contains(Weekday::Fri));
    assert!(Weekend::from_weekdays(&[Weekday::Fri, Weekday::Sat]).contains(Weekday::Fri));
    assert!(!Weekend::NONE.contains(Weekday::Sun));

    let years = YearRange::try_between(Year::new(2000)?, Year::new(2030)?)?;
    assert!(years.contains(Year::new(2026)?));
    assert!(!years.contains(Year::new(2031)?));

    assert_eq!(Ordinal::try_from_u8(3)?, Ordinal::Third);
    assert_eq!(Month::try_from_u8(12)?, Month::Dec);
    assert_eq!(Weekday::try_from_u8(1)?, Weekday::Mon);
    Ok(())
}

/// `Generation` — the parameters a schedule was built from — is public
/// so callers can re-derive the coupon lattice a schedule sits on.
#[test]
fn schedule_exposes_its_generation() -> Result<(), TimeError> {
    let start = Date::from_ymd(2025, Month::Mar, 31)?;
    let schedule: Schedule = ScheduleBuilder::new(
        start,
        Date::from_ymd(2026, Month::Mar, 31)?,
        Period::Months(3),
        calendars::WEEKENDS_ONLY,
    )
    .with_end_of_month(true)
    .build()?;

    let generation: Generation = schedule.generation().expect("a generated schedule");
    assert_eq!(generation.tenor, Period::Months(3));
    assert!(generation.end_of_month);
    // The lattice steps in whole months from the anchor, not by chaining.
    assert_eq!(
        generation.step(start, 1)?,
        Date::from_ymd(2025, Month::Jun, 30)?
    );
    assert_eq!(
        schedule.reference_periods().count(),
        schedule.periods().count()
    );
    Ok(())
}

/// The error type is inspectable and implements the traits a caller
/// needs to fold it into their own error enum.
#[derive(Debug)]
#[allow(dead_code)]
enum DownstreamError {
    Time(TimeError),
}

fn rendered(err: TimeError) -> String {
    use core::fmt::Write as _;
    let mut s = String::new();
    write!(s, "{err}").expect("writing to a String cannot fail");
    s
}

#[test]
fn errors_are_usable_downstream() {
    let err = Date::from_ymd(2026, Month::Feb, 30).unwrap_err();
    assert_eq!(err, TimeError::DayOutOfRange);

    // Display, Debug and equality — enough to fold into a downstream
    // error enum without reaching for anything private.
    assert!(!rendered(err).is_empty());
    assert_eq!(err, Date::from_ymd(2026, Month::Feb, 30).unwrap_err());

    let wrapped = DownstreamError::Time(err);
    assert!(matches!(wrapped, DownstreamError::Time(_)));
}

#[cfg(feature = "serde")]
#[test]
fn serde_round_trips_through_the_public_types() -> Result<(), TimeError> {
    let date = Date::from_ymd(2026, Month::Jul, 4)?;
    let json = serde_json::to_string(&date).expect("Date serializes");
    let back: Date = serde_json::from_str(&json).expect("Date deserializes");
    assert_eq!(date, back);

    let period = Period::Months(6);
    let back: Period =
        serde_json::from_str(&serde_json::to_string(&period).expect("ser")).expect("de");
    assert_eq!(period, back);
    Ok(())
}

#[cfg(feature = "chrono")]
#[test]
fn chrono_conversions_work_at_the_boundary() -> Result<(), TimeError> {
    use chrono::NaiveDate;

    let naive = NaiveDate::from_ymd_opt(2026, 7, 4).expect("a real date");
    let date: Date = naive.try_into()?;
    assert_eq!(date, Date::from_ymd(2026, Month::Jul, 4)?);
    assert_eq!(NaiveDate::from(date), naive);

    // Outside fasti's supported range the conversion fails rather than
    // saturating or panicking.
    let too_early = NaiveDate::from_ymd_opt(1899, 1, 1).expect("a real date");
    assert!(Date::try_from(too_early).is_err());
    Ok(())
}
