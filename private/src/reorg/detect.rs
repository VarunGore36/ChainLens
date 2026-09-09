use alloy_primitives::B256;

use crate::rpc::EthClient;
use crate::store::BlockStore;

use super::ReorgError;

pub async fn detect<C: EthClient, S: BlockStore>(
    client: &C,
    store: &S,
    incoming_number: u64,
    incoming_parent_hash: B256,
    max_depth: u64,
) -> Result<Option<u64>, ReorgError> {
    if incoming_number == 0 {
        return Ok(None);
    }

    let prev_number = incoming_number - 1;

    let (_, stored_hash) = store.last_indexed_block().await?.unwrap_or((0, vec![]));

    if prev_number == 0 || stored_hash == incoming_parent_hash.as_slice() {
        return Ok(None);
    }

    let ancestor = find_ancestor(client, store, prev_number, max_depth).await?;
    Ok(Some(ancestor))
}

async fn find_ancestor<C: EthClient, S: BlockStore>(
    client: &C,
    store: &S,
    start: u64,
    max_depth: u64,
) -> Result<u64, ReorgError> {
    for depth in 0..=max_depth {
        if start < depth {
            return Err(ReorgError::DepthExceeded { depth, max_depth });
        }

        let block_number = start - depth;

        let block = client
            .get_block_by_number(block_number, false)
            .await?
            .ok_or_else(|| {
                crate::rpc::RpcError::Deserialize(format!("block {block_number} not found"))
            })?;

        let (_, stored_hash) = store.last_indexed_block().await?.unwrap_or((0, vec![]));

        if block_number == 0 || stored_hash == block.hash.as_slice() {
            return Ok(block_number);
        }
    }

    Err(ReorgError::DepthExceeded {
        depth: max_depth + 1,
        max_depth,
    })
}
