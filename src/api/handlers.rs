use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use serde::Deserialize;
use sqlx::{PgPool, Row};

use super::dto::*;
use crate::intelligence;

fn to_hex(bytes: &[u8]) -> String {
    hex::encode(bytes)
}

fn from_hex(s: &str) -> Result<Vec<u8>, String> {
    hex::decode(s).map_err(|e| format!("invalid hex: {e}"))
}

pub async fn get_block(
    State(pool): State<PgPool>,
    Path(number_or_hash): Path<String>,
) -> Result<Json<BlockResponse>, (StatusCode, Json<ErrorResponse>)> {
    let row = if let Ok(number) = number_or_hash.parse::<i64>() {
        sqlx::query(
            "SELECT number, hash, parent_hash, timestamp, miner, gas_used, gas_limit, base_fee::text, tx_count
             FROM blocks WHERE number = $1"
        )
        .bind(number)
        .fetch_optional(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })))?
    } else {
        let hash_bytes = from_hex(&number_or_hash)
            .map_err(|e| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })))?;
        sqlx::query(
            "SELECT number, hash, parent_hash, timestamp, miner, gas_used, gas_limit, base_fee::text, tx_count
             FROM blocks WHERE hash = $1"
        )
        .bind(hash_bytes)
        .fetch_optional(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })))?
    };

    match row {
        Some(r) => Ok(Json(BlockResponse {
            number: r.get("number"),
            hash: to_hex(&r.get::<Vec<u8>, _>("hash")),
            parent_hash: to_hex(&r.get::<Vec<u8>, _>("parent_hash")),
            timestamp: r.get::<String, _>("timestamp"),
            miner: to_hex(&r.get::<Vec<u8>, _>("miner")),
            gas_used: r.get("gas_used"),
            gas_limit: r.get("gas_limit"),
            base_fee: r.get("base_fee"),
            tx_count: r.get("tx_count"),
        })),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "block not found".to_string(),
            }),
        )),
    }
}

pub async fn get_transaction(
    State(pool): State<PgPool>,
    Path(hash): Path<String>,
) -> Result<Json<TransactionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let hash_bytes =
        from_hex(&hash).map_err(|e| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })))?;

    let row = sqlx::query(
        "SELECT hash, block_number, tx_index, from_addr, to_addr, value::text, nonce, tx_type, gas_limit, gas_price::text, status, gas_used
         FROM transactions WHERE hash = $1"
    )
    .bind(hash_bytes)
    .fetch_optional(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })))?;

    match row {
        Some(r) => Ok(Json(TransactionResponse {
            hash: to_hex(&r.get::<Vec<u8>, _>("hash")),
            block_number: r.get("block_number"),
            tx_index: r.get("tx_index"),
            from_addr: to_hex(&r.get::<Vec<u8>, _>("from_addr")),
            to_addr: r.get::<Option<Vec<u8>>, _>("to_addr").map(|b| to_hex(&b)),
            value: r.get("value"),
            nonce: r.get("nonce"),
            tx_type: r.get("tx_type"),
            gas_limit: r.get("gas_limit"),
            gas_price: r.get("gas_price"),
            status: r.get("status"),
            gas_used: r.get("gas_used"),
        })),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "transaction not found".to_string(),
            }),
        )),
    }
}

#[derive(Debug, Deserialize)]
pub struct AddressQuery {
    pub limit: Option<i64>,
    pub before_block: Option<i64>,
}

pub async fn get_address_transactions(
    State(pool): State<PgPool>,
    Path(address): Path<String>,
    Query(query): Query<AddressQuery>,
) -> Result<Json<Vec<AddressTransactionResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let address_bytes = from_hex(&address)
        .map_err(|e| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })))?;

    let limit = query.limit.unwrap_or(50).min(500);
    let before_block = query.before_block.unwrap_or(i64::MAX);

    let rows = sqlx::query(
        "SELECT tx_hash, block_number, tx_index, direction
         FROM address_transactions
         WHERE address = $1 AND block_number < $2
         ORDER BY block_number DESC, tx_index DESC
         LIMIT $3",
    )
    .bind(address_bytes)
    .bind(before_block)
    .bind(limit)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    let result: Vec<AddressTransactionResponse> = rows
        .iter()
        .map(|r| AddressTransactionResponse {
            tx_hash: to_hex(&r.get::<Vec<u8>, _>("tx_hash")),
            block_number: r.get("block_number"),
            tx_index: r.get("tx_index"),
            direction: r.get("direction"),
        })
        .collect();

    Ok(Json(result))
}

#[derive(Debug, Deserialize)]
pub struct EventQuery {
    pub topic0: Option<String>,
    pub from_block: Option<i64>,
    pub to_block: Option<i64>,
    pub limit: Option<i64>,
}

pub async fn get_contract_events(
    State(pool): State<PgPool>,
    Path(address): Path<String>,
    Query(query): Query<EventQuery>,
) -> Result<Json<Vec<LogResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let address_bytes = from_hex(&address)
        .map_err(|e| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })))?;

    let limit = query.limit.unwrap_or(50).min(500);
    let from_block = query.from_block.unwrap_or(0);
    let to_block = query.to_block.unwrap_or(i64::MAX);

    let topic0_bytes = query
        .topic0
        .map(|t| from_hex(&t))
        .transpose()
        .map_err(|e| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })))?;

    let rows = sqlx::query(
        "SELECT block_number, log_index, tx_hash, address, topic0, topic1, topic2, topic3
         FROM logs
         WHERE address = $1
           AND block_number >= $2
           AND block_number <= $3
           AND ($4::bytea IS NULL OR topic0 = $4)
         ORDER BY block_number DESC, log_index DESC
         LIMIT $5",
    )
    .bind(address_bytes)
    .bind(from_block)
    .bind(to_block)
    .bind(topic0_bytes)
    .bind(limit)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    let result: Vec<LogResponse> = rows
        .iter()
        .map(|r| LogResponse {
            block_number: r.get("block_number"),
            log_index: r.get("log_index"),
            tx_hash: to_hex(&r.get::<Vec<u8>, _>("tx_hash")),
            address: to_hex(&r.get::<Vec<u8>, _>("address")),
            topic0: r.get::<Option<Vec<u8>>, _>("topic0").map(|b| to_hex(&b)),
            topic1: r.get::<Option<Vec<u8>>, _>("topic1").map(|b| to_hex(&b)),
            topic2: r.get::<Option<Vec<u8>>, _>("topic2").map(|b| to_hex(&b)),
            topic3: r.get::<Option<Vec<u8>>, _>("topic3").map(|b| to_hex(&b)),
        })
        .collect();

    Ok(Json(result))
}

pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

pub async fn status(
    State(pool): State<PgPool>,
) -> Result<Json<StatusResponse>, (StatusCode, Json<ErrorResponse>)> {
    let row =
        sqlx::query("SELECT last_indexed_number, finalized_number FROM indexer_state WHERE id = 1")
            .fetch_optional(&pool)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: e.to_string(),
                    }),
                )
            })?;

    let (last_indexed, finalized) = match row {
        Some(r) => (
            r.get::<Option<i64>, _>("last_indexed_number"),
            r.get::<Option<i64>, _>("finalized_number"),
        ),
        None => (None, None),
    };

    let lag = match (last_indexed, finalized) {
        (Some(l), Some(f)) => Some(l - f),
        (Some(l), None) => Some(l),
        _ => None,
    };

    Ok(Json(StatusResponse {
        last_indexed_number: last_indexed,
        finalized_number: finalized,
        lag,
    }))
}

pub async fn explain_transaction(
    State(pool): State<PgPool>,
    Path(hash): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    match intelligence::actions::explain_transaction(&pool, &hash).await {
        Ok(explanation) => {
            let _ = intelligence::persist::persist_transaction_actions(&pool, &explanation).await;
            match serde_json::to_value(explanation) {
                Ok(v) => Ok(Json(v)),
                Err(e) => Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("Serialization error: {}", e),
                    }),
                )),
            }
        }
        Err(e) => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Transaction not found: {}", e),
            }),
        )),
    }
}

pub async fn get_address_intelligence(
    State(pool): State<PgPool>,
    Path(address): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    match intelligence::address::analyze_address(&pool, &address).await {
        Ok(intel) => {
            let _ = intelligence::persist::persist_address_stats(&pool, &intel).await;
            match serde_json::to_value(intel) {
                Ok(v) => Ok(Json(v)),
                Err(e) => Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("Serialization error: {}", e),
                    }),
                )),
            }
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to analyze address: {}", e),
            }),
        )),
    }
}

#[derive(Debug, Deserialize)]
pub struct GraphQuery {
    pub depth: Option<u32>,
    pub limit: Option<u32>,
}

pub async fn get_address_graph(
    State(pool): State<PgPool>,
    Path(address): Path<String>,
    Query(query): Query<GraphQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let depth = query.depth.unwrap_or(2).min(5);
    let limit = query.limit.unwrap_or(50).min(200);

    match intelligence::graph::build_graph(&pool, &address, depth, limit).await {
        Ok(graph) => {
            for edge in &graph.edges {
                let _ = intelligence::persist::persist_address_relationship(
                    &pool,
                    &edge.from,
                    &edge.to,
                    &format!("{:?}", edge.relationship).to_lowercase(),
                    edge.count,
                    edge.total_value.as_deref(),
                )
                .await;
            }
            match serde_json::to_value(graph) {
                Ok(v) => Ok(Json(v)),
                Err(e) => Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("Serialization error: {}", e),
                    }),
                )),
            }
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to build graph: {}", e),
            }),
        )),
    }
}

pub async fn get_contract_intelligence(
    State(pool): State<PgPool>,
    Path(address): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    match intelligence::contract::analyze_contract(&pool, &address).await {
        Ok(intel) => {
            let _ = intelligence::persist::persist_contract_profile(&pool, &intel).await;
            match serde_json::to_value(intel) {
                Ok(v) => Ok(Json(v)),
                Err(e) => Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("Serialization error: {}", e),
                    }),
                )),
            }
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to analyze contract: {}", e),
            }),
        )),
    }
}

pub async fn get_anomalies(
    State(pool): State<PgPool>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    match intelligence::anomaly::detect_anomalies(&pool).await {
        Ok(anomalies) => {
            for anomaly in &anomalies {
                let _ = intelligence::persist::persist_anomaly(&pool, anomaly).await;
            }
            match serde_json::to_value(anomalies) {
                Ok(v) => Ok(Json(v)),
                Err(e) => Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("Serialization error: {}", e),
                    }),
                )),
            }
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to detect anomalies: {}", e),
            }),
        )),
    }
}

pub async fn get_block_analytics(
    State(pool): State<PgPool>,
    Path(number): Path<i64>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    match intelligence::mev::analyze_block(&pool, number).await {
        Ok(analytics) => {
            let _ = intelligence::persist::persist_block_analytics(&pool, &analytics).await;
            for event in &analytics.mev_events {
                let _ = intelligence::persist::persist_mev_event(&pool, event).await;
            }
            match serde_json::to_value(analytics) {
                Ok(v) => Ok(Json(v)),
                Err(e) => Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("Serialization error: {}", e),
                    }),
                )),
            }
        }
        Err(e) => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Block not found: {}", e),
            }),
        )),
    }
}
