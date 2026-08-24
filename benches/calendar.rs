//! Calendar rule-evaluation benchmarks: `cargo bench --bench calendar`.
//!
//! Every built-in calendar is measured three ways: a day walked in
//! sequence, a cold call on a date with no locality, and an adjustment.
//! Counters are per day or per call, so `divan`'s throughput column
//! reads as the cost of one.

use divan::counter::ItemsCount;
use divan::{Bencher, black_box};
use fasti::{BusinessDayConvention, Calendar, Date, Month, Period, calendars};

fn main() {
    divan::main();
}

/// Every built-in calendar, plus the two market-neutral baselines.
const CALENDARS: [(&str, Calendar<'static>); 12] = [
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

/// The names `divan` runs each benchmark over.
const NAMES: [&str; 12] = [
    "null",
    "weekends_only",
    "target",
    "france_settlement",
    "france_exchange",
    "uk_settlement",
    "us_settlement",
    "us_federal_reserve",
    "us_government_bond",
    "us_sofr",
    "us_nerc",
    "us_nyse",
];

fn calendar(name: &str) -> Calendar<'static> {
    let mut i = 0;
    while i < CALENDARS.len() {
        let (candidate, cal) = CALENDARS[i];
        if candidate.as_bytes() == name.as_bytes() {
            return cal;
        }
        i += 1;
    }
    calendars::NULL_CALENDAR
}

/// A century: 1926-01-01 ..= 2025-12-31, 36 525 days. The smoke run
/// under `cargo test --benches` walks one year instead.
fn century() -> (Date, Date) {
    let start = Date::from_ymd(1926, Month::Jan, 1).unwrap_or(Date::MIN);
    let end_year = if cfg!(debug_assertions) { 1927 } else { 2026 };
    (
        start,
        Date::from_ymd(end_year, Month::Jan, 1).unwrap_or(Date::MAX),
    )
}

/// How many scattered dates the per-call benchmarks probe.
const PROBES: usize = if cfg!(debug_assertions) { 128 } else { 8192 };

/// A deterministic scatter of in-range dates, so the per-call numbers
/// measure a cold call rather than a sequential walk.
fn scattered(n: usize) -> Vec<Date> {
    let mut state: u32 = 0x9E37_79B9;
    let mut out = Vec::with_capacity(n);
    while out.len() < n {
        // xorshift32: integer-only, reproducible run to run.
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        if let Ok(d) = Date::from_serial(state % (Date::MAX.serial() + 1)) {
            out.push(d);
        }
    }
    out
}

/// Enumerating business days a day at a time — the cost the whole crate
/// pays per day of any range it walks.
#[divan::bench(args = NAMES)]
fn walk_century(bencher: Bencher<'_, '_>, name: &str) {
    let cal = calendar(name);
    let (start, end) = century();
    let days = u64::from(end.serial() - start.serial());
    bencher.counter(ItemsCount::new(days)).bench(|| {
        let mut open = 0u32;
        let mut day = start;
        while day < end {
            if cal.is_business_day(black_box(day)) {
                open += 1;
            }
            day = day.add_days(1).unwrap_or(Date::MAX);
        }
        open
    });
}

/// The same range through the iterator, which is what callers actually
/// write.
#[divan::bench(args = NAMES)]
fn business_days_iter(bencher: Bencher<'_, '_>, name: &str) {
    let cal = calendar(name);
    let (start, end) = century();
    let days = u64::from(end.serial() - start.serial());
    bencher
        .counter(ItemsCount::new(days))
        .bench(|| cal.business_days(black_box(start)..black_box(end)).count());
}

/// A single call on a date with no locality: the per-call path, with
/// nothing to reuse from the call before.
#[divan::bench(args = NAMES)]
fn is_business_day(bencher: Bencher<'_, '_>, name: &str) {
    let cal = calendar(name);
    let probes = scattered(PROBES);
    bencher
        .counter(ItemsCount::new(probes.len()))
        .bench(|| probes.iter().filter(|d| cal.is_business_day(**d)).count());
}

/// Adjustment, which evaluates the predicate three or four times per
/// call and so magnifies whatever a single evaluation costs.
#[divan::bench(args = NAMES)]
fn adjust(bencher: Bencher<'_, '_>, name: &str) {
    let cal = calendar(name);
    let probes = scattered(PROBES);
    bencher.counter(ItemsCount::new(probes.len())).bench(|| {
        let mut acc = 0u32;
        for &d in &probes {
            if let Ok(rolled) = cal.adjust(d, BusinessDayConvention::ModifiedFollowing) {
                acc = acc.wrapping_add(rolled.serial());
            }
        }
        acc
    });
}

/// A schedule-shaped workload: ten years of monthly roll dates, the
/// shape `ScheduleBuilder` walks.
#[divan::bench]
fn advance_monthly_rolls(bencher: Bencher<'_, '_>) {
    let cal = calendars::us::SETTLEMENT;
    let anchor = Date::from_ymd(2000, Month::Jan, 3).unwrap_or(Date::MIN);
    bencher.counter(ItemsCount::new(120usize)).bench(|| {
        let mut acc = 0u32;
        for months in 0..120i32 {
            if let Ok(d) = cal.advance(
                black_box(anchor),
                Period::Months(months),
                BusinessDayConvention::ModifiedFollowing,
                false,
            ) {
                acc = acc.wrapping_add(d.serial());
            }
        }
        acc
    });
}
