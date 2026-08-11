//! US market calendars — ports of `QuantLib`'s
//! [`UnitedStates`](https://github.com/lballabio/QuantLib/blob/master/ql/time/calendars/unitedstates.cpp)
//! market variants, historical pre-1971 rules included.
//!
//! - [`SETTLEMENT`] — generic US settlement calendar.
//! - [`NERC`] — energy off-peak calendar (6 holidays).
//! - [`FEDERAL_RESERVE`] — Fed Bankwire (Settlement, no Saturday-back shift).
//! - [`NYSE`] — New York Stock Exchange (Good Friday, historic closings).
//! - [`GOVERNMENT_BOND`] — Settlement + Good Friday with post-1996 NFP exception.
//! - [`SOFR`] — Government Bond with Good Friday always observed.
//!
//! Planned: `LIBOR_IMPACT`.

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
