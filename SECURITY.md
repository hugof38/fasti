# Security policy

fasti is pre-1.0; fixes land on the latest release line only.

## Reporting

Do not open a public issue. Use a
[private advisory](https://github.com/hugof38/fasti/security/advisories/new),
or contact the maintainer in `.github/CODEOWNERS` if that is
unavailable to you. Expect a reply within a week; an accepted report
ships as a fix and an advisory together, crediting you unless you ask
otherwise.

## Scope

The crate is `no_std` with no I/O, no clock, no network and
`unsafe_code = "forbid"`, which empties most of the usual categories.
In scope: a panic or overflow reachable from safe public API with
in-range inputs, unbounded allocation driven by caller-supplied input,
and any soundness hole.

**Wrong calendar or day-count data is a correctness bug, not a
vulnerability.** It can have real financial consequences, but it is not
a security boundary, and reporting it privately only slows the fix and
hides it from the people who could confirm it. Open a normal issue
using the "Calendar or day-count data" template and cite a published
source.
