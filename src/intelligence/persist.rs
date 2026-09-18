use sqlx::PgPool;

use super::TransactionExplanation;

pub async fn persist_transaction_actions(
    pool: &PgPool,
    explanation: &TransactionExplanation,
) -> Result<(), sqlx::Error> {
    let tx_hash = hex::decode(explanation.tx_hash.trim_start_matches("0x"))
        .map_err(|_| sqlx::Error::Protocol("invalid tx hash".into()))?;

    for action in &explanation.actions {
        let from_bytes = hex::decode(action.from.trim_start_matches("0x"))
            .map_err(|_| sqlx::Error::Protocol("invalid from address".into()))?;

        let to_bytes = action
            .to
            .as_ref()
            .map(|a| hex::decode(a.trim_start_matches("0x")))
            .transpose()
            .map_err(|_| sqlx::Error::Protocol("invalid to address".into()))?;

        let token_bytes = action
            .token_address
            .as_ref()
            .map(|a| hex::decode(a.trim_start_matches("0x")))
            .transpose()
            .map_err(|_| sqlx::Error::Protocol("invalid token address".into()))?;

        let spender_bytes = action
            .spender
            .as_ref()
            .map(|a| hex::decode(a.trim_start_matches("0x")))
            .transpose()
            .map_err(|_| sqlx::Error::Protocol("invalid spender address".into()))?;

        sqlx::query(
            "INSERT INTO transaction_actions (tx_hash, action_type, from_addr, to_addr, token_address, amount, spender, description)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             ON CONFLICT DO NOTHING",
        )
        .bind(&tx_hash)
        .bind(format!("{:?}", action.action_type).to_lowercase())
        .bind(&from_bytes)
        .bind(to_bytes.as_deref())
        .bind(token_bytes.as_deref())
        .bind(action.amount.as_deref().and_then(|a| a.parse::<i64>().ok()))
        .bind(spender_bytes.as_deref())
        .bind(&action.description)
        .execute(pool)
        .await?;
    }

    Ok(())
}

pub async fn persist_address_stats(
    pool: &PgPool,
    intel: &super::address::AddressIntelligence,
) -> Result<(), sqlx::Error> {
    let addr_bytes = hex::decode(intel.address.trim_start_matches("0x"))
        .map_err(|_| sqlx::Error::Protocol("invalid address".into()))?;

    sqlx::query(
        "INSERT INTO address_stats (address, first_seen, last_active, total_transactions,
         total_eth_sent, total_eth_received, unique_contracts_called, unique_callers,
         token_transfers_sent, token_transfers_received, behavior_classifications, activity_level)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
         ON CONFLICT (address) DO UPDATE SET
         first_seen = EXCLUDED.first_seen,
         last_active = EXCLUDED.last_active,
         total_transactions = EXCLUDED.total_transactions,
         total_eth_sent = EXCLUDED.total_eth_sent,
         total_eth_received = EXCLUDED.total_eth_received,
         unique_contracts_called = EXCLUDED.unique_contracts_called,
         unique_callers = EXCLUDED.unique_callers,
         token_transfers_sent = EXCLUDED.token_transfers_sent,
         token_transfers_received = EXCLUDED.token_transfers_received,
         behavior_classifications = EXCLUDED.behavior_classifications,
         activity_level = EXCLUDED.activity_level,
         updated_at = now()",
    )
    .bind(&addr_bytes)
    .bind(&intel.first_seen)
    .bind(&intel.last_active)
    .bind(intel.total_transactions)
    .bind(&intel.total_eth_sent)
    .bind(&intel.total_eth_received)
    .bind(intel.unique_contracts_called)
    .bind(intel.unique_callers)
    .bind(intel.token_transfers_sent)
    .bind(intel.token_transfers_received)
    .bind(&intel.behavior_classifications)
    .bind(&intel.activity_level)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn persist_contract_profile(
    pool: &PgPool,
    intel: &super::contract::ContractIntelligence,
) -> Result<(), sqlx::Error> {
    let addr_bytes = hex::decode(intel.address.trim_start_matches("0x"))
        .map_err(|_| sqlx::Error::Protocol("invalid address".into()))?;

    let creator_bytes = intel
        .creator
        .as_ref()
        .map(|a| hex::decode(a.trim_start_matches("0x")))
        .transpose()
        .map_err(|_| sqlx::Error::Protocol("invalid creator".into()))?;

    let deploy_tx_bytes = intel
        .deployment_tx
        .as_ref()
        .map(|a| hex::decode(a.trim_start_matches("0x")))
        .transpose()
        .map_err(|_| sqlx::Error::Protocol("invalid deploy tx".into()))?;

    sqlx::query(
        "INSERT INTO contract_profiles (address, first_seen, deployment_block, creator, deployment_tx,
         total_transactions, unique_callers, unique_contracts_called, token_transfers, event_count,
         function_selectors, is_token, is_nft)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
         ON CONFLICT (address) DO UPDATE SET
         first_seen = EXCLUDED.first_seen,
         deployment_block = EXCLUDED.deployment_block,
         creator = EXCLUDED.creator,
         deployment_tx = EXCLUDED.deployment_tx,
         total_transactions = EXCLUDED.total_transactions,
         unique_callers = EXCLUDED.unique_callers,
         unique_contracts_called = EXCLUDED.unique_contracts_called,
         token_transfers = EXCLUDED.token_transfers,
         event_count = EXCLUDED.event_count,
         function_selectors = EXCLUDED.function_selectors,
         is_token = EXCLUDED.is_token,
         is_nft = EXCLUDED.is_nft,
         updated_at = now()",
    )
    .bind(&addr_bytes)
    .bind(&intel.first_seen)
    .bind(intel.deployment_block)
    .bind(creator_bytes.as_deref())
    .bind(deploy_tx_bytes.as_deref())
    .bind(intel.total_transactions)
    .bind(intel.unique_callers)
    .bind(intel.unique_contracts_called)
    .bind(intel.token_transfers)
    .bind(intel.event_count)
    .bind(&intel.function_selectors)
    .bind(intel.is_token)
    .bind(intel.is_nft)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn persist_anomaly(
    pool: &PgPool,
    anomaly: &super::anomaly::Anomaly,
) -> Result<i64, sqlx::Error> {
    let entity_bytes = hex::decode(anomaly.entity.trim_start_matches("0x"))
        .unwrap_or_else(|_| anomaly.entity.as_bytes().to_vec());

    let block_num = anomaly.block_number;
    let tx_hash = anomaly
        .tx_hash
        .as_ref()
        .map(|h| hex::decode(h.trim_start_matches("0x")).unwrap_or_default());

    let evidence_json =
        serde_json::to_value(&anomaly.evidence).unwrap_or(serde_json::Value::Array(vec![]));

    let id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO anomalies (anomaly_type, severity, detected_at, entity, entity_type,
         description, observed_value, baseline_value, threshold, evidence, block_number, tx_hash)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
         ON CONFLICT DO NOTHING
         RETURNING id",
    )
    .bind(format!("{:?}", anomaly.anomaly_type).to_lowercase())
    .bind(format!("{:?}", anomaly.severity).to_lowercase())
    .bind(&anomaly.detected_at)
    .bind(&entity_bytes)
    .bind(&anomaly.entity_type)
    .bind(&anomaly.description)
    .bind(anomaly.observed_value)
    .bind(anomaly.baseline_value)
    .bind(anomaly.threshold)
    .bind(evidence_json)
    .bind(block_num)
    .bind(tx_hash.as_deref())
    .fetch_optional(pool)
    .await?;

    Ok(id.unwrap_or(0))
}

pub async fn persist_mev_event(
    pool: &PgPool,
    event: &super::mev::MevEvent,
) -> Result<i64, sqlx::Error> {
    let addr_bytes: Vec<Vec<u8>> = event
        .involved_addresses
        .iter()
        .filter_map(|a| hex::decode(a.trim_start_matches("0x")).ok())
        .collect();

    let tx_bytes: Vec<Vec<u8>> = event
        .involved_transactions
        .iter()
        .filter_map(|a| hex::decode(a.trim_start_matches("0x")).ok())
        .collect();

    let id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO mev_events (mev_type, severity, block_number, detected_at, description,
         involved_addresses, involved_transactions, confidence)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         ON CONFLICT DO NOTHING
         RETURNING id",
    )
    .bind(format!("{:?}", event.mev_type).to_lowercase())
    .bind(&event.severity)
    .bind(event.block_number)
    .bind(&event.timestamp)
    .bind(&event.description)
    .bind(&addr_bytes)
    .bind(&tx_bytes)
    .bind(event.confidence)
    .fetch_optional(pool)
    .await?;

    Ok(id.unwrap_or(0))
}

pub async fn persist_block_analytics(
    pool: &PgPool,
    analytics: &super::mev::BlockAnalytics,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO block_analytics (block_number, total_priority_fees, avg_priority_fee,
         max_priority_fee, mev_event_count, unusual_tx_count)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (block_number) DO UPDATE SET
         total_priority_fees = EXCLUDED.total_priority_fees,
         avg_priority_fee = EXCLUDED.avg_priority_fee,
         max_priority_fee = EXCLUDED.max_priority_fee,
         mev_event_count = EXCLUDED.mev_event_count,
         unusual_tx_count = EXCLUDED.unusual_tx_count",
    )
    .bind(analytics.block_number)
    .bind(&analytics.total_priority_fees)
    .bind(&analytics.avg_priority_fee)
    .bind(&analytics.max_priority_fee)
    .bind(i32::try_from(analytics.mev_events.len()).unwrap_or(i32::MAX))
    .bind(i32::try_from(analytics.unusual_transactions.len()).unwrap_or(i32::MAX))
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn persist_address_relationship(
    pool: &PgPool,
    from: &str,
    to: &str,
    relationship: &str,
    count: i64,
    total_value: Option<&str>,
) -> Result<(), sqlx::Error> {
    let from_bytes = hex::decode(from.trim_start_matches("0x"))
        .map_err(|_| sqlx::Error::Protocol("invalid from".into()))?;
    let to_bytes = hex::decode(to.trim_start_matches("0x"))
        .map_err(|_| sqlx::Error::Protocol("invalid to".into()))?;

    sqlx::query(
        "INSERT INTO address_relationships (from_addr, to_addr, relationship_type, interaction_count, total_value)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (from_addr, to_addr, relationship_type) DO UPDATE SET
         interaction_count = EXCLUDED.interaction_count,
         total_value = EXCLUDED.total_value,
         last_seen = now()",
    )
    .bind(&from_bytes)
    .bind(&to_bytes)
    .bind(relationship)
    .bind(count)
    .bind(total_value)
    .execute(pool)
    .await?;

    Ok(())
}
