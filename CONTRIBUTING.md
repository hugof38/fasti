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

`bindings/python` holds the `fasti-dates` distribution (import name
`fasti`). It is **its own cargo workspace with its own `Cargo.lock`**, so
pyo3 never reaches the core crate's MSRV job, lockfile or `cargo deny`
run; the root manifest excludes it from the packaged `.crate`. Working
on it:

```bash
cd bindings/python
maturin develop            # build the extension into the active venv
pytest                     # suite, plus every >>> example in the
                           # package and README-py.md
mypy                       # stubs and tests, strict
cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
```

The binding is a translation layer, not a second library. Every Python
name maps to one in the crate, with the same method names, the same
semantics and the same argument order; anything with no crate
counterpart has to earn its place in a sentence. Dates cross the
boundary as `datetime.date` and nothing else, and year fractions come
back as `fractions.Fraction` — the crate is float-free and that has to
survive the boundary. See
[`bindings/python/README-py.md`](./bindings/python/README-py.md) for the
rest of the rules, and the note there on why the file is not called
`README.md`.

Two things worth knowing before you change something:

- CI pins **Python 3.10**, the abi3 floor. `doctest` compares exception
  messages differently before 3.11, so write an expected traceback as
  one line ending in an ellipsis.
- Doc comments on `#[pyclass]` and `#[pyfunction]` items become Python
  docstrings, and their `>>>` examples run under pytest. Write them
  without markdown code fences — a fence becomes expected output.

## Releases

Two independent trains, whose tag patterns do not overlap:

| Tag | Workflow | Publishes | Version of record |
|---|---|---|---|
| `v*` | `release.yml` | the crate, to crates.io | `Cargo.toml` |
| `py-v*` | `release-python.yml` | `fasti-dates`, to PyPI | `bindings/python/Cargo.toml` |

Both carry the same version number, so a bug report naming one
identifies the same code on either side of the boundary. The full
procedure — the one-time PyPI setup, and the pre-release dry run that
comes before spending a version number — is in
[`RELEASING.md`](./RELEASING.md).

Each workflow refuses to publish if its tag disagrees with its own
manifest. The Python train builds one abi3 wheel per platform
(manylinux and musllinux for x86_64 and aarch64, macOS for both
architectures, Windows x64), plus a free-threaded 3.14t wheel per
platform, checks them with `abi3audit --strict` and
`twine check --strict`, and publishes through PyPI trusted publishing
from a `pypi` environment. `py-v0.2.0` is not matched by `v*`, so
tagging one never fires the other.

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
