//! Coupons and accrued interest for a 5-year semiannual US Treasury
//! note, the way Treasuries actually accrue.
//!
//! Run with:
//!
//! ```bash
//! cargo run --example treasury_schedule
//! ```
//!
//! What this demonstrates:
//!
//! 1. Treasuries accrue on the **unadjusted** coupon grid: the 15th of
//!    January and July, whatever day of the week it is. So the
//!    accrual schedule keeps `ScheduleBuilder`'s defaults, which are
//!    bond conventions: `Unadjusted` dates, generated backward from
//!    maturity.
//! 2. Interest is counted ACT/ACT ICMA, bound to that schedule. Every
//!    regular period is then exactly half a year, so every coupon is
//!    exactly half the annual coupon: $21,250.00 on $1mm at 4.25%.
//! 3. Payment is a separate question: a coupon date that is not a
//!    business day pays on the next one, without changing the
//!    accrual. The January 2028 coupon pays on the 18th.
//! 4. Money is computed once, in integer cents, with an explicit
//!    rounding mode (`Fraction::scale`).
//! 5. Accrued interest for a trade settling T+1 business day.

use fasti::{
    ActActICMA, BusinessDayConvention, Date, Fraction, Frequency, Month, Period, RoundingMode,
    ScheduleBuilder, TimeError, calendars::us,
};

/// 4.25% on a $1,000,000 face, in cents.
const FACE_CENTS: i64 = 100_000_000;

fn dollars(cents: i64) -> String {
    format!("{}.{:02}", cents / 100, cents % 100)
}

fn main() -> Result<(), TimeError> {
    let issue = Date::from_ymd(2025, Month::Jan, 15)?;
    let maturity = Date::from_ymd(2030, Month::Jan, 15)?;
    let rate = Fraction::new(425, 10_000)?;
    let calendar = us::GOVERNMENT_BOND;

    // The accrual grid: unadjusted coupon dates, backward from maturity.
    // These are ScheduleBuilder's defaults, which are bond conventions.
    let schedule = ScheduleBuilder::new(issue, maturity, Period::Months(6), calendar).build()?;
    let day_count = ActActICMA::new(Frequency::Semiannual).bind(&schedule)?;

    println!("UST 4.25% {issue} -> {maturity}, semiannual, ACT/ACT ICMA, $1,000,000 face\n");
    println!(
        "{:>3}  {:<12}  {:<12}  {:<12}  {:>8}  {:>12}",
        "#", "accrues from", "to", "paid on", "fraction", "coupon ($)",
    );
    println!("{}", "-".repeat(70));

    let mut total = 0;
    for (i, period) in schedule.periods().enumerate() {
        let fraction = day_count.try_year_fraction(period.start, period.end)?;
        let factor = rate
            .checked_mul(fraction)
            .ok_or(TimeError::FractionOverflow)?;
        let coupon = factor.scale(FACE_CENTS, RoundingMode::HalfEven)?;
        total += coupon;
        let paid = calendar.adjust(period.end, BusinessDayConvention::Following)?;
        println!(
            "{:>3}  {:<12}  {:<12}  {:<12}  {:>8}  {:>12}",
            i + 1,
            period.start,
            period.end,
            paid,
            fraction.to_string(),
            dollars(coupon),
        );
    }
    println!("{}", "-".repeat(70));
    println!("{:>60}  {:>12}\n", "total coupons:", dollars(total));

    // Accrued interest for a trade on Tuesday 2026-11-17, settling T+1.
    let trade = Date::from_ymd(2026, Month::Nov, 17)?;
    let settlement = calendar.add_business_days(trade, 1)?;
    // The coupon period containing settlement: the last coupon date on
    // or before it. On a coupon date itself, accrued interest is zero.
    let Some(period) = schedule
        .periods()
        .find(|p| p.start <= settlement && settlement < p.end)
    else {
        println!("{settlement} is outside the bond's life");
        return Ok(());
    };
    let accrued_fraction = day_count.try_year_fraction(period.start, settlement)?;
    let accrued = rate
        .checked_mul(accrued_fraction)
        .ok_or(TimeError::FractionOverflow)?
        .scale(FACE_CENTS, RoundingMode::HalfEven)?;
    println!(
        "traded {trade}, settles {settlement}: accrued from {} = {} of a year, ${}",
        period.start,
        accrued_fraction,
        dollars(accrued),
    );
    Ok(())
}
