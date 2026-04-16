//! Per-client Stratum TCP read loop: buffering, protocol checks, JSON-RPC dispatch.
//!
//! The full loop lives in [`read_loop`] to keep this module root as a thin entry point.

mod read_loop;

pub(crate) use read_loop::spawn_client_listener;
