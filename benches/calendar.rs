//! Calendar rule-evaluation benchmarks: `cargo bench --bench calendar`.
//!
//! No benchmarking dev-dependency on purpose — `criterion` and `divan`
//! buy statistical rigour with a dependency subtree `cargo deny` has to
//! clear, and the quantities here are coarse. The harness takes the
//! minimum of several repetitions of a large fixed workload, a stable
//! estimator for integer computation with no allocation and no I/O.
//! `cargo test --all-targets` runs it as a fast smoke pass, so it
//! cannot rot unnoticed.

use std::hint::black_box;
use std::time::{Duration, Instant};

use fasti::{BusinessDayConvention, Calendar, Date, Month, Period, calendars};

/// The century measured: 1926-01-01 ..= 2025-12-31, 36 525 days.
fn century() -> (Date, Date) {
    (
        Date::from_ymd(1926, Month::Jan, 1).unwrap_or(Date::MIN),
        Date::from_ymd(2026, Month::Jan, 1).unwrap_or(Date::MAX),
    )
}

/// Every built-in calendar, plus the two market-neutral baselines.
const CALENDARS: [(&str, Calendar<'static>); 12] = [
    ("NULL_CALENDAR", calendars::NULL_CALENDAR),
    ("WEEKENDS_ONLY", calendars::WEEKENDS_ONLY),
    ("TARGET", calendars::TARGET),
    ("france::SETTLEMENT", calendars::france::SETTLEMENT),
    ("france::EXCHANGE", calendars::france::EXCHANGE),
    ("uk::SETTLEMENT", calendars::uk::SETTLEMENT),
    ("us::SETTLEMENT", calendars::us::SETTLEMENT),
    ("us::FEDERAL_RESERVE", calendars::us::FEDERAL_RESERVE),
    ("us::GOVERNMENT_BOND", calendars::us::GOVERNMENT_BOND),
    ("us::SOFR", calendars::us::SOFR),
    ("us::NERC", calendars::us::NERC),
    ("us::NYSE", calendars::us::NYSE),
];

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

/// Minimum wall time over `reps` runs of `f`, which must return a value
/// derived from the work so it cannot be optimised away.
fn best_of<T>(reps: u32, mut f: impl FnMut() -> T) -> Duration {
    let mut best = Duration::MAX;
    for _ in 0..reps {
        let start = Instant::now();
        let out = f();
        let elapsed = start.elapsed();
        black_box(out);
        best = best.min(elapsed);
    }
    best
}

/// Cost per unit of work, in picoseconds — integer arithmetic only, as
/// the crate forbids floats in tests and benchmarks alike.
fn per_op_ps(elapsed: Duration, ops: u64) -> u128 {
    if ops == 0 {
        return 0;
    }
    elapsed.as_nanos().saturating_mul(1000) / u128::from(ops)
}

/// Render picoseconds as nanoseconds with one decimal place.
fn ns(ps: u128) -> String {
    format!("{}.{}", ps / 1000, (ps % 1000) / 100)
}

fn main() {
    // A debug build measures the optimiser, not the code, so an
    // unoptimised run (`cargo test --all-targets` builds one) does a
    // smoke pass instead: it proves the benchmark still compiles and
    // runs without pretending the numbers mean anything.
    let smoke = cfg!(debug_assertions) || std::env::args().any(|a| a == "--test");
    let reps = if smoke { 1 } else { 7 };
    let (start, end) = century();
    let end = if smoke {
        start.add_days(60).unwrap_or(end)
    } else {
        end
    };
    let days = u64::from(end.serial() - start.serial());
    let probes = scattered(if smoke { 256 } else { 8192 });
    let n_probes = probes.len() as u64;

    println!();
    if smoke {
        println!("fasti calendar benchmarks — SMOKE RUN (unoptimised build); numbers are noise");
    } else {
        println!("fasti calendar benchmarks — {days} days walked per iteration, best of {reps}");
    }
    println!();
    println!(
        "| calendar | walk ns/day | is_business_day ns/call | adjust ns/call | business_days ns/day |",
    );
    println!("|---|---|---|---|---|");

    for (name, cal) in CALENDARS {
        // Sequential walk: the range-enumeration cost the task measures.
        let walk = best_of(reps, || {
            let mut n = 0u32;
            let mut d = start;
            while d < end {
                if cal.is_business_day(black_box(d)) {
                    n += 1;
                }
                d = d.add_days(1).unwrap_or(Date::MAX);
            }
            n
        });

        // Scattered single calls: the per-call path, no locality.
        let calls = best_of(reps, || {
            let mut n = 0u32;
            for &d in &probes {
                if cal.is_business_day(black_box(d)) {
                    n += 1;
                }
            }
            n
        });

        // adjust: evaluates the predicate several times per call.
        let adjust = best_of(reps, || {
            let mut acc = 0u32;
            for &d in &probes {
                if let Ok(a) = cal.adjust(black_box(d), BusinessDayConvention::ModifiedFollowing) {
                    acc = acc.wrapping_add(a.serial());
                }
            }
            acc
        });

        // The iterator, which is where a per-year memo can live.
        let iter = best_of(reps, || cal.business_days(start..end).count());

        println!(
            "| {name} | {} | {} | {} | {} |",
            ns(per_op_ps(walk, days)),
            ns(per_op_ps(calls, n_probes)),
            ns(per_op_ps(adjust, n_probes)),
            ns(per_op_ps(iter, days)),
        );
    }

    // A schedule-shaped workload: ten years of monthly roll dates, the
    // shape `ScheduleBuilder` walks.
    let cal = calendars::us::SETTLEMENT;
    let anchor = Date::from_ymd(2000, Month::Jan, 3).unwrap_or(Date::MIN);
    // One schedule is 120 dates; time many of them so the measurement
    // is not dominated by the clock.
    let schedules: u32 = if smoke { 2 } else { 500 };
    let rolls = u64::from(schedules) * 120;
    let advance = best_of(reps, || {
        let mut acc = 0u32;
        for _ in 0..schedules {
            for i in 0..120i32 {
                if let Ok(d) = cal.advance(
                    black_box(anchor),
                    Period::Months(i),
                    BusinessDayConvention::ModifiedFollowing,
                    false,
                ) {
                    acc = acc.wrapping_add(d.serial());
                }
            }
        }
        acc
    });
    println!();
    println!(
        "us::SETTLEMENT, 120 monthly roll dates: advance() {} ns/call",
        ns(per_op_ps(advance, rolls)),
    );
}
