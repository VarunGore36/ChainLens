use std::time::Duration;

use anyhow::Context;
use chainlens::metrics;
use chainlens::pipeline::{committer, head_watcher, sequencer, worker};
use chainlens::rpc::EthClient;
use chainlens::rpc::http::{HttpRpcClient, HttpRpcConfig};
use chainlens::rpc::ratelimit::RateLimit;
use chainlens::rpc::retry::RetryPolicy;
use chainlens::store::BlockStore;
use chainlens::store::postgres::PostgresStore;
use chainlens::{config, db, shutdown, telemetry};
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if let Err(error) = dotenvy::dotenv() {
        if !error.not_found() {
            return Err(error).context("could not read .env");
        }
    }

    let config = config::Config::load().context("invalid configuration")?;
    telemetry::init(config.log_format).context("could not install the log subscriber")?;

    let builder = metrics_exporter_prometheus::PrometheusBuilder::new();
    builder
        .install()
        .context("failed to install Prometheus metrics exporter")?;

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        rpc_url = %config.rpc_url,
        database_url = %config.database_url,
        backfill_from = config.backfill_from,
        head_poll_secs = config.head_poll_secs,
        worker_count = config.worker_count,
        fetch_queue_depth = config.fetch_queue_depth,
        "chainlens starting"
    );

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

    let store = PostgresStore::new(pool.clone());
    store
        .run_migrations()
        .await
        .context("failed to run database migrations")?;
    tracing::info!("migrations applied");

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

    let mut cursor = store
        .last_indexed_block()
        .await
        .context("failed to read cursor")?
        .map(|(n, _)| n)
        .unwrap_or(config.backfill_from.saturating_sub(1));

    tracing::info!(cursor, "starting pipeline");

    let poll_interval = Duration::from_secs(config.head_poll_secs);
    let mut blocks_indexed: u64 = 0;
    let mut last_log = std::time::Instant::now();

    loop {
        if shutdown.is_cancelled() {
            tracing::info!("shutdown requested, stopping pipeline");
            break;
        }

        let head = tokio::select! {
            result = head_watcher::watch(&rpc_client, poll_interval, shutdown.clone()) => {
                match result {
                    Some(h) => h,
                    None => break,
                }
            }
        };

        tracing::debug!(
            latest = head.latest,
            safe = head.safe,
            finalized = head.finalized,
            cursor,
            "chain head"
        );

        let start_block = cursor.max(config.backfill_from);
        let blocks_to_fetch: Vec<u64> = (start_block..=head.latest)
            .take(config.fetch_queue_depth * 2)
            .collect();

        if blocks_to_fetch.is_empty() {
            tokio::select! {
                () = tokio::time::sleep(poll_interval) => {}
                () = shutdown.cancelled() => break,
            }
            continue;
        }

        let (fetch_tx, fetch_rx) = mpsc::channel(config.fetch_queue_depth);
        let (worker_tx, worker_rx) = mpsc::channel(config.fetch_queue_depth);
        let (seq_tx, mut seq_rx) = mpsc::channel(config.fetch_queue_depth);

        let worker_handle = {
            let client = rpc_client.clone();
            let worker_count = config.worker_count;
            tokio::spawn(async move {
                worker::spawn_workers(client, worker_count, fetch_rx, worker_tx).await;
            })
        };

        let expected = blocks_to_fetch[0];
        let seq_handle = tokio::spawn(async move {
            sequencer::run(worker_rx, seq_tx, expected).await;
        });

        for block_number in blocks_to_fetch {
            if fetch_tx.send(block_number).await.is_err() {
                break;
            }
        }

        drop(fetch_tx);

        while let Some(fetched) = seq_rx.recv().await {
            if shutdown.is_cancelled() {
                break;
            }

            let block_number = fetched.number;

            match committer::commit_block(&rpc_client, &store, block_number, config.max_reorg_depth)
                .await
            {
                Ok(reorg_occurred) => {
                    if reorg_occurred {
                        tracing::info!("reorg handled, rewinding cursor");
                        cursor = store
                            .last_indexed_block()
                            .await
                            .ok()
                            .flatten()
                            .map(|(n, _)| n)
                            .unwrap_or(config.backfill_from.saturating_sub(1));
                        break;
                    }

                    blocks_indexed += 1;
                    cursor = block_number;

                    let lag = head.latest.saturating_sub(cursor);
                    metrics::set_indexing_lag(lag);

                    if last_log.elapsed() >= Duration::from_secs(10) {
                        tracing::info!(cursor, lag, blocks_indexed, "indexing progress");
                        last_log = std::time::Instant::now();
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, block = block_number, "failed to commit block");
                    tokio::select! {
                        () = tokio::time::sleep(Duration::from_secs(1)) => {}
                        () = shutdown.cancelled() => break,
                    }
                }
            }
        }

        let _ = worker_handle.await;
        let _ = seq_handle.await;
    }

    tracing::info!("closing the connection pool");
    pool.close().await;
    tracing::info!("shutdown complete");

    Ok(())
}
