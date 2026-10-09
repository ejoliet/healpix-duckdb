# ADR-0003: Pin DuckDB v1.5.6 and rebuild per DuckDB release

- **Status**: Accepted
- **Date**: 2026-10-09
- **Open Question**: #3 — "DuckDB version pin and bump policy (follow LTS line?)"
- **Gate**: 0

## Context

The README's Prerequisites table said "DuckDB 1.4.x, pinned in `Makefile`".
That is stale: `duckdb/extension-template-rs@main` and
`duckdb/extension-ci-tools@v1.5.6` both target **v1.5.6**.

The decisive constraint is the ABI, not a preference. The Rust template sets:

```make
# Set to 1 to enable Unstable API (binaries will only work on TARGET_DUCKDB_VERSION,
# forwards compatibility will be broken)
# Note: currently extension-template-rs requires this, as duckdb-rs relies on
# unstable C API functionality
USE_UNSTABLE_C_API=1
```

This is **not optional for a Rust extension**: `duckdb-rs` uses unstable C API
surface, so the build stamps `FIELD5 (abi_type) = C_STRUCT_UNSTABLE` into the
extension metadata. The loader enforces it. Observed directly on this machine:

```
$ duckdb --version
v1.5.5 (Variegata) d8cdaa33fd

$ duckdb -unsigned -c "LOAD 'build/debug/healpix.duckdb_extension'; SELECT hpx_ang2pix(12, 266.405, -28.936);"
Invalid Input Error: Failed to load 'build/debug/healpix.duckdb_extension',
The file was built specifically for DuckDB version 'v1.5.6' and can only be
loaded with that version of DuckDB. (this version of DuckDB is 'v1.5.5')
```

So "which version do we pin" has no clever answer: there is exactly one
version per build, and a DuckDB patch release invalidates every binary.
`INSTALL healpix FROM community` works for end users because the
community-extensions service builds and serves one artifact **per DuckDB
version per platform**; the version matrix is the pipeline's job, not ours.

The version appears in exactly three places, which must agree:

| File | Field | Value |
|------|-------|-------|
| `Makefile` | `TARGET_DUCKDB_VERSION` | `v1.5.6` |
| `.github/workflows/ci.yml` | `duckdb_version`, `ci_tools_version` | `v1.5.6` |
| `extension-ci-tools` submodule | git tag | `v1.5.6` (`3fd6109f`) |

`Cargo.toml` pins `duckdb = "~1.10506.0"`, whose version encodes DuckDB
1.5.6 (`1.<major><minor><patch>` → `1.1 05 06`), so the crate is pinned to the
same release by construction.

## Options

| # | Policy | Consequence | Verdict |
|---|--------|-------------|---------|
| A | Pin `v1.5.6`; bump deliberately, one ADR-free version bump per release, CI matrix does the rest | Matches the template and ci-tools defaults; one place to change; users get the right artifact from the community service | **Chosen** |
| B | Pin an LTS line and stay there | There is no separate LTS build channel in community-extensions; the pipeline builds against the versions it supports. Pinning "LTS" would just mean pinning an older number and shipping stale binaries | Rejected |
| C | Set `USE_UNSTABLE_C_API=0` for forward compatibility across versions | Not available: `duckdb-rs` requires the unstable C API. Turning it off does not compile | Rejected — not a real option |
| D | Track `main`/latest automatically | Every DuckDB patch silently breaks the build with no human in the loop; golden fixtures and CI would churn | Rejected |

## Decision

Pin **DuckDB v1.5.6** via `TARGET_DUCKDB_VERSION` in the `Makefile`, the
`v1.5.6` tag of the `extension-ci-tools` submodule, and `duckdb_version` /
`ci_tools_version` in `.github/workflows/ci.yml`.

Bump policy: a version bump is a single coordinated change to those three
places plus `Cargo.toml`'s `duckdb` dependency, landed as its own commit, with
`make test` green before and after. Per AGENTS.md ("Don't change DuckDB version
pin without an ADR"), **this ADR is the standing authorisation for routine
bumps that keep all four in lockstep**; a new ADR is required only to pin to
something other than the version that `extension-ci-tools` itself targets, or
to change the ABI mode.

The README Prerequisites table should be corrected from "1.4.x" to "1.5.6".

## Consequences

- **A contributor's system DuckDB CLI will usually refuse to load a local
  build**, exactly as shown above. This is expected, not a bug. Use the
  matching CLI version, or test through `make test`, which uses the pinned
  `duckdb==1.5.6` Python package in `configure/venv`.
- The `Quick Start` section of the README tells users to run `duckdb -unsigned`
  and `LOAD 'build/release/...'`. That only works if their CLI is exactly
  v1.5.6; the README should say so.
- Every DuckDB release requires a rebuild and a re-run of the golden fixtures.
  Because the fixtures assert against independent Python oracles rather than
  against DuckDB itself, a version bump should produce **zero** fixture diffs;
  any diff is a real regression (invariant 4).
- Gate 5's `description.yml` does not pin a DuckDB version — the community
  pipeline supplies it — so this decision lives entirely in our build files.
