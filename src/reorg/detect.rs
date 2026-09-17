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

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::B256;

    #[test]
    fn genesis_block_never_triggers_reorg() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let result = detect(
                &crate::test_helpers::mock_rpc::MockRpcClient::new(false),
                &MockStore {
                    hash: vec![1u8; 32],
                },
                0,
                B256::ZERO,
                128,
            )
            .await
            .unwrap();
            assert_eq!(result, None, "block 0 never triggers reorg");
        });
    }

    #[test]
    fn block_1_parent_hash_mismatch_short_circuits() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let store = MockStore {
                hash: vec![0xAAu8; 32],
            };
            let result = detect(
                &crate::test_helpers::mock_rpc::MockRpcClient::new(false),
                &store,
                1,
                B256::with_last_byte(0xBB),
                128,
            )
            .await
            .unwrap();
            assert_eq!(
                result, None,
                "prev_number==0 short-circuits to Ok(None) — reorg at block 1 is never detected"
            );
        });
    }

    #[test]
    fn empty_store_parent_hash_mismatch_triggers_ancestor_search() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let store = MockStore { hash: vec![] };
            let result = detect(
                &crate::test_helpers::mock_rpc::MockRpcClient::new(false),
                &store,
                100,
                B256::with_last_byte(0xBB),
                128,
            )
            .await;
            assert!(
                result.is_err(),
                "empty stored hash != incoming parent, triggers ancestor search which fails"
            );
        });
    }

    struct MockStore {
        hash: Vec<u8>,
    }

    #[async_trait::async_trait]
    impl crate::store::BlockStore for MockStore {
        async fn commit_block(
            &self,
            _block: &crate::domain::IndexedBlock,
        ) -> Result<(), crate::store::StoreError> {
            Ok(())
        }
        async fn last_indexed_block(
            &self,
        ) -> Result<Option<(u64, Vec<u8>)>, crate::store::StoreError> {
            Ok(Some((100, self.hash.clone())))
        }
        async fn run_migrations(&self) -> Result<(), crate::store::StoreError> {
            Ok(())
        }
        fn get_pool(&self) -> &sqlx::PgPool {
            panic!("no pool in mock")
        }
    }
}
