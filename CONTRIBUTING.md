# Developing duckdb-healpix

How to build, test, and change the extension locally. The spec is `README.md`;
the rules the spec does not state are in `AGENTS.md`. This file is the
mechanics.

## Compatibility

The repository pins one DuckDB version, and everything else follows from it.
The pin lives in three places that must agree (`docs/adr/0003`):

| Where | Field | Current value |
|-------|-------|---------------|
| `Makefile` | `TARGET_DUCKDB_VERSION` | `v1.5.6` |
| `.github/workflows/ci.yml` | `duckdb_version`, `ci_tools_version` | `v1.5.6` |
| `extension-ci-tools/` submodule | git tag | `v1.5.6` |

`Cargo.toml` pins `duckdb = "~1.10506.0"`, whose version number encodes
DuckDB 1.5.6, so the crate cannot drift from the Makefile.

What you need, and how to check it:

| Component | Required | Why | Check |
|-----------|----------|-----|-------|
| Rust toolchain | **≥ 1.85.1** (enforced by `rust-version` in `Cargo.toml`) | MSRV of the `duckdb` 1.10506 crate; `cdshealpix` needs 1.81 | `rustc --version` |
| Python | **≥ 3.10** | `make configure` builds a venv with the `duckdb==1.5.6` wheel, which requires 3.10+ | `python3 --version` |
| DuckDB, to `LOAD` a build interactively | **exactly `v1.5.6`** | the binary is stamped for one version; any other refuses it (see below) | `duckdb --version` |
| DuckDB, to run `make test` | none — the venv supplies 1.5.6 | | |
| `make`, `git` | any recent | build driver, submodules | |
| `uv` | any | runs `tools/` scripts | `uv --version` |
| Emscripten SDK | `latest` | **WASM builds only**; CI installs `latest` | `emcc --version` |
| Node | ≥ 20 | **Gate 5 WASM parity test only**; not needed yet | |

Supported hosts: Linux and macOS for local development (both amd64 and
arm64). Windows is built and tested by CI; a local Windows loop has not been
exercised and is not documented here.

When the DuckDB pin is bumped, the three places above move together with the
`duckdb` crate version, in one commit (`docs/adr/0003`, bump policy). Check
`git log -- Makefile` if this table and the `Makefile` ever disagree; the
`Makefile` wins.

## First build

```bash
git clone --recurse-submodules https://github.com/ejoliet/duckdb-healpix.git
cd duckdb-healpix
# already cloned without submodules?  git submodule update --init

make configure     # once: creates configure/venv (duckdb==1.5.6 + sqllogictest runner),
                   # writes configure/platform.txt and extension_version.txt
make test
```

Expected tail of `make test`:

```
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Running DEBUG tests..
[1/1] test/sql/pixel.test
SUCCESS
```

`make test` is the local gate. It is what `AGENTS.md` requires before any step
is declared done. It runs, in order:

1. `cargo test` — Rust unit tests (and `proptest` from Gate 1)
2. `make debug` — builds `build/debug/healpix.duckdb_extension`
3. sqllogictest over every `test/sql/*.test`

Do not add prerequisites to `test_debug` / `test_release`. The community CI
calls those targets directly, inside a Docker container, on a tree it has
already built. See the `AIDEV-INVARIANT` in the `Makefile`.

## The edit → test loop

```bash
# edit src/*.rs or test/sql/*.test, then:
make test
```

Faster, when you know which layer you touched:

```bash
cargo test                                   # Rust only, ~1 s
make debug && make test_debug                # rebuild + all sqllogictests, no cargo test

# one sqllogictest file (both flags are required):
./configure/venv/bin/python3 -m duckdb_sqllogictest \
    --test-dir test/sql \
    --external-extension build/debug/healpix.duckdb_extension \
    --file-path test/sql/pixel.test
```

Before you call anything done, CI also enforces:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

Run `cargo fmt` (no `--check`) to fix formatting in place.

## Trying queries interactively

Use the venv's Python, which has the matching DuckDB 1.5.6:

```bash
./configure/venv/bin/python3
```

```python
import duckdb
con = duckdb.connect(config={"allow_unsigned_extensions": "true"})
con.execute("LOAD 'build/debug/healpix.duckdb_extension'")
con.sql("SELECT hpx_ang2pix(12, 266.405, -28.936)").show()
```

### Using the DuckDB CLI instead

Only a CLI whose version **exactly** matches `TARGET_DUCKDB_VERSION` in the
`Makefile` can load the build:

```bash
duckdb --version          # must print v1.5.6 (the current pin)
duckdb -unsigned
```

```sql
LOAD 'build/debug/healpix.duckdb_extension';
SELECT hpx_ang2pix(12, 266.405, -28.936);
```

Any other version — newer or older — refuses the file with:

```
Invalid Input Error: Failed to load 'build/debug/healpix.duckdb_extension',
The file was built specifically for DuckDB version 'v1.5.6' and can only be
loaded with that version of DuckDB. (this version of DuckDB is '<yours>')
```

This is expected, not a bug: `duckdb-rs` needs the *unstable* C API, so every
binary is stamped for exactly one DuckDB release (`docs/adr/0003`). If your
installed CLI does not match, use the venv Python above rather than changing
the pin.

## Adding or changing a function

Per `AGENTS.md`, all four are required, every time:

1. Implement it. New modules in `src/lib.rs` **must** be declared as
   `#[path = "name.rs"] mod name;` — a bare `mod name;` compiles natively and
   silently breaks the WASM build (`docs/adr/0002`, Consequences).
2. Add a row to `function_descriptions.csv` (description *and* example — the
   generator refuses rows missing either).
3. Add a case to the right `test/sql/*.test`.
4. Regenerate the reference:
   ```bash
   uv run tools/gen_functions_doc.py      # writes docs/functions.md
   ```
   If the function is ADQL-relevant, also update `docs/adql-mapping.md`.

Then `make test`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`.

Rules that will fail review if missed (`AGENTS.md`, Invariants and Constraints):

- No `unwrap()` outside `#[cfg(test)]`. Every FFI entry point wrapped in
  `catch_unwind` (`src/pixel.rs` `HpxAng2Pix::invoke` is the pattern).
- No `std::fs`, `std::net`, `std::env`, `include_str!` anywhere in `src/`.
- `f64` only. Degrees at the SQL boundary, radians inside, converted once.
- Error messages start with `healpix:`.
- NESTED only; level-29 `BIGINT`; half-open `[lo, hi)`.
- Never edit `test/golden/` by hand (Gate 1+); only `make oracle` writes it.

A cheap sanity check that your test can actually fail: break the math on
purpose (`depth - 1`, flip a sign), run `make test`, confirm it goes red, then
revert. A test that stays green under mutation proves nothing.

## WASM build (optional locally; CI always does it)

```bash
rustup target add wasm32-unknown-emscripten

git clone --depth 1 https://github.com/emscripten-core/emsdk.git ~/emsdk
~/emsdk/emsdk install latest && ~/emsdk/emsdk activate latest
source ~/emsdk/emsdk_env.sh

make wasm_mvp        # also: wasm_eh, wasm_threads
ls -la build/wasm_mvp/extension/healpix/healpix.duckdb_extension.wasm
```

Use emsdk **`latest`**, which is what CI installs. A pinned older release
(3.1.57 was tried) fails at the `wasm-opt` step with
`Unknown option '--enable-bulk-memory-opt'`.

**After any `make wasm_*`, run `make configure` again.** The WASM targets
rewrite `configure/platform.txt` to `wasm_mvp` etc., and the next native
`make test` will stamp the wrong platform until it is reset.

There is no local WASM *runtime* test yet; that is Gate 5's `make test-wasm`.

## Layout you will touch

```
src/lib.rs                  registration only
src/pixel.rs                hpx_* functions; check_order / check_ra_dec live here
src/wasm_lib.rs             do not edit — re-exports lib.rs as a staticlib for emcc
test/sql/*.test             sqllogictest, one file per function family
function_descriptions.csv   source of truth for docs/functions.md
docs/adr/                   one file per decision; read before changing anything they cover
implementation-notes.md     one line per non-obvious choice; DEVIATION: marks departures from the spec
extension-ci-tools/         submodule, pinned to the DuckDB version in the Makefile; never edit
```

## Build artifacts (all gitignored)

```
target/                     cargo
build/debug|release/        native extension + metadata
build/wasm_*/               WASM extension
configure/                  venv, platform.txt, extension_version.txt
```

`make clean` removes `build/` and the cargo target; `make clean_all` also
removes `configure/` (you will need `make configure` again).
