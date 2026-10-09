//! duckdb-healpix — HEALPix, MOC and spherical ADQL predicates for DuckDB.
//!
//! Gate 0 scope: registration plus `hpx_ang2pix` only. Every other function in
//! the README API contract is gated behind the WASM decision in
//! `docs/adr/0002-wasm-build-target.md`.
//!
//! AIDEV-INVARIANT: this extension performs no file, network or environment
//! access. DuckDB-WASM's C-API file system reports phantom entries, so HATS
//! pruning is a SQL macro over `read_csv()` rather than Rust I/O.
#![deny(unsafe_op_in_unsafe_fn)]

#[path = "pixel.rs"]
mod pixel;

use std::error::Error;

use duckdb::{duckdb_entrypoint_c_api, Connection, Result};

#[duckdb_entrypoint_c_api]
pub unsafe fn extension_entrypoint(con: Connection) -> Result<(), Box<dyn Error>> {
    con.register_scalar_function::<pixel::HpxAng2Pix>("hpx_ang2pix")?;
    Ok(())
}
