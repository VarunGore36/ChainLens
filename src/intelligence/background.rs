use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::intelligence::persist;
use crate::store::BlockStore;

#[derive(Debug)]
pub enum IntelligenceTask {
    BlockCommitted { block_number: u64 },
    Shutdown,
}

#[derive(Debug)]
pub struct BackgroundProcessor {
    tx: mpsc::Sender<IntelligenceTask>,
}

impl BackgroundProcessor {
    pub fn new<S: BlockStore + 'static>(store: S, shutdown: CancellationToken) -> Self {
        let (tx, mut rx) = mpsc::channel::<IntelligenceTask>(100);

        tokio::spawn(async move {
            tracing::info!("intelligence background processor started");

            loop {
                tokio::select! {
                    Some(task) = rx.recv() => {
                        match task {
                            IntelligenceTask::BlockCommitted { block_number } => {
                                if let Err(e) = Self::process_block(&store, block_number).await {
                                    tracing::error!(
                                        error = %e,
                                        block = block_number,
                                        "failed to process intelligence for block"
                                    );
                                }
                            }
                            IntelligenceTask::Shutdown => {
                                tracing::info!("intelligence background processor shutting down");
                                break;
                            }
                        }
                    }
                    _ = shutdown.cancelled() => {
                        tracing::info!("intelligence background processor cancelled");
                        break;
                    }
                }
            }
        });

        Self { tx }
    }

    pub async fn notify_block_committed(&self, block_number: u64) {
        let _ = self
            .tx
            .send(IntelligenceTask::BlockCommitted { block_number })
            .await;
    }

    pub async fn shutdown(&self) {
        let _ = self.tx.send(IntelligenceTask::Shutdown).await;
    }

    async fn process_block<S: BlockStore>(
        store: &S,
        block_number: u64,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let pool = store.get_pool();

        let tx_hashes = sqlx::query_scalar::<_, Vec<u8>>(
            "SELECT hash FROM transactions WHERE block_number = $1",
        )
        .bind(i64::try_from(block_number).unwrap_or(i64::MAX))
        .fetch_all(pool)
        .await?;

        for hash_bytes in &tx_hashes {
            let tx_hash = format!("0x{}", hex::encode(hash_bytes));

            match crate::intelligence::actions::explain_transaction(pool, &tx_hash).await {
                Ok(explanation) => {
                    let _ = persist::persist_transaction_actions(pool, &explanation).await;
                }
                Err(e) => {
                    tracing::debug!(tx_hash = %tx_hash, error = %e, "failed to explain transaction");
                }
            }
        }

        let addresses = sqlx::query_scalar::<_, Vec<u8>>(
            "SELECT DISTINCT from_addr FROM transactions WHERE block_number = $1
             UNION
             SELECT DISTINCT to_addr FROM transactions WHERE block_number = $1 AND to_addr IS NOT NULL"
        )
        .bind(i64::try_from(block_number).unwrap_or(i64::MAX))
        .fetch_all(pool)
        .await?;

        for addr_bytes in &addresses {
            let addr = format!("0x{}", hex::encode(addr_bytes));

            match crate::intelligence::address::analyze_address(pool, &addr).await {
                Ok(intel) => {
                    let _ = persist::persist_address_stats(pool, &intel).await;
                }
                Err(e) => {
                    tracing::debug!(address = %addr, error = %e, "failed to analyze address");
                }
            }
        }

        let contracts = sqlx::query_scalar::<_, Vec<u8>>(
            "SELECT DISTINCT to_addr FROM transactions WHERE block_number = $1 AND to_addr IS NOT NULL"
        )
        .bind(i64::try_from(block_number).unwrap_or(i64::MAX))
        .fetch_all(pool)
        .await?;

        for addr_bytes in &contracts {
            let addr = format!("0x{}", hex::encode(addr_bytes));

            match crate::intelligence::contract::analyze_contract(pool, &addr).await {
                Ok(intel) => {
                    let _ = persist::persist_contract_profile(pool, &intel).await;
                }
                Err(e) => {
                    tracing::debug!(contract = %addr, error = %e, "failed to analyze contract");
                }
            }
        }

        match crate::intelligence::anomaly::detect_anomalies(pool).await {
            Ok(anomalies) => {
                for anomaly in &anomalies {
                    let _ = persist::persist_anomaly(pool, anomaly).await;
                }
            }
            Err(e) => {
                tracing::debug!(error = %e, "failed to detect anomalies");
            }
        }

        match crate::intelligence::mev::analyze_block(pool, block_number as i64).await {
            Ok(analytics) => {
                let _ = persist::persist_block_analytics(pool, &analytics).await;
                for event in &analytics.mev_events {
                    let _ = persist::persist_mev_event(pool, event).await;
                }
            }
            Err(e) => {
                tracing::debug!(error = %e, "failed to analyze block MEV");
            }
        }

        tracing::debug!(block = block_number, "intelligence processed for block");
        Ok(())
    }
}
