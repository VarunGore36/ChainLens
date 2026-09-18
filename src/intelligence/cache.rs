use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct CacheEntry<T: Clone> {
    pub value: T,
    pub inserted_at: Instant,
    pub ttl: Duration,
}

impl<T: Clone> CacheEntry<T> {
    pub fn is_expired(&self) -> bool {
        self.inserted_at.elapsed() > self.ttl
    }
}

impl<T: Clone + fmt::Debug> fmt::Debug for CacheEntry<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CacheEntry")
            .field("value", &self.value)
            .field("ttl", &self.ttl)
            .finish()
    }
}

#[derive(Clone)]
pub struct IntelligenceCache<T: Clone + Send + Sync> {
    entries: Arc<RwLock<HashMap<String, CacheEntry<T>>>>,
    max_entries: usize,
    default_ttl: Duration,
}

impl<T: Clone + Send + Sync + fmt::Debug> fmt::Debug for IntelligenceCache<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IntelligenceCache")
            .field("max_entries", &self.max_entries)
            .field("default_ttl", &self.default_ttl)
            .finish()
    }
}

impl<T: Clone + Send + Sync> IntelligenceCache<T> {
    pub fn new(max_entries: usize, default_ttl: Duration) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            max_entries,
            default_ttl,
        }
    }

    pub async fn get(&self, key: &str) -> Option<T> {
        let entries = self.entries.read().await;
        entries.get(key).and_then(|entry| {
            if entry.is_expired() {
                None
            } else {
                Some(entry.value.clone())
            }
        })
    }

    pub async fn set(&self, key: String, value: T) {
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
                value,
                inserted_at: Instant::now(),
                ttl: self.default_ttl,
            },
        );
    }

    pub async fn set_with_ttl(&self, key: String, value: T, ttl: Duration) {
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
                value,
                inserted_at: Instant::now(),
                ttl,
            },
        );
    }

    pub async fn remove(&self, key: &str) {
        let mut entries = self.entries.write().await;
        entries.remove(key);
    }

    pub async fn clear(&self) {
        let mut entries = self.entries.write().await;
        entries.clear();
    }

    pub async fn cleanup_expired(&self) {
        let mut entries = self.entries.write().await;
        entries.retain(|_, entry| !entry.is_expired());
    }

    pub async fn len(&self) -> usize {
        let entries = self.entries.read().await;
        entries.len()
    }

    pub async fn is_empty(&self) -> bool {
        let entries = self.entries.read().await;
        entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn cache_set_and_get() {
        let cache = IntelligenceCache::new(100, Duration::from_secs(60));
        cache.set("key1".to_string(), "value1".to_string()).await;
        let result = cache.get("key1").await;
        assert_eq!(result, Some("value1".to_string()));
    }

    #[tokio::test]
    async fn cache_get_missing_key() {
        let cache = IntelligenceCache::<String>::new(100, Duration::from_secs(60));
        let result = cache.get("missing").await;
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn cache_expiration() {
        let cache = IntelligenceCache::new(100, Duration::from_millis(10));
        cache.set("key1".to_string(), "value1".to_string()).await;
        tokio::time::sleep(Duration::from_millis(20)).await;
        let result = cache.get("key1").await;
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn cache_custom_ttl() {
        let cache = IntelligenceCache::new(100, Duration::from_secs(60));
        cache
            .set_with_ttl(
                "key1".to_string(),
                "value1".to_string(),
                Duration::from_millis(10),
            )
            .await;
        tokio::time::sleep(Duration::from_millis(20)).await;
        let result = cache.get("key1").await;
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn cache_max_entries_eviction() {
        let cache = IntelligenceCache::new(2, Duration::from_secs(60));
        cache.set("key1".to_string(), "value1".to_string()).await;
        cache.set("key2".to_string(), "value2".to_string()).await;
        cache.set("key3".to_string(), "value3".to_string()).await;
        assert_eq!(cache.len().await, 2);
    }

    #[tokio::test]
    async fn cache_remove() {
        let cache = IntelligenceCache::new(100, Duration::from_secs(60));
        cache.set("key1".to_string(), "value1".to_string()).await;
        cache.remove("key1").await;
        assert_eq!(cache.get("key1").await, None);
    }

    #[tokio::test]
    async fn cache_clear() {
        let cache = IntelligenceCache::new(100, Duration::from_secs(60));
        cache.set("key1".to_string(), "value1".to_string()).await;
        cache.set("key2".to_string(), "value2".to_string()).await;
        cache.clear().await;
        assert_eq!(cache.len().await, 0);
    }

    #[tokio::test]
    async fn cache_cleanup_expired() {
        let cache = IntelligenceCache::new(100, Duration::from_millis(10));
        cache.set("key1".to_string(), "value1".to_string()).await;
        cache.set("key2".to_string(), "value2".to_string()).await;
        tokio::time::sleep(Duration::from_millis(20)).await;
        cache.cleanup_expired().await;
        assert_eq!(cache.len().await, 0);
    }
}
