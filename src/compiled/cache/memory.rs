use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;

use super::super::CompiledQuery;

const DEFAULT_MAX_SIZE: usize = 1000;

pub struct MemoryCache {
    entries: RwLock<HashMap<String, (CompiledQuery, u64)>>, // hash -> (query, access_order)
    max_size: usize,
    counter: AtomicU64,
}

impl MemoryCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            max_size,
            counter: AtomicU64::new(0),
        }
    }

    pub fn get(&self, hash: &str) -> Option<CompiledQuery> {
        let mut entries = self.entries.write().ok()?;
        let entry = entries.get_mut(hash)?;
        entry.1 = self.counter.fetch_add(1, Ordering::Relaxed);
        Some(entry.0.clone())
    }

    pub fn put(&self, query: CompiledQuery) {
        let mut entries = match self.entries.write() {
            Ok(e) => e,
            Err(_) => return,
        };

        let access_order = self.counter.fetch_add(1, Ordering::Relaxed);

        // If already present, update in place
        if entries.contains_key(&query.hash) {
            entries.insert(query.hash.clone(), (query, access_order));
            return;
        }

        // Evict if at capacity
        if entries.len() >= self.max_size {
            let lru_key = entries
                .iter()
                .min_by_key(|(_, (_, order))| *order)
                .map(|(k, _)| k.clone());
            if let Some(key) = lru_key {
                entries.remove(&key);
            }
        }

        entries.insert(query.hash.clone(), (query, access_order));
    }

    pub fn remove(&self, hash: &str) {
        if let Ok(mut entries) = self.entries.write() {
            entries.remove(hash);
        }
    }

    pub fn clear(&self) {
        if let Ok(mut entries) = self.entries.write() {
            entries.clear();
        }
    }

    pub fn len(&self) -> usize {
        self.entries.read().map(|e| e.len()).unwrap_or(0)
    }
}

impl Default for MemoryCache {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_SIZE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bson::doc;

    fn make_query(hash: &str) -> CompiledQuery {
        CompiledQuery {
            hash: hash.to_string(),
            intent: "test".to_string(),
            collection: "col".to_string(),
            database: "db".to_string(),
            mql: super::super::super::CompiledMql::Find {
                filter: doc! {},
                options: None,
            },
            template: None,
            created_at: 0,
        }
    }

    #[test]
    fn test_put_and_get() {
        let cache = MemoryCache::new(10);
        let query = make_query("abc123");
        cache.put(query.clone());

        let result = cache.get("abc123");
        assert!(result.is_some());
        assert_eq!(result.unwrap().hash, "abc123");
    }

    #[test]
    fn test_get_missing_returns_none() {
        let cache = MemoryCache::new(10);
        assert!(cache.get("nonexistent").is_none());
    }

    #[test]
    fn test_eviction_at_max_size() {
        let cache = MemoryCache::new(2);
        cache.put(make_query("a"));
        cache.put(make_query("b"));

        // Access "a" to make it more recent
        cache.get("a");

        // Insert "c" — should evict "b" (least recently used)
        cache.put(make_query("c"));

        assert_eq!(cache.len(), 2);
        assert!(cache.get("a").is_some());
        assert!(cache.get("b").is_none());
        assert!(cache.get("c").is_some());
    }

    #[test]
    fn test_remove() {
        let cache = MemoryCache::new(10);
        cache.put(make_query("x"));
        assert_eq!(cache.len(), 1);
        cache.remove("x");
        assert_eq!(cache.len(), 0);
        assert!(cache.get("x").is_none());
    }

    #[test]
    fn test_clear() {
        let cache = MemoryCache::new(10);
        cache.put(make_query("a"));
        cache.put(make_query("b"));
        assert_eq!(cache.len(), 2);
        cache.clear();
        assert_eq!(cache.len(), 0);
    }
}
