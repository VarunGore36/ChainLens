use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Duration;

use axum::http::StatusCode;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct ApiCache {
    entries: Arc<RwLock<std::collections::HashMap<String, CacheEntry>>>,
    max_entries: usize,
    default_ttl: Duration,
}

impl fmt::Debug for ApiCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApiCache")
            .field("max_entries", &self.max_entries)
            .field("default_ttl", &self.default_ttl)
            .finish()
    }
}

struct CacheEntry {
    body: Vec<u8>,
    status: StatusCode,
    inserted_at: std::time::Instant,
    ttl: Duration,
}

impl CacheEntry {
    fn is_expired(&self) -> bool {
        self.inserted_at.elapsed() > self.ttl
    }
}

impl ApiCache {
    pub fn new(max_entries: usize, default_ttl: Duration) -> Self {
        Self {
            entries: Arc::new(RwLock::new(std::collections::HashMap::new())),
            max_entries,
            default_ttl,
        }
    }

    pub async fn get(&self, key: &str) -> Option<(Vec<u8>, StatusCode)> {
        let entries = self.entries.read().await;
        entries.get(key).and_then(|entry| {
            if entry.is_expired() {
                None
            } else {
                Some((entry.body.clone(), entry.status))
            }
        })
    }

    pub async fn set(&self, key: String, body: Vec<u8>, status: StatusCode) {
        let mut entries = self.entries.write().await;

        if entries.len() >= self.max_entries {
            let oldest_key = entries
                .iter()
                .min_by_key(|(_, e)| e.inserted_at)
                .map(|(k, _)| k.clone());

            if let Some(k) = oldest_key {
                entries.remove(&k);
            }
        }

        entries.insert(
            key,
            CacheEntry {
                body,
                status,
                inserted_at: std::time::Instant::now(),
                ttl: self.default_ttl,
            },
        );
    }

    pub async fn invalidate_prefix(&self, prefix: &str) {
        let mut entries = self.entries.write().await;
        entries.retain(|k, _| !k.starts_with(prefix));
    }

    pub async fn clear(&self) {
        let mut entries = self.entries.write().await;
        entries.clear();
    }
}

pub fn cache_key(method: &str, path: &str, query: &str) -> String {
    let mut hasher = DefaultHasher::new();
    method.hash(&mut hasher);
    path.hash(&mut hasher);
    query.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cache_set_and_get() {
        let cache = ApiCache::new(100, Duration::from_secs(60));
        cache
            .set("key1".to_string(), b"body".to_vec(), StatusCode::OK)
            .await;
        let result = cache.get("key1").await;
        assert!(result.is_some());
        assert_eq!(result.unwrap().0, b"body");
    }

    #[tokio::test]
    async fn cache_get_missing() {
        let cache = ApiCache::new(100, Duration::from_secs(60));
        assert!(cache.get("missing").await.is_none());
    }

    #[tokio::test]
    async fn cache_expiration() {
        let cache = ApiCache::new(100, Duration::from_millis(10));
        cache
            .set("key1".to_string(), b"body".to_vec(), StatusCode::OK)
            .await;
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(cache.get("key1").await.is_none());
    }

    #[tokio::test]
    async fn cache_invalidate_prefix() {
        let cache = ApiCache::new(100, Duration::from_secs(60));
        cache
            .set(
                "/api/v1/addresses/0x123".to_string(),
                b"body1".to_vec(),
                StatusCode::OK,
            )
            .await;
        cache
            .set(
                "/api/v1/addresses/0x456".to_string(),
                b"body2".to_vec(),
                StatusCode::OK,
            )
            .await;
        cache
            .set(
                "/api/v1/blocks/100".to_string(),
                b"body3".to_vec(),
                StatusCode::OK,
            )
            .await;

        cache.invalidate_prefix("/api/v1/addresses").await;

        assert!(cache.get("/api/v1/addresses/0x123").await.is_none());
        assert!(cache.get("/api/v1/addresses/0x456").await.is_none());
        assert!(cache.get("/api/v1/blocks/100").await.is_some());
    }

    #[test]
    fn cache_key_generation() {
        let k1 = cache_key("GET", "/api/v1/blocks/100", "");
        let k2 = cache_key("GET", "/api/v1/blocks/100", "");
        let k3 = cache_key("GET", "/api/v1/blocks/200", "");
        assert_eq!(k1, k2);
        assert_ne!(k1, k3);
    }
}
