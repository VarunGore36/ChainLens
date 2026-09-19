use axum::http::{HeaderValue, Method, header};
use tower_http::cors::CorsLayer;

pub fn cors_layer() -> CorsLayer {
    let origins = [
        "http://localhost:3000",
        "http://localhost:8080",
        "http://127.0.0.1:8080",
        "https://chain-lens-chi.vercel.app",
    ];

    let mut cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
        .max_age(std::time::Duration::from_secs(3600));

    for origin in origins {
        if let Ok(val) = origin.parse::<HeaderValue>() {
            cors = cors.allow_origin(val);
        }
    }

    cors
}

pub fn cors_layer_permissive() -> CorsLayer {
    CorsLayer::permissive()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cors_layer_creation() {
        let _layer = cors_layer();
    }
}
