use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use sqlx::Row;

#[derive(Debug, Serialize, Deserialize)]
pub struct AddressCluster {
    pub cluster_id: i64,
    pub addresses: Vec<String>,
    pub total_interactions: i64,
    pub first_seen: Option<String>,
    pub last_seen: Option<String>,
    pub cluster_type: String,
}

pub async fn find_clusters(
    pool: &PgPool,
    address: &str,
    max_depth: i32,
) -> Result<AddressCluster, sqlx::Error> {
    let addr_bytes = hex::decode(address.trim_start_matches("0x"))
        .map_err(|_| sqlx::Error::Protocol("invalid address".into()))?;

    let mut visited = std::collections::HashSet::new();
    let mut queue = std::collections::VecDeque::new();
    let mut addresses = Vec::new();
    let mut total_interactions = 0i64;

    visited.insert(addr_bytes.clone());
    queue.push_back((addr_bytes.clone(), 0i32));
    addresses.push(address.to_string());

    while let Some((current_addr, depth)) = queue.pop_front() {
        if depth >= max_depth {
            break;
        }

        let related = sqlx::query(
            "SELECT DISTINCT to_addr, COUNT(*) as cnt
             FROM transactions
             WHERE from_addr = $1 AND to_addr IS NOT NULL
             GROUP BY to_addr
             HAVING COUNT(*) >= 3
             ORDER BY cnt DESC
             LIMIT 10",
        )
        .bind(&current_addr)
        .fetch_all(pool)
        .await?;

        for row in &related {
            let to_addr: Vec<u8> = row.get("to_addr");
            let cnt: i64 = row.get("cnt");

            if !visited.contains(&to_addr) {
                visited.insert(to_addr.clone());
                queue.push_back((to_addr.clone(), depth + 1));
                addresses.push(format!("0x{}", hex::encode(&to_addr)));
                total_interactions += cnt;
            }
        }

        let related_from = sqlx::query(
            "SELECT DISTINCT from_addr, COUNT(*) as cnt
             FROM transactions
             WHERE to_addr = $1
             GROUP BY from_addr
             HAVING COUNT(*) >= 3
             ORDER BY cnt DESC
             LIMIT 10",
        )
        .bind(&current_addr)
        .fetch_all(pool)
        .await?;

        for row in &related_from {
            let from_addr: Vec<u8> = row.get("from_addr");
            let cnt: i64 = row.get("cnt");

            if !visited.contains(&from_addr) {
                visited.insert(from_addr.clone());
                queue.push_back((from_addr.clone(), depth + 1));
                addresses.push(format!("0x{}", hex::encode(&from_addr)));
                total_interactions += cnt;
            }
        }
    }

    let first_last = sqlx::query(
        "SELECT MIN(b.timestamp::text) as first_seen, MAX(b.timestamp::text) as last_seen
         FROM transactions t
         JOIN blocks b ON b.number = t.block_number
         WHERE t.from_addr = $1 OR t.to_addr = $1",
    )
    .bind(&addr_bytes)
    .fetch_one(pool)
    .await?;

    let first_seen: Option<String> = first_last.get("first_seen");
    let last_seen: Option<String> = first_last.get("last_seen");

    let cluster_type = if addresses.len() > 10 {
        "large_cluster".to_string()
    } else if addresses.len() > 5 {
        "medium_cluster".to_string()
    } else {
        "small_cluster".to_string()
    };

    Ok(AddressCluster {
        cluster_id: 0,
        addresses,
        total_interactions,
        first_seen,
        last_seen,
        cluster_type,
    })
}

pub async fn detect_contract_deployers(
    pool: &PgPool,
    limit: i64,
) -> Result<Vec<(String, i64)>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT t.from_addr, COUNT(DISTINCT t.contract_address) as contracts_deployed
         FROM transactions t
         WHERE t.contract_address IS NOT NULL
         GROUP BY t.from_addr
         HAVING COUNT(DISTINCT t.contract_address) >= 2
         ORDER BY contracts_deployed DESC
         LIMIT $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let mut deployers = Vec::new();
    for row in &rows {
        let addr: Vec<u8> = row.get("from_addr");
        let count: i64 = row.get("contracts_deployed");
        deployers.push((format!("0x{}", hex::encode(&addr)), count));
    }

    Ok(deployers)
}

pub async fn detect_token_whales(
    pool: &PgPool,
    token_address: &str,
    limit: i64,
) -> Result<Vec<(String, String)>, sqlx::Error> {
    let token_bytes = hex::decode(token_address.trim_start_matches("0x"))
        .map_err(|_| sqlx::Error::Protocol("invalid address".into()))?;

    let rows = sqlx::query(
        "SELECT to_addr, SUM(value::numeric)::text as total_received
         FROM token_transfers
         WHERE token_address = $1
         GROUP BY to_addr
         ORDER BY SUM(value::numeric) DESC
         LIMIT $2",
    )
    .bind(&token_bytes)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let mut whales = Vec::new();
    for row in &rows {
        let addr: Vec<u8> = row.get("to_addr");
        let total: String = row.get("total_received");
        whales.push((format!("0x{}", hex::encode(&addr)), total));
    }

    Ok(whales)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cluster_serialization() {
        let cluster = AddressCluster {
            cluster_id: 1,
            addresses: vec!["0x1111".to_string(), "0x2222".to_string()],
            total_interactions: 10,
            first_seen: Some("2024-01-01".to_string()),
            last_seen: Some("2024-01-15".to_string()),
            cluster_type: "small_cluster".to_string(),
        };
        let json = serde_json::to_string(&cluster).unwrap();
        assert!(json.contains("small_cluster"));
        assert!(json.contains("0x1111"));
    }
}
