use std::sync::Arc;

use axum::Router;
use axum::middleware;
use axum::routing::get;
use sqlx::PgPool;

use super::cors;
use super::docs;
use super::handlers;
use super::logging;
use super::websocket::{WsState, ws_handler};

#[derive(Clone, Debug)]
pub struct AppState {
    pub pool: PgPool,
    pub ws: Arc<WsState>,
}

pub fn router(pool: PgPool, ws_state: Arc<WsState>) -> Router {
    let state = AppState { pool, ws: ws_state };

    Router::new()
        .route("/block/{number_or_hash}", get(handlers::get_block))
        .route("/transaction/{hash}", get(handlers::get_transaction))
        .route(
            "/address/{address}/transactions",
            get(handlers::get_address_transactions),
        )
        .route(
            "/contract/{address}/events",
            get(handlers::get_contract_events),
        )
        .route("/health", get(handlers::health))
        .route("/status", get(handlers::status))
        .route("/ws", get(ws_handler))
        .route("/docs", get(docs::api_docs))
        .route(
            "/api/v1/transactions/{hash}/explain",
            get(handlers::explain_transaction),
        )
        .route(
            "/api/v1/addresses/{address}/intelligence",
            get(handlers::get_address_intelligence),
        )
        .route(
            "/api/v1/addresses/{address}/graph",
            get(handlers::get_address_graph),
        )
        .route(
            "/api/v1/contracts/{address}/intelligence",
            get(handlers::get_contract_intelligence),
        )
        .route("/api/v1/anomalies", get(handlers::get_anomalies))
        .route(
            "/api/v1/blocks/{number}/analytics",
            get(handlers::get_block_analytics),
        )
        .route(
            "/api/v1/addresses/{address}/export",
            get(handlers::export_address),
        )
        .route(
            "/api/v1/addresses/{address}/trends",
            get(handlers::get_address_trends),
        )
        .route(
            "/api/v1/contracts/{address}/trends",
            get(handlers::get_contract_trends),
        )
        .route(
            "/api/v1/anomalies/trends",
            get(handlers::get_anomaly_trends),
        )
        .route("/api/v1/mev/trends", get(handlers::get_mev_trends))
        .route(
            "/api/v1/addresses/{address}/cluster",
            get(handlers::get_address_cluster),
        )
        .route(
            "/api/v1/contracts/deployers",
            get(handlers::get_contract_deployers),
        )
        .route(
            "/api/v1/tokens/{address}/whales",
            get(handlers::get_token_whales),
        )
        .layer(middleware::from_fn(logging::request_logger))
        .layer(cors::cors_layer())
        .with_state(state)
}
