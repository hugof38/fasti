# Releasing

Two artifacts, two tag patterns, one version number. The crate goes to
crates.io on `v*`; `fasti-dates` goes to PyPI on `py-v*`. The patterns do
not overlap — `py-v0.2.0` does not match `v*` — so tagging one never
fires the other, and each workflow refuses to publish if its tag
disagrees with its own manifest.

Keeping the two numbers equal is a choice, not a constraint. It means a
reported version identifies the same code whichever side of the boundary
it was seen from, which is worth more than the freedom to let them
drift.

| | crate | `fasti-dates` |
|---|---|---|
| tag | `v0.2.0` | `py-v0.2.0` |
| workflow | `release.yml` | `release-python.yml` |
| version of record | `Cargo.toml` | `bindings/python/Cargo.toml` |
| index | crates.io | PyPI |
| credential | `CARGO_REGISTRY_TOKEN` in the `crates-io` environment | trusted publishing from the `pypi` environment |

## Before the first PyPI release

Both are one-time and both live outside the repository.

1. **A pending publisher on PyPI.** `fasti-dates` does not exist there
   yet, and trusted publishing normally attaches to a project that does,
   so the first release needs PyPI → *Your projects* → *Publishing* →
   *Add a pending publisher*. The name is not `fasti-py`: PyPI strips a
   `py` affix before comparing names, so that one collides with the
   unrelated `fasti` already on the index and is refused.

   | field | value |
   |---|---|
   | PyPI Project Name | `fasti-dates` |
   | Owner | `hugof38` |
   | Repository name | `fasti` |
   | Workflow name | `release-python.yml` |
   | Environment name | `pypi` |

   It becomes an ordinary trusted publisher after the first upload.

2. **A `pypi` GitHub environment.** Settings → Environments → New
   environment, named `pypi`. The name has to match the PyPI form
   exactly or the OIDC exchange fails with a 403 that does not say why.
   Referencing an environment that does not exist silently creates one
   with no rules, so make it deliberately and give it what a job that
   publishes irreversibly should have: a required reviewer, and
   deployment tags limited to `py-v*`.

The crate side already has its `crates-io` environment and token.

## Release

Run the whole thing from `main`, after the release-prep commit has
merged.

1. **Check the manifests and the changelog agree.** Both `Cargo.toml`
   files say the version being released, and `CHANGELOG.md` has a
   `## [<version>]` heading — `release.yml` greps for that heading and
   refuses to publish without it.

2. **Rehearse the Python train first**, with the manual run described
   below. It builds and audits every artifact without uploading, which
   is the cheap way to find out that a wheel does not build on a
   platform no CI job covers.

3. **Tag the crate**, and wait for it to finish:

   ```bash
   git tag v0.2.0 && git push origin v0.2.0
   ```

   `verify` (tag, manifest, changelog) → `gates` (fmt, clippy, tests,
   doctests, `cargo package`) → `publish` → a GitHub release whose body
   is that changelog section.

4. **Tag the bindings:**

   ```bash
   git tag py-v0.2.0 && git push origin py-v0.2.0
   ```

   `verify` → `gates` (fmt, clippy, pytest, mypy on 3.10, the abi3
   floor) → `sdist` + seven abi3 wheels + seven free-threaded 3.14t
   wheels → `audit` (`twine check --strict` on everything,
   `abi3audit --strict` on the abi3 wheels) → `publish`.

5. **Confirm both**, from outside the checkout:

   ```bash
   cargo add fasti@0.2.0 --dry-run
   python -m venv /tmp/v && /tmp/v/bin/pip install fasti-dates==0.2.0
   /tmp/v/bin/python -c "import fasti; print(fasti.__version__)"
   ```

   Check the PyPI page shows `README-py.md` and not the crate's README —
   they collide in the sdist, which is why the file has that name.

## The rehearsal

The Python release path has never executed: `release-python.yml` only
reaches `main` with the bindings themselves, so nothing in it has run
beyond YAML parsing. The parts most likely to break are the ones CI
never exercises — 3.14t wheels inside a manylinux container, aarch64
under QEMU, and the macOS x86\_64 runner.

Run it manually first, from Actions → *Release (Python)* → *Run
workflow*, on `main`. `verify` and `publish` are gated on the ref being
a `py-v*` tag, so a manual run does everything except upload: sdist,
seven abi3 wheels, seven free-threaded 3.14t wheels, `twine check
--strict` over all of them and `abi3audit --strict` over the abi3 ones.

What to look for, in the order it is likely to be what breaks:

- fifteen artifacts reach `audit` — one sdist, seven abi3 wheels, seven
  3.14t wheels. A missing one is a build that failed on a platform no
  CI job covers.
- `abi3audit --strict` passes on the seven abi3 wheels, and is not
  asked about the 3.14t ones, which are version-specific by design.
- `publish` shows as skipped rather than failed.

This costs nothing and can be repeated. A pre-release tag would prove
one extra thing — that the OIDC handshake with PyPI works — but that
step uploads nothing when it fails, so a misconfigured publisher costs
a re-tag rather than a version number, and it is not worth leaving
permanent pre-releases on the index to find out early.

## When a tag goes wrong

Nothing is uploaded unless every build and both audits passed:
`publish` needs `audit`, and `audit` needs all three build jobs. A
failure anywhere upstream leaves PyPI untouched, so the recovery is:

```bash
git push origin :py-v0.2.0      # delete the remote tag
git tag -d py-v0.2.0            # and the local one
# fix, merge the fix to main, then tag again
```

The one case that does spend the version is a failure *inside* the
upload after some files have landed, because PyPI will not accept a
second file under a name it already has. If that happens, do not fight
it: yank what is there and release `0.2.1`. Deleting a release does not
free the version either.
