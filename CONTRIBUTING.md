# Contributing to fasti

Thanks for your interest! Issues and pull requests are welcome. By
participating you agree to the
[Code of Conduct](./CODE_OF_CONDUCT.md); to report a suspected
vulnerability, follow [SECURITY.md](./SECURITY.md) rather than opening
an issue.

## Development workflow

The crate is a single standard Cargo library. All of these must pass
before a change is considered green (CI enforces them):

```bash
cargo test --locked --all-features --all-targets
cargo test --locked --all-features --doc
cargo clippy --locked --all-features --all-targets -- -D warnings
cargo fmt --all --check
RUSTDOCFLAGS="-D warnings" cargo doc --locked --all-features --no-deps
cargo deny check          # licenses, advisories, banned dependencies
```

Doctests do not run under `--all-targets`, which is why they get their
own line. The minimum supported Rust version is pinned in `Cargo.toml`
(`rust-version`); CI builds against both stable and the MSRV.

`Cargo.lock` is committed even though this is a library. It does not
affect what downstream users resolve — it exists so that the MSRV job
tests a dependency set known to build on that toolchain, instead of
turning red when an unrelated crate raises its own MSRV. A separate CI
job runs `cargo update` first, so the declared version requirements
stay honest and the lockfile never becomes load-bearing. Regenerate it
with `cargo update` when you change a dependency; never hand-edit it.

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
- `tests/public_api.rs` compiles as a separate crate against the
  published surface only. Anything reachable in-crate but not
  re-exported from the root fails there rather than in a downstream
  build, so a new `pub` type belongs in that file's import list.

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

## Python bindings

`bindings/python` is a PyO3 extension module published to PyPI as
`fasti-py` and imported as `fasti`. It is a separate cargo workspace with its own `Cargo.lock`, on
purpose: pyo3 and a C toolchain have no business in the core crate's
MSRV job, lockfile, or `cargo deny` run.

```bash
cd bindings/python
python -m venv .venv && . .venv/bin/activate
pip install maturin pytest mypy
maturin develop           # build and install into the venv
pytest tests -q           # includes every >>> example in the package
mypy python/fasti         # the stubs are part of the surface
cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
```

Ground rules on top of the crate's own:

- The boundary types are Python's. Dates in and out are
  `datetime.date`. Nothing else is a date, and near-misses are refused
  by name rather than coerced: a string has a format
  (`datetime.date.fromisoformat`'s job, not a second date grammar for us
  to maintain), and a `datetime.datetime` has a time and often a zone,
  so which day it falls on is the caller's decision to write down. Year
  fractions are `fractions.Fraction` — never a float, for the same
  reason the crate has none.
- Every name a Python user can reach belongs in
  `python/fasti/_fasti.pyi`. Tests fail on drift in either direction.
- A date position is annotated `datetime.date` outright, never behind an
  alias. The `*Like` aliases stand for genuine unions (`"6M"` really is
  a `Period`); a date is not one, and spelling it out is what makes the
  type error read `expected "date"` at the call site.
  `tests/test_typing.py` runs mypy over the stubs to hold that.
- Doc comments on `#[pyclass]` and `#[pyfunction]` items become Python
  docstrings, and their `>>>` examples run under pytest. Write them as
  doctests, without markdown fences.
- A new convention/calendar/rule name needs the alias spellings a caller
  is likely to type; matching ignores case and punctuation. Aliases are
  a convenience, never the definition: the canonical spelling is what a
  value prints, pickles as, and is offered in an error. Two spellings a
  reader would take for synonyms must not mean different things — `fed`
  and `federal` did, so `federal` is not a spelling of anything.
- Every type stays immutable (`#[pyclass(frozen)]`) and picklable. A
  new type needs a `__reduce__` that names a `_rebuild_*` function and
  the arguments to replay — not a serialized copy of its internals —
  and a round-trip test that checks the rebuilt value *behaves* the
  same, not merely that it compares equal.
- Anything that can run long without touching Python — walking a date
  range, say — belongs inside `Python::detach`, so a thread waiting on
  the interpreter is not waiting on us.
- The version in `bindings/python/Cargo.toml` is the PyPI version, and
  the `py-v*` release tag has to match it. Keep it in step with the
  crate version unless a binding-only fix needs to ship alone.

## Releasing

Two release trains, one repository. The tag is the trigger; the version
of record is the one in the manifest, and each workflow refuses to
publish if the two disagree — which is the failure that would otherwise
burn a version number on a registry that never lets it go.

| Release | Bump | Tag | Workflow | Publishes |
|---|---|---|---|---|
| Rust crate | `Cargo.toml` | `v0.2.0` | `release.yml` | crates.io + a GitHub release |
| Python package | `bindings/python/Cargo.toml` | `py-v0.2.0` | `release-python.yml` | PyPI (wheels + sdist) |

The patterns do not overlap: `py-v0.2.0` does not match `v*`, so tagging
one train never fires the other. To release both, bump both manifests in
the same commit and push both tags — the runs are independent, and one
failing does not block the other.

```bash
# Rust only
git tag v0.2.0 && git push origin v0.2.0

# Python only
git tag py-v0.2.0 && git push origin py-v0.2.0

# Both
git push origin v0.2.0 py-v0.2.0
```

Tag the merge commit on `main`, not a branch head — a tag is what the
workflow checks out.

Keep the two versions in step unless a binding-only fix genuinely needs
to ship alone. The wheel is built from the crate *source at that commit*
(a path dependency, not the crates.io release), so a Python release can
carry core changes that crates.io has not seen. Matching version numbers
are what makes "which crate is in this wheel?" answerable.

`CHANGELOG.md` is keyed by crate version, and the Rust workflow refuses
to release a version with no section in it. A Python-only release has no
section of its own; note it under the crate version it ships.

Before a train's first run, its credentials have to exist: the crates.io
job needs `CARGO_REGISTRY_TOKEN` in the `crates-io` environment (or a
crates.io Trusted Publisher), and the PyPI job needs a Trusted Publisher
configured for this repository and `release-python.yml`, plus a `pypi`
environment. Both are one-time setup outside the repository.

If a release fails *before* publishing — a gate, a mismatched tag — fix
it, delete the tag, and re-tag. If it fails *after* one registry has
accepted the upload, that version is spent: neither crates.io nor PyPI
allows re-uploading one, so bump and tag again.

## Commit hygiene

Small commits that each leave the workspace green. Write commit
messages that explain *why*, not just *what*.

## AI-assisted contributions

They are welcome, on one condition: you have read the diff you are
submitting and can defend it in review. Review effort is the scarce
resource here, and a patch its author cannot explain spends more of it
than it saves.

Two things matter more than usual in this crate, and generated code is
unreliable at both:

- **Calendar data must trace to a published source**, not to a model's
  recollection of one. Cite the source in a comment; a plausible-looking
  holiday table with no citation will be asked for one.
- **Ported logic must match its stated upstream.** If a change claims
  QuantLib parity, that claim gets checked against the upstream file.

You do not need to disclose tool use, and there is no sign-off
requirement. Submitting a patch means you have the right to contribute
it under the dual license above.
