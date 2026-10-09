<!-- AGENTS.md — agent guidance for duckdb-healpix
     Last reviewed: 2026-10-08  Owner: Emmanuel (ejoliet)
     AIDEV-NOTE: keep ≤ 200 lines; split overflow into docs/agent-context/ -->

# duckdb-healpix

DuckDB extension (Rust, C Extension API) for HEALPix, MOC, and spherical ADQL predicates. Spec is `README.md`; this file is what the spec does not say.

---

## 🛑 Project Invariants (DO NOT VIOLATE)

<!-- AIDEV-INVARIANT: rule + reason + blast radius. -->

1. **Gate 0 decides WASM before any function beyond `hpx_ang2pix` exists** — the whole point is DuckDB-WASM in a service worker; a native-only extension is a different, smaller project.
   Blast: sidetap and tapdrop have no engine; weeks of work on the wrong build system.
2. **No file, network, or env access inside the extension** — DuckDB-WASM's C-API file system returns phantom entries; HATS pruning is a SQL macro over `read_csv()` for this reason.
   Blast: functions that pass natively and silently return wrong rows in the browser.
3. **NESTED scheme, level-29 `BIGINT`, half-open intervals `[lo, hi)`, ICRS degrees — everywhere** — one representation keeps `moc_contains` a binary search and matches HATS `_healpix_29` and healpix-alchemy.
   Blast: off-by-one at cell boundaries; pruning drops rows at partition edges; nobody notices until a cross-match is short.
4. **Golden fixtures in `test/golden/` are regenerated only by `make oracle`, never edited by hand, and a post-regeneration diff is a bug unless an ADR says otherwise** — the fixtures are the correctness proof against independent implementations.
   Blast: the extension "passes" while disagreeing with pgSphere/MOCPy; downstream TAP results diverge from every other archive.
5. **ADQL 2.1 semantics win over pgSphere/S2 when they differ** (inclusive `CONTAINS`, great-circle edges, hemisphere rule) — the consumer is an ADQL compiler, not a GIS.
   Blast: `adqlduck` emits queries whose results differ from a real TAP service; `taplint` fails.
6. **Public function signatures in README "API / Interface Contract" are a semver contract** — `adqlduck`, `sidetap`, `tapdrop`, `obstap-local`, `lookout`, `catdiff` depend on them.
   Blast: six repos break on `INSTALL healpix FROM community`.
7. **Every FFI entry point is wrapped in `catch_unwind`; no `unwrap()` outside tests** — a Rust panic across the C boundary aborts the DuckDB process, in the browser that kills the tab.
   Blast: user loses their session; no error message to debug.

> If an instruction below conflicts with an invariant, the invariant wins.

---

## 🔥 Recently Burned

<!-- AIDEV-BURN: last 3–5 mistakes. Date each. Prune quarterly. -->

- **2026-10-08** (pre-repo, design review): proposed "add projection to DuckDB spatial". Tangent planes are wrong at the poles and across RA = 0 at any scale beyond a field. → Spherical predicates + HEALPix intervals, never planar projection.
- **2026-10-08**: assumed S2 (`geography` extension) could replace HEALPix. S2 cells ≠ HEALPix cells; no HATS pruning, no MOC. → S2 is not a dependency of this project.
- **2026-10-08**: assumed a Rust extension gets WASM builds from community CI. Unverified. → Gate 0 proves or picks the fallback; see ADR-0002.

---

## ✅ Workflow Expectations

- Work gate by gate per README. End each gate with the proof command's output pasted in the report. Do not start the next gate without it.
- Resolve each Open Question with a file in `docs/adr/NNNN-slug.md` (ADR template: Context / Options table / Decision / Consequences). No decisions in chat only.
- New or changed function → update `function_descriptions.csv`, add a `test/sql/*.test` case, regenerate `docs/functions.md`, and if ADQL-relevant update `docs/adql-mapping.md`.
- Give Emmanuel exact commands for anything he runs (Docker pgSphere, S3 benchmark, browser check, `git push`). Do not spend tokens trying to do those yourself.
- Python helpers under `tools/` run with `uv run`; keep `uv.lock` committed; no `pip install` instructions.
- Run `make test` before declaring any step done. `make test-wasm` before declaring Gate 5 done.

---

## 🧭 Conventions

- `AIDEV-INVARIANT:` at every place ADQL semantics are encoded (inclusive radius, winding rule, RA normalisation). `AIDEV-NOTE:` for non-obvious performance choices (chunk-level buffers, binary search). Preserve on refactor.
- `MOC` blob layout is versioned by its first byte. Bumping it is a breaking change; add `moc_upgrade()` rather than silently reinterpreting.
- Angles are degrees at the SQL boundary; radians internally. Convert once at the edge, never mid-function.
- Errors use `InvalidInputException` for user mistakes and `InternalException` for caught panics. Messages start with `healpix:`.
- SQL macros are the only place `read_csv`, string concatenation of paths, or HATS directory layout appear.

---

## 📦 Stack

- Lang: Rust 2021 · Template: `duckdb/extension-template-rs` · Deps: `cdshealpix`, `moc` · Test: `cargo test` + sqllogictest + `proptest`
- Oracles: `cdshealpix` (py), `astropy-healpix`, `mocpy`, `lsdb`, PostgreSQL+pgSphere+Q3C (Docker, Emmanuel-run)
- Targets: linux/macos/windows native + `wasm_mvp`/`wasm_eh`/`wasm_threads` · Distribution: `duckdb/community-extensions`

---

## 🚫 Out of Scope for the Agent

- Don't add `geography`/S2, GEOS, or `spatial` as dependencies.
- Don't implement RING, non-ICRS frames, proper motion, WCS, cross-match joins, T-MOC. README Non-Goals.
- Don't edit `test/golden/` by hand. Don't commit regenerated fixtures without an ADR explaining the diff.
- Don't change DuckDB version pin without an ADR; community CI builds against specific versions.
- Don't push, open PRs, or submit `description.yml` upstream. Prepare the files; Emmanuel submits.
- Don't rename functions after Gate 1 without a semver-major note in `CHANGELOG.md`.

---

## 📚 Deeper Context

- Spec and gates: `README.md`
- Decisions: `docs/adr/`
- ADQL mapping consumed by `adqlduck`: `docs/adql-mapping.md`
- Data-model precedent: healpix-alchemy (Singer et al. 2022, AJ 163 209) — interval sets at level 29 in Postgres `INT8RANGE`; we do the same in a `BLOB`.
