//! France market calendars — ports of `QuantLib`'s
//! [`France`](https://github.com/lballabio/QuantLib/blob/master/ql/time/calendars/france.cpp)
//! calendar variants. No weekend shift: weekend holidays are simply lost.
//!
//! Deviation: Ascension and Whit Monday use true Easter offsets, not `QuantLib`'s fixed-date bug.

mod exchange;
mod settlement;

pub use exchange::EXCHANGE;
pub use settlement::SETTLEMENT;
