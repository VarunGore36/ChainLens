use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use sqlx::Row;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MevType {
    PossibleSandwich,
    PossibleArbitrage,
    PossibleLiquidation,
    PriorityFeeAnomaly,
    RepeatedProtocolInteraction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MevEvent {
    pub id: i64,
    pub mev_type: MevType,
    pub severity: String,
    pub block_number: i64,
    pub timestamp: String,
    pub description: String,
    pub involved_addresses: Vec<String>,
    pub involved_transactions: Vec<String>,
    pub estimated_value: Option<String>,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockAnalytics {
    pub block_number: i64,
    pub timestamp: String,
    pub tx_count: i64,
    pub gas_used: i64,
    pub gas_limit: i64,
    pub base_fee: Option<String>,
    pub total_priority_fees: String,
    pub avg_priority_fee: String,
    pub max_priority_fee: String,
    pub mev_events: Vec<MevEvent>,
    pub unusual_transactions: Vec<String>,
}

pub async fn analyze_block(
    pool: &PgPool,
    block_number: i64,
) -> Result<BlockAnalytics, sqlx::Error> {
    let block = sqlx::query(
        "SELECT number, timestamp::text, tx_count, gas_used, gas_limit,
                base_fee::text
         FROM blocks WHERE number = $1",
    )
    .bind(block_number)
    .fetch_optional(pool)
    .await?;

    let block = match block {
        Some(b) => b,
        None => return Err(sqlx::Error::RowNotFound),
    };

    let timestamp: String = block.get("timestamp");
    let tx_count: i64 = block.get("tx_count");
    let gas_used: i64 = block.get("gas_used");
    let gas_limit: i64 = block.get("gas_limit");
    let base_fee: Option<String> = block.get("base_fee");

    let priority_fees = sqlx::query(
        "SELECT COALESCE(SUM(max_priority_fee_per_gas::numeric), 0)::text as total,
                COALESCE(AVG(max_priority_fee_per_gas::numeric), 0)::text as avg,
                COALESCE(MAX(max_priority_fee_per_gas::numeric), 0)::text as max
         FROM transactions
         WHERE block_number = $1 AND max_priority_fee_per_gas IS NOT NULL",
    )
    .bind(block_number)
    .fetch_one(pool)
    .await?;

    let total_priority_fees: String = priority_fees.get("total");
    let avg_priority_fee: String = priority_fees.get("avg");
    let max_priority_fee: String = priority_fees.get("max");

    let mut mev_events = Vec::new();

    mev_events.extend(detect_sandwich_patterns(pool, block_number, &timestamp).await?);
    mev_events.extend(detect_arbitrage_patterns(pool, block_number, &timestamp).await?);
    mev_events.extend(detect_priority_fee_anomalies(pool, block_number, &timestamp).await?);
    mev_events.extend(detect_repeated_protocol_interactions(pool, block_number, &timestamp).await?);

    let unusual_txs = detect_unusual_transactions(pool, block_number).await?;

    Ok(BlockAnalytics {
        block_number,
        timestamp,
        tx_count,
        gas_used,
        gas_limit,
        base_fee,
        total_priority_fees,
        avg_priority_fee,
        max_priority_fee,
        mev_events,
        unusual_transactions: unusual_txs,
    })
}

async fn detect_sandwich_patterns(
    pool: &PgPool,
    block_number: i64,
    timestamp: &str,
) -> Result<Vec<MevEvent>, sqlx::Error> {
    let mut events = Vec::new();

    let txs = sqlx::query(
        "SELECT hash, from_addr, to_addr, value::text, tx_index
         FROM transactions
         WHERE block_number = $1
         ORDER BY tx_index",
    )
    .bind(block_number)
    .fetch_all(pool)
    .await?;

    if txs.len() < 3 {
        return Ok(events);
    }

    for i in 1..txs.len() - 1 {
        let prev = &txs[i - 1];
        let curr = &txs[i];
        let next = &txs[i + 1];

        let prev_from: Vec<u8> = prev.get("from_addr");
        let next_from: Vec<u8> = next.get("from_addr");

        let prev_to: Option<Vec<u8>> = prev.get("to_addr");
        let next_to: Option<Vec<u8>> = next.get("to_addr");

        if prev_from == next_from && prev_to == next_to {
            let hash: Vec<u8> = curr.get("hash");
            let prev_hash: Vec<u8> = prev.get("hash");
            let next_hash: Vec<u8> = next.get("hash");

            events.push(MevEvent {
                id: 0,
                mev_type: MevType::PossibleSandwich,
                severity: "high".to_string(),
                block_number,
                timestamp: timestamp.to_string(),
                description: format!(
                    "Possible sandwich attack detected: same address (0x{}) sent transactions before and after a swap",
                    hex::encode(&prev_from)
                ),
                involved_addresses: vec![
                    format!("0x{}", hex::encode(&prev_from)),
                ],
                involved_transactions: vec![
                    format!("0x{}", hex::encode(&prev_hash)),
                    format!("0x{}", hex::encode(&hash)),
                    format!("0x{}", hex::encode(&next_hash)),
                ],
                estimated_value: None,
                confidence: 0.6,
            });
        }
    }

    Ok(events)
}

async fn detect_arbitrage_patterns(
    pool: &PgPool,
    block_number: i64,
    timestamp: &str,
) -> Result<Vec<MevEvent>, sqlx::Error> {
    let mut events = Vec::new();

    let same_addr_txs = sqlx::query(
        "SELECT from_addr, COUNT(*) as tx_count,
                array_agg(hash) as hashes
         FROM transactions
         WHERE block_number = $1
         GROUP BY from_addr
         HAVING COUNT(*) >= 3",
    )
    .bind(block_number)
    .fetch_all(pool)
    .await?;

    for row in &same_addr_txs {
        let from_addr: Vec<u8> = row.get("from_addr");
        let tx_count: i64 = row.get("tx_count");
        let hashes: Vec<Vec<u8>> = row.get("hashes");

        let hash_strs: Vec<String> = hashes
            .iter()
            .map(|h| format!("0x{}", hex::encode(h)))
            .collect();

        events.push(MevEvent {
            id: 0,
            mev_type: MevType::PossibleArbitrage,
            severity: "medium".to_string(),
            block_number,
            timestamp: timestamp.to_string(),
            description: format!(
                "Possible arbitrage: address 0x{} made {} transactions in same block",
                hex::encode(&from_addr),
                tx_count
            ),
            involved_addresses: vec![format!("0x{}", hex::encode(&from_addr))],
            involved_transactions: hash_strs,
            estimated_value: None,
            confidence: 0.4,
        });
    }

    Ok(events)
}

async fn detect_priority_fee_anomalies(
    pool: &PgPool,
    block_number: i64,
    timestamp: &str,
) -> Result<Vec<MevEvent>, sqlx::Error> {
    let mut events = Vec::new();

    let avg_fee = sqlx::query(
        "SELECT COALESCE(AVG(max_priority_fee_per_gas::numeric), 0)::text as avg
         FROM transactions
         WHERE block_number = $1 AND max_priority_fee_per_gas IS NOT NULL",
    )
    .bind(block_number)
    .fetch_one(pool)
    .await?;

    let avg_str: String = avg_fee.get("avg");
    let avg: f64 = avg_str.parse().unwrap_or(0.0);

    if avg > 0.0 {
        let high_fee_txs = sqlx::query(
            "SELECT hash, from_addr, max_priority_fee_per_gas::text
             FROM transactions
             WHERE block_number = $1
               AND max_priority_fee_per_gas::numeric > $2
             ORDER BY max_priority_fee_per_gas::numeric DESC
             LIMIT 5",
        )
        .bind(block_number)
        .bind(format!("{}", avg * 5.0))
        .fetch_all(pool)
        .await?;

        for row in &high_fee_txs {
            let hash: Vec<u8> = row.get("hash");
            let from: Vec<u8> = row.get("from_addr");
            let fee: String = row.get("max_priority_fee_per_gas");

            events.push(MevEvent {
                id: 0,
                mev_type: MevType::PriorityFeeAnomaly,
                severity: "low".to_string(),
                block_number,
                timestamp: timestamp.to_string(),
                description: format!(
                    "Transaction paid {} priority fee, significantly above average ({})",
                    fee, avg_str
                ),
                involved_addresses: vec![format!("0x{}", hex::encode(&from))],
                involved_transactions: vec![format!("0x{}", hex::encode(&hash))],
                estimated_value: None,
                confidence: 0.5,
            });
        }
    }

    Ok(events)
}

async fn detect_repeated_protocol_interactions(
    pool: &PgPool,
    block_number: i64,
    timestamp: &str,
) -> Result<Vec<MevEvent>, sqlx::Error> {
    let mut events = Vec::new();

    let repeated = sqlx::query(
        "SELECT to_addr, COUNT(*) as interaction_count
         FROM transactions
         WHERE block_number = $1 AND to_addr IS NOT NULL
         GROUP BY to_addr
         HAVING COUNT(*) >= 5",
    )
    .bind(block_number)
    .fetch_all(pool)
    .await?;

    for row in &repeated {
        let to_addr: Vec<u8> = row.get("to_addr");
        let count: i64 = row.get("interaction_count");

        events.push(MevEvent {
            id: 0,
            mev_type: MevType::RepeatedProtocolInteraction,
            severity: "low".to_string(),
            block_number,
            timestamp: timestamp.to_string(),
            description: format!(
                "Contract 0x{} was called {} times in single block",
                hex::encode(&to_addr),
                count
            ),
            involved_addresses: vec![format!("0x{}", hex::encode(&to_addr))],
            involved_transactions: vec![],
            estimated_value: None,
            confidence: 0.3,
        });
    }

    Ok(events)
}

async fn detect_unusual_transactions(
    pool: &PgPool,
    block_number: i64,
) -> Result<Vec<String>, sqlx::Error> {
    let mut unusual = Vec::new();

    let large_value = sqlx::query(
        "SELECT hash, value::text
         FROM transactions
         WHERE block_number = $1
           AND value::numeric > 100000000000000000000
         ORDER BY value::numeric DESC
         LIMIT 5",
    )
    .bind(block_number)
    .fetch_all(pool)
    .await?;

    for row in &large_value {
        let hash: Vec<u8> = row.get("hash");
        unusual.push(format!("0x{}", hex::encode(&hash)));
    }

    Ok(unusual)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mev_type_serialization() {
        assert_eq!(
            serde_json::to_string(&MevType::PossibleSandwich).unwrap(),
            "\"possible_sandwich\""
        );
        assert_eq!(
            serde_json::to_string(&MevType::PossibleArbitrage).unwrap(),
            "\"possible_arbitrage\""
        );
    }

    #[test]
    fn block_analytics_structure() {
        let analytics = BlockAnalytics {
            block_number: 12345,
            timestamp: "2024-01-01".to_string(),
            tx_count: 100,
            gas_used: 15000000,
            gas_limit: 30000000,
            base_fee: Some("1000000000".to_string()),
            total_priority_fees: "5000000000000".to_string(),
            avg_priority_fee: "50000000000".to_string(),
            max_priority_fee: "200000000000".to_string(),
            mev_events: vec![],
            unusual_transactions: vec![],
        };
        assert_eq!(analytics.block_number, 12345);
        assert_eq!(analytics.tx_count, 100);
    }

    #[test]
    fn all_mev_types_serialize() {
        let types = vec![
            MevType::PossibleSandwich,
            MevType::PossibleArbitrage,
            MevType::PossibleLiquidation,
            MevType::PriorityFeeAnomaly,
            MevType::RepeatedProtocolInteraction,
        ];
        for mt in types {
            let json = serde_json::to_string(&mt).unwrap();
            assert!(!json.is_empty());
        }
    }

    #[test]
    fn mev_event_creation() {
        let event = MevEvent {
            id: 1,
            mev_type: MevType::PossibleSandwich,
            severity: "high".to_string(),
            block_number: 18000000,
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            description: "Possible sandwich detected".to_string(),
            involved_addresses: vec!["0x1111".to_string()],
            involved_transactions: vec!["0xabc".to_string(), "0xdef".to_string()],
            estimated_value: Some("1000000000000000000".to_string()),
            confidence: 0.75,
        };
        assert_eq!(event.mev_type, MevType::PossibleSandwich);
        assert_eq!(event.confidence, 0.75);
    }

    #[test]
    fn mev_event_serialization() {
        let event = MevEvent {
            id: 1,
            mev_type: MevType::PossibleArbitrage,
            severity: "medium".to_string(),
            block_number: 12345,
            timestamp: "2024-01-01".to_string(),
            description: "test".to_string(),
            involved_addresses: vec![],
            involved_transactions: vec![],
            estimated_value: None,
            confidence: 0.5,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("possible_arbitrage"));
        assert!(json.contains("medium"));
    }

    #[test]
    fn block_analytics_serialization() {
        let analytics = BlockAnalytics {
            block_number: 999,
            timestamp: "2024-01-01".to_string(),
            tx_count: 50,
            gas_used: 10000000,
            gas_limit: 30000000,
            base_fee: None,
            total_priority_fees: "0".to_string(),
            avg_priority_fee: "0".to_string(),
            max_priority_fee: "0".to_string(),
            mev_events: vec![],
            unusual_transactions: vec![],
        };
        let json = serde_json::to_string(&analytics).unwrap();
        assert!(json.contains("999"));
        assert!(json.contains("50"));
    }

    #[test]
    fn mev_event_with_all_types() {
        let types = vec![
            MevType::PossibleSandwich,
            MevType::PossibleArbitrage,
            MevType::PossibleLiquidation,
            MevType::PriorityFeeAnomaly,
            MevType::RepeatedProtocolInteraction,
        ];
        for mt in types {
            let event = MevEvent {
                id: 1,
                mev_type: mt,
                severity: "medium".to_string(),
                block_number: 100,
                timestamp: "2024-01-01".to_string(),
                description: "test".to_string(),
                involved_addresses: vec![],
                involved_transactions: vec![],
                estimated_value: None,
                confidence: 0.5,
            };
            let json = serde_json::to_string(&event).unwrap();
            assert!(!json.is_empty());
        }
    }

    #[test]
    fn mev_event_with_multiple_addresses() {
        let event = MevEvent {
            id: 1,
            mev_type: MevType::PossibleSandwich,
            severity: "high".to_string(),
            block_number: 18000000,
            timestamp: "2024-01-01".to_string(),
            description: "Sandwich attack".to_string(),
            involved_addresses: vec![
                "0x1111".to_string(),
                "0x2222".to_string(),
                "0x3333".to_string(),
            ],
            involved_transactions: vec![
                "0xaaa".to_string(),
                "0xbbb".to_string(),
                "0xccc".to_string(),
            ],
            estimated_value: Some("5000000000000000000".to_string()),
            confidence: 0.85,
        };
        assert_eq!(event.involved_addresses.len(), 3);
        assert_eq!(event.involved_transactions.len(), 3);
        assert_eq!(event.confidence, 0.85);
    }

    #[test]
    fn block_analytics_with_events() {
        let analytics = BlockAnalytics {
            block_number: 18000000,
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            tx_count: 200,
            gas_used: 30000000,
            gas_limit: 30000000,
            base_fee: Some("20000000000".to_string()),
            total_priority_fees: "5000000000000".to_string(),
            avg_priority_fee: "25000000000".to_string(),
            max_priority_fee: "100000000000".to_string(),
            mev_events: vec![MevEvent {
                id: 1,
                mev_type: MevType::PossibleArbitrage,
                severity: "medium".to_string(),
                block_number: 18000000,
                timestamp: "2024-01-01T00:00:00Z".to_string(),
                description: "Arbitrage detected".to_string(),
                involved_addresses: vec!["0x1111".to_string()],
                involved_transactions: vec!["0xabc".to_string()],
                estimated_value: Some("1000000000000000000".to_string()),
                confidence: 0.6,
            }],
            unusual_transactions: vec!["0xdef".to_string()],
        };
        assert_eq!(analytics.mev_events.len(), 1);
        assert_eq!(analytics.unusual_transactions.len(), 1);
        assert_eq!(analytics.mev_events[0].mev_type, MevType::PossibleArbitrage);
    }

    #[test]
    fn block_analytics_with_base_fee() {
        let analytics = BlockAnalytics {
            block_number: 18000000,
            timestamp: "2024-01-01".to_string(),
            tx_count: 100,
            gas_used: 15000000,
            gas_limit: 30000000,
            base_fee: Some("20000000000".to_string()),
            total_priority_fees: "1000000000000".to_string(),
            avg_priority_fee: "10000000000".to_string(),
            max_priority_fee: "50000000000".to_string(),
            mev_events: vec![],
            unusual_transactions: vec![],
        };
        assert_eq!(analytics.base_fee.unwrap(), "20000000000");
        assert_eq!(analytics.tx_count, 100);
    }

    #[test]
    fn mev_confidence_bounds() {
        let event_low = MevEvent {
            id: 1,
            mev_type: MevType::PossibleSandwich,
            severity: "low".to_string(),
            block_number: 100,
            timestamp: "2024-01-01".to_string(),
            description: "test".to_string(),
            involved_addresses: vec![],
            involved_transactions: vec![],
            estimated_value: None,
            confidence: 0.0,
        };
        let event_high = MevEvent {
            id: 2,
            mev_type: MevType::PossibleSandwich,
            severity: "high".to_string(),
            block_number: 100,
            timestamp: "2024-01-01".to_string(),
            description: "test".to_string(),
            involved_addresses: vec![],
            involved_transactions: vec![],
            estimated_value: None,
            confidence: 1.0,
        };
        assert_eq!(event_low.confidence, 0.0);
        assert_eq!(event_high.confidence, 1.0);
    }
}
