# ADR-0006: Single-column throwing functions can reject a query whose result rows are all valid

- **Status**: Accepted (known limitation; scope corrected 2026-10-09 after a second investigation)
- **Date**: 2026-10-09
- **Open Question**: none — found during Gate 0 review
- **Gate**: 0 (discovered); applies to every throwing function in Gates 1–3

## Context

DuckDB scalar functions declare an error mode. A function declaring
`CANNOT_ERROR` promises never to throw, and in exchange the executor may
evaluate it on values no live row references: `TryExecuteDictionaryExpression`
evaluates each entry of a dictionary vector once instead of once per
referencing row.

**The C Extension API exposes no error-mode setter.** There is no
`duckdb_scalar_function_set_error_mode` in `duckdb.h` and nothing equivalent in
`duckdb-rs` 1.10506.0. Every C-API scalar function carries the default mode
while ours throw via the C API wrapper:

```cpp
// duckdb/src/main/capi/scalar_function-c.cpp:208-210 (v1.5.6)
c_bind_info.info.function(c_function_info, c_input, c_result);
if (!function_info.success) {
    throw InvalidInputException(function_info.error);
}
```

Not a DuckDB bug: the optimization is correct given the declaration we are
forced to make.

### Exact preconditions (from source, not inferred)

`duckdb/src/execution/expression_executor/execute_function.cpp` (v1.5.6):

1. **Exactly one non-constant argument** (lines 24–37). The constructor walks
   the children; a second non-foldable child calls `input_col_idx.SetInvalid()`
   and the call site is permanently ineligible.
2. That argument must arrive as a `DICTIONARY_VECTOR` **with a storage
   dictionary id** (lines 59–67: `DictionaryId(..).empty()` → bail).
3. Dictionary size < 20 000 and chunk fill ratio ≥ 0.5 (lines 50–51, 70–76).
4. Return type is not `STRUCT` (line 15).

Precondition 1 is decisive for this extension: **every coordinate function in
the README API takes both `ra` and `dec`**. `hpx_ang2pix(12, ra, dec)`,
`hpx29(ra, dec)`, `sky_in_cone(ra, dec, …)`, `moc_from_cone(ra, dec, …)` all
have two column arguments and are therefore never eligible, regardless of
storage.

### What reproduces and what does not

Reproduces — one column argument, dictionary vector produced by a hash join:

```sql
WITH dict AS (SELECT * FROM (VALUES (0, 10.0::DOUBLE), (1, 400.0::DOUBLE)) v(k, ra)),
     rows AS (SELECT 0 AS k FROM range(3000))
SELECT count(*) FROM rows JOIN dict USING(k) WHERE hpx_ang2pix(12, ra, 0.0) >= 0;
-- Invalid Input Error: healpix: ra must be in [0,360), got 400
```

Join output: 3000 rows, one distinct `ra = 10.0`, **zero** rows with 400.
Replacing 400 with a legal 300 returns 3000. Pinned in `test/sql/pixel.test`
with a passing control.

Does **not** reproduce — a 20 000-row catalogue with `-999` sentinels in `ra`
and `dec`, filtered out, all of the following returning the correct 19 960:

| Shape | Source | Result |
|---|---|---|
| `hpx_ang2pix(12, ra, dec)` filtered by flag | Parquet | ok |
| `hpx_ang2pix(12, ra, dec)` filtered by `ra >= 0 AND dec >= -90` | Parquet | ok |
| same, function in SELECT list after WHERE | Parquet | ok |
| **single-column** `hpx_ang2pix(12, ra, 0.0)` filtered by flag | Parquet | ok |
| **single-column** `hpx_ang2pix(12, ra, 0.0)` filtered by flag | native DuckDB table | ok |
| two-column, native table | native | ok |

The two-column rows are immune by precondition 1. The single-column rows passed
because neither Parquet nor native storage delivered a dictionary vector for a
`DOUBLE` column in these tests (the Parquet writer chose `PLAIN`); only the
hash join did. That is an observation about these runs, not a guarantee about
DuckDB storage.

### Correction to the first version of this ADR

The first version claimed "a user whose catalogue contains any out-of-domain
RA/Dec may see a query fail even after filtering those rows out" and promoted
`healpix_strict_input=false` to a required mitigation. That overstated it: the
realistic two-column call is structurally ineligible. The hazard is real but
narrow, and it lives in **functions with a single column argument**, which
`hpx_ang2pix` only has when a caller passes a literal for one coordinate.

## Where the hazard actually lives (Gates 1–3)

Functions whose natural call shape has one column argument and which validate
it are the exposed ones:

- `hpx_parent(12, ipix, 8)`, `hpx_children(…)`, `hpx_range29(12, ipix)`,
  `hpx_from_uniq(uniq)` — validate `ipix`/`uniq` range
- `moc_from_json(col)`, `moc_from_ascii(col)`, `moc_from_fits(col)` —
  reject malformed input
- `moc_contains(moc_literal, hpx29_col)` — eligible, but does not throw on the
  column argument, so harmless

A bad value in such a column that reaches the function as a join-produced
dictionary vector can fail the query even if no result row holds it.

## Options

| # | Option | Cost | Verdict |
|---|--------|------|---------|
| A | Accept; pin with the regression test; document the single-column exposure; implement `healpix_strict_input=false` in Gate 1 as the README already promises, but as a feature, not an urgent mitigation | A documented sharp edge with a narrow trigger | **Chosen** |
| B | `set_volatile()` on throwing functions | Also disables constant folding and CSE on the hottest functions, permanently, for a narrow corner case | Rejected |
| C | Return NULL instead of erroring on out-of-domain input | Contradicts the README error table and makes bad data silent | Rejected |
| D | Flip `healpix_strict_input` to default `false` | No longer justified by this ADR; the realistic call is immune. Stands or falls on semantics alone — see ADR-0007 if raised | Not decided here |

## Decision

**Option A.** No change to the error contract or to the `strict` default from
this ADR. The regression test and its control stay. The Gate 1
`healpix_strict_input=false` deliverable is the README's, not an emergency.

## Consequences

- Each new throwing function with a single column argument must be assessed
  against precondition 1 at design time, and a dictionary-vector test added
  where the exposure is real (Gate 1: `hpx_parent`, `hpx_from_uniq`; Gate 2:
  the `moc_from_*` parsers).
- The README's `healpix_strict_input=false` is still a useful escape hatch for
  the narrow case, and should be documented as such — not as the fix for a
  broad failure.
- If the C API ever gains an error-mode setter, revisit: declaring the function
  as able to throw fixes this at the root.
