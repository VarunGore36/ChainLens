use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use sqlx::Row;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractIntelligence {
    pub address: String,
    pub first_seen: Option<String>,
    pub deployment_block: Option<i64>,
    pub creator: Option<String>,
    pub deployment_tx: Option<String>,
    pub total_transactions: i64,
    pub unique_callers: i64,
    pub unique_contracts_called: i64,
    pub token_transfers: i64,
    pub event_count: i64,
    pub activity_over_time: Vec<ActivityPoint>,
    pub function_selectors: Vec<String>,
    pub is_token: bool,
    pub is_nft: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityPoint {
    pub block_number: i64,
    pub timestamp: String,
    pub tx_count: i64,
    pub event_count: i64,
}

pub async fn analyze_contract(
    pool: &PgPool,
    address: &str,
) -> Result<ContractIntelligence, sqlx::Error> {
    let addr_bytes =
        hex::decode(address.trim_start_matches("0x")).map_err(|_| sqlx::Error::RowNotFound)?;

    let first_seen_query = sqlx::query(
        "SELECT MIN(b.number) as first_block, MIN(b.timestamp::text) as first_seen
         FROM transactions t
         JOIN blocks b ON b.number = t.block_number
         WHERE t.to_addr = $1 OR t.from_addr = $1",
    )
    .bind(&addr_bytes)
    .fetch_one(pool)
    .await?;

    let first_seen: Option<String> = first_seen_query.get("first_seen");
    let deployment_block: Option<i64> = first_seen_query.get("first_block");

    let creator_query = sqlx::query(
        "SELECT from_addr, hash FROM transactions
         WHERE contract_address = $1
         LIMIT 1",
    )
    .bind(&addr_bytes)
    .fetch_optional(pool)
    .await?;

    let (creator, deployment_tx) = match creator_query {
        Some(row) => {
            let from: Vec<u8> = row.get("from_addr");
            let hash: Vec<u8> = row.get("hash");
            (
                Some(format!("0x{}", hex::encode(&from))),
                Some(format!("0x{}", hex::encode(&hash))),
            )
        }
        None => (None, None),
    };

    let tx_count = sqlx::query("SELECT COUNT(*) as count FROM transactions WHERE to_addr = $1")
        .bind(&addr_bytes)
        .fetch_one(pool)
        .await?;
    let total_transactions: i64 = tx_count.get("count");

    let callers = sqlx::query(
        "SELECT COUNT(DISTINCT from_addr) as count FROM transactions WHERE to_addr = $1",
    )
    .bind(&addr_bytes)
    .fetch_one(pool)
    .await?;
    let unique_callers: i64 = callers.get("count");

    let contracts = sqlx::query(
        "SELECT COUNT(DISTINCT to_addr) as count
         FROM transactions
         WHERE from_addr = $1 AND to_addr IS NOT NULL",
    )
    .bind(&addr_bytes)
    .fetch_one(pool)
    .await?;
    let unique_contracts_called: i64 = contracts.get("count");

    let token_count =
        sqlx::query("SELECT COUNT(*) as count FROM token_transfers WHERE token_address = $1")
            .bind(&addr_bytes)
            .fetch_one(pool)
            .await?;
    let token_transfers: i64 = token_count.get("count");

    let event_count_query = sqlx::query("SELECT COUNT(*) as count FROM logs WHERE address = $1")
        .bind(&addr_bytes)
        .fetch_one(pool)
        .await?;
    let event_count: i64 = event_count_query.get("count");

    let activity = sqlx::query(
        "SELECT b.number, b.timestamp::text,
                COUNT(DISTINCT t.hash) as tx_count,
                COUNT(DISTINCT l.log_index) as event_count
         FROM blocks b
         LEFT JOIN transactions t ON t.block_number = b.number AND t.to_addr = $1
         LEFT JOIN logs l ON l.block_number = b.number AND l.address = $1
         WHERE b.number >= $2
         GROUP BY b.number, b.timestamp
         ORDER BY b.number DESC
         LIMIT 100",
    )
    .bind(&addr_bytes)
    .bind(deployment_block.unwrap_or(0))
    .fetch_all(pool)
    .await?;

    let activity_over_time: Vec<ActivityPoint> = activity
        .iter()
        .map(|r| ActivityPoint {
            block_number: r.get("number"),
            timestamp: r.get("timestamp"),
            tx_count: r.get::<i64, _>("tx_count"),
            event_count: r.get::<i64, _>("event_count"),
        })
        .collect();

    let function_selectors = sqlx::query(
        "SELECT DISTINCT SUBSTRING(input FROM 1 FOR 4) as selector
         FROM transactions
         WHERE to_addr = $1 AND LENGTH(input) >= 4
         LIMIT 20",
    )
    .bind(&addr_bytes)
    .fetch_all(pool)
    .await?;

    let selectors: Vec<String> = function_selectors
        .iter()
        .filter_map(|r| {
            let sel: Vec<u8> = r.get("selector");
            if sel.len() == 4 {
                Some(format!("0x{}", hex::encode(&sel)))
            } else {
                None
            }
        })
        .collect();

    let is_token =
        sqlx::query("SELECT COUNT(*) as count FROM token_transfers WHERE token_address = $1")
            .bind(&addr_bytes)
            .fetch_one(pool)
            .await?;
    let token_count_val: i64 = is_token.get("count");

    let is_nft = sqlx::query(
        "SELECT COUNT(*) as count FROM token_transfers WHERE token_address = $1 AND standard = 1",
    )
    .bind(&addr_bytes)
    .fetch_one(pool)
    .await?;
    let nft_count: i64 = is_nft.get("count");

    Ok(ContractIntelligence {
        address: address.to_string(),
        first_seen,
        deployment_block,
        creator,
        deployment_tx,
        total_transactions,
        unique_callers,
        unique_contracts_called,
        token_transfers,
        event_count,
        activity_over_time,
        function_selectors: selectors,
        is_token: token_count_val > 0,
        is_nft: nft_count > 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activity_point_serialization() {
        let point = ActivityPoint {
            block_number: 12345,
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            tx_count: 10,
            event_count: 25,
        };
        let json = serde_json::to_string(&point).unwrap();
        assert!(json.contains("12345"));
        assert!(json.contains("10"));
    }

    #[test]
    fn contract_intelligence_structure() {
        let intel = ContractIntelligence {
            address: "0x1234".to_string(),
            first_seen: None,
            deployment_block: None,
            creator: None,
            deployment_tx: None,
            total_transactions: 0,
            unique_callers: 0,
            unique_contracts_called: 0,
            token_transfers: 0,
            event_count: 0,
            activity_over_time: vec![],
            function_selectors: vec![],
            is_token: false,
            is_nft: false,
        };
        assert_eq!(intel.address, "0x1234");
        assert!(!intel.is_token);
    }
}
