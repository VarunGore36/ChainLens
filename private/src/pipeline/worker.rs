use tokio::sync::mpsc;

use crate::decode::block::decode_block;
use crate::domain::IndexedBlock;
use crate::rpc::EthClient;

use super::PipelineError;

#[derive(Debug)]
pub struct FetchedBlock {
    pub number: u64,
    pub block: IndexedBlock,
}

pub async fn spawn_workers<C: EthClient + Clone + 'static>(
    client: C,
    worker_count: usize,
    rx: mpsc::Receiver<u64>,
    tx: mpsc::Sender<FetchedBlock>,
) {
    let rx = std::sync::Arc::new(tokio::sync::Mutex::new(rx));
    let mut handles = Vec::with_capacity(worker_count);

    for _ in 0..worker_count {
        let client = client.clone();
        let rx = rx.clone();
        let tx = tx.clone();

        let handle = tokio::spawn(async move {
            loop {
                let block_number = {
                    let mut guard = rx.lock().await;
                    guard.recv().await
                };

                match block_number {
                    Some(num) => match fetch_and_decode(&client, num).await {
                        Ok(block) => {
                            if tx.send(block).await.is_err() {
                                break;
                            }
                        }
                        Err(e) => {
                            tracing::error!(error = %e, block = num, "worker failed to fetch block");
                        }
                    },
                    None => break,
                }
            }
        });

        handles.push(handle);
    }

    drop(tx);

    for handle in handles {
        let _ = handle.await;
    }
}

async fn fetch_and_decode<C: EthClient>(
    client: &C,
    block_number: u64,
) -> Result<FetchedBlock, PipelineError> {
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

    Ok(FetchedBlock {
        number: block_number,
        block: indexed,
    })
}
