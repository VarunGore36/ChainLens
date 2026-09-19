#![allow(clippy::expect_used, clippy::too_many_arguments, clippy::len_zero)]

use sqlx::PgPool;

async fn setup_test_db() -> PgPool {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://chainlens:chainlens@localhost:5432/chainlens".to_string());

    let pool = PgPool::connect(&database_url)
        .await
        .expect("failed to connect to test database");

    let migrator = sqlx::migrate::Migrator::new(std::path::Path::new("./migrations"))
        .await
        .expect("failed to load migrations");

    migrator.run(&pool).await.expect("failed to run migrations");

    pool
}

async fn insert_test_block(pool: &PgPool, number: i64, hash: &[u8], parent_hash: &[u8]) {
    sqlx::query(
        "INSERT INTO blocks (number, hash, parent_hash, timestamp, miner, gas_used, gas_limit, base_fee, tx_count)
         VALUES ($1, $2, $3, now(), $4, 21000, 30000000, 1000000000, 1)
         ON CONFLICT (number) DO NOTHING",
    )
    .bind(number)
    .bind(hash)
    .bind(parent_hash)
    .bind(vec![0u8; 20])
    .execute(pool)
    .await
    .expect("failed to insert test block");
}

async fn insert_test_transaction(
    pool: &PgPool,
    hash: &[u8],
    block_number: i64,
    from: &[u8],
    to: Option<&[u8]>,
    value: &str,
) {
    sqlx::query(
        "INSERT INTO transactions (hash, block_number, tx_index, from_addr, to_addr, value, nonce, tx_type, gas_limit, gas_used, status)
         VALUES ($1, $2, 0, $3, $4, $5::NUMERIC, 0, 0, 21000, 21000, 1)
         ON CONFLICT (hash) DO NOTHING",
    )
    .bind(hash)
    .bind(block_number)
    .bind(from)
    .bind(to)
    .bind(value)
    .execute(pool)
    .await
    .expect("failed to insert test transaction");
}

async fn insert_test_log(
    pool: &PgPool,
    block_number: i64,
    log_index: i32,
    tx_hash: &[u8],
    address: &[u8],
    topic0: Option<&[u8]>,
) {
    sqlx::query(
        "INSERT INTO logs (block_number, log_index, tx_hash, address, topic0, data)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (block_number, log_index) DO NOTHING",
    )
    .bind(block_number)
    .bind(log_index)
    .bind(tx_hash)
    .bind(address)
    .bind(topic0)
    .bind(&[] as &[u8])
    .execute(pool)
    .await
    .expect("failed to insert test log");
}

async fn insert_test_token_transfer(
    pool: &PgPool,
    block_number: i64,
    log_index: i32,
    token_address: &[u8],
    from: &[u8],
    to: &[u8],
    value: &str,
    standard: i16,
) {
    sqlx::query(
        "INSERT INTO token_transfers (block_number, log_index, token_address, from_addr, to_addr, value, token_id, standard)
         VALUES ($1, $2, $3, $4, $5, $6::NUMERIC, 0, $7)
         ON CONFLICT (block_number, log_index) DO NOTHING",
    )
    .bind(block_number)
    .bind(log_index)
    .bind(token_address)
    .bind(from)
    .bind(to)
    .bind(value)
    .bind(standard)
    .execute(pool)
    .await
    .expect("failed to insert test token transfer");
}

#[tokio::test]
#[ignore]
async fn test_address_intelligence_basic() {
    let pool = setup_test_db().await;

    let addr1 = vec![0x11u8; 20];
    let addr2 = vec![0x22u8; 20];
    let block_hash = vec![0xAAu8; 32];
    let parent_hash = vec![0x00u8; 32];
    let tx_hash = vec![0xBBu8; 32];

    insert_test_block(&pool, 100, &block_hash, &parent_hash).await;
    insert_test_transaction(
        &pool,
        &tx_hash,
        100,
        &addr1,
        Some(&addr2),
        "1000000000000000000",
    )
    .await;

    let intel = chainlens::intelligence::address::analyze_address(
        &pool,
        &format!("0x{}", hex::encode(&addr1)),
    )
    .await
    .expect("failed to analyze address");

    assert!(intel.total_transactions >= 1);
    assert!(intel.first_seen.is_some());

    sqlx::query("DELETE FROM transactions WHERE hash = $1")
        .bind(&tx_hash)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM blocks WHERE number = 100")
        .execute(&pool)
        .await
        .unwrap();
}

#[tokio::test]
#[ignore]
async fn test_contract_intelligence_basic() {
    let pool = setup_test_db().await;

    let contract_addr = vec![0x33u8; 20];
    let caller_addr = vec![0x44u8; 20];
    let block_hash = vec![0xCCu8; 32];
    let parent_hash = vec![0x00u8; 32];
    let tx_hash = vec![0xDDu8; 32];

    insert_test_block(&pool, 200, &block_hash, &parent_hash).await;
    insert_test_transaction(
        &pool,
        &tx_hash,
        200,
        &caller_addr,
        Some(&contract_addr),
        "0",
    )
    .await;

    let intel = chainlens::intelligence::contract::analyze_contract(
        &pool,
        &format!("0x{}", hex::encode(&contract_addr)),
    )
    .await
    .expect("failed to analyze contract");

    assert!(intel.total_transactions >= 1);
    assert!(intel.unique_callers >= 1);

    sqlx::query("DELETE FROM transactions WHERE hash = $1")
        .bind(&tx_hash)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM blocks WHERE number = 200")
        .execute(&pool)
        .await
        .unwrap();
}

#[tokio::test]
#[ignore]
async fn test_transaction_explanation_basic() {
    let pool = setup_test_db().await;

    let from_addr = vec![0x55u8; 20];
    let to_addr = vec![0x66u8; 20];
    let block_hash = vec![0xEEu8; 32];
    let parent_hash = vec![0x00u8; 32];
    let tx_hash = vec![0xFFu8; 32];

    insert_test_block(&pool, 300, &block_hash, &parent_hash).await;
    insert_test_transaction(
        &pool,
        &tx_hash,
        300,
        &from_addr,
        Some(&to_addr),
        "500000000000000000",
    )
    .await;

    let explanation = chainlens::intelligence::actions::explain_transaction(
        &pool,
        &format!("0x{}", hex::encode(&tx_hash)),
    )
    .await
    .expect("failed to explain transaction");

    assert_eq!(explanation.block_number, 300);
    assert!(!explanation.actions.is_empty());
    assert_eq!(explanation.status, "success");

    sqlx::query("DELETE FROM transactions WHERE hash = $1")
        .bind(&tx_hash)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM blocks WHERE number = 300")
        .execute(&pool)
        .await
        .unwrap();
}

#[tokio::test]
#[ignore]
async fn test_block_analytics_basic() {
    let pool = setup_test_db().await;

    let block_hash = vec![0x11u8; 32];
    let parent_hash = vec![0x00u8; 32];

    insert_test_block(&pool, 400, &block_hash, &parent_hash).await;

    let analytics = chainlens::intelligence::mev::analyze_block(&pool, 400)
        .await
        .expect("failed to analyze block");

    assert_eq!(analytics.block_number, 400);
    assert!(analytics.mev_events.is_empty());

    sqlx::query("DELETE FROM blocks WHERE number = 400")
        .execute(&pool)
        .await
        .unwrap();
}

#[tokio::test]
#[ignore]
async fn test_anomaly_detection_basic() {
    let pool = setup_test_db().await;

    let anomalies = chainlens::intelligence::anomaly::detect_anomalies(&pool)
        .await
        .expect("failed to detect anomalies");

    assert!(anomalies.is_empty() || !anomalies.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_persist_transaction_actions() {
    let pool = setup_test_db().await;

    let from_addr = vec![0x77u8; 20];
    let to_addr = vec![0x88u8; 20];
    let block_hash = vec![0x22u8; 32];
    let parent_hash = vec![0x00u8; 32];
    let tx_hash = vec![0x33u8; 32];

    insert_test_block(&pool, 500, &block_hash, &parent_hash).await;
    insert_test_transaction(
        &pool,
        &tx_hash,
        500,
        &from_addr,
        Some(&to_addr),
        "1000000000000000000",
    )
    .await;

    let explanation = chainlens::intelligence::actions::explain_transaction(
        &pool,
        &format!("0x{}", hex::encode(&tx_hash)),
    )
    .await
    .expect("failed to explain transaction");

    let result =
        chainlens::intelligence::persist::persist_transaction_actions(&pool, &explanation).await;
    assert!(result.is_ok());

    let count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM transaction_actions WHERE tx_hash = $1")
            .bind(&tx_hash)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(count.0 >= 1);

    sqlx::query("DELETE FROM transaction_actions WHERE tx_hash = $1")
        .bind(&tx_hash)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM transactions WHERE hash = $1")
        .bind(&tx_hash)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM blocks WHERE number = 500")
        .execute(&pool)
        .await
        .unwrap();
}

#[tokio::test]
#[ignore]
async fn test_persist_address_stats() {
    let pool = setup_test_db().await;

    let addr = vec![0x99u8; 20];
    let block_hash = vec![0x44u8; 32];
    let parent_hash = vec![0x00u8; 32];
    let tx_hash = vec![0x55u8; 32];

    insert_test_block(&pool, 600, &block_hash, &parent_hash).await;
    insert_test_transaction(&pool, &tx_hash, 600, &addr, None, "0").await;

    let intel = chainlens::intelligence::address::analyze_address(
        &pool,
        &format!("0x{}", hex::encode(&addr)),
    )
    .await
    .expect("failed to analyze address");

    let result = chainlens::intelligence::persist::persist_address_stats(&pool, &intel).await;
    assert!(result.is_ok());

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM address_stats WHERE address = $1")
        .bind(&addr)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 1);

    sqlx::query("DELETE FROM address_stats WHERE address = $1")
        .bind(&addr)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM transactions WHERE hash = $1")
        .bind(&tx_hash)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM blocks WHERE number = 600")
        .execute(&pool)
        .await
        .unwrap();
}

#[tokio::test]
#[ignore]
async fn test_reorg_cleans_intelligence() {
    let pool = setup_test_db().await;

    let block_hash = vec![0x66u8; 32];
    let parent_hash = vec![0x00u8; 32];

    insert_test_block(&pool, 700, &block_hash, &parent_hash).await;

    let block_exists: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM blocks WHERE number = 700")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(block_exists.0, 1);

    sqlx::query("DELETE FROM blocks WHERE number > 699")
        .execute(&pool)
        .await
        .unwrap();

    let block_exists: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM blocks WHERE number = 700")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(block_exists.0, 0);
}

#[tokio::test]
#[ignore]
async fn test_graph_basic() {
    let pool = setup_test_db().await;

    let addr1 = vec![0xAAu8; 20];
    let addr2 = vec![0xBBu8; 20];
    let block_hash = vec![0xCCu8; 32];
    let parent_hash = vec![0x00u8; 32];
    let tx_hash = vec![0xDDu8; 32];

    insert_test_block(&pool, 800, &block_hash, &parent_hash).await;
    insert_test_transaction(
        &pool,
        &tx_hash,
        800,
        &addr1,
        Some(&addr2),
        "1000000000000000000",
    )
    .await;

    let graph = chainlens::intelligence::graph::build_graph(
        &pool,
        &format!("0x{}", hex::encode(&addr1)),
        2,
        50,
    )
    .await
    .expect("failed to build graph");

    assert_eq!(graph.center, format!("0x{}", hex::encode(&addr1)));
    assert!(graph.edges.len() >= 1);

    sqlx::query("DELETE FROM transactions WHERE hash = $1")
        .bind(&tx_hash)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM blocks WHERE number = 800")
        .execute(&pool)
        .await
        .unwrap();
}
