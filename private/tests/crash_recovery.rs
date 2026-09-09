use std::time::Duration;

use alloy_primitives::{Address, B256, Bytes, U256};

use chainlens::domain::IndexedBlock;
use chainlens::store::BlockStore;
use chainlens::store::postgres::PostgresStore;

#[allow(clippy::cast_possible_truncation)]
fn make_test_block(number: u64, parent_hash: B256) -> IndexedBlock {
    let hash = B256::with_last_byte(number as u8);

    let tx = chainlens::domain::Transaction {
        hash: B256::with_last_byte((number * 10) as u8),
        block_number: number,
        tx_index: 0,
        from: Address::with_last_byte(1),
        to: Some(Address::with_last_byte(2)),
        value: U256::from(1000u64),
        nonce: 0,
        tx_type: 0,
        gas_limit: 21000,
        gas_price: Some(U256::from(7u64)),
        max_fee_per_gas: None,
        max_priority_fee_per_gas: None,
        input: Bytes::new(),
        status: Some(1),
        gas_used: 21000,
        effective_gas_price: Some(U256::from(7u64)),
        contract_address: None,
    };

    let block = chainlens::domain::Block {
        number,
        hash,
        parent_hash,
        timestamp: 1000 + number * 12,
        miner: Address::ZERO,
        gas_used: 21000,
        gas_limit: 30000000,
        base_fee_per_gas: Some(7),
        tx_count: 1,
    };

    IndexedBlock {
        block,
        transactions: vec![tx],
        logs: vec![],
        token_transfers: vec![],
    }
}

fn make_chain(count: u64) -> Vec<IndexedBlock> {
    let mut blocks = Vec::new();
    let mut parent = B256::ZERO;

    for i in 1..=count {
        let block = make_test_block(i, parent);
        parent = block.block.hash;
        blocks.push(block);
    }

    blocks
}

#[tokio::test]
#[ignore]
async fn commit_is_idempotent() {
    let pool = connect_test_db().await;
    let store = PostgresStore::new(pool.clone());
    store.run_migrations().await.unwrap();

    let blocks = make_chain(5);

    for block in &blocks {
        store.commit_block(block).await.unwrap();
    }

    for block in &blocks {
        store.commit_block(block).await.unwrap();
    }

    let (cursor, _) = store.last_indexed_block().await.unwrap().unwrap();
    assert_eq!(cursor, 5);

    pool.close().await;
}

#[tokio::test]
#[ignore]
async fn cursor_advances_in_same_transaction() {
    let pool = connect_test_db().await;
    let store = PostgresStore::new(pool.clone());
    store.run_migrations().await.unwrap();

    let blocks = make_chain(3);

    for block in &blocks {
        store.commit_block(block).await.unwrap();
    }

    let (cursor, hash) = store.last_indexed_block().await.unwrap().unwrap();
    assert_eq!(cursor, 3);
    assert_eq!(hash, blocks[2].block.hash.as_slice());

    pool.close().await;
}

#[tokio::test]
#[ignore]
async fn no_gaps_in_block_numbers() {
    let pool = connect_test_db().await;
    let store = PostgresStore::new(pool.clone());
    store.run_migrations().await.unwrap();

    let blocks = make_chain(10);

    for block in &blocks {
        store.commit_block(block).await.unwrap();
    }

    let result = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM generate_series(
            (SELECT MIN(number) FROM blocks),
            (SELECT MAX(number) FROM blocks)
        ) s
        LEFT JOIN blocks b ON b.number = s
        WHERE b.number IS NULL",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(result, 0);

    pool.close().await;
}

#[tokio::test]
#[ignore]
async fn parent_hash_chain_is_contiguous() {
    let pool = connect_test_db().await;
    let store = PostgresStore::new(pool.clone());
    store.run_migrations().await.unwrap();

    let blocks = make_chain(10);

    for block in &blocks {
        store.commit_block(block).await.unwrap();
    }

    let result = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*)
         FROM blocks b
         JOIN blocks prev ON prev.number = b.number - 1
         WHERE b.parent_hash != prev.hash AND b.number > 0",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(result, 0);

    pool.close().await;
}

#[tokio::test]
#[ignore]
async fn cursor_matches_stored_block_hash() {
    let pool = connect_test_db().await;
    let store = PostgresStore::new(pool.clone());
    store.run_migrations().await.unwrap();

    let blocks = make_chain(5);

    for block in &blocks {
        store.commit_block(block).await.unwrap();
    }

    let result = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*)
         FROM indexer_state s
         JOIN blocks b ON b.number = s.last_indexed_number
         WHERE s.id = 1 AND s.last_indexed_hash != b.hash",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(result, 0);

    pool.close().await;
}

#[allow(clippy::expect_used, clippy::unwrap_used)]
async fn connect_test_db() -> sqlx::PgPool {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://chainlens:chainlens@localhost:5432/chainlens".to_string());

    sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&url)
        .await
        .expect("failed to connect to test database")
}
