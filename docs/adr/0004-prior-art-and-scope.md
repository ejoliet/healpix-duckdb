# ADR-0004: Prior art does not cover HEALPix/MOC/HATS; v1 scope stands

- **Status**: Accepted (with one unverified item, see Open Items)
- **Date**: 2026-10-09
- **Open Question**: #4 — "Is pyacid's `XMATCH` already a DuckDB extension? If yes, scope v1 to MOC + pruning only and collaborate"
- **Gate**: 0

## Context

The question behind OQ4 is broader than `pyacid`: *does anything already give
DuckDB HEALPix pixels, MOC regions, or HATS partition pruning?* If so, this
project should narrow to the uncovered part and collaborate rather than
duplicate.

Survey of `duckdb/community-extensions@main`, 363 published extensions,
checked 2026-10-09. Every astronomy-, sphere-, or grid-adjacent extension:

| Extension | Lang | What it does | HEALPix? | MOC? | HATS pruning? |
|-----------|------|--------------|----------|------|---------------|
| `astro` | C++ | 62 functions: equatorial/galactic transforms, angular separation, photometry, dust extinction, cosmological distances, Keplerian orbits, sidereal time, FITS I/O via cfitsio | No | No | No |
| `celestial` | C++ | Astronomical coordinate utilities; `angular_separation_deg` | No | No | No |
| `geography` | C++ | Google S2 spherical indexing and analysis | No — S2 cells are not HEALPix cells | No | No |
| `s2raster` | Rust | S2 scalar/table functions plus GeoTIFF raster reads | No | No | No |
| `h3` | C++ | Uber H3 hexagonal grid indexing | No — different tessellation | No | No |
| `duck_dggs` | C++ | DGGRID v8 discrete global grid systems; default ISEA4H; `geo_to_seqnum`, `seqnum_to_geo`, `seqnum_to_boundary` | Not exposed — the API is DGGRID *seqnum* over a `GEOMETRY`, requires `spatial`, and emits planar WKT polygons | No | No |

Nothing in the registry provides a HEALPix NESTED index, a MOC type, MOC set
operations, MOC FITS/JSON/ASCII I/O, or HATS partition pruning.

Two findings sharpen the case rather than weaken it:

1. **`geography` declares `excluded_platforms: "wasm_mvp;wasm_eh;wasm_threads"`.**
   The S2 extension does not ship a browser build at all. The README's claim
   that S2 is "unverified in WASM" is understated: it is *excluded*. Since the
   entire point of this project is DuckDB-WASM in a service worker (invariant
   1), S2 could not have been the substrate even if its cells were usable.
2. **`astro` and `celestial` both ship an angular-separation function.** This
   is the one genuine overlap with our Gate 3 `sky_distance`. It is narrow:
   neither offers `sky_in_cone`/`sky_in_polygon` with ADQL 2.1 semantics
   (inclusive `CONTAINS`, great-circle edges, hemisphere rule — invariant 5),
   which is the contract `adqlduck` compiles against.

Outside DuckDB, the alternatives are all in other engines: `lsdb`/`hats`
(Python/Dask), `healpix-alchemy` (PostgreSQL `INT8RANGE`), `pgSphere` + `Q3C`
(PostgreSQL), `astropy-healpix` and `mocpy` (Python). These are this project's
**oracles**, not its competitors — they are why `test/golden/` can prove
correctness against independent implementations (invariant 4).

### On `pyacid`

**Not verified.** The delegated prior-art search for `pyacid` / `XMATCH` did
not return before this gate closed, and no `pyacid` extension appears in the
community-extensions registry (the authoritative list for the thing OQ4 asks
about: "is it already a *DuckDB extension*?"). The registry answer is
therefore **no** — whatever `pyacid` is, it is not a published DuckDB
community extension.

This is sufficient to unblock Gate 0, because the decision it gates
(narrow v1 to MOC + pruning and collaborate) is only triggered by an existing
DuckDB extension. It is not sufficient to claim no cross-match prior art
exists anywhere, and no such claim is made here.

## Options

| # | Scope | Rationale | Verdict |
|---|-------|-----------|---------|
| A | Full README v1: pixels, MOC, sky predicates, HATS pruning | No DuckDB extension covers any of the four; the gap is total | **Chosen** |
| B | Narrow to MOC + pruning only, reuse `astro`/`celestial` for angles and `geography` for the sphere | Would force users to load three extensions, inherit non-ADQL semantics from functions written for other purposes, and inherit `geography`'s WASM exclusion — violating invariant 1 | Rejected |
| C | Contribute HEALPix functions into `astro` instead of a new extension | `astro` is C++/cmake with a cfitsio dependency and a different maintainer and remit (photometry, cosmology, FITS). HEALPix+MOC is not a few more scalar functions; it is a type, set algebra, I/O formats, and a partition-pruning model | Rejected |

## Decision

**Option A.** The v1 scope in the README stands unchanged. Cross-match stays a
Non-Goal for v1 (to be built in the compiler on top of `hpx_ang2pix`
bucketing), so even a confirmed `pyacid XMATCH` would not have overlapped v1's
deliverables.

## Consequences

- No collaboration or scope reduction is triggered; Gates 1–5 proceed as
  written.
- `sky_distance` knowingly duplicates `astro_angular_separation` and
  `angular_separation_deg`. `docs/adql-mapping.md` (Gate 5) should state that
  ours is specified by ADQL 2.1, so the duplication is a semantics choice, not
  an oversight.
- `geography`'s WASM exclusion is worth re-checking at Gate 5: if S2 ever ships
  a WASM build, the "no browser alternative" argument weakens, though the
  HEALPix/MOC/HATS gap remains.
- This ADR's survey is a point-in-time snapshot of 363 extensions. Re-run it
  before the Gate 5 upstream submission.

## Open Items

- [ ] Confirm what `pyacid` is and whether its `XMATCH` is a DuckDB extension,
      a Python UDF, or SQL macros. Does not block Gates 0–4; revisit before
      Gate 5. If it turns out to be a compiled DuckDB extension doing HEALPix
      bucketed cross-match, reopen this ADR — it would not change v1 scope
      (cross-match is a Non-Goal) but it would change the Gate 5 positioning
      and is a collaboration lead.
