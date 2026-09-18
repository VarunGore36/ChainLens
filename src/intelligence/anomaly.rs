use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use sqlx::Row;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnomalyType {
    LargeTransfer,
    ActivitySpike,
    NewWalletHighValue,
    ContractInteractionSpike,
    CoordinatedActivity,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Anomaly {
    pub id: i64,
    pub anomaly_type: AnomalyType,
    pub severity: Severity,
    pub detected_at: String,
    pub entity: String,
    pub entity_type: String,
    pub description: String,
    pub observed_value: f64,
    pub baseline_value: f64,
    pub threshold: f64,
    pub evidence: Vec<String>,
    pub block_number: Option<i64>,
    pub tx_hash: Option<String>,
}

pub async fn detect_anomalies(pool: &PgPool) -> Result<Vec<Anomaly>, sqlx::Error> {
    let mut anomalies = Vec::new();

    anomalies.extend(detect_large_transfers(pool).await?);
    anomalies.extend(detect_activity_spikes(pool).await?);
    anomalies.extend(detect_new_wallet_high_value(pool).await?);
    anomalies.extend(detect_contract_interaction_spikes(pool).await?);
    anomalies.extend(detect_coordinated_activity(pool).await?);

    anomalies.sort_by_key(|a| std::cmp::Reverse(severity_order(&a.severity)));

    Ok(anomalies)
}

async fn detect_large_transfers(pool: &PgPool) -> Result<Vec<Anomaly>, sqlx::Error> {
    let mut anomalies = Vec::new();

    let avg_result = sqlx::query(
        "SELECT COALESCE(AVG(value::numeric), 0)::text as avg_value
         FROM transactions
         WHERE value::numeric > 0",
    )
    .fetch_one(pool)
    .await?;
    let avg_value_str: String = avg_result.get("avg_value");
    let avg_value: f64 = avg_value_str.parse().unwrap_or(0.0);

    let threshold = avg_value * 10.0;

    let large_transfers = sqlx::query(
        "SELECT t.hash, t.from_addr, t.to_addr, t.value::text, t.block_number,
                b.timestamp::text
         FROM transactions t
         JOIN blocks b ON b.number = t.block_number
         WHERE t.value::numeric > $1
         ORDER BY t.value::numeric DESC
         LIMIT 10",
    )
    .bind(threshold.to_string())
    .fetch_all(pool)
    .await?;

    for row in &large_transfers {
        let hash: Vec<u8> = row.get("hash");
        let from: Vec<u8> = row.get("from_addr");
        let to: Option<Vec<u8>> = row.get("to_addr");
        let value: String = row.get("value");
        let block_number: i64 = row.get("block_number");
        let timestamp: String = row.get("timestamp");

        let value_f64: f64 = value.parse().unwrap_or(0.0);
        let severity = if value_f64 > avg_value * 100.0 {
            Severity::Critical
        } else if value_f64 > avg_value * 50.0 {
            Severity::High
        } else {
            Severity::Medium
        };

        anomalies.push(Anomaly {
            id: 0,
            anomaly_type: AnomalyType::LargeTransfer,
            severity,
            detected_at: timestamp,
            entity: format!("0x{}", hex::encode(&from)),
            entity_type: "address".to_string(),
            description: format!(
                "Transfer of {} wei is significantly above average ({} wei)",
                value, avg_value_str
            ),
            observed_value: value_f64,
            baseline_value: avg_value,
            threshold,
            evidence: vec![
                format!("tx_hash: 0x{}", hex::encode(&hash)),
                format!("value: {}", value),
                format!("from: 0x{}", hex::encode(&from)),
                format!(
                    "to: {}",
                    to.map(|t| format!("0x{}", hex::encode(&t)))
                        .unwrap_or_default()
                ),
            ],
            block_number: Some(block_number),
            tx_hash: Some(format!("0x{}", hex::encode(&hash))),
        });
    }

    Ok(anomalies)
}

async fn detect_activity_spikes(pool: &PgPool) -> Result<Vec<Anomaly>, sqlx::Error> {
    let mut anomalies = Vec::new();

    let recent_blocks = sqlx::query(
        "SELECT number, tx_count, timestamp::text
         FROM blocks
         ORDER BY number DESC
         LIMIT 100",
    )
    .fetch_all(pool)
    .await?;

    if recent_blocks.len() < 10 {
        return Ok(anomalies);
    }

    let avg_tx_count: f64 = recent_blocks
        .iter()
        .map(|r| r.get::<i32, _>("tx_count") as f64)
        .sum::<f64>()
        / recent_blocks.len() as f64;

    let threshold = avg_tx_count * 3.0;

    for block in &recent_blocks {
        let tx_count: i32 = block.get("tx_count");
        let block_number: i64 = block.get("number");
        let timestamp: String = block.get("timestamp");

        if tx_count as f64 > threshold {
            anomalies.push(Anomaly {
                id: 0,
                anomaly_type: AnomalyType::ActivitySpike,
                severity: if tx_count as f64 > threshold * 2.0 {
                    Severity::High
                } else {
                    Severity::Medium
                },
                detected_at: timestamp,
                entity: format!("{}", block_number),
                entity_type: "block".to_string(),
                description: format!(
                    "Block {} has {} transactions, significantly above average ({:.0})",
                    block_number, tx_count, avg_tx_count
                ),
                observed_value: tx_count as f64,
                baseline_value: avg_tx_count,
                threshold,
                evidence: vec![
                    format!("block_number: {}", block_number),
                    format!("tx_count: {}", tx_count),
                    format!("avg_tx_count: {:.0}", avg_tx_count),
                ],
                block_number: Some(block_number),
                tx_hash: None,
            });
        }
    }

    Ok(anomalies)
}

async fn detect_new_wallet_high_value(pool: &PgPool) -> Result<Vec<Anomaly>, sqlx::Error> {
    let mut anomalies = Vec::new();

    let high_value_threshold = "1000000000000000000000";

    let new_wallets = sqlx::query(
        "SELECT t.from_addr, MIN(t.block_number) as first_block,
                MAX(b.timestamp::text) as first_seen,
                COUNT(*) as tx_count,
                SUM(t.value::numeric)::text as total_value
         FROM transactions t
         JOIN blocks b ON b.number = t.block_number
         WHERE t.value::numeric > $1
         GROUP BY t.from_addr
         HAVING COUNT(*) <= 5
         ORDER BY first_block DESC
         LIMIT 20",
    )
    .bind(high_value_threshold)
    .fetch_all(pool)
    .await?;

    for row in &new_wallets {
        let from_addr: Vec<u8> = row.get("from_addr");
        let first_seen: String = row.get("first_seen");
        let tx_count: i64 = row.get("tx_count");
        let total_value: String = row.get("total_value");

        let total_f64: f64 = total_value.parse().unwrap_or(0.0);

        anomalies.push(Anomaly {
            id: 0,
            anomaly_type: AnomalyType::NewWalletHighValue,
            severity: Severity::High,
            detected_at: first_seen,
            entity: format!("0x{}", hex::encode(&from_addr)),
            entity_type: "address".to_string(),
            description: format!(
                "New wallet with only {} transactions performed high-value operations ({} wei total)",
                tx_count, total_value
            ),
            observed_value: total_f64,
            baseline_value: 0.0,
            threshold: 1000.0,
            evidence: vec![
                format!("address: 0x{}", hex::encode(&from_addr)),
                format!("tx_count: {}", tx_count),
                format!("total_value: {}", total_value),
            ],
            block_number: None,
            tx_hash: None,
        });
    }

    Ok(anomalies)
}

async fn detect_contract_interaction_spikes(pool: &PgPool) -> Result<Vec<Anomaly>, sqlx::Error> {
    let mut anomalies = Vec::new();

    let recent_contracts = sqlx::query(
        "SELECT to_addr, COUNT(*) as interaction_count,
                MIN(b.timestamp::text) as first_seen,
                MAX(b.timestamp::text) as last_seen
         FROM transactions t
         JOIN blocks b ON b.number = t.block_number
         WHERE t.to_addr IS NOT NULL
         GROUP BY to_addr
         HAVING COUNT(*) > 100
         ORDER BY interaction_count DESC
         LIMIT 20",
    )
    .fetch_all(pool)
    .await?;

    for row in &recent_contracts {
        let to_addr: Vec<u8> = row.get("to_addr");
        let count: i64 = row.get("interaction_count");
        let last_seen: String = row.get("last_seen");

        if count > 1000 {
            anomalies.push(Anomaly {
                id: 0,
                anomaly_type: AnomalyType::ContractInteractionSpike,
                severity: if count > 10000 {
                    Severity::High
                } else {
                    Severity::Medium
                },
                detected_at: last_seen,
                entity: format!("0x{}", hex::encode(&to_addr)),
                entity_type: "contract".to_string(),
                description: format!(
                    "Contract received {} interactions, indicating unusual activity",
                    count
                ),
                observed_value: count as f64,
                baseline_value: 100.0,
                threshold: 1000.0,
                evidence: vec![
                    format!("contract: 0x{}", hex::encode(&to_addr)),
                    format!("interaction_count: {}", count),
                ],
                block_number: None,
                tx_hash: None,
            });
        }
    }

    Ok(anomalies)
}

async fn detect_coordinated_activity(pool: &PgPool) -> Result<Vec<Anomaly>, sqlx::Error> {
    let mut anomalies = Vec::new();

    let recent_blocks = sqlx::query(
        "SELECT DISTINCT block_number
         FROM transactions
         ORDER BY block_number DESC
         LIMIT 10",
    )
    .fetch_all(pool)
    .await?;

    for block_row in &recent_blocks {
        let block_number: i64 = block_row.get("block_number");

        let coordinated = sqlx::query(
            "SELECT t1.to_addr, COUNT(DISTINCT t1.from_addr) as addr_count,
                    array_agg(DISTINCT t1.from_addr) as addresses,
                    MIN(b.timestamp::text) as first_seen
             FROM transactions t1
             JOIN blocks b ON b.number = t1.block_number
             WHERE t1.block_number = $1
               AND t1.to_addr IS NOT NULL
             GROUP BY t1.to_addr
             HAVING COUNT(DISTINCT t1.from_addr) >= 5",
        )
        .bind(block_number)
        .fetch_all(pool)
        .await?;

        for row in &coordinated {
            let to_addr: Vec<u8> = row.get("to_addr");
            let addr_count: i64 = row.get("addr_count");
            let addresses: Vec<Vec<u8>> = row.get("addresses");
            let first_seen: String = row.get("first_seen");

            let addr_strs: Vec<String> = addresses
                .iter()
                .take(5)
                .map(|a| format!("0x{}", hex::encode(a)))
                .collect();

            anomalies.push(Anomaly {
                id: 0,
                anomaly_type: AnomalyType::CoordinatedActivity,
                severity: if addr_count >= 10 {
                    Severity::High
                } else {
                    Severity::Medium
                },
                detected_at: first_seen,
                entity: format!("0x{}", hex::encode(&to_addr)),
                entity_type: "contract".to_string(),
                description: format!(
                    "{} addresses interacted with same contract in block {}",
                    addr_count, block_number
                ),
                observed_value: addr_count as f64,
                baseline_value: 1.0,
                threshold: 5.0,
                evidence: vec![
                    format!("contract: 0x{}", hex::encode(&to_addr)),
                    format!("block: {}", block_number),
                    format!("addresses: {}", addr_strs.join(", ")),
                ],
                block_number: Some(block_number),
                tx_hash: None,
            });
        }
    }

    Ok(anomalies)
}

fn severity_order(s: &Severity) -> u8 {
    match s {
        Severity::Critical => 4,
        Severity::High => 3,
        Severity::Medium => 2,
        Severity::Low => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_serialization() {
        assert_eq!(serde_json::to_string(&Severity::High).unwrap(), "\"high\"");
        assert_eq!(
            serde_json::to_string(&Severity::Critical).unwrap(),
            "\"critical\""
        );
    }

    #[test]
    fn anomaly_type_serialization() {
        assert_eq!(
            serde_json::to_string(&AnomalyType::LargeTransfer).unwrap(),
            "\"large_transfer\""
        );
    }

    #[test]
    fn severity_ordering() {
        assert!(severity_order(&Severity::Critical) > severity_order(&Severity::High));
        assert!(severity_order(&Severity::High) > severity_order(&Severity::Medium));
        assert!(severity_order(&Severity::Medium) > severity_order(&Severity::Low));
    }

    #[test]
    fn anomaly_creation() {
        let anomaly = Anomaly {
            id: 1,
            anomaly_type: AnomalyType::LargeTransfer,
            severity: Severity::High,
            detected_at: "2024-01-01T00:00:00Z".to_string(),
            entity: "0x1234".to_string(),
            entity_type: "address".to_string(),
            description: "Large transfer detected".to_string(),
            observed_value: 1000000.0,
            baseline_value: 1000.0,
            threshold: 5000.0,
            evidence: vec!["tx_hash: 0xabc".to_string()],
            block_number: Some(12345),
            tx_hash: Some("0xabc".to_string()),
        };
        assert_eq!(anomaly.severity, Severity::High);
        assert_eq!(anomaly.observed_value, 1000000.0);
    }

    #[test]
    fn anomaly_serialization() {
        let anomaly = Anomaly {
            id: 1,
            anomaly_type: AnomalyType::ActivitySpike,
            severity: Severity::Medium,
            detected_at: "2024-01-01".to_string(),
            entity: "0xabcd".to_string(),
            entity_type: "block".to_string(),
            description: "test".to_string(),
            observed_value: 100.0,
            baseline_value: 50.0,
            threshold: 75.0,
            evidence: vec![],
            block_number: None,
            tx_hash: None,
        };
        let json = serde_json::to_string(&anomaly).unwrap();
        assert!(json.contains("activity_spike"));
        assert!(json.contains("medium"));
    }

    #[test]
    fn all_anomaly_types_serialize() {
        let types = vec![
            AnomalyType::LargeTransfer,
            AnomalyType::ActivitySpike,
            AnomalyType::NewWalletHighValue,
            AnomalyType::ContractInteractionSpike,
            AnomalyType::CoordinatedActivity,
        ];
        for at in types {
            let json = serde_json::to_string(&at).unwrap();
            assert!(!json.is_empty());
        }
    }
}
