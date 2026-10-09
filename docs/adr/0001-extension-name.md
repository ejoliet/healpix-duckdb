# ADR-0001: The extension is named `healpix`

- **Status**: Accepted
- **Date**: 2026-10-09
- **Open Question**: #1 — "Extension name: `healpix` (clear, but DuckDB Foundation may reserve it) vs `hpx` vs `skypix`"
- **Gate**: 0

## Context

The extension name is a semver-level public contract: it is what six downstream
repos write in `INSTALL <name> FROM community; LOAD <name>;`, and renaming it
after Gate 1 requires a semver-major note in `CHANGELOG.md` (AGENTS.md, Out of
Scope). The README flagged a risk that the DuckDB Foundation might reserve
`healpix`.

Evidence (checked 2026-10-09 against `duckdb/community-extensions@main`,
363 published extensions in `extensions/`):

- There is **no** extension named `healpix`, `hpx`, or `skypix`.
- The DuckDB core/in-tree extension list (`autocomplete`, `aws`, `azure`,
  `delta`, `excel`, `fts`, `httpfs`, `iceberg`, `icu`, `inet`, `jemalloc`,
  `json`, `mysql`, `parquet`, `postgres`, `spatial`, `sqlite`, `tpcds`, `tpch`,
  `ui`, `vss`) contains no HEALPix or astronomy name. Nothing suggests a
  reservation.
- Adjacent names that *do* exist and are **not** collisions, but do set
  expectations for how this one is read:
  - `astro` — astronomical calculations and FITS I/O, 62 functions, C++.
  - `celestial` — astronomical coordinate utilities, C++.
  - `geography` — S2 spherical geometry, C++.
  - `h3` — H3 hexagonal grid indexing, C++.
  - `duck_dggs` — DGGRID v8 discrete global grid systems, C++.
  - `s2raster` — S2 plus GeoTIFF, Rust.
- No naming rule in the community-extensions repo restricts the character set
  beyond lowercase identifier form, and `healpix` is a valid SQL identifier and
  a valid Rust crate name.

The three candidates differ in legibility, not availability:

- `healpix` is the actual, universally used name of the tessellation
  (Hierarchical Equal Area isoLatitude Pixelisation). Every astronomer
  searching for this will search that word.
- `hpx` collides in search with HPX, the well-known C++ parallel runtime
  library, and is already this project's *function* prefix (`hpx_ang2pix`), so
  reusing it as the extension name is a confusing double meaning.
- `skypix` is a different, historical pixelisation scheme (LSST's SkyPix) and
  would actively mislead.

## Options

| # | Name | Discoverability | Collision risk | Verdict |
|---|------|-----------------|----------------|---------|
| A | `healpix` | Exact, canonical term | None found in community or core | **Chosen** |
| B | `hpx` | Poor — collides with the HPX C++ runtime; duplicates the function prefix | None in DuckDB, high in web search | Rejected |
| C | `skypix` | Actively misleading — names a *different* scheme | None in DuckDB, semantic collision in astronomy | Rejected |

## Decision

The extension is named **`healpix`**. The crate is `healpix`, the built
artifact is `healpix.duckdb_extension`, and `description.yml` (Gate 5) will
declare `name: healpix` with
`description: HEALPix and MOC for astronomy on the sphere`.

Function names keep the `hpx_`, `moc_`, and `sky_` prefixes already fixed by
the README API contract. The extension name and the function prefix are
deliberately different words.

## Consequences

- Downstream repos (`adqlduck`, `sidetap`, `tapdrop`, `obstap-local`,
  `lookout`, `catdiff`) can hard-code `INSTALL healpix FROM community;`.
- The name is claimed only when `description.yml` is merged upstream in Gate 5.
  Until then another project could take it. This is a real but low risk given
  that no astronomy HEALPix extension exists today; the mitigation is to not
  dawdle between Gate 4 and Gate 5, not to pick a worse name now.
- `astro` and `celestial` both already provide an angular-separation function.
  `sky_distance` (Gate 3) will overlap them numerically. That is acceptable:
  this extension's `sky_in_cone` / `sky_in_polygon` must match **ADQL 2.1**
  semantics exactly (invariant 5), which is a stricter contract than either of
  those extensions offers, and a user should not have to load three astronomy
  extensions to run one cone search.
