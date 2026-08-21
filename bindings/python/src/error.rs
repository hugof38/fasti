//! The Python exception `fasti` raises, and the mapping from the core
//! crate's [`TimeError`].

use fasti::TimeError;
use pyo3::exceptions::PyValueError;
use pyo3::{PyErr, create_exception};

create_exception!(
    fasti,
    FastiError,
    PyValueError,
    "Raised when an argument is outside what `fasti` can represent — a date \
     beyond 1901-01-01..=2199-12-31, an unknown convention name, a schedule \
     whose dates do not increase.\n\n\
     Subclasses :class:`ValueError`, so ``except ValueError`` catches it."
);

/// Map a core-crate error onto [`FastiError`], preserving its message.
pub fn err(e: TimeError) -> PyErr {
    FastiError::new_err(e.to_string())
}

/// Build a [`FastiError`] from a message.
pub fn invalid(msg: impl Into<String>) -> PyErr {
    FastiError::new_err(msg.into())
}
