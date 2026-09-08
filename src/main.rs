//! The indexer binary.
//!
//! Phase 1 is the process skeleton: resolve configuration, install logging, reach
//! PostgreSQL, then wait for a shutdown signal. There is no pipeline yet.
//!
//! Phase 2 adds the RPC client: it is constructed, its capabilities are probed,
//! and it is held ready for the Phase 5 pipeline. The process idles until a
//! shutdown signal arrives.

use std::time::Duration;

use anyhow::Context;
use chainlens::rpc::EthClient;
use chainlens::rpc::http::{HttpRpcClient, HttpRpcConfig};
use chainlens::rpc::ratelimit::RateLimit;
use chainlens::rpc::retry::RetryPolicy;
use chainlens::{config, db, shutdown, telemetry};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env before parsing, so its values are visible to clap's environment
    // fallback. A missing file is normal — in a container the values arrive from
    // the environment directly — but a malformed one is a configuration fault and
    // is treated like any other.
    if let Err(error) = dotenvy::dotenv() {
        if !error.not_found() {
            return Err(error).context("could not read .env");
        }
    }

    // Both of these run before the first log line exists, so their failures go to
    // stderr through main's return value. That is the correct destination: a
    // process that cannot read its own configuration has nowhere to log to.
    let config = config::Config::load().context("invalid configuration")?;
    telemetry::init(config.log_format).context("could not install the log subscriber")?;

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        rpc_url = %config.rpc_url,
        database_url = %config.database_url,
        db_max_connections = config.db_max_connections,
        db_connect_timeout_secs = config.db_connect_timeout_secs,
        rpc_rate_limit = config.rpc_rate_limit,
        rpc_timeout_secs = config.rpc_timeout_secs,
        rpc_max_retries = config.rpc_max_retries,
        log_format = ?config.log_format,
        "chainlens starting"
    );

    // Registered before anything that can block, and selected against below, so
    // Ctrl-C during a hanging connect returns immediately instead of waiting out
    // the connect timeout.
    let shutdown = shutdown::on_signal();

    let pool = tokio::select! {
        result = db::connect(&config) => result?,
        () = shutdown.cancelled() => {
            tracing::info!("shutdown requested before the database was reachable");
            return Ok(());
        }
    };

    let target = config.database_url.to_string();
    let server = db::server_version(&pool, &target).await?;
    tracing::info!(postgres = %server, "database ready");

    // Phase 2: construct the RPC client and probe its capabilities.
    let rpc_config = HttpRpcConfig {
        url: config.rpc_url.expose().to_string(),
        rate_limit: RateLimit::new(config.rpc_rate_limit),
        retry: RetryPolicy {
            max_retries: config.rpc_max_retries,
            base_delay: Duration::from_millis(200),
            max_delay: Duration::from_secs(8),
        },
        timeout: Duration::from_secs(config.rpc_timeout_secs),
    };

    let rpc_client = tokio::select! {
        result = HttpRpcClient::new(rpc_config) => result?,
        () = shutdown.cancelled() => {
            tracing::info!("shutdown requested during RPC client setup");
            pool.close().await;
            return Ok(());
        }
    };

    tracing::info!(
        supports_block_receipts = rpc_client.supports_block_receipts(),
        "RPC client ready"
    );

    tracing::info!("no pipeline yet; phase 2 idles until a shutdown signal arrives");
    shutdown.cancelled().await;

    // `close` waits for checked-out connections to be returned rather than
    // dropping them. From Phase 4 that is what gives the committer the chance to
    // finish the transaction it is inside instead of having it rolled back.
    tracing::info!("closing the connection pool");
    pool.close().await;
    tracing::info!("shutdown complete");

    Ok(())
}
