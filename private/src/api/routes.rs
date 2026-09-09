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
        .with_state(pool)
}
