use std::time::Instant;

use axum::middleware::Next;
use axum::response::Response;

pub async fn request_logger(request: axum::extract::Request, next: Next) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let start = Instant::now();

    let response = next.run(request).await;

    let duration = start.elapsed();
    let status = response.status();

    tracing::info!(
        method = %method,
        uri = %uri,
        status = %status.as_u16(),
        duration_ms = %duration.as_millis(),
        "request completed"
    );

    response
}

pub async fn error_logger(request: axum::extract::Request, next: Next) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();

    let response = next.run(request).await;

    if response.status().is_server_error() || response.status().is_client_error() {
        tracing::warn!(
            method = %method,
            uri = %uri,
            status = %response.status().as_u16(),
            "request failed"
        );
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_logger_creation() {
        // Just verify the module compiles
    }
}
