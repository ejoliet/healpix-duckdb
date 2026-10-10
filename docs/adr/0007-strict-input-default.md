# ADR-0007: `healpix_strict_input` defaults to `true`

- **Status**: Accepted — maintainer decision, 2026-10-09
- **Date**: 2026-10-09
- **Open Question**: none in the README table; raised by ADR-0006
- **Gate**: 0 (decision), 1 (implementation of the `false` path)

## Context

The README Configuration Reference defines:

| Setting | Type | Default | Description |
|---|---|---|---|
| `healpix_strict_input` | `BOOL` | `true` | Error on `ra ∉ [0,360)` or `dec ∉ [-90,90]`; if false, normalise RA and clamp Dec |

Gate 0 implements only the strict path, hard-coded in `check_ra_dec`
(`src/pixel.rs`). ADR-0006 briefly argued that the lenient default should be
reconsidered because a throwing function can be evaluated on dictionary entries
no result row references. Its corrected version shows that hazard is confined
to single-column call shapes, so the default is a semantics question on its
own merits. This ADR records that question and its answer.

What the two settings do, concretely:

```sql
SELECT hpx_ang2pix(12, 400.0, 0.0);
-- strict=true : Invalid Input Error: healpix: ra must be in [0,360), got 400
-- strict=false: computed as ra = 40.0   (400 mod 360)

SELECT hpx_ang2pix(12, 10.0, 95.0);
-- strict=true : Invalid Input Error: healpix: dec must be in [-90,90], got 95
-- strict=false: computed as dec = 90.0  (clamped)
```

The two halves of "lenient" are not equally defensible:

- **RA normalisation is mathematically sound.** RA is an angle on a circle;
  400° *is* 40°, and −10° *is* 350°. Nothing is lost.
- **Dec clamping fabricates data.** A declination of 95° is not a wrap-around;
  it is off the sphere. In practice it is a unit error (radians passed as
  degrees), a swapped column, or a sentinel such as `-999`. Clamping turns it
  into a plausible-looking position at the pole that nothing downstream can
  distinguish from a real one.

The consumer that justifies this project is `adqlduck`, an ADQL compiler for
TAP services (invariant 5: ADQL 2.1 semantics win). ADQL 2.1 does not define
behaviour for out-of-range coordinates, so the spec gives no instruction
either way; the decision rests on what an archive-grade tool should do with
input it cannot interpret.

## Options

| # | Default | Gains | Costs | Verdict |
|---|---------|-------|-------|---------|
| A | `true` — as the README states | Bad data surfaces loudly at the point of entry. Sentinels and unit errors cannot leak into pixel indices, MOCs, or cross-match results. The `false` path remains as a documented, explicit opt-in | Users with wrap-around RA in their data must filter, normalise at ingest, or flip the setting | **Chosen** |
| B | `false` | Nothing ever throws; wrap-around RA works unprompted | Dec 95° silently becomes 90°. A unit bug looks like a valid pole position. Silent wrong answers in an archive tool are more expensive than a loud refusal, and are found later by someone else | Rejected |
| C | No setting: RA always normalised, Dec always strict | Keeps the sound half of lenient and discards the unsound half | Changes the README's public contract, which is a semver-level promise to six downstream repos (invariant 6); worth considering only as a deliberate spec revision | Not chosen; recorded as the best alternative if the setting is ever reopened |

## Decision

**`healpix_strict_input` defaults to `true`.** Maintainer decision,
2026-10-09.

1. The strict path is the default and the only path in Gate 0.
2. The `false` path is implemented in Gate 1 exactly as the README describes
   (normalise RA, clamp Dec), as a feature and explicit opt-in — not as an
   emergency mitigation (see ADR-0006, corrected).
3. The setting is read once per function invocation at the SQL boundary and
   applied inside the shared validation helpers (`check_ra_dec`), so every
   `hpx_*`, `moc_*`, and `sky_*` function inherits the behaviour from one
   place.
4. User-facing documentation for the setting must state plainly that `false`
   concedes Dec clamping, so whoever flips it knows what they are trading.

## Consequences

- Catalogues with sentinel coordinates must be filtered before calling any
  `hpx_*`/`sky_*` function, or loaded with `SET healpix_strict_input = false`
  on a connection that understands the trade. The error message names the
  offending value, so the filter to write is obvious.
- The `AIDEV-INVARIANT` on `check_ra_dec` in `src/pixel.rs` stays accurate:
  RA is half-open `[0, 360)` so that 360 and 0 cannot name the same position
  through different paths.
- Golden fixtures (`test/golden/`, Gate 1 onward) are generated from the
  Python oracles with in-range coordinates only. Strict mode guarantees the
  extension never sees anything the oracles did not, which is what makes
  byte-equality against them a meaningful test.
- Option C stays on record. If a future ADQL revision or a consumer requirement
  calls for always-normalised RA, it is the recommended shape, and it is a
  README change first.
