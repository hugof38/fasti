# Security policy

## Supported versions

fasti is pre-1.0. Only the most recent release line receives security
fixes; there are no long-term support branches yet.

| Version | Supported |
|---|---|
| 0.1.x | yes |

## Reporting a vulnerability

Please **do not open a public issue** for a security report.

Use GitHub's private reporting instead:
[Report a vulnerability](https://github.com/hugof38/fasti/security/advisories/new).
If private reporting is unavailable to you, email the maintainer listed
in `.github/CODEOWNERS`.

Expect an acknowledgement within a week. If the report is accepted, a
fix and an advisory are published together, and you are credited in the
advisory unless you ask otherwise.

## What counts as a vulnerability here

fasti is a `no_std` library with no I/O, no clock, no network, no
timezone database, and `unsafe_code = "forbid"`. That rules out most of
the usual categories. What is in scope:

- A panic or arithmetic overflow reachable from safe public API with
  in-range inputs. The crate documents itself as panic-free outside of
  indexing operators; a counter-example is a bug.
- Unbounded allocation driven by caller-supplied input — for example a
  `Schedule` or date range that allocates disproportionately to its
  arguments.
- Any soundness hole, which would necessarily also be a `forbid(unsafe_code)`
  violation.

## What does not

**Incorrect calendar or day-count data is a correctness bug, not a
security vulnerability** — report it as a normal issue. This matters
here: fasti is used in financial calculations, so a wrong holiday date
can have real consequences, but it is not a security boundary and gets
more eyes faster in public. Use the "Calendar or day-count data"
issue template and cite a published source.
