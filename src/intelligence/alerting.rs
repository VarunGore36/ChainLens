use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use sqlx::Row;

#[derive(Debug, Serialize, Deserialize)]
pub struct Alert {
    pub id: i64,
    pub alert_type: String,
    pub severity: String,
    pub entity: String,
    pub message: String,
    pub triggered_at: String,
    pub acknowledged: bool,
}

pub async fn create_alert(
    pool: &PgPool,
    alert_type: &str,
    severity: &str,
    entity: &str,
    message: &str,
) -> Result<i64, sqlx::Error> {
    let entity_bytes =
        hex::decode(entity.trim_start_matches("0x")).unwrap_or_else(|_| entity.as_bytes().to_vec());

    let id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO alerts (alert_type, severity, entity, message)
         VALUES ($1, $2, $3, $4)
         RETURNING id",
    )
    .bind(alert_type)
    .bind(severity)
    .bind(&entity_bytes)
    .bind(message)
    .fetch_one(pool)
    .await?;

    Ok(id)
}

pub async fn get_alerts(
    pool: &PgPool,
    limit: i64,
    acknowledged: Option<bool>,
) -> Result<Vec<Alert>, sqlx::Error> {
    let rows = if let Some(ack) = acknowledged {
        sqlx::query(
            "SELECT id, alert_type, severity, encode(entity, 'hex') as entity, message, 
                    triggered_at::text, acknowledged
             FROM alerts
             WHERE acknowledged = $1
             ORDER BY triggered_at DESC
             LIMIT $2",
        )
        .bind(ack)
        .bind(limit)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query(
            "SELECT id, alert_type, severity, encode(entity, 'hex') as entity, message,
                    triggered_at::text, acknowledged
             FROM alerts
             ORDER BY triggered_at DESC
             LIMIT $1",
        )
        .bind(limit)
        .fetch_all(pool)
        .await?
    };

    let mut alerts = Vec::new();
    for row in &rows {
        let entity: String = row.get("entity");
        alerts.push(Alert {
            id: row.get("id"),
            alert_type: row.get("alert_type"),
            severity: row.get("severity"),
            entity: format!("0x{}", entity),
            message: row.get("message"),
            triggered_at: row.get("triggered_at"),
            acknowledged: row.get("acknowledged"),
        });
    }

    Ok(alerts)
}

pub async fn acknowledge_alert(pool: &PgPool, alert_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE alerts SET acknowledged = true WHERE id = $1")
        .bind(alert_id)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn check_and_create_alerts(pool: &PgPool) -> Result<Vec<Alert>, sqlx::Error> {
    let mut new_alerts = Vec::new();

    // Check for large transfers
    let large_transfers = sqlx::query(
        "SELECT t.hash, t.from_addr, t.to_addr, t.value::text, t.block_number
         FROM transactions t
         WHERE t.value::numeric > 1000000000000000000000
         AND t.block_number > (SELECT COALESCE(MAX(block_number), 0) - 100 FROM blocks)
         ORDER BY t.value::numeric DESC
         LIMIT 5",
    )
    .fetch_all(pool)
    .await?;

    for row in &large_transfers {
        let hash: Vec<u8> = row.get("hash");
        let value: String = row.get("value");
        let block: i64 = row.get("block_number");

        let alert_id = create_alert(
            pool,
            "large_transfer",
            "high",
            &format!("0x{}", hex::encode(&hash)),
            &format!("Large transfer of {} wei at block {}", value, block),
        )
        .await?;

        new_alerts.push(Alert {
            id: alert_id,
            alert_type: "large_transfer".to_string(),
            severity: "high".to_string(),
            entity: format!("0x{}", hex::encode(&hash)),
            message: format!("Large transfer of {} wei at block {}", value, block),
            triggered_at: "now".to_string(),
            acknowledged: false,
        });
    }

    Ok(new_alerts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alert_serialization() {
        let alert = Alert {
            id: 1,
            alert_type: "large_transfer".to_string(),
            severity: "high".to_string(),
            entity: "0x1234".to_string(),
            message: "test".to_string(),
            triggered_at: "2024-01-01".to_string(),
            acknowledged: false,
        };
        let json = serde_json::to_string(&alert).unwrap();
        assert!(json.contains("large_transfer"));
    }
}
