# ADR-0002: The Rust C-API extension template builds DuckDB-WASM targets

- **Status**: Accepted
- **Date**: 2026-10-09
- **Open Question**: #2 — "Does `extension-template-rs` build WASM targets in community CI? If not: C++ template + `cxx`, or native-only + separate wasm-bindgen build?"
- **Gate**: 0 (blocking; invariant 1)

## Context

Invariant 1 of `AGENTS.md` says Gate 0 decides WASM before any function beyond
`hpx_ang2pix` exists, because the consumers that justify this project
(`sidetap`, `tapdrop`) run DuckDB-WASM in a service worker. A native-only
extension is a different, smaller project. The README recorded the question as
unverified: it was not known whether a Rust extension built on the DuckDB **C
Extension API** could produce `wasm_mvp` / `wasm_eh` / `wasm_threads`
artifacts, or whether WASM was a C++-template-only capability.

Evidence gathered (all from pinned sources, checked 2026-10-09):

1. `duckdb/extension-template-rs@main` ships `src/wasm_lib.rs`, whose entire
   purpose is to re-export `src/lib.rs` as a `staticlib` example target,
   because "to build the Wasm target, a `staticlib` crate-type is required"
   while native builds need `cdylib`. A WASM-only source file in the official
   Rust template is only there if WASM is a supported path.
2. `duckdb/extension-ci-tools@v1.5.6`,
   `makefiles/c_api_extensions/base.Makefile` defines first-class targets:
   ```
   DUCKDB_WASM_PLATFORM=$(filter wasm_mvp wasm_eh wasm_threads,$(DUCKDB_PLATFORM))
   wasm_mvp:     DUCKDB_PLATFORM=wasm_mvp     make configure release move_wasm_extension
   wasm_eh:      DUCKDB_PLATFORM=wasm_eh      make configure release move_wasm_extension
   wasm_threads: DUCKDB_PLATFORM=wasm_threads make configure release move_wasm_extension
   ```
   and links them with `emcc ... -sSIDE_MODULE=2
   -sEXPORTED_FUNCTIONS="_$(EXTENSION_NAME)_init_c_api"`. The exported symbol
   is the **C API** entrypoint, i.e. this path is designed for C-API
   extensions, which is exactly what a Rust extension is.
3. The same repo's `makefiles/c_api_extensions/rust.Makefile` sets
   `TARGET=wasm32-unknown-emscripten` when `DUCKDB_WASM_PLATFORM` is non-empty.
   This is the Rust-specific makefile; it has a WASM branch.
4. `duckdb/extension-ci-tools@v1.5.6`,
   `.github/workflows/_extension_distribution.yml` — the reusable workflow that
   `duckdb/community-extensions` calls — contains a `wasm:` job (line 1041)
   driven by a `wasm_matrix` generated from `config/distribution_matrix.json`,
   which installs the Rust toolchain with `targets: wasm32-unknown-emscripten`
   (line 1133) and runs `make ${{ matrix.duckdb_arch }}` (line 1201), i.e. the
   `wasm_mvp` / `wasm_eh` / `wasm_threads` targets above.

The WASM job is opt-**out** (via `exclude_archs`), not opt-in. Our
`.github/workflows/ci.yml` does not exclude any WASM arch, so all three build.

5. **Six Rust extensions already published to `duckdb/community-extensions`
   ship WASM platforms** (their `description.yml` either omits
   `excluded_platforms` or excludes only non-WASM archs): `cog`,
   `duckfn_statrs`, `duckfn_quantstats`, `duckdb_rphonetic`,
   `html_readability`, `laterite_ags4`. Rust + WASM is not merely supported in
   principle; it is in production. (Most Rust extensions *do* exclude WASM —
   45 of 55 — but that reflects their own dependencies, not a platform limit.
   Notably `geography`, the S2 extension, excludes all three.)

### Decisive evidence: it was built, not inferred

Everything above is read from source. It was then confirmed by actually
building the extension with emsdk `latest` from this repository:

```
$ make wasm_mvp && make wasm_eh && make wasm_threads
wasm_mvp      raw=184531B  gzip=64431B
wasm_eh       raw=184531B  gzip=64430B
wasm_threads  raw=184531B  gzip=64432B
```

All three produce
`build/<platform>/extension/healpix/healpix.duckdb_extension.wasm` with
correct metadata (`FIELD2 (duckdb_platform) = wasm_mvp`, `FIELD5 (abi_type) =
C_STRUCT_UNSTABLE`). The Rust side — including `cdshealpix` and the whole
`duckdb` / `libduckdb-sys` / `arrow` tree — compiles for
`wasm32-unknown-emscripten`, and `emcc` links the side module successfully.

This is stronger than the gate's stated proof ("CI run shows WASM job result"),
which the agent cannot produce because it requires a push. It is not a
substitute for seeing the job green in CI; see Follow-up.

One caveat worth recording: an arbitrary emsdk pin (3.1.57) failed at the
`wasm-opt` step with `Unknown option '--enable-bulk-memory-opt'`, a
toolchain-version skew, not a project problem. emsdk `latest` — which is what
`emscripten-core/setup-emsdk@v13` installs in CI — works.

## Options

| # | Option | WASM support | Cost | Verdict |
|---|--------|--------------|------|---------|
| A | `duckdb/extension-template-rs` (C Extension API, Rust) | `wasm_mvp`, `wasm_eh`, `wasm_threads` via `extension-ci-tools`, emcc side-module linking | None beyond the template; one submodule | **Chosen** |
| B | C++ template + `cxx` bridge to the Rust HEALPix crates | Also supported | Two languages, two build systems, a hand-written FFI bridge to debug under emscripten | Rejected — solves a problem that does not exist |
| C | Native-only extension + a separate `wasm-bindgen` build of the same crates | N/A — would not be a DuckDB extension in the browser | Two distribution channels; `sidetap`/`tapdrop` could not `LOAD healpix` in WASM; two divergent code paths to keep in agreement | Rejected — defeats the purpose of the project |

## Decision

**Option A.** Build on `duckdb/extension-template-rs` with the DuckDB C
Extension API. WASM is a supported, first-class target of the same build:
`make wasm_mvp`, `make wasm_eh`, `make wasm_threads`. No C++ bridge and no
second distribution channel.

`extension-ci-tools` is vendored as a git submodule pinned to tag `v1.5.6`
(commit `3fd6109fc01555673b7c7123eba8d0d143fe8ce4`), matching
`TARGET_DUCKDB_VERSION` in the `Makefile` and `ci_tools_version` in
`.github/workflows/ci.yml`.

Invariant 1 is hereby satisfied: work may proceed past `hpx_ang2pix` to Gate 1.

## Consequences

- The project keeps a single Rust codebase for native and browser targets, so
  `test/wasm/parity.mjs` (Gate 5) compares two builds of the *same* source
  rather than two implementations.
- The `staticlib` example target in `Cargo.toml` (`[[example]] name = "healpix"
  path = "src/wasm_lib.rs"`) must be kept in sync with `src/lib.rs`. It
  contains no logic of its own and must not be edited, per the template's own
  comment. Adding a new module to `src/lib.rs` requires no change here, but
  changing the crate name does.
- `src/lib.rs` declares submodules with `#[path = "pixel.rs"] mod pixel;`
  rather than a bare `mod pixel;`. This is **load-bearing for the WASM build**:
  `src/wasm_lib.rs` includes `lib.rs` as `mod lib;`, which would otherwise make
  Rust resolve submodules under `src/lib/`. Every future module must use the
  same `#[path = ...]` form or the WASM target stops compiling while the native
  target keeps working.
- WASM artifacts are side modules linked by `emcc`, so the no-file-I/O
  invariant (invariant 2) is not merely good practice: emscripten's virtual FS
  is what makes DuckDB-WASM's C-API file system report phantom entries.
- Gate 5's size budget (< 2 MB gzipped) is now a real constraint on dependency
  choice: `cdshealpix` and `moc` must not pull in heavy transitive deps.
- The three WASM platforms are built, but **nothing is proven about runtime
  behaviour** until `make test-wasm` exists in Gate 5. This ADR establishes
  that the artifacts build, not that they are correct.

## Follow-up (requires a push; cannot be done by the agent)

The CI evidence above is read from the pinned workflow definitions. To convert
it into an observed green/red run, push the branch and read the "DuckDB-Wasm"
job in the Actions tab:

```bash
git push origin main
gh run watch
gh run view --log --job "DuckDB-Wasm"
```
