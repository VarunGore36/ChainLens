//! ChainLens: an Ethereum indexer with reorg-safe, crash-resumable ingestion.
//!
//! A library with thin binaries on top. The library target is what lets
//! integration tests under `tests/` exercise real code, and what lets the Phase
//! 10 query API binary share configuration, telemetry, and the store rather than
//! duplicating them.
//!
//! # Module layout
//!
//! A directory appears when the first file goes into it. There is deliberately no
//! empty `rpc/`, `domain/`, or `store/` yet: an empty module directory advertises
//! structure that has not been earned, and the phase that fills it in usually
//! wants a different shape than the placeholder implied.
//!
//! # Errors
//!
//! Errors are per-module enums — [`config::ConfigError`], [`db::DbError`],
//! [`telemetry::TelemetryError`] — rather than one crate-wide `ChainLensError`.
//! A single shared enum forces every caller to match on variants that cannot
//! occur at that call site, and grows a variant every time any module gains a
//! failure mode. Per-module enums keep a function signature an honest statement
//! of what that function can fail at.
//!
//! The binaries flatten all of them into `anyhow::Error`, because by the time an
//! error reaches `main` the only remaining decision is what to print. That is
//! also why there is no `src/error.rs`: with errors living beside the code that
//! produces them, it would have nothing to hold.

pub mod config;
pub mod db;
pub mod rpc;
pub mod shutdown;
pub mod telemetry;
