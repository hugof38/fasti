//! Coupons and accrued interest for a 5-year semiannual US Treasury
//! note.
//!
//! Run with:
//!
//! ```bash
//! cargo run --example treasury_schedule
//! ```
//!
//! What this demonstrates:
//!
//! 1. `ScheduleBuilder`'s defaults are bond conventions: coupons accrue
//!    on the unadjusted 15th of January and July, backward from
//!    maturity, and each is paid on the next business day.
//! 2. ACT/ACT ICMA measures each coupon's accrual period, so every
//!    regular coupon is exactly half the annual coupon: $21,250.00 on
//!    $1mm at 4.25%. The January 2028 coupon accrues to the 15th but is
//!    paid on the 18th.
//! 3. Money is computed once, in integer cents, with an explicit
//!    rounding mode (`Fraction::scale`).
//! 4. Accrued interest for a trade settling T+1: the period running on
//!    the settlement date, cut short there.

use fasti::{
    AccrualDayCount, ActActICMA, Date, Fraction, Month, Period, RoundingMode, ScheduleBuilder,
    TimeError, calendars::us,
};

/// 4.25% on a $1,000,000 face, in cents.
const FACE_CENTS: i64 = 100_000_000;

fn dollars(cents: i64) -> String {
    format!("{}.{:02}", cents / 100, cents % 100)
}

fn cents(rate: Fraction, fraction: Fraction) -> Result<i64, TimeError> {
    rate.checked_mul(fraction)
        .ok_or(TimeError::FractionOverflow)?
        .scale(FACE_CENTS, RoundingMode::HalfEven)
}

fn main() -> Result<(), TimeError> {
    let issue = Date::from_ymd(2025, Month::Jan, 15)?;
    let maturity = Date::from_ymd(2030, Month::Jan, 15)?;
    let rate = Fraction::new(425, 10_000)?;
    let calendar = us::GOVERNMENT_BOND;
    let schedule = ScheduleBuilder::new(issue, maturity, Period::Months(6), calendar).build()?;

    println!("UST 4.25% {issue} -> {maturity}, semiannual, ACT/ACT ICMA, $1,000,000 face\n");
    println!(
        "{:>3}  {:<12}  {:<12}  {:<12}  {:>8}  {:>12}",
        "#", "accrues from", "to", "paid on", "fraction", "coupon ($)",
    );
    println!("{}", "-".repeat(70));

    let mut total = 0;
    for (i, coupon) in schedule.coupons().enumerate() {
        let fraction = ActActICMA.accrual_fraction(&coupon.accrual)?;
        let amount = cents(rate, fraction)?;
        total += amount;
        println!(
            "{:>3}  {:<12}  {:<12}  {:<12}  {:>8}  {:>12}",
            i + 1,
            coupon.accrual.start(),
            coupon.accrual.end(),
            coupon.payment,
            fraction.to_string(),
            dollars(amount),
        );
    }
    println!("{}", "-".repeat(70));
    println!("{:>60}  {:>12}\n", "total coupons:", dollars(total));

    // Accrued interest for a trade on Tuesday 2026-11-17, settling T+1.
    let trade = Date::from_ymd(2026, Month::Nov, 17)?;
    let settlement = calendar.add_business_days(trade, 1)?;
    let Some(to_date) = schedule
        .accrual_period(settlement)
        .and_then(|period| period.until(settlement))
    else {
        println!("{settlement} is outside the bond's life");
        return Ok(());
    };
    let fraction = ActActICMA.accrual_fraction(&to_date)?;
    println!(
        "traded {trade}, settles {settlement}: accrued from {} = {fraction} of a year, ${}",
        to_date.start(),
        dollars(cents(rate, fraction)?),
    );
    Ok(())
}
