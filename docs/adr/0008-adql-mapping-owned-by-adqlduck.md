# ADR-0008: ADQL mapping is owned by adqlduck; this repo owns a contract test

- **Status**: Accepted — maintainer handoff, 2026-10-11 (`HANDOFF-adqlduck.md`)
- **Date**: 2026-10-13
- **Open Question**: none; raised by the adqlduck handoff
- **Gate**: 0 (decision); the contract test grows with Gates 1–4

## Context

README, AGENTS.md, ADR-0004 and ADR-0005 assumed `docs/adql-mapping.md`
lives in this repo. adqlduck now has two backends: `plain` (SQL macros, no
extension) and `healpix` (this extension). It generates its mapping doc from
the compiler's rule table, one column per backend, and its CI fails on drift.
A mapping doc kept here could not be tested against the compiler, so it would
drift.

## Options

| # | Option | Gains | Costs | Verdict |
|---|--------|-------|-------|---------|
| A | Doc here, tested by adqlduck | Single-repo docs | adqlduck CI must read this repo; drift is caught late, in the wrong repo | Rejected |
| B | Doc generated in adqlduck; contract test here | Mapping is derived from the rule table it documents; this repo pins only what adqlduck calls | Two places to read for the full picture | **Chosen** |
| C | Both | Nothing | Two copies that must agree by hand | Rejected |

## Decision

**B.** adqlduck generates and owns `docs/adql-mapping.md`. This repo owns
`test/sql/contract_adqlduck.test`, which asserts exactly what adqlduck emits
or depends on. Changing an existing case there is a semver-level contract
change and needs a `CHANGELOG.md` entry.

This supersedes the `docs/adql-mapping.md` mentions in the Consequences of
ADR-0004 and ADR-0005. Those ADRs are not edited.

## Consequences

- ADR-0004's point "state that ours is ADQL 2.1-specified" and ADR-0005's
  point "`healpix: internal error` is the stable discriminator" move into the
  header of `test/sql/contract_adqlduck.test` and into adqlduck's generated doc.
- The `adql_circle` / `adql_polygon` aliases are removed from the README API:
  adqlduck emits the real function names, so the aliases had no consumer.
- Contract cases are added at the end of each gate per the handoff table
  (Gate 1 `hpx29`, Gate 2 `moc_*`, Gate 3 `sky_*` and ADR-0009, Gate 4
  `hats_partitions` output schema).
- `make test` runs the contract file with the rest of `test/sql/`; no new
  Make target.
