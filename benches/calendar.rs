//! Calendar-evaluation benchmarks over every built-in calendar.
//!
//! Run with `cargo bench --bench calendar`. The throughput column reads
//! as cost-of-one thanks to `ItemsCount`: the `walk` group reports
//! nanoseconds per day walked, the others nanoseconds per call.
//!
//! Under `cargo test --benches` (debug assertions on) each benchmark
//! runs once over a one-year smoke range so the file cannot rot.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use divan::counter::ItemsCount;
use divan::{Bencher, black_box};
use fasti::{BusinessDayConvention, Calendar, Date, Month, Period, calendars};

fn main() {
    divan::main();
}

/// One row per built-in: the bench-arg label and the calendar it names.
const CALENDARS: &[(&str, Calendar<'static>)] = &[
    ("null", calendars::NULL_CALENDAR),
    ("weekends_only", calendars::WEEKENDS_ONLY),
    ("target", calendars::TARGET),
    ("france_settlement", calendars::france::SETTLEMENT),
    ("france_exchange", calendars::france::EXCHANGE),
    ("uk_settlement", calendars::uk::SETTLEMENT),
    ("us_settlement", calendars::us::SETTLEMENT),
    ("us_federal_reserve", calendars::us::FEDERAL_RESERVE),
    ("us_government_bond", calendars::us::GOVERNMENT_BOND),
    ("us_sofr", calendars::us::SOFR),
    ("us_nerc", calendars::us::NERC),
    ("us_nyse", calendars::us::NYSE),
];

const NAMES: [&str; CALENDARS.len()] = {
    let mut names = [""; CALENDARS.len()];
    let mut i = 0;
    while i < names.len() {
        names[i] = CALENDARS[i].0;
        i += 1;
    }
    names
};

fn lookup(name: &str) -> Calendar<'static> {
    CALENDARS.iter().find(|(n, _)| *n == name).unwrap().1
}

/// The serial range walked: 1926..2026 (36 525 days), or one year in
/// the debug smoke mode.
fn range() -> (u32, u32) {
    let (from, to) = if cfg!(debug_assertions) {
        (2025, 2026)
    } else {
        (1926, 2026)
    };
    let start = Date::from_ymd(from, Month::Jan, 1).unwrap().serial();
    let end = Date::from_ymd(to, Month::Jan, 1).unwrap().serial();
    (start, end)
}

/// The range sampled with a prime stride, so repeated calls visit
/// every weekday and month over time — the point-query workload.
fn sampled_dates() -> Vec<Date> {
    let (start, end) = range();
    (start..end)
        .step_by(23)
        .map(|s| Date::from_serial(s).unwrap())
        .collect()
}

/// `is_business_day` over every day of the range — the bulk-walk shape
/// of holiday enumeration and schedule generation.
#[divan::bench(args = NAMES)]
fn walk(bencher: Bencher<'_, '_>, name: &str) {
    let cal = lookup(name);
    let (start, end) = range();
    bencher
        .counter(ItemsCount::new(u64::from(end - start)))
        .bench(|| {
            let mut business_days = 0u32;
            for serial in start..end {
                let d = Date::from_serial(serial).unwrap();
                if black_box(cal).is_business_day(black_box(d)) {
                    business_days += 1;
                }
            }
            business_days
        });
}

/// Point queries spread across the year — the single-call shape.
#[divan::bench(args = NAMES)]
fn is_business_day(bencher: Bencher<'_, '_>, name: &str) {
    let cal = lookup(name);
    let dates = sampled_dates();
    bencher
        .counter(ItemsCount::new(dates.len()))
        .bench(|| -> u32 {
            let mut business_days = 0u32;
            for &d in &dates {
                if black_box(cal).is_business_day(black_box(d)) {
                    business_days += 1;
                }
            }
            business_days
        });
}

/// `adjust` under `ModifiedFollowing` over the same point queries.
#[divan::bench(args = NAMES)]
fn adjust(bencher: Bencher<'_, '_>, name: &str) {
    let cal = lookup(name);
    let dates = sampled_dates();
    bencher
        .counter(ItemsCount::new(dates.len()))
        .bench(|| -> u32 {
            let mut acc = 0u32;
            for &d in &dates {
                acc = acc.wrapping_add(
                    black_box(cal)
                        .adjust(black_box(d), BusinessDayConvention::ModifiedFollowing)
                        .unwrap()
                        .serial(),
                );
            }
            acc
        });
}

/// 120 monthly rolls under `ModifiedFollowing` — the schedule-stepping
/// shape.
#[divan::bench(args = NAMES)]
fn advance(bencher: Bencher<'_, '_>, name: &str) {
    let cal = lookup(name);
    let rolls: i32 = if cfg!(debug_assertions) { 12 } else { 120 };
    let anchor = Date::from_ymd(2010, Month::Jan, 31).unwrap();
    bencher
        .counter(ItemsCount::new(u64::try_from(rolls).unwrap()))
        .bench(|| -> u32 {
            let mut acc = 0u32;
            for months in 1..=rolls {
                acc = acc.wrapping_add(
                    black_box(cal)
                        .advance(
                            black_box(anchor),
                            Period::Months(months),
                            BusinessDayConvention::ModifiedFollowing,
                            true,
                        )
                        .unwrap()
                        .serial(),
                );
            }
            acc
        });
}
