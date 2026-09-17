use axum::Router;
use axum::routing::get;
use sqlx::PgPool;

use super::handlers;

pub fn router(pool: PgPool) -> Router {
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
        .with_state(pool)
}
