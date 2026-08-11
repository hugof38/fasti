//! France market calendars — settlement and Paris Bourse exchange.
//!
//! Ports of `QuantLib`'s
//! [`France`](https://github.com/lballabio/QuantLib/blob/master/ql/time/calendars/france.cpp)
//! calendar variants. French statutory holidays are observed on their
//! natural date — there is no weekend-shift convention — so any
//! holiday falling on a Saturday or Sunday is simply lost.
//!
//! **Note on `QuantLib` compatibility:** `QuantLib`'s France Settlement
//! encodes Ascension and Whit Monday as fixed May-10 and May-21
//! calendar dates, which is a bug (those are Easter-relative dates
//! that vary by year). This port uses correct Easter offsets from
//! [`EasterOffset`](crate::EasterOffset). The rest of the holiday set
//! matches `QuantLib` exactly.

mod exchange;
mod settlement;

pub use exchange::EXCHANGE;
pub use settlement::SETTLEMENT;
