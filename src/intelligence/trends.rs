use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use sqlx::Row;

#[derive(Debug, Serialize, Deserialize)]
pub struct AddressTrend {
    pub date: String,
    pub tx_count: i64,
    pub unique_contracts: i64,
    pub total_value: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ContractTrend {
    pub date: String,
    pub interaction_count: i64,
    pub unique_callers: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnomalyTrend {
    pub date: String,
    pub anomaly_count: i64,
    pub critical_count: i64,
    pub high_count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MevTrend {
    pub date: String,
    pub mev_count: i64,
    pub sandwich_count: i64,
    pub arbitrage_count: i64,
}

pub async fn get_address_trends(
    pool: &PgPool,
    address: &str,
    days: i32,
) -> Result<Vec<AddressTrend>, sqlx::Error> {
    let addr_bytes = hex::decode(address.trim_start_matches("0x"))
        .map_err(|_| sqlx::Error::Protocol("invalid address".into()))?;

    let rows = sqlx::query(
        "SELECT DATE(b.timestamp) as date,
                COUNT(*) as tx_count,
                COUNT(DISTINCT t.to_addr) as unique_contracts,
                COALESCE(SUM(t.value::numeric), 0)::text as total_value
         FROM transactions t
         JOIN blocks b ON b.number = t.block_number
         WHERE (t.from_addr = $1 OR t.to_addr = $1)
           AND b.timestamp > NOW() - INTERVAL '1 day' * $2
         GROUP BY DATE(b.timestamp)
         ORDER BY date DESC",
    )
    .bind(&addr_bytes)
    .bind(days)
    .fetch_all(pool)
    .await?;

    let mut trends = Vec::new();
    for row in &rows {
        trends.push(AddressTrend {
            date: row.get::<String, _>("date"),
            tx_count: row.get("tx_count"),
            unique_contracts: row.get("unique_contracts"),
            total_value: row.get("total_value"),
        });
    }

    Ok(trends)
}

pub async fn get_contract_trends(
    pool: &PgPool,
    address: &str,
    days: i32,
) -> Result<Vec<ContractTrend>, sqlx::Error> {
    let addr_bytes = hex::decode(address.trim_start_matches("0x"))
        .map_err(|_| sqlx::Error::Protocol("invalid address".into()))?;

    let rows = sqlx::query(
        "SELECT DATE(b.timestamp) as date,
                COUNT(*) as interaction_count,
                COUNT(DISTINCT t.from_addr) as unique_callers
         FROM transactions t
         JOIN blocks b ON b.number = t.block_number
         WHERE t.to_addr = $1
           AND b.timestamp > NOW() - INTERVAL '1 day' * $2
         GROUP BY DATE(b.timestamp)
         ORDER BY date DESC",
    )
    .bind(&addr_bytes)
    .bind(days)
    .fetch_all(pool)
    .await?;

    let mut trends = Vec::new();
    for row in &rows {
        trends.push(ContractTrend {
            date: row.get::<String, _>("date"),
            interaction_count: row.get("interaction_count"),
            unique_callers: row.get("unique_callers"),
        });
    }

    Ok(trends)
}

pub async fn get_anomaly_trends(
    pool: &PgPool,
    days: i32,
) -> Result<Vec<AnomalyTrend>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT DATE(detected_at) as date,
                COUNT(*) as anomaly_count,
                COUNT(*) FILTER (WHERE severity = 'critical') as critical_count,
                COUNT(*) FILTER (WHERE severity = 'high') as high_count
         FROM anomalies
         WHERE detected_at > NOW() - INTERVAL '1 day' * $1
         GROUP BY DATE(detected_at)
         ORDER BY date DESC",
    )
    .bind(days)
    .fetch_all(pool)
    .await?;

    let mut trends = Vec::new();
    for row in &rows {
        trends.push(AnomalyTrend {
            date: row.get::<String, _>("date"),
            anomaly_count: row.get("anomaly_count"),
            critical_count: row.get("critical_count"),
            high_count: row.get("high_count"),
        });
    }

    Ok(trends)
}

pub async fn get_mev_trends(pool: &PgPool, days: i32) -> Result<Vec<MevTrend>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT DATE(detected_at) as date,
                COUNT(*) as mev_count,
                COUNT(*) FILTER (WHERE mev_type = 'possible_sandwich') as sandwich_count,
                COUNT(*) FILTER (WHERE mev_type = 'possible_arbitrage') as arbitrage_count
         FROM mev_events
         WHERE detected_at > NOW() - INTERVAL '1 day' * $1
         GROUP BY DATE(detected_at)
         ORDER BY date DESC",
    )
    .bind(days)
    .fetch_all(pool)
    .await?;

    let mut trends = Vec::new();
    for row in &rows {
        trends.push(MevTrend {
            date: row.get::<String, _>("date"),
            mev_count: row.get("mev_count"),
            sandwich_count: row.get("sandwich_count"),
            arbitrage_count: row.get("arbitrage_count"),
        });
    }

    Ok(trends)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_trend_serialization() {
        let trend = AddressTrend {
            date: "2024-01-01".to_string(),
            tx_count: 10,
            unique_contracts: 5,
            total_value: "1000000000000000000".to_string(),
        };
        let json = serde_json::to_string(&trend).unwrap();
        assert!(json.contains("2024-01-01"));
        assert!(json.contains("10"));
    }

    #[test]
    fn anomaly_trend_serialization() {
        let trend = AnomalyTrend {
            date: "2024-01-01".to_string(),
            anomaly_count: 5,
            critical_count: 1,
            high_count: 2,
        };
        let json = serde_json::to_string(&trend).unwrap();
        assert!(json.contains("critical_count"));
    }
}
