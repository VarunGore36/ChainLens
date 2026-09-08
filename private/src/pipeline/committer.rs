use crate::decode::block::decode_block;
use crate::rpc::EthClient;
use crate::store::BlockStore;

use super::PipelineError;

pub async fn commit_block<C: EthClient, S: BlockStore>(
    client: &C,
    store: &S,
    block_number: u64,
) -> Result<(), PipelineError> {
    let block = client
        .get_block_by_number(block_number, true)
        .await?
        .ok_or_else(|| {
            PipelineError::Rpc(crate::rpc::RpcError::Deserialize(format!(
                "block {block_number} not found"
            )))
        })?;

    let receipts = if client.supports_block_receipts() {
        client.get_block_receipts(block_number).await?
    } else {
        let mut receipts = Vec::with_capacity(block.transactions.len());
        for tx in &block.transactions {
            let receipt = client
                .get_transaction_receipt(tx.hash)
                .await?
                .ok_or_else(|| {
                    PipelineError::Rpc(crate::rpc::RpcError::Deserialize(format!(
                        "receipt for tx {} not found",
                        tx.hash
                    )))
                })?;
            receipts.push(receipt);
        }
        receipts
    };

    let indexed = decode_block(&block, &receipts)?;

    store.commit_block(&indexed).await?;

    Ok(())
}
