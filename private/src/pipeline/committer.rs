use std::time::Instant;

use crate::decode::block::decode_block;
use crate::metrics;
use crate::reorg::{ReorgEvent, detect, rollback};
use crate::rpc::EthClient;
use crate::store::BlockStore;

use super::PipelineError;

pub async fn commit_block<C: EthClient, S: BlockStore>(
    client: &C,
    store: &S,
    block_number: u64,
    max_reorg_depth: u64,
) -> Result<bool, PipelineError> {
    let start = Instant::now();

    let block = client
        .get_block_by_number(block_number, true)
        .await?
        .ok_or_else(|| {
            PipelineError::Rpc(crate::rpc::RpcError::Deserialize(format!(
                "block {block_number} not found"
            )))
        })?;

    metrics::record_rpc_request("eth_getBlockByNumber", start.elapsed().as_secs_f64(), true);

    if block_number > 0 {
        let reorg_ancestor = detect::detect(
            client,
            store,
            block_number,
            block.parent_hash,
            max_reorg_depth,
        )
        .await?;

        if let Some(ancestor) = reorg_ancestor {
            tracing::warn!(ancestor, block_number, "reorg detected, rolling back");

            let orphaned_hashes = collect_orphaned_hashes(store, ancestor, block_number).await?;

            let event = ReorgEvent {
                common_ancestor: ancestor,
                depth: block_number - ancestor,
                orphaned_from: ancestor + 1,
                orphaned_to: block_number,
                orphaned_hashes,
            };

            let pool = store.get_pool();
            rollback::rollback(pool, &event)
                .await
                .map_err(crate::store::StoreError::Database)?;

            metrics::record_reorg(event.depth);
            tracing::info!(ancestor, depth = event.depth, "rollback complete");

            return Ok(true);
        }
    }

    let receipts = if client.supports_block_receipts() {
        let start = Instant::now();
        let receipts = client.get_block_receipts(block_number).await?;
        metrics::record_rpc_request("eth_getBlockReceipts", start.elapsed().as_secs_f64(), true);
        receipts
    } else {
        let mut receipts = Vec::with_capacity(block.transactions.len());
        for tx in &block.transactions {
            let start = Instant::now();
            let receipt = client
                .get_transaction_receipt(tx.hash)
                .await?
                .ok_or_else(|| {
                    PipelineError::Rpc(crate::rpc::RpcError::Deserialize(format!(
                        "receipt for tx {} not found",
                        tx.hash
                    )))
                })?;
            metrics::record_rpc_request(
                "eth_getTransactionReceipt",
                start.elapsed().as_secs_f64(),
                true,
            );
            receipts.push(receipt);
        }
        receipts
    };

    let indexed = decode_block(&block, &receipts)?;

    let start = Instant::now();
    store.commit_block(&indexed).await?;
    metrics::record_db_commit(start.elapsed().as_secs_f64());

    metrics::record_block_committed(indexed.transactions.len() as u64, indexed.logs.len() as u64);

    Ok(false)
}

async fn collect_orphaned_hashes<S: BlockStore>(
    _store: &S,
    from: u64,
    to: u64,
) -> Result<Vec<Vec<u8>>, PipelineError> {
    let mut hashes = Vec::new();
    for _ in from..=to {
        hashes.push(vec![0u8; 32]);
    }
    Ok(hashes)
}
