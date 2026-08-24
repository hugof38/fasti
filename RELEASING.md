# Releasing

Two artifacts, two tag patterns, one version number. The crate goes to
crates.io on `v*`; `fasti-py` goes to PyPI on `py-v*`. The patterns do
not overlap — `py-v0.2.0` does not match `v*` — so tagging one never
fires the other, and each workflow refuses to publish if its tag
disagrees with its own manifest.

Keeping the two numbers equal is a choice, not a constraint. It means a
reported version identifies the same code whichever side of the boundary
it was seen from, which is worth more than the freedom to let them
drift.

| | crate | `fasti-py` |
|---|---|---|
| tag | `v0.2.0` | `py-v0.2.0` |
| workflow | `release.yml` | `release-python.yml` |
| version of record | `Cargo.toml` | `bindings/python/Cargo.toml` |
| index | crates.io | PyPI |
| credential | `CARGO_REGISTRY_TOKEN` in the `crates-io` environment | trusted publishing from the `pypi` environment |

## Before the first PyPI release

Both are one-time and both live outside the repository.

1. **A pending publisher on PyPI.** `fasti-py` does not exist there yet,
   and trusted publishing normally attaches to a project that does, so
   the first release needs PyPI → *Your projects* → *Publishing* → *Add
   a pending publisher*:

   | field | value |
   |---|---|
   | PyPI Project Name | `fasti-py` |
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

2. **Dry-run the Python train first.** See below. PyPI never lets a
   version be re-uploaded, so a half-published `0.2.0` is burned and the
   next attempt has to be `0.2.1`. The pre-release costs one extra tag
   and buys the knowledge that fifteen artifacts actually build.

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
   python -m venv /tmp/v && /tmp/v/bin/pip install fasti-py==0.2.0
   /tmp/v/bin/python -c "import fasti; print(fasti.__version__)"
   ```

   Check the PyPI page shows `README-py.md` and not the crate's README —
   they collide in the sdist, which is why the file has that name.

## The pre-release dry run

The Python release path has never executed: `release-python.yml` only
reaches `main` with the bindings themselves, so nothing in it has run
beyond YAML parsing. The parts most likely to break are the ones CI
never exercises — 3.14t wheels inside a manylinux container, aarch64
under QEMU, and the macOS x86_64 runner.

A pre-release runs all of it against the real PyPI without spending the
version. `pip` will not install it without `--pre`.

Cargo's `0.2.0-rc.1` renders as PEP 440 `0.2.0rc1`, which is what the
artifacts are named and what PyPI stores. `verify` compares the tag
suffix to the manifest verbatim, so the tag carries Cargo's spelling.

Tag a commit that exists only as a tag, so no branch carries an rc
version:

```bash
git switch --detach main
sed -i '0,/^version = "0.2.0"$/s//version = "0.2.0-rc.1"/' bindings/python/Cargo.toml
(cd bindings/python && cargo update -p fasti-py --offline)
git commit -am "fasti-py 0.2.0-rc.1, for a release dry run"
git tag py-v0.2.0-rc.1
git push origin py-v0.2.0-rc.1        # the tag only, never the commit
git switch -                          # main is untouched
```

Watch the run. What it proves, in order of how likely it is to be what
breaks:

- fifteen artifacts land in `audit` — one sdist, seven abi3 wheels,
  seven 3.14t wheels. A missing one is a build that failed on a
  platform no CI job covers.
- `abi3audit --strict` passes on the seven abi3 wheels, and is not
  asked about the 3.14t ones, which are version-specific by design.
- the OIDC exchange with PyPI works, which is the step that fails if
  the pending publisher or the environment name is wrong by a
  character.

Then install it somewhere clean:

```bash
python -m venv /tmp/rc && /tmp/rc/bin/pip install --pre fasti-py==0.2.0rc1
/tmp/rc/bin/python -c "import fasti, datetime as dt; \
  from fasti.calendars import us; \
  print(fasti.__version__, us.SETTLEMENT.is_business_day(dt.date(2026, 7, 3)))"
```

If it all passes, the real tags are steps 3 and 4 above. If it does not,
the fix lands on `main` and the next dry run is `py-v0.2.0-rc.2` — rc
numbers are cheap, `0.2.0` is not.

Delete the dangling rc commit's tag once it has served its purpose
(`git push origin :py-v0.2.0-rc.1`); the pre-release stays on PyPI,
where it is harmless and hidden from `pip` without `--pre`.
