use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use sqlx::Row;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressIntelligence {
    pub address: String,
    pub first_seen: Option<String>,
    pub last_active: Option<String>,
    pub total_transactions: i64,
    pub total_eth_sent: String,
    pub total_eth_received: String,
    pub unique_contracts_called: i64,
    pub unique_callers: i64,
    pub token_transfers_sent: i64,
    pub token_transfers_received: i64,
    pub behavior_classifications: Vec<String>,
    pub activity_level: String,
}

pub async fn analyze_address(
    pool: &PgPool,
    address: &str,
) -> Result<AddressIntelligence, sqlx::Error> {
    let addr_bytes =
        hex::decode(address.trim_start_matches("0x")).map_err(|_| sqlx::Error::RowNotFound)?;

    let first_last = sqlx::query(
        "SELECT MIN(b.timestamp::text) as first_seen, MAX(b.timestamp::text) as last_active
         FROM transactions t
         JOIN blocks b ON b.number = t.block_number
         WHERE t.from_addr = $1 OR t.to_addr = $1",
    )
    .bind(&addr_bytes)
    .fetch_one(pool)
    .await?;

    let first_seen: Option<String> = first_last.get("first_seen");
    let last_active: Option<String> = first_last.get("last_active");

    let tx_count = sqlx::query("SELECT COUNT(*) as count FROM transactions WHERE from_addr = $1")
        .bind(&addr_bytes)
        .fetch_one(pool)
        .await?;
    let total_transactions: i64 = tx_count.get("count");

    let eth_sent = sqlx::query(
        "SELECT COALESCE(SUM(value::numeric), 0)::text as total FROM transactions WHERE from_addr = $1",
    )
    .bind(&addr_bytes)
    .fetch_one(pool)
    .await?;
    let total_eth_sent: String = eth_sent.get("total");

    let eth_received = sqlx::query(
        "SELECT COALESCE(SUM(value::numeric), 0)::text as total FROM transactions WHERE to_addr = $1",
    )
    .bind(&addr_bytes)
    .fetch_one(pool)
    .await?;
    let total_eth_received: String = eth_received.get("total");

    let contracts = sqlx::query(
        "SELECT COUNT(DISTINCT to_addr) as count FROM transactions WHERE from_addr = $1 AND to_addr IS NOT NULL",
    )
    .bind(&addr_bytes)
    .fetch_one(pool)
    .await?;
    let unique_contracts_called: i64 = contracts.get("count");

    let callers = sqlx::query(
        "SELECT COUNT(DISTINCT from_addr) as count FROM transactions WHERE to_addr = $1",
    )
    .bind(&addr_bytes)
    .fetch_one(pool)
    .await?;
    let unique_callers: i64 = callers.get("count");

    let token_sent =
        sqlx::query("SELECT COUNT(*) as count FROM token_transfers WHERE from_addr = $1")
            .bind(&addr_bytes)
            .fetch_one(pool)
            .await?;
    let token_transfers_sent: i64 = token_sent.get("count");

    let token_recv =
        sqlx::query("SELECT COUNT(*) as count FROM token_transfers WHERE to_addr = $1")
            .bind(&addr_bytes)
            .fetch_one(pool)
            .await?;
    let token_transfers_received: i64 = token_recv.get("count");

    let mut classifications = Vec::new();

    if total_transactions > 100 {
        classifications.push("active_user".to_string());
    }
    if total_transactions == 0 {
        classifications.push("inactive".to_string());
    }

    let eth_sent_val: u128 = total_eth_sent.parse().unwrap_or(0);
    let eth_recv_val: u128 = total_eth_received.parse().unwrap_or(0);
    if eth_sent_val > 1_000_000_000_000_000_000 || eth_recv_val > 1_000_000_000_000_000_000 {
        classifications.push("high_value_transfer".to_string());
    }

    if unique_contracts_called > 10 {
        classifications.push("contract_interactor".to_string());
    }

    if token_transfers_sent > 50 || token_transfers_received > 50 {
        classifications.push("token_trader".to_string());
    }

    let dex_contracts = sqlx::query(
        "SELECT COUNT(*) as count FROM transactions t
         WHERE (t.from_addr = $1 OR t.to_addr = $1)
         AND t.input LIKE '\\x%'",
    )
    .bind(&addr_bytes)
    .fetch_one(pool)
    .await?;
    let dex_count: i64 = dex_contracts.get("count");
    if dex_count > 20 {
        classifications.push("dex_participant".to_string());
    }

    let nft_transfers = sqlx::query(
        "SELECT COUNT(*) as count FROM token_transfers
         WHERE (from_addr = $1 OR to_addr = $1) AND standard = 1",
    )
    .bind(&addr_bytes)
    .fetch_one(pool)
    .await?;
    let nft_count: i64 = nft_transfers.get("count");
    if nft_count > 0 {
        classifications.push("nft_participant".to_string());
    }

    if total_transactions > 0 && unique_callers == 0 {
        classifications.push("deployer".to_string());
    }

    let activity_level = if total_transactions > 1000 {
        "very_high".to_string()
    } else if total_transactions > 100 {
        "high".to_string()
    } else if total_transactions > 10 {
        "medium".to_string()
    } else if total_transactions > 0 {
        "low".to_string()
    } else {
        "none".to_string()
    };

    Ok(AddressIntelligence {
        address: address.to_string(),
        first_seen,
        last_active,
        total_transactions,
        total_eth_sent: wei_to_eth(&total_eth_sent),
        total_eth_received: wei_to_eth(&total_eth_received),
        unique_contracts_called,
        unique_callers,
        token_transfers_sent,
        token_transfers_received,
        behavior_classifications: classifications,
        activity_level,
    })
}

fn wei_to_eth(wei: &str) -> String {
    let wei_val: u128 = wei.parse().unwrap_or(0);
    let eth = wei_val as f64 / 1e18;
    format!("{:.6}", eth)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wei_to_eth_test() {
        assert_eq!(wei_to_eth("1000000000000000000"), "1.000000");
        assert_eq!(wei_to_eth("0"), "0.000000");
    }
}
