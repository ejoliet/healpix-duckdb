# duckdb-healpix

> HEALPix pixels, IVOA MOC (Multi-Order Coverage) regions, and ADQL-exact cone and polygon search for DuckDB and DuckDB-WASM.

**GitHub About**: `HEALPix, MOC, and exact sky-region queries for DuckDB, native and in the browser.`

![CI](https://github.com/ejoliet/duckdb-healpix/actions/workflows/ci.yml/badge.svg)
![DuckDB](https://img.shields.io/badge/duckdb-1.4%2B-yellow)
![Rust](https://img.shields.io/badge/rust-1.80%2B-orange)
![License](https://img.shields.io/badge/license-MIT-lightgrey)

**RDD type**: A — spec. An agent implements from this file plus `AGENTS.md`.
**Status**: Gate 0 not started. Date: 2026-10-08.

---

## Purpose

**Problem**: DuckDB `spatial` is planar. It is wrong near the poles and across RA = 0, and it knows nothing about HEALPix, MOC, or HATS partitioning. Every astronomy tool on DuckDB re-implements "cone → HEALPix prefilter → haversine" by hand, differently, and untested.

**Solution**: one extension, `LOAD healpix;`, that gives SQL:

- HEALPix NESTED pixel math at any order 0–29.
- A `MOC` type stored as sorted level-29 interval pairs (the healpix-alchemy / Reinecke & Hivon model), with union, intersect, contains, area, and FITS/JSON/ASCII I/O.
- ADQL-exact `CIRCLE` and `POLYGON` predicates with great-circle edges.
- HATS partition pruning: a MOC becomes the list of `Norder/Dir/Npix` Parquet files to read.

**Scope**: the shared primitive under `adqlduck` (ADQL→DuckDB compiler), `sidetap`, `tapdrop`, `obstap-local`, `lookout`, and `catdiff`. Must run in DuckDB CLI, Python, and DuckDB-WASM in a service worker.

> 💡 Spherical geometry alone is not the gap — the `geography` (S2) community extension already does that. S2 cells are not HEALPix cells, so it cannot prune HATS or speak MOC. This extension targets exactly that gap.

---

## Architecture

```
ADQL / SQL
   │
   ▼
adqlduck compiler ──► SQL calling healpix functions
                              │
          ┌───────────────────┼───────────────────┐
          ▼                   ▼                   ▼
   pixel functions       MOC type + ops     sky predicates
   (cdshealpix)          (moc crate)        (own, great-circle)
          │                   │                   │
          └───────────────────┼───────────────────┘
                              ▼
            hats_partitions() SQL macro = read_csv(partition_info) ⋈ moc_overlaps_cell
                              │
                              ▼
            read_parquet([pruned files]) WHERE _healpix_29 BETWEEN lo AND hi AND sky_in_cone(...)
```

**Two-pass query model** (the design invariant):

1. **Coarse**: `moc_ranges(moc)` → `[lo, hi)` level-29 intervals → `BETWEEN` on the sorted `_healpix_29` column. Parquet row-group min/max statistics prune without any index.
2. **Exact**: `sky_in_cone` / `sky_in_polygon` only on rows from partially-covered cells (BMOC `full = false`). Rows from fully-covered cells skip the predicate.

**Key components**:

| Component | Responsibility | Backing crate |
|-----------|---------------|---------------|
| `src/pixel.rs` | `hpx_*` scalar functions, order 0–29, NESTED only | `cdshealpix` |
| `src/moc.rs` | `MOC` logical type (BLOB alias), constructors, set ops, I/O | `moc` |
| `src/sky.rs` | `sky_distance`, `sky_in_cone`, `sky_in_polygon` | own code, `f64` only |
| `src/macros.sql` | `hats_partitions`, `adql_*` convenience macros | SQL macros, no Rust |
| `src/lib.rs` | registration, versioning, `function_descriptions.csv` | `extension-template-rs` |

The extension does **no file I/O**. HATS pruning is a SQL macro over `read_csv()` so it works identically in WASM, where the C API file system is unreliable.

---

## Recommended Stack

| Layer | Chosen | Why | Rejected |
|-------|--------|-----|----------|
| Extension framework | `duckdb/extension-template-rs` (C Extension API) | Official; produces `.duckdb_extension` with `make`; no C++ glue | C++ template + `cxx` bridge (more surface, slower CI); `quack-rs` / `duckfn` (good, but a second abstraction layer to debug under WASM) |
| HEALPix math | `cdshealpix` | CDS reference Rust impl; BMOC gives full/partial flags per cell; already runs in Aladin Lite WASM | `healpix` C++ via FFI (no WASM story); hand-rolled (no) |
| MOC | `moc` (cds-moc-rust) | MOC 2.0 spec, FITS/JSON/ASCII I/O, ships MOCWasm | own interval code (would reinvent) |
| Spherical predicates | own, in `sky.rs` | ADQL semantics are specific (winding, hemisphere rule); ~200 lines; no dependency | `geography` (S2): correct but C++, unverified in WASM, and a 2nd geometry model |
| Test harness | DuckDB sqllogictest (`test/sql/*.test`) + Rust `proptest` + Python oracle scripts | sqllogictest is what community CI runs | pytest-only (cannot run in extension CI) |
| Oracles | `cdshealpix` (Python), `astropy-healpix`, `mocpy`, PostgreSQL + pgSphere + Q3C (Docker, fixtures committed) | Independent implementations | none |

> ⚠️ **Gate 0 blocker**: it is unverified whether the Rust template produces `wasm_mvp` / `wasm_eh` / `wasm_threads` artifacts in the community-extensions CI. Resolve before any other work. See Agent Build Instructions, Gate 0.

---

## Repository Layout

```
duckdb-healpix/
├── src/
│   ├── lib.rs               # entrypoint, registration, version()
│   ├── pixel.rs             # hpx_* scalars
│   ├── moc.rs               # MOC type, ops, I/O
│   ├── sky.rs               # sky_* predicates
│   └── macros.sql           # SQL macros registered at load
├── test/
│   ├── sql/                 # sqllogictest files, one per function family
│   │   ├── pixel.test
│   │   ├── moc.test
│   │   ├── sky.test
│   │   ├── hats.test
│   │   └── edge_cases.test  # poles, RA wrap, hemisphere polygons
│   ├── golden/              # committed oracle outputs (Parquet/CSV), generated by tools/oracle/
│   └── wasm/                # Node parity runner
├── tools/
│   ├── oracle/              # Python: generate test/golden from cdshealpix, mocpy, astropy-healpix
│   ├── pgsphere/            # docker-compose + SQL to regenerate Q3C/pgSphere fixtures
│   └── bench/               # Gaia DR3 HATS cone/pruning benchmark vs lsdb
├── docs/
│   ├── functions.md         # generated from function_descriptions.csv
│   ├── adql-mapping.md      # ADQL geometry → SQL (for adqlduck)
│   └── adr/                 # decisions (0001-interval-sets.md, ...)
├── description.yml          # duckdb/community-extensions manifest
├── Cargo.toml
├── Makefile                 # from extension-template-rs
└── README.md
```

---

## Prerequisites

| Requirement | Version | Notes |
|-------------|---------|-------|
| Rust | 1.80+ | `rustup`; `wasm32-unknown-emscripten` target for WASM builds |
| DuckDB | 1.4.x | pinned in `Makefile`; see Open Questions for bump policy |
| Python | 3.11+ | oracle generation only; `uv` managed |
| Node | 20+ | WASM parity test only |
| Docker | any | pgSphere/Q3C fixtures only; run by Emmanuel, not the agent |

No credentials. No network at test time: all fixtures are committed.

---

## Quick Start

```bash
git clone https://github.com/ejoliet/duckdb-healpix.git
cd duckdb-healpix
make                          # builds build/release/extension/healpix/healpix.duckdb_extension
duckdb -unsigned
```

```sql
LOAD 'build/release/extension/healpix/healpix.duckdb_extension';

-- pixel math
SELECT hpx_ang2pix(12, 266.405, -28.936) AS ipix;

-- MOC from a cone, as pruning ranges
SELECT * FROM moc_ranges(moc_from_cone(266.405, -28.936, 0.5, 12));

-- exact cone search on a sorted _healpix_29 column (two-pass)
WITH m AS (SELECT moc_from_cone(266.405, -28.936, 0.5, 12) AS moc)
SELECT c.*
FROM read_parquet('gaia/**/*.parquet') c, m
WHERE moc_contains(m.moc, c._healpix_29)
  AND sky_in_cone(c.ra, c.dec, 266.405, -28.936, 0.5);
```

Python (`uvx`-style consumer, no clone):

```python
import duckdb
con = duckdb.connect()
con.execute("INSTALL healpix FROM community; LOAD healpix;")  # after Gate 5
```

---

## Configuration Reference

The extension has no environment variables. Behaviour is controlled by SQL settings:

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `healpix_default_order` | `INT` | `12` | Order used by `moc_from_*` when `order` is omitted |
| `healpix_strict_input` | `BOOL` | `true` | Error on `ra ∉ [0,360)` or `dec ∉ [-90,90]`; if false, normalise RA and clamp Dec |

---

## API / Interface Contract

All angles are **degrees, ICRS, `DOUBLE`**. All pixel indices are **NESTED, `BIGINT`**. Order is `INT` in `[0, 29]`. RING scheme is not supported (see Non-Goals).

### Pixel functions

| Function | Returns | Notes |
|----------|---------|-------|
| `hpx_ang2pix(order, ra, dec)` | `BIGINT` | NESTED index |
| `hpx_pix2ang(order, ipix)` | `STRUCT(ra DOUBLE, dec DOUBLE)` | cell centre |
| `hpx29(ra, dec)` | `BIGINT` | `= hpx_ang2pix(29, ra, dec)`; the sort key |
| `hpx_parent(order, ipix, to_order)` | `BIGINT` | `to_order ≤ order` |
| `hpx_children(order, ipix, to_order)` | `LIST<BIGINT>` | `to_order ≥ order`; cap at 4^8 children per call |
| `hpx_range29(order, ipix)` | `STRUCT(lo BIGINT, hi BIGINT)` | half-open level-29 interval of the cell |
| `hpx_to_uniq(order, ipix)` / `hpx_from_uniq(uniq)` | `BIGINT` / `STRUCT(order, ipix)` | UNIQ encoding |
| `hpx_cone_cells(order, ra, dec, radius)` | `LIST<STRUCT(ipix BIGINT, full BOOL)>` | BMOC: `full=true` ⇒ cell entirely inside cone |
| `hpx_polygon_cells(order, ra[], dec[])` | same | BMOC for polygon |
| `hpx_cell_area_sr(order)` | `DOUBLE` | steradians |

### MOC type and functions

`MOC` is a logical type over `BLOB`. Layout: `u8 version (=1)`, `u8 depth (max order, ≤29)`, then `N × (i64 lo, i64 hi)` little-endian, sorted, disjoint, half-open at level 29. Empty MOC = header only.

| Function | Returns | Notes |
|----------|---------|-------|
| `moc_from_cone(ra, dec, radius [, order])` | `MOC` | approx coverage (all overlapping cells) |
| `moc_from_cone_inner(ra, dec, radius [, order])` | `MOC` | fully-inside cells only |
| `moc_from_polygon(ra[], dec[] [, order])` | `MOC` | great-circle edges |
| `moc_from_cells(order, ipix[])` | `MOC` | |
| `moc_from_ranges(lo[], hi[])` | `MOC` | normalises (sort + merge) |
| `moc_from_json(json)` / `moc_to_json(moc)` | `MOC` / `VARCHAR` | MOC 2.0 JSON |
| `moc_from_ascii(txt)` / `moc_to_ascii(moc)` | `MOC` / `VARCHAR` | MOC 2.0 ASCII |
| `moc_from_fits(blob)` / `moc_to_fits(moc)` | `MOC` / `BLOB` | FITS MOC (bytes, no file I/O) |
| `moc_ranges(moc)` | `LIST<STRUCT(lo, hi)>` | for `BETWEEN` pruning |
| `moc_contains(moc, hpx29)` | `BOOL` | binary search over ranges |
| `moc_contains_cell(moc, order, ipix)` | `BOOL` | cell fully inside |
| `moc_overlaps_cell(moc, order, ipix)` | `BOOL` | any overlap — the pruning primitive |
| `moc_union(a, b)` / `moc_intersect(a, b)` / `moc_diff(a, b)` | `MOC` | |
| `moc_union_agg(moc)` | `MOC` | aggregate |
| `moc_area_sr(moc)` / `moc_area_deg2(moc)` | `DOUBLE` | |
| `moc_degrade(moc, order)` | `MOC` | |
| `moc_is_empty(moc)`, `moc_ncells(moc)`, `moc_depth(moc)` | scalars | |

### Sky predicates (exact, great-circle)

| Function | Returns | Notes |
|----------|---------|-------|
| `sky_distance(ra1, dec1, ra2, dec2)` | `DOUBLE` | degrees; Vincenty formula, stable at antipodes and small angles |
| `sky_in_cone(ra, dec, ra0, dec0, radius)` | `BOOL` | `distance ≤ radius`, inclusive, matches ADQL `CONTAINS(POINT, CIRCLE)` |
| `sky_in_polygon(ra, dec, ra[], dec[])` | `BOOL` | ADQL 2.1: edges are great circles; polygon is the side that does **not** contain the antipode of the vertex centroid; error if a polygon crosses itself |

### SQL macros

| Macro | Purpose |
|-------|---------|
| `hats_partitions(root, moc)` | `SELECT norder, npix, root \|\| '/dataset/Norder=' \|\| norder \|\| '/Dir=' \|\| (npix // 10000 * 10000) \|\| '/Npix=' \|\| npix \|\| '.parquet' AS path FROM read_csv(root \|\| '/partition_info.csv') WHERE moc_overlaps_cell(moc, "Norder", "Npix")` |
| `adql_circle(ra, dec, r)` / `adql_polygon(ra[], dec[])` | thin aliases documented in `docs/adql-mapping.md` for the compiler |

### Error handling

| Condition | Behaviour |
|-----------|-----------|
| order ∉ [0,29] | `InvalidInputException: healpix: order must be in [0,29], got N` |
| ra/dec out of range with `healpix_strict_input=true` | `InvalidInputException` with the offending value |
| MOC blob with bad version/length | `InvalidInputException: healpix: not a MOC v1 blob` |
| polygon < 3 vertices, or self-intersecting | `InvalidInputException` |
| NULL in any argument | NULL out (DuckDB default null propagation) |
| Rust panic | caught at FFI boundary, surfaced as `InternalException` with message; never aborts the process |

---

## Testing — the proof the agent must produce

Every gate ends with a command that exits 0 and is listed in CI. No gate is "done" on inspection.

```bash
make test            # cargo test (unit + proptest) + sqllogictest on test/sql/
make test-golden     # sqllogistest files that load test/golden/*.parquet and assert equality
make test-wasm       # builds wasm_mvp, runs test/wasm/parity.mjs under Node, diffs vs native
make oracle          # regenerates test/golden/ from Python oracles (uv run tools/oracle/*.py)
make bench           # tools/bench on Gaia DR3 HATS sample (Emmanuel runs; needs S3)
```

| Suite | Oracle | What it proves |
|-------|--------|----------------|
| `pixel.test` | `cdshealpix` Python, `astropy-healpix` | `ang2pix`/`pix2ang` round-trip on 1M random points, all orders 0–29; exact `BIGINT` equality |
| `edge_cases.test` | same + hand-computed | Dec = ±90 exactly, RA = 0, 360, 359.999…, cone centred on a pole, cone straddling RA = 0, radius 0, radius 180 |
| `moc.test` | `mocpy` | cone/polygon MOCs byte-equal after normalisation; union/intersect/diff; JSON/ASCII/FITS round-trip against MOCPy output |
| `sky.test` | PostgreSQL + pgSphere + Q3C (fixtures in `test/golden/pgsphere_*.csv`) | identical row sets for 200 cones and 50 polygons over a 100k-row Gaia DR3 sample; polygons include RA-wrapping, pole-containing, concave, and > hemisphere |
| `hats.test` | `lsdb` | `hats_partitions()` returns exactly the partitions `lsdb` reads for the same cone; pruned scan row set equals full scan row set |
| Rust `proptest` | invariants | `moc_from_ranges` output always sorted/disjoint; `moc_union(a,b) ⊇ a`; `moc_contains(moc, hpx29(pix2ang(p))) = true` for every `p` in `moc` |
| `test/wasm/parity.mjs` | native build | every `.test` file's query output identical in DuckDB-WASM |

Regeneration rule: `test/golden/` is committed. `make oracle` must be reproducible (`uv.lock` pinned). Any diff after regeneration is a test failure, not a fixture update, unless the ADR explains why.

---

## Non-Goals (v1)

- RING indexing scheme. Convert at ingest.
- Frames other than ICRS; no Galactic/ecliptic conversion.
- Epoch/proper-motion propagation.
- FITS/gWCS pixel↔sky transforms. Footprints are computed at ingest with astropy and stored as polygons/MOCs.
- Cross-match join operator. Build it in the compiler with `hpx_ang2pix` bucketing once v1 ships.
- T-MOC / ST-MOC (time coverage).
- ADQL `BOX` (deprecated in ADQL 2.1) and `REGION` STC-S strings.
- Any file I/O inside the extension.

---

## Downstream Contracts

These tools consume this extension. Changing a signature below is a breaking change (semver major).

| Consumer | Uses | Promise |
|----------|------|---------|
| `adqlduck` | `moc_from_cone/polygon`, `moc_contains`, `sky_in_cone`, `sky_in_polygon`, `sky_distance` | ADQL `CONTAINS`, `INTERSECTS`, `DISTANCE` compile 1:1; mapping table lives in `docs/adql-mapping.md` and is tested by `adqlduck` |
| `sidetap`, `tapdrop` | `hats_partitions`, two-pass pattern | WASM build on the community endpoint; load time < 1 s |
| `obstap-local` | `moc_from_polygon`, `moc_overlaps_cell`, `moc_to_ascii` | `s_region` polygons → MOC at ingest; ObsCore spatial queries via `moc_overlaps` |
| `lookout` | MOC I/O, `moc_intersect`, `moc_area_deg2` | compare archive coverage MOCs |
| `catdiff` | `hpx29`, `hpx_parent` | bucketed release diffs |

---

## Open Questions

| # | Question | Decide by |
|---|----------|-----------|
| 1 | Extension name: `healpix` (clear, but DuckDB Foundation may reserve it) vs `hpx` vs `skypix` | Gate 0 |
| 2 | Does `extension-template-rs` build WASM targets in community CI? If not: C++ template + `cxx`, or native-only + separate wasm-bindgen build? | Gate 0 |
| 3 | DuckDB version pin and bump policy (follow LTS line?) | Gate 0 |
| 4 | Is pyacid's `XMATCH` already a DuckDB extension? If yes, scope v1 to MOC + pruning only and collaborate | Gate 0 |
| 5 | Polygon hemisphere rule: ADQL 2.1 text vs pgSphere behaviour differ in degenerate cases. Pick ADQL; document divergence | Gate 2 |

---

## Agent Build Instructions

> Implement end-to-end using only this README and `AGENTS.md`. Resolve Open Questions 1–4 in Gate 0 and record each in `docs/adr/`. Stop at the end of each gate and report the proof command output.

### Gates

| Gate | Deliverable | Done when (proof) |
|------|-------------|-------------------|
| 0 | Template cloned; `cdshealpix` dep; only `hpx_ang2pix`; CI with native + attempted WASM targets; ADRs for OQ 1–4 | `make test` passes natively; CI run shows WASM job result (pass **or** documented failure with chosen fallback) |
| 1 | All pixel functions; `tools/oracle/pixel.py`; `pixel.test`, `edge_cases.test` | `make test-golden` passes; proptest round-trip 1M points |
| 2 | `MOC` type, constructors, set ops, I/O; `tools/oracle/moc.py` | `moc.test` passes byte-equal vs MOCPy |
| 3 | `sky_*` predicates; pgSphere fixtures (SQL + compose in `tools/pgsphere/`, fixtures committed by Emmanuel) | `sky.test` passes: zero row-set diffs on 250 regions |
| 4 | `hats_partitions` macro; `tools/oracle/hats.py` with `lsdb` | `hats.test` passes: pruned = full scan; partition list = lsdb |
| 5 | `test/wasm/parity.mjs`; `description.yml` with `description: HEALPix and MOC for astronomy on the sphere`; `docs/functions.md` generated; `docs/adql-mapping.md` | `make test-wasm` passes; `description.yml` validates against community-extensions schema |

### Division of labour

Agent runs: `cargo`, `make test*`, `uv run tools/oracle/*.py` (deps: `cdshealpix`, `mocpy`, `astropy-healpix`, `lsdb`, `pyarrow` — all pip-installable, no GPU, no large downloads).

Emmanuel runs, with exact commands supplied by the agent in each gate report:

```bash
# pgSphere/Q3C fixtures (Gate 3)
docker compose -f tools/pgsphere/compose.yml up -d && uv run tools/pgsphere/make_fixtures.py

# Gaia DR3 HATS benchmark (Gate 4), needs S3 read
uv run tools/bench/cone_bench.py --catalog s3://stpubdata/... --radius 0.5

# browser check (Gate 5)
npx serve test/wasm/page && open http://localhost:3000

# push + CI
git push origin main
```

### Constraints

- Rust 2021, `#![deny(unsafe_op_in_unsafe_fn)]`, `clippy -D warnings`.
- No `unwrap()` outside tests. Every FFI entry wrapped in `catch_unwind`.
- `f64` everywhere; no `f32`.
- Vectorised execution: process DuckDB data chunks, never per-row allocation in hot loops.
- No network, no file I/O, no `std::env` reads in the extension.
- SQL macros live in `src/macros.sql` and are registered verbatim at load; test them like functions.
- `AIDEV-NOTE:` / `AIDEV-INVARIANT:` comments at every ADQL-semantics decision point.

### Acceptance Criteria

- [ ] `make test`, `make test-golden`, `make test-wasm` pass in CI on Linux, macOS, Windows, and WASM.
- [ ] Every function has `description`, `example` in `function_descriptions.csv`; `docs/functions.md` regenerated from it.
- [ ] Zero diffs vs pgSphere/Q3C on the committed 250-region fixture.
- [ ] `hats_partitions` matches `lsdb` on 100 random cones over Gaia DR3.
- [ ] WASM extension < 2 MB gzipped; `LOAD` < 1 s on a laptop.
- [ ] `docs/adql-mapping.md` complete for `CONTAINS`, `INTERSECTS`, `DISTANCE`, `CIRCLE`, `POLYGON`, `POINT`.
- [ ] Open Questions 1–5 each have an ADR.

---

## Next Steps

1. Gate 0: answer the WASM question and the name question. Nothing else matters until these are settled.
2. Gates 1–2: pixels and MOC. This alone is useful to `lookout` and `catdiff`.
3. Gate 3: predicates; Emmanuel regenerates pgSphere fixtures once.
4. Gate 4: HATS pruning; run the Gaia DR3 benchmark and write up numbers vs `lsdb`.
5. Gate 5: submit `description.yml` to `duckdb/community-extensions`; blog post with the browser demo; point `adqlduck` at the mapping doc.
