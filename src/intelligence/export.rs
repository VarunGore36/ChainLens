use serde::Serialize;
use sqlx::PgPool;
use sqlx::Row;

#[derive(Debug, Serialize)]
pub struct ExportRow {
    pub block_number: i64,
    pub timestamp: String,
    pub tx_hash: String,
    pub from: String,
    pub to: Option<String>,
    pub value: String,
    pub action_type: String,
    pub description: String,
}

pub async fn export_address_transactions_csv(
    pool: &PgPool,
    address: &str,
    limit: i64,
) -> Result<String, sqlx::Error> {
    let addr_bytes = hex::decode(address.trim_start_matches("0x"))
        .map_err(|_| sqlx::Error::Protocol("invalid address".into()))?;

    let rows = sqlx::query(
        "SELECT t.block_number, b.timestamp::text, t.hash, t.from_addr, t.to_addr, t.value::text,
                COALESCE(ta.action_type, 'unknown') as action_type,
                COALESCE(ta.description, '') as description
         FROM transactions t
         JOIN blocks b ON b.number = t.block_number
         LEFT JOIN transaction_actions ta ON ta.tx_hash = t.hash
         WHERE t.from_addr = $1 OR t.to_addr = $1
         ORDER BY t.block_number DESC
         LIMIT $2",
    )
    .bind(&addr_bytes)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let mut wtr = csv::Writer::from_writer(vec![]);

    for row in &rows {
        let tx_hash: Vec<u8> = row.get("hash");
        let from: Vec<u8> = row.get("from_addr");
        let to: Option<Vec<u8>> = row.get("to_addr");

        let record = ExportRow {
            block_number: row.get("block_number"),
            timestamp: row.get("timestamp"),
            tx_hash: format!("0x{}", hex::encode(&tx_hash)),
            from: format!("0x{}", hex::encode(&from)),
            to: to.map(|t| format!("0x{}", hex::encode(&t))),
            value: row.get("value"),
            action_type: row.get("action_type"),
            description: row.get("description"),
        };
        wtr.serialize(&record)
            .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
    }

    let data = wtr
        .into_inner()
        .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
    String::from_utf8(data).map_err(|e| sqlx::Error::Protocol(e.to_string()))
}

pub async fn export_address_transactions_json(
    pool: &PgPool,
    address: &str,
    limit: i64,
) -> Result<Vec<ExportRow>, sqlx::Error> {
    let addr_bytes = hex::decode(address.trim_start_matches("0x"))
        .map_err(|_| sqlx::Error::Protocol("invalid address".into()))?;

    let rows = sqlx::query(
        "SELECT t.block_number, b.timestamp::text, t.hash, t.from_addr, t.to_addr, t.value::text,
                COALESCE(ta.action_type, 'unknown') as action_type,
                COALESCE(ta.description, '') as description
         FROM transactions t
         JOIN blocks b ON b.number = t.block_number
         LEFT JOIN transaction_actions ta ON ta.tx_hash = t.hash
         WHERE t.from_addr = $1 OR t.to_addr = $1
         ORDER BY t.block_number DESC
         LIMIT $2",
    )
    .bind(&addr_bytes)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let mut result = Vec::new();

    for row in &rows {
        let tx_hash: Vec<u8> = row.get("hash");
        let from: Vec<u8> = row.get("from_addr");
        let to: Option<Vec<u8>> = row.get("to_addr");

        result.push(ExportRow {
            block_number: row.get("block_number"),
            timestamp: row.get("timestamp"),
            tx_hash: format!("0x{}", hex::encode(&tx_hash)),
            from: format!("0x{}", hex::encode(&from)),
            to: to.map(|t| format!("0x{}", hex::encode(&t))),
            value: row.get("value"),
            action_type: row.get("action_type"),
            description: row.get("description"),
        });
    }

    Ok(result)
}

pub async fn export_anomalies_csv(pool: &PgPool) -> Result<String, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, anomaly_type, severity, detected_at::text, encode(entity, 'hex') as entity,
                entity_type, description, observed_value, baseline_value, threshold
         FROM anomalies
         ORDER BY detected_at DESC
         LIMIT 1000",
    )
    .fetch_all(pool)
    .await?;

    let mut wtr = csv::Writer::from_writer(vec![]);

    #[derive(Serialize)]
    struct AnomalyExport {
        id: i64,
        anomaly_type: String,
        severity: String,
        detected_at: String,
        entity: String,
        entity_type: String,
        description: String,
        observed_value: f64,
        baseline_value: f64,
        threshold: f64,
    }

    for row in &rows {
        let entity: String = row.get("entity");
        let record = AnomalyExport {
            id: row.get("id"),
            anomaly_type: row.get("anomaly_type"),
            severity: row.get("severity"),
            detected_at: row.get("detected_at"),
            entity: format!("0x{}", entity),
            entity_type: row.get("entity_type"),
            description: row.get("description"),
            observed_value: row.get("observed_value"),
            baseline_value: row.get("baseline_value"),
            threshold: row.get("threshold"),
        };
        wtr.serialize(&record)
            .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
    }

    let data = wtr
        .into_inner()
        .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
    String::from_utf8(data).map_err(|e| sqlx::Error::Protocol(e.to_string()))
}

pub async fn export_mev_events_csv(
    pool: &PgPool,
    block_number: Option<i64>,
) -> Result<String, sqlx::Error> {
    let rows = if let Some(bn) = block_number {
        sqlx::query(
            "SELECT id, mev_type, severity, block_number, detected_at::text, description, confidence
             FROM mev_events
             WHERE block_number = $1
             ORDER BY block_number DESC",
        )
        .bind(bn)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query(
            "SELECT id, mev_type, severity, block_number, detected_at::text, description, confidence
             FROM mev_events
             ORDER BY block_number DESC
             LIMIT 1000",
        )
        .fetch_all(pool)
        .await?
    };

    let mut wtr = csv::Writer::from_writer(vec![]);

    #[derive(Serialize)]
    struct MevExport {
        id: i64,
        mev_type: String,
        severity: String,
        block_number: i64,
        detected_at: String,
        description: String,
        confidence: f64,
    }

    for row in &rows {
        let record = MevExport {
            id: row.get("id"),
            mev_type: row.get("mev_type"),
            severity: row.get("severity"),
            block_number: row.get("block_number"),
            detected_at: row.get("detected_at"),
            description: row.get("description"),
            confidence: row.get("confidence"),
        };
        wtr.serialize(&record)
            .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
    }

    let data = wtr
        .into_inner()
        .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
    String::from_utf8(data).map_err(|e| sqlx::Error::Protocol(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_row_serializes() {
        let row = ExportRow {
            block_number: 12345,
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            tx_hash: "0xabc".to_string(),
            from: "0x1111".to_string(),
            to: Some("0x2222".to_string()),
            value: "1000000000000000000".to_string(),
            action_type: "eth_transfer".to_string(),
            description: "Transferred 1 ETH".to_string(),
        };
        let json = serde_json::to_string(&row).unwrap();
        assert!(json.contains("0xabc"));
        assert!(json.contains("eth_transfer"));
    }
}
