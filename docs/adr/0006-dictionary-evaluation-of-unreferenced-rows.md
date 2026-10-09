# ADR-0006: Strict validation can reject a query whose result rows are all valid

- **Status**: Accepted (known limitation, mitigation scheduled for Gate 1)
- **Date**: 2026-10-09
- **Open Question**: none — found during Gate 0 review
- **Gate**: 0 (discovered), mitigation in Gate 1

## Context

DuckDB scalar functions declare an error mode. A function that declares
`CANNOT_ERROR` promises never to throw, and in exchange the executor is
entitled to evaluate it on values that no live row references — in particular
`TryExecuteDictionaryExpression` evaluates every entry of a dictionary vector
once, rather than once per referencing row, which is a large win for
low-cardinality columns.

**The C Extension API exposes no error-mode setter.** There is no
`duckdb_scalar_function_set_error_mode` in `duckdb.h`, and `duckdb-rs` 1.10506.0
exposes nothing equivalent (verified: no `can_throw`, `CanThrow`, or
`set_error_mode` symbol anywhere in the crate or `libduckdb-sys`). Every C-API
scalar function therefore carries the default mode while `hpx_ang2pix` does in
fact throw, via `InvalidInputException` raised by the C API wrapper:

```cpp
// duckdb/src/main/capi/scalar_function-c.cpp:201-210 (v1.5.6)
auto all_const = input.AllConstant();
input.Flatten();
...
c_bind_info.info.function(c_function_info, c_input, c_result);
if (!function_info.success) {
    throw InvalidInputException(function_info.error);
}
```

This is not a DuckDB bug. The optimization is correct *given the declaration we
are forced to make*; we break the promise involuntarily.

### Reproduced, not theorised

The hazard was raised in review at MEDIUM confidence with reachability for
`DOUBLE` unconfirmed. It is confirmed:

```sql
WITH dict AS (SELECT * FROM (VALUES (0, 10.0::DOUBLE), (1, 400.0::DOUBLE)) v(k, ra)),
     rows AS (SELECT 0 AS k FROM range(3000))
SELECT count(*) FROM rows JOIN dict USING(k) WHERE hpx_ang2pix(12, ra, 0.0) >= 0;
-- Invalid Input Error: healpix: ra must be in [0,360), got 400
```

Every joined row has `k = 0`, so the join output has exactly one distinct value
(`ra = 10.0`), 3000 rows, and **zero** rows with `ra = 400.0` — confirmed by
running the same join with `COUNT(*) WHERE ra = 400.0`, which returns 0. The
illegal value exists only as an unreferenced dictionary entry. Replacing
`400.0` with a legal `300.0` makes the identical query return 3000, so the join
shape is not the cause.

Parquet `PLAIN_DICTIONARY` columns did **not** reproduce it in the cases tried;
the reproducer needs a dictionary vector surviving into the expression, which
the hash join produces.

## Options

| # | Option | Cost | Verdict |
|---|--------|------|---------|
| A | Accept, pin with a regression test, and make `healpix_strict_input=false` a real escape hatch in Gate 1 | A documented sharp edge; the README already promises the setting that defuses it | **Chosen** |
| B | `set_volatile()` on the function | Suppresses the dictionary optimization, and also constant folding and common-subexpression reuse, on the hottest function in the extension. Pays a permanent vectorised-path cost to fix a corner case that a setting already addresses | Rejected |
| C | Stop erroring on out-of-domain input; return NULL instead | Contradicts the README error table and `healpix_strict_input=true` being the default. Silent wrong answers are worse than a loud refusal (invariant 3's spirit) | Rejected |
| D | Flip `healpix_strict_input` to default `false` | Changes a documented default from Gate 0 on the strength of a corner case; the README is the spec and this is Emmanuel's call, not the agent's | Rejected for now; raised below |

## Decision

**Option A.**

1. The behaviour is pinned by a `statement error` case plus a passing control
   case in `test/sql/pixel.test`, so it is a known, observed property rather
   than a latent surprise. If DuckDB's behaviour changes, that test fails and
   points here.
2. `healpix_strict_input=false` is **promoted from a convenience to the
   documented mitigation** and must be implemented in Gate 1. It was previously
   deferred out of Gate 0 as unnecessary surface; this ADR is the reason that
   deferral ends. With `strict=false` the function normalises RA and clamps Dec
   instead of throwing, which removes the throw entirely and makes the
   `CANNOT_ERROR` declaration honest.
3. No change to the error contract or to the default in Gate 0.

## Consequences

- A user whose catalogue contains any out-of-domain RA/Dec may see a query fail
  even after filtering those rows out, whenever the planner produces a
  dictionary vector for the coordinate column. Low-cardinality coordinate
  columns are unusual in real catalogues, which is why this is a sharp edge and
  not a blocker — but ingest pipelines with sentinel values (`-999`, `9999`) are
  exactly the case that would hit it.
- Gate 1 gains a required deliverable: the `healpix_strict_input=false` path,
  with its own test asserting that the reproducer above *succeeds* when the
  setting is off.
- Every future `hpx_*`, `moc_*`, and `sky_*` function that validates its input
  inherits this behaviour. The validation helpers are shared (`check_order`,
  `check_ra_dec`), so the mitigation applies once, in those helpers.
- If the C API ever gains an error-mode setter, revisit: declaring the function
  as able to throw would fix this at the root and let strict mode stay on.

## Decision needed from Emmanuel

Should `healpix_strict_input` keep defaulting to `true`? Option D (default to
`false`, normalise instead of reject) would make this class of failure
impossible and matches what most astronomy tooling does with RA wrap-around,
but it trades a loud error for silent normalisation and contradicts the current
README. Not changed unilaterally.
