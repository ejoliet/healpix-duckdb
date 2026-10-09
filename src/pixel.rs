//! HEALPix pixel scalar functions.
//!
//! AIDEV-INVARIANT: NESTED scheme only. RING is a non-goal (README Non-Goals);
//! convert at ingest.
//! AIDEV-INVARIANT: angles are ICRS degrees at the SQL boundary and radians
//! internally. The conversion happens exactly once, here at the edge, and never
//! mid-function.

use std::error::Error;
use std::panic::{catch_unwind, AssertUnwindSafe};

use duckdb::{
    core::{DataChunkHandle, LogicalTypeId},
    vscalar::{ScalarFunctionSignature, VScalar},
    vtab::arrow::WritableVector,
    Result,
};

/// AIDEV-INVARIANT: 29 is the maximum order everywhere in this extension. It is
/// the level of the `_healpix_29` HATS sort key and of every MOC interval
/// endpoint, and it is the deepest order whose cell index fits a signed i64.
pub const MAX_ORDER: i32 = 29;

pub fn check_order(order: i32) -> Result<u8, Box<dyn Error>> {
    if !(0..=MAX_ORDER).contains(&order) {
        return Err(format!("healpix: order must be in [0,29], got {order}").into());
    }
    Ok(order as u8)
}

/// AIDEV-INVARIANT: `healpix_strict_input` defaults to true, so coordinates
/// outside the ICRS domain are a user error rather than silently normalised.
/// RA is half-open `[0, 360)` so that 360 and 0 cannot both name the same sky
/// position through different code paths.
pub fn check_ra_dec(ra: f64, dec: f64) -> Result<(), Box<dyn Error>> {
    if !ra.is_finite() || !(0.0..360.0).contains(&ra) {
        return Err(format!("healpix: ra must be in [0,360), got {ra}").into());
    }
    if !dec.is_finite() || !(-90.0..=90.0).contains(&dec) {
        return Err(format!("healpix: dec must be in [-90,90], got {dec}").into());
    }
    Ok(())
}

/// NESTED HEALPix index of (ra, dec) at `order`.
pub fn ang2pix(order: i32, ra: f64, dec: f64) -> Result<i64, Box<dyn Error>> {
    let depth = check_order(order)?;
    check_ra_dec(ra, dec)?;
    Ok(cdshealpix::nested::hash(depth, ra.to_radians(), dec.to_radians()) as i64)
}

pub struct HpxAng2Pix;

impl VScalar for HpxAng2Pix {
    type State = ();

    fn invoke(
        _state: &Self::State,
        input: &mut DataChunkHandle,
        output: &mut dyn WritableVector,
    ) -> Result<(), Box<dyn Error>> {
        // AIDEV-INVARIANT: every FFI entry point is wrapped in catch_unwind. A
        // Rust panic unwinding across the C boundary aborts the whole DuckDB
        // process; in DuckDB-WASM that kills the browser tab with no message.
        //
        // AIDEV-NOTE: this is a second, INNER layer. duckdb-rs already contains
        // panics for us in `contain_callback` (callback.rs). It is kept anyway
        // so the invariant is enforced and auditable in this repository instead
        // of being silently hostage to a transitive dependency's internals.
        //
        // AIDEV-NOTE: the returned message, not the exception class, is what
        // distinguishes an internal error from a user error. The C Extension
        // API has only one error channel (duckdb_scalar_function_set_error) and
        // DuckDB raises everything from it as Invalid Input Error, so the
        // README's `InternalException` is unreachable. See
        // docs/adr/0005-error-class-and-panic-containment.md.
        match catch_unwind(AssertUnwindSafe(|| ang2pix_chunk(input, output))) {
            Ok(res) => res,
            Err(_) => Err("healpix: internal error (panic caught in hpx_ang2pix)".into()),
        }
    }

    fn signatures() -> Vec<ScalarFunctionSignature> {
        vec![ScalarFunctionSignature::exact(
            vec![
                LogicalTypeId::Integer.into(),
                LogicalTypeId::Double.into(),
                LogicalTypeId::Double.into(),
            ],
            LogicalTypeId::Bigint.into(),
        )]
    }
}

// AIDEV-NOTE: vectorised over the whole data chunk, with the null mask applied
// in a second pass so the value slice can be borrowed mutably without
// allocating a scratch buffer per chunk.
fn ang2pix_chunk(
    input: &mut DataChunkHandle,
    output: &mut dyn WritableVector,
) -> Result<(), Box<dyn Error>> {
    let len = input.len();
    let order_vec = input.flat_vector(0);
    let ra_vec = input.flat_vector(1);
    let dec_vec = input.flat_vector(2);

    // SAFETY: the payload really is `len` contiguous values, and not a
    // single-element CONSTANT payload or a dictionary index array, because
    // DuckDB flattens the whole input chunk before it calls any C-API scalar
    // function: `CAPIScalarFunction` runs `input.Flatten()` immediately before
    // invoking the callback (duckdb/src/main/capi/scalar_function-c.cpp:202,
    // v1.5.6). That upstream flatten is the guarantee.
    //
    // AIDEV-NOTE: `flat_vector(i)` is NOT what provides it. That method is only
    // `FlatVector::from_raw(duckdb_data_chunk_get_vector(..))` — it stores the
    // pointer and a capacity and inspects nothing, so its name asserts the
    // representation rather than establishing it. Do not rewrite this comment
    // to claim the Rust side normalises anything.
    //
    // This matters because a literal argument would otherwise arrive as a
    // CONSTANT vector — `hpx_ang2pix(12, ra, dec)` is the common case — and
    // reading `len` elements from a one-element allocation would run off the
    // end. The "Vector representation" case in test/sql/pixel.test exercises
    // literal and column arguments over 3000-row chunks.
    //
    // The element types match the declared signature: INTEGER -> i32,
    // DOUBLE -> f64.
    let orders = unsafe { order_vec.as_slice_with_len::<i32>(len) };
    let ras = unsafe { ra_vec.as_slice_with_len::<f64>(len) };
    let decs = unsafe { dec_vec.as_slice_with_len::<f64>(len) };

    let is_null = |i: usize| {
        order_vec.row_is_null(i as u64)
            || ra_vec.row_is_null(i as u64)
            || dec_vec.row_is_null(i as u64)
    };

    let mut out = output.flat_vector();
    {
        // SAFETY: the signature declares BIGINT, so the output storage is i64.
        // `as_mut_slice_with_len(len)` bounds the slice to the chunk's row
        // count; the plain `as_mut_slice()` would hand back `capacity()`
        // elements (STANDARD_VECTOR_SIZE, 2048) which is wider than the rows
        // that actually exist. Nothing else aliases this storage while `out` is
        // mutably borrowed.
        let out_slice = unsafe { out.as_mut_slice_with_len::<i64>(len) };
        // AIDEV-NOTE: the `?` below can return mid-chunk, leaving earlier rows
        // written and the null mask not yet applied. That is safe because
        // DuckDB discards the output chunk of a scalar function that reports an
        // error — the partially written values are never observable by a query.
        // Every row is written before it could be read, so no element is left
        // uninitialised either way. Covered by the "error raised mid-chunk"
        // case in test/sql/pixel.test.
        for i in 0..len {
            if is_null(i) {
                out_slice[i] = 0;
                continue;
            }
            out_slice[i] = ang2pix(orders[i], ras[i], decs[i])?;
        }
    }
    for i in 0..len {
        if is_null(i) {
            out.set_null(i);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_bounds_are_enforced() {
        assert!(check_order(-1).is_err());
        assert!(check_order(30).is_err());
        assert_eq!(check_order(0).unwrap(), 0);
        assert_eq!(check_order(29).unwrap(), 29);
    }

    #[test]
    fn order_error_message_matches_contract() {
        let msg = check_order(30).unwrap_err().to_string();
        assert_eq!(msg, "healpix: order must be in [0,29], got 30");
    }

    #[test]
    fn ra_dec_domain_is_half_open_in_ra() {
        assert!(check_ra_dec(0.0, 0.0).is_ok());
        assert!(check_ra_dec(359.999, 0.0).is_ok());
        assert!(check_ra_dec(360.0, 0.0).is_err());
        assert!(check_ra_dec(-0.001, 0.0).is_err());
        assert!(check_ra_dec(0.0, 90.0).is_ok());
        assert!(check_ra_dec(0.0, -90.0).is_ok());
        assert!(check_ra_dec(0.0, 90.001).is_err());
        assert!(check_ra_dec(0.0, f64::NAN).is_err());
    }

    // AIDEV-NOTE: these expected values were produced independently by
    // astropy-healpix (`HEALPix(nside=2**order, order='nested')
    // .lonlat_to_healpix`) and agree with cdshealpix on every row. This is the
    // Gate 0 smoke oracle; the full 1M-point round-trip against the Python
    // oracles lands in Gate 1 via `tools/oracle/pixel.py`.
    #[test]
    fn matches_astropy_healpix_oracle() {
        const ORACLE: &[(i32, f64, f64, i64)] = &[
            (0, 45.0, 60.0, 0),
            (0, 45.0, -60.0, 8),
            (0, 45.0, 0.0, 0),
            (0, 0.0, 0.0, 4),
            (0, 180.0, 0.0, 6),
            (0, 0.0, 90.0, 0),
            (8, 266.405, -28.936, 461282),
            (10, 266.405, -28.936, 7380519),
            (12, 266.405, -28.936, 118088310),
            (29, 123.456, -45.678, 2759817459388122447),
            (29, 0.0, 90.0, 288230376151711743),
            (29, 0.0, -90.0, 2305843009213693952),
        ];
        for &(order, ra, dec, expected) in ORACLE {
            assert_eq!(
                ang2pix(order, ra, dec).unwrap(),
                expected,
                "order={order} ra={ra} dec={dec}"
            );
        }
    }

    #[test]
    fn parent_prefix_holds_across_orders() {
        // A NESTED index at order k is the order k-1 index times 4 plus the
        // child offset, so dividing by 4 must walk up the tree.
        let deep = ang2pix(10, 266.405, -28.936).unwrap();
        let shallow = ang2pix(8, 266.405, -28.936).unwrap();
        assert_eq!(deep >> 4, shallow);
    }

    #[test]
    fn poles_are_representable_at_max_order() {
        assert!(ang2pix(29, 0.0, 90.0).is_ok());
        assert!(ang2pix(29, 0.0, -90.0).is_ok());
    }

    #[test]
    fn level_29_index_fits_in_i64() {
        let npix: i64 = 12 * (1i64 << (2 * 29));
        let v = ang2pix(29, 123.456, -45.678).unwrap();
        assert!(v >= 0 && v < npix);
    }
}
