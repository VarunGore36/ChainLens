use std::time::Duration;

use tokio_util::sync::CancellationToken;

use crate::rpc::BlockTag;
use crate::rpc::EthClient;

use super::ChainHead;

pub async fn watch<C: EthClient>(
    client: &C,
    poll_interval: Duration,
    shutdown: CancellationToken,
) -> Option<ChainHead> {
    loop {
        if shutdown.is_cancelled() {
            return None;
        }

        match fetch_head(client).await {
            Ok(head) => return Some(head),
            Err(e) => {
                tracing::warn!(error = %e, "failed to fetch chain head, retrying");
                tokio::select! {
                    () = tokio::time::sleep(poll_interval) => {}
                    () = shutdown.cancelled() => return None,
                }
            }
        }
    }
}

async fn fetch_head<C: EthClient>(client: &C) -> Result<ChainHead, crate::rpc::RpcError> {
    let latest = client.get_block_number(BlockTag::Latest).await?;
    let safe = client.get_block_number(BlockTag::Safe).await?;
    let finalized = client.get_block_number(BlockTag::Finalized).await?;

    Ok(ChainHead {
        latest,
        safe,
        finalized,
    })
}
