//! US market calendars — settlement, NERC, Federal Reserve, etc.
//!
//! These are ports of `QuantLib`'s
//! [`UnitedStates`](https://github.com/lballabio/QuantLib/blob/master/ql/time/calendars/unitedstates.cpp)
//! market variants, expressed as sequences of [`Rule`](crate::Rule)s
//! over a Saturday/Sunday weekend. Historical variants (pre-1971
//! Washington's Birthday / Memorial Day, Veterans Day during the
//! 1971–1977 Uniform Monday Holiday Act transition) are included.
//!
//! Variants currently shipped:
//!
//! - [`SETTLEMENT`] — generic US settlement calendar.
//! - [`NERC`] — North American Energy Reliability Council off-peak
//!   calendar (6 holidays + pre-1971 Memorial Day variant).
//! - [`FEDERAL_RESERVE`] — Federal Reserve Bankwire System calendar
//!   (same holiday set as Settlement but no Saturday-back shift).
//! - [`NYSE`] — New York Stock Exchange calendar (MLK since 1998, no
//!   Columbus/Veterans, Good Friday, pre-1981 election days, 20+
//!   special historic closings).
//! - [`GOVERNMENT_BOND`] — US government bond market calendar
//!   (Settlement + Good Friday with post-1996 NFP exception + Veterans
//!   Day `SunForward` + 3 historic closings).
//! - [`SOFR`] — SOFR fixing calendar (Government Bond with Good Friday
//!   always observed, no NFP exception).
//!
//! Variant planned for follow-up: `LIBOR_IMPACT`.

mod federal_reserve;
mod government_bond;
mod nerc;
mod nyse;
mod settlement;
mod sofr;

pub use federal_reserve::FEDERAL_RESERVE;
pub use government_bond::GOVERNMENT_BOND;
pub use nerc::NERC;
pub use nyse::NYSE;
pub use settlement::SETTLEMENT;
pub use sofr::SOFR;
