# ADR-0005: The C Extension API cannot choose an exception class; caught panics surface as Invalid Input Error

- **Status**: Accepted
- **Date**: 2026-10-09
- **Open Question**: none — this ADR records an unplanned conflict between the
  README error-handling contract and what the DuckDB C Extension API permits.
- **Gate**: 0

## Context

The README "Error handling" table specifies two different exception classes:

| Condition | Specified behaviour |
|-----------|---------------------|
| order ∉ [0,29], bad ra/dec, bad MOC blob, bad polygon | `InvalidInputException` |
| Rust panic | caught at FFI boundary, surfaced as **`InternalException`**, never aborts the process |

AGENTS.md repeats it: "Errors use `InvalidInputException` for user mistakes and
`InternalException` for caught panics."

Implementing `hpx_ang2pix` showed this is not achievable. In
`duckdb` crate 1.10506.0 a scalar function has exactly one error channel:

```rust
// duckdb-1.10506.0/src/vscalar/mod.rs:177
impl CallbackErrorSink for ScalarFunctionInfo {
    fn set_c_error(&self, error: &CStr) {
        unsafe { duckdb_scalar_function_set_error(self.0, error.as_ptr()) };
    }
}
```

`duckdb_scalar_function_set_error` takes a message and nothing else. There is
no exception-class parameter in the C Extension API, and DuckDB raises the
result as **Invalid Input Error**. Observed:

```
InvalidInputException: Invalid Input Error: healpix: order must be in [0,29], got 30
InvalidInputException: Invalid Input Error: healpix: ra must be in [0,360), got 400
InvalidInputException: Invalid Input Error: healpix: dec must be in [-90,90], got 91
```

A returned `Err` and a caught panic therefore arrive at the user as the same
exception class. Only the message can distinguish them.

Separately, and usefully: **`duckdb-rs` already contains panics itself.** Every
scalar callback is routed through `contain_callback`:

```rust
// duckdb-1.10506.0/src/callback.rs:28-43
pub(crate) fn contain_callback(sink: &impl CallbackErrorSink, callback: ...) {
    if let Err(error) = catch_boxed_callback(callback) { sink.report_error(&error); }
}
fn catch_boxed_callback(callback: ...) -> Result<(), String> {
    match catch_unwind(AssertUnwindSafe(callback)) {
        ...
        Err(payload) => Err(format!("Rust callback panicked: {}", take_panic_payload(payload))),
    }
}
```

Its doc comment states: "Callback panics and panics from error formatting or
destruction are contained here and never unwind to the caller."

So the *safety* half of invariant 7 (a panic must never cross the C boundary
and abort the process) is already guaranteed by the crate, independently of
anything this project writes.

## Options

| # | Option | Consequence | Verdict |
|---|--------|-------------|---------|
| A | Keep both the crate's containment and our own inner `catch_unwind`; accept one exception class; distinguish by message prefix | Invariant 7 holds visibly in our own source and does not silently depend on crate internals; the message carries the distinction the class cannot | **Chosen** |
| B | Drop our `catch_unwind` and rely solely on `duckdb-rs` | Invariant 7 becomes invisible in this repo and silently hostage to a transitive dependency's internals. A crate upgrade that changed `contain_callback` would remove the guard with no local signal. In WASM the failure mode is a dead browser tab | Rejected — the cost of keeping it is a `match` |
| C | Patch/fork DuckDB or the C API to expose an exception class | Enormous scope, upstream-owned, for a cosmetic distinction | Rejected |
| D | Amend the README to drop `InternalException` | Correct eventually, but the README is the spec and is Emmanuel's to change | Deferred to Emmanuel — see Consequences |

## Decision

**Option A.**

1. Every FFI entry point keeps its own explicit `catch_unwind`, per invariant 7.
   It is a second, inner layer. Its value is that invariant 7 is enforced and
   auditable in this repository rather than delegated to a dependency.
2. Internal errors are distinguished by **message**, not by class. A caught
   panic produces `healpix: internal error (panic caught in <function>)`; a
   user mistake produces `healpix: <what the user got wrong>`. Both arrive as
   `InvalidInputException`.
3. All messages keep the `healpix:` prefix, so the message contract in AGENTS.md
   is satisfied in full.

## Consequences

- The README "Error handling" table's `InternalException` row is **not
  implementable** and should be corrected to say: caught panics are surfaced as
  `InvalidInputException` with an `healpix: internal error (...)` message, and
  never abort the process. Flagged for Emmanuel; not edited here, because the
  README is the spec.
- Tests must assert on **message text**, not on exception class, when
  distinguishing user error from internal error. `test/sql/pixel.test` already
  does this (`statement error` matches the message).
- `adqlduck` and other consumers cannot branch on exception class to tell a bad
  user query from an extension bug. If any consumer needs that, the stable
  discriminator is the `healpix: internal error` substring. Worth stating in
  `docs/adql-mapping.md` at Gate 5.
- Re-check on every `duckdb` crate bump: if the C API ever gains an
  exception-class parameter, revisit point 2. The containment in point 1 stays
  regardless.
