## What this changes

<!-- The behavior difference, in a sentence or two. Link the issue if there is one. -->

## Why

<!-- Commit messages here explain *why*, not *what* — same for the PR. -->

## Checks

- [ ] `cargo test --all-features --all-targets` and `cargo test --all-features --doc`
- [ ] `cargo clippy --all-features --all-targets -- -D warnings`
- [ ] `cargo fmt --all --check`
- [ ] `RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps`

## If this touches calendar or day-count data

- [ ] Anchored to a published source, cited in a comment
- [ ] At least one full recent year verified against that source
- [ ] Historical rule transitions covered
- [ ] Upstream file attributed if ported from QuantLib, with any deliberate deviation documented

## If this changes the public API

- [ ] Every new `pub` item has a doc comment, with a runnable example where one is informative
- [ ] CHANGELOG.md updated under `## [Unreleased]`
- [ ] Nothing added to the required dependency set (see ARCHITECTURE.md)
