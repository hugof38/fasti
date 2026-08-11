# Contributing to fasti

Thanks for your interest! Issues and pull requests are welcome.

## Development workflow

The crate is a single standard Cargo library. All four of these must
pass before a change is considered green (CI enforces them):

```bash
cargo test --all-features --all-targets
cargo clippy --all-features --all-targets -- -D warnings
cargo fmt --all --check
RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps
```

Doctests run as part of `cargo test`. The minimum supported Rust
version is pinned in `Cargo.toml` (`rust-version`); CI builds against
both stable and the MSRV.

## Ground rules

Read [`ARCHITECTURE.md`](./ARCHITECTURE.md) first — it explains the
design constraints. The short version:

- No new runtime dependencies without prior discussion; no external
  date crates ever.
- No `f64`/`f32` anywhere, including tests.
- No `unwrap`/`expect`/`panic`/`todo` in library code (tests are fine,
  behind the module-level allow).
- Keep everything `const` that can be `const`.
- Do not reintroduce a `HolidayRule` trait — the `Rule` enum with the
  fn-pointer `Custom` variant is the chosen design.
- Do not widen the 1901..=2199 date range.
- Every `pub` item gets a doc comment, with a runnable example
  wherever an example is informative (`missing_docs` is warn +
  `cargo doc -D warnings`).

## Tests

- New behavior needs example-based tests with independently verifiable
  anchors (real published holiday dates, ISDA paper examples, …).
- New invariants (additivity, idempotence, monotonicity) should be
  property tests — see the existing proptest suites for the house
  style.
- Calendar data must be checkable against public sources; cite the
  source in a comment when it is not obvious.

## Adding a calendar

1. New module under `src/calendars/<country>/`, one file per market
   variant.
2. Express the holiday set as `Rule`s in a `pub const Calendar<'static>`.
3. Document the holiday table in the doc comment (holiday | rule), with
   effective-year boundaries in brackets.
4. Anchor tests: at least one full recent year verified against a
   published holiday list, plus the historical rule transitions.
5. If ported from QuantLib, attribute the upstream file in the module
   docs and document any deliberate deviation.

## Commit hygiene

Small commits that each leave the workspace green. Write commit
messages that explain *why*, not just *what*.
