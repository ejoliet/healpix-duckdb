---
name: duckdb-ext-test
description: How to build and test the duckdb-healpix extension, and which test command proves which gate. Use this before declaring any step, function, or gate done, after editing anything under src/ or test/, when a test fails, or whenever you are about to invent a cargo/duckdb/sqllogictest command. Never run tests any other way in this repo.
---

# duckdb-ext-test

Three commands. Run them in this order. Paste the tail of the output in your report.

| Command | Run when | Proves |
|---|---|---|
| `make test` | after any edit to `src/` or `test/sql/` | builds; `cargo test` + proptest + sqllogictest on `test/sql/` pass natively |
| `make test-golden` | after `make test` passes; always before ending Gates 1–4 | results equal committed oracle fixtures in `test/golden/` |
| `make test-wasm` | before ending Gate 0 (attempt) and Gate 5; after any FFI or registration change | DuckDB-WASM output is identical to native |

## Rules

- A gate is done only when its proof command exits 0. Name the command and paste the last ~20 lines.
- Run a single file while iterating: `make test TEST_FILE=test/sql/moc.test`. Run the full suite before reporting.
- A `test-golden` failure is a code bug. Do not edit `test/golden/`. Do not run `make oracle` to "fix" a failure.
- `make oracle` is only for adding new fixtures. If it changes existing files, stop and write an ADR in `docs/adr/` explaining the diff.
- New function → add a case in the matching `test/sql/*.test` before implementing it.
- If `make test-wasm` fails on the toolchain (not on results), record it in the Gate 0 ADR and tell Emmanuel. Do not work around it silently.

## Emmanuel runs these, not you

Give him the exact command and stop:

```bash
docker compose -f tools/pgsphere/compose.yml up -d && uv run tools/pgsphere/make_fixtures.py   # Gate 3 fixtures
uv run tools/bench/cone_bench.py --radius 0.5                                                   # Gate 4 benchmark, needs S3
npx serve test/wasm/page                                                                        # Gate 5 browser check
```

## When a test fails

1. Re-run only the failing file with `TEST_FILE=`.
2. Edge case (pole, RA 0/360, radius 0/180)? Check `AIDEV-INVARIANT` comments in `src/sky.rs` and `src/pixel.rs` first.
3. Native passes, WASM fails? Suspect file I/O, threads, or a panic across FFI (AGENTS.md invariants 2 and 7).
