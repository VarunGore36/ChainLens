use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context;
use chainlens::{api, config, db};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if let Err(error) = dotenvy::dotenv() {
        if !error.not_found() {
            return Err(error).context("could not read .env");
        }
    }

    let config = config::Config::load().context("invalid configuration")?;

    let pool = db::connect(&config)
        .await
        .context("failed to connect to database")?;

    let server = db::server_version(&pool, &config.database_url.to_string()).await?;
    eprintln!("connected to {server}");

    let ws_state = Arc::new(api::websocket::WsState::new(1000));
    eprintln!("websocket state initialized");

    let app = api::routes::router(pool, ws_state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    eprintln!("listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
