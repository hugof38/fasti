//! Build a coupon schedule for a 5-year semiannual US Treasury note
//! and print each period's accrual against the actual/actual ISDA
//! day-count convention.
//!
//! Run with:
//!
//! ```bash
//! cargo run -p fasti --example treasury_schedule
//! ```
//!
//! What this demonstrates:
//!
//! 1. Pick the `us::SETTLEMENT` calendar — the standard "is the bond
//!    market open" calendar in `QuantLib`'s vocabulary.
//! 2. Build a backward-generated semiannual schedule from issue to
//!    maturity. Backward is conventional for bonds: any irregular
//!    accrual period lands at the front (closest to issue), not the
//!    back. Coupon dates are anchored on the maturity date.
//! 3. Apply `ModifiedFollowing` to interior coupon dates (move to the
//!    next business day, but never cross a month boundary) and leave
//!    maturity `Unadjusted` (the bond legally pays on its stated
//!    maturity date).
//! 4. Walk `Schedule::periods()` and price each accrual using
//!    `ActActISDA`.

#![allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]

use fasti::{
    ActActISDA, BusinessDayConvention, Date, DayCount, Month, Period, ScheduleBuilder, TimeError,
    calendars::us,
};

/// 5-year, semiannual, 4.25% Treasury issued 2025-01-15 maturing
/// 2030-01-15 on a $1,000,000 face.
const FACE_CENTS: u128 = 100_000_000;
const COUPON_BPS: u128 = 425;

fn main() -> Result<(), TimeError> {
    let issue = Date::from_ymd(2025, Month::Jan, 15)?;
    let maturity = Date::from_ymd(2030, Month::Jan, 15)?;

    let schedule = ScheduleBuilder::new(issue, maturity, Period::Months(6), us::SETTLEMENT)
        .backwards()
        .with_convention(BusinessDayConvention::ModifiedFollowing)
        .with_termination_convention(BusinessDayConvention::Unadjusted)
        .build()?;

    println!("Treasury 4.25% {issue} -> {maturity}, semiannual, US settlement calendar");
    println!(
        "{} coupon dates ({} accrual periods)\n",
        schedule.len(),
        schedule.len() - 1,
    );
    println!(
        "{:>3}  {:<12}  {:<12}  {:>16}  {:>14}",
        "#", "start", "end", "yf (act/act)", "coupon ($)",
    );
    println!("{}", "-".repeat(64));

    let dc = ActActISDA;
    let mut total_cents: u128 = 0;
    for (i, period) in schedule.periods().enumerate() {
        let (num, den) = dc.year_fraction(period.start, period.end).parts();
        // coupon = face × rate × year_fraction, all in integer cents.
        let cents = FACE_CENTS * COUPON_BPS * num as u128 / (10_000 * u128::from(den));
        total_cents += cents;
        println!(
            "{:>3}  {:<12}  {:<12}  {:>7} / {:<6}  {:>10}.{:02}",
            i + 1,
            period.start,
            period.end,
            num,
            den,
            cents / 100,
            cents % 100,
        );
    }
    println!("{}", "-".repeat(64));
    println!(
        "{:>49}  ${:>9}.{:02}",
        "total coupons:",
        total_cents / 100,
        total_cents % 100,
    );

    Ok(())
}
