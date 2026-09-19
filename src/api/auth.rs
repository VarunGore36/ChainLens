use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;

#[derive(Clone, Debug)]
pub struct ApiAuth {
    pub api_key: Option<String>,
}

impl ApiAuth {
    pub fn new(api_key: Option<String>) -> Self {
        Self { api_key }
    }

    pub async fn middleware(request: axum::extract::Request, next: Next) -> Response {
        let auth = request.extensions().get::<ApiAuth>().cloned();

        if let Some(auth) = auth {
            if let Some(required_key) = &auth.api_key {
                let provided = request
                    .headers()
                    .get("X-API-Key")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("");

                if provided != required_key {
                    let mut response = Response::new(axum::body::Body::from("unauthorized"));
                    *response.status_mut() = StatusCode::UNAUTHORIZED;
                    return response;
                }
            }
        }

        next.run(request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_creation() {
        let auth = ApiAuth::new(Some("test-key".to_string()));
        assert!(auth.api_key.is_some());
    }

    #[test]
    fn auth_none_is_open() {
        let auth = ApiAuth::new(None);
        assert!(auth.api_key.is_none());
    }
}
