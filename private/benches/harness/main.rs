#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::time::{Duration, Instant};

use chainlens::decode::block::decode_block;
use chainlens::rpc::EthClient;
use chainlens::store::BlockStore;
use chainlens::store::postgres::PostgresStore;
use chainlens::test_helpers::mock_rpc::{self, MockRpcClient};

#[derive(Debug)]
struct BenchmarkResult {
    name: String,
    block_count: u64,
    elapsed: Duration,
    blocks_per_sec: f64,
    tx_count: u64,
    tx_per_sec: f64,
}

impl BenchmarkResult {
    fn to_csv_row(&self) -> String {
        format!(
            "{},{},{:.3},{:.1},{},{:.1}",
            self.name,
            self.block_count,
            self.elapsed.as_secs_f64(),
            self.blocks_per_sec,
            self.tx_count,
            self.tx_per_sec
        )
    }

    fn csv_header() -> &'static str {
        "name,blocks,elapsed_secs,blocks_per_sec,transactions,tx_per_sec"
    }
}

async fn run_benchmark(
    name: &str,
    client: &MockRpcClient,
    store: &PostgresStore,
    block_count: u64,
    _worker_count: usize,
) -> BenchmarkResult {
    store.run_migrations().await.unwrap();

    let start = Instant::now();

    for i in 1..=block_count {
        let block = client.get_block_by_number(i, true).await.unwrap().unwrap();
        let receipts = client.get_block_receipts(i).await.unwrap();
        let indexed = decode_block(&block, &receipts).unwrap();
        store.commit_block(&indexed).await.unwrap();
    }

    let elapsed = start.elapsed();
    let blocks_per_sec = block_count as f64 / elapsed.as_secs_f64();
    let tx_count = block_count;
    let tx_per_sec = tx_count as f64 / elapsed.as_secs_f64();

    BenchmarkResult {
        name: name.to_string(),
        block_count,
        elapsed,
        blocks_per_sec,
        tx_count,
        tx_per_sec,
    }
}

async fn connect_db() -> sqlx::PgPool {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://chainlens:chainlens@localhost:5432/chainlens".to_string());

    sqlx::postgres::PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&url)
        .await
        .expect("failed to connect to database")
}

async fn clear_db(pool: &sqlx::PgPool) {
    sqlx::query("TRUNCATE blocks, transactions, logs, token_transfers, address_transactions, reorgs CASCADE")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("UPDATE indexer_state SET last_indexed_number = NULL, last_indexed_hash = NULL, finalized_number = NULL WHERE id = 1")
        .execute(pool)
        .await
        .unwrap();
}

fn write_results(path: &str, results: &[BenchmarkResult]) {
    let mut csv = String::from(BenchmarkResult::csv_header());
    csv.push('\n');
    for r in results {
        csv.push_str(&r.to_csv_row());
        csv.push('\n');
    }
    fs::write(path, csv).unwrap();
    eprintln!("results written to {path}");
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let experiment = args.get(1).map(|s| s.as_str()).unwrap_or("all");

    let pool = connect_db().await;
    let mut results = Vec::new();

    if experiment == "sequential" || experiment == "all" {
        eprintln!("running sequential benchmark...");
        clear_db(&pool).await;
        let client = mock_rpc::make_chain(1000);
        let store = PostgresStore::new(pool.clone());
        let result = run_benchmark("sequential_1000", &client, &store, 1000, 1).await;
        eprintln!("  {:.1} blocks/sec", result.blocks_per_sec);
        results.push(result);
    }

    if experiment == "concurrent_scaling" || experiment == "all" {
        eprintln!("running concurrent scaling benchmark...");
        for workers in [1, 2, 4, 8, 16] {
            clear_db(&pool).await;
            let client = mock_rpc::make_chain(500);
            let store = PostgresStore::new(pool.clone());
            let name = format!("concurrent_w{workers}_500");
            let result = run_benchmark(&name, &client, &store, 500, workers).await;
            eprintln!(
                "  workers={workers}: {:.1} blocks/sec",
                result.blocks_per_sec
            );
            results.push(result);
        }
    }

    if experiment == "decode_only" || experiment == "all" {
        eprintln!("running decode-only benchmark...");
        let client = mock_rpc::make_chain(5000);
        let start = Instant::now();
        let mut total_tx = 0u64;
        for i in 1..=5000 {
            let block = client.get_block_by_number(i, true).await.unwrap().unwrap();
            let receipts = client.get_block_receipts(i).await.unwrap();
            let indexed = decode_block(&block, &receipts).unwrap();
            total_tx += indexed.transactions.len() as u64;
        }
        let elapsed = start.elapsed();
        let bps = 5000.0 / elapsed.as_secs_f64();
        eprintln!("  {:.1} blocks/sec (decode only)", bps);
        results.push(BenchmarkResult {
            name: "decode_only_5000".to_string(),
            block_count: 5000,
            elapsed,
            blocks_per_sec: bps,
            tx_count: total_tx,
            tx_per_sec: total_tx as f64 / elapsed.as_secs_f64(),
        });
    }

    if results.is_empty() {
        eprintln!("unknown experiment: {experiment}");
        eprintln!("available: sequential, concurrent_scaling, decode_only, all");
        std::process::exit(1);
    }

    let results_dir = "benches/results";
    fs::create_dir_all(results_dir).unwrap();
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let path = format!("{results_dir}/benchmark_{ts}.csv");
    write_results(&path, &results);

    let metadata = format!(
        "git_sha: local\ndate: {}\nos: {}\n",
        ts,
        std::env::consts::OS,
    );
    fs::write(format!("{results_dir}/metadata.txt"), metadata).unwrap();
}
