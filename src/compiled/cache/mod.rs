pub mod atlas;
pub mod disk;
pub mod memory;

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::connection::pool::ConnectionPool;

use super::CompiledQuery;
use atlas::AtlasCache;
use disk::DiskCache;
use memory::MemoryCache;

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub l1_hits: u64,
    pub l1_misses: u64,
    pub l2_hits: u64,
    pub l2_misses: u64,
    pub l3_hits: u64,
    pub l3_misses: u64,
    pub evictions: u64,
    pub l1_size: usize,
}

pub struct CacheHierarchy {
    l1: MemoryCache,
    l2: Option<DiskCache>,
    l3: Option<AtlasCache>,
    l1_hits: AtomicU64,
    l1_misses: AtomicU64,
    l2_hits: AtomicU64,
    l2_misses: AtomicU64,
    l3_hits: AtomicU64,
    l3_misses: AtomicU64,
    evictions: AtomicU64,
}

impl CacheHierarchy {
    pub fn new(pool: Option<ConnectionPool>, disk_dir: Option<PathBuf>) -> Self {
        let l2 = disk_dir.and_then(|d| DiskCache::new(d).ok());
        let l3 = pool.map(AtlasCache::new);
        Self {
            l1: MemoryCache::default(),
            l2,
            l3,
            l1_hits: AtomicU64::new(0),
            l1_misses: AtomicU64::new(0),
            l2_hits: AtomicU64::new(0),
            l2_misses: AtomicU64::new(0),
            l3_hits: AtomicU64::new(0),
            l3_misses: AtomicU64::new(0),
            evictions: AtomicU64::new(0),
        }
    }

    /// Look up a compiled query by hash. Checks L1 -> L2 -> L3, promotes on miss.
    pub async fn get(&self, hash: &str) -> Option<CompiledQuery> {
        // L1
        if let Some(query) = self.l1.get(hash) {
            self.l1_hits.fetch_add(1, Ordering::Relaxed);
            return Some(query);
        }
        self.l1_misses.fetch_add(1, Ordering::Relaxed);

        // L2
        if let Some(ref l2) = self.l2 {
            if let Some(query) = l2.get(hash) {
                self.l2_hits.fetch_add(1, Ordering::Relaxed);
                self.l1.put(query.clone()); // promote to L1
                return Some(query);
            }
            self.l2_misses.fetch_add(1, Ordering::Relaxed);
        }

        // L3
        if let Some(ref l3) = self.l3 {
            if let Some(query) = l3.get(hash).await {
                self.l3_hits.fetch_add(1, Ordering::Relaxed);
                self.l1.put(query.clone()); // promote to L1
                if let Some(ref l2) = self.l2 {
                    let _ = l2.put(&query); // promote to L2
                }
                return Some(query);
            }
            self.l3_misses.fetch_add(1, Ordering::Relaxed);
        }
        None
    }

    /// Store a compiled query in all cache levels.
    pub async fn put(&self, query: &CompiledQuery) {
        self.l1.put(query.clone());
        if let Some(ref l2) = self.l2 {
            let _ = l2.put(query);
        }
        if let Some(ref l3) = self.l3 {
            l3.put(query).await;
        }
    }

    /// Remove a compiled query from all cache levels.
    pub async fn remove(&self, hash: &str) {
        self.l1.remove(hash);
        if let Some(ref l2) = self.l2 {
            let _ = l2.remove(hash);
        }
        if let Some(ref l3) = self.l3 {
            l3.remove(hash).await;
        }
    }

    /// Load all queries from L3 (Atlas) into L1 and L2. Called on startup.
    pub async fn warm_from_atlas(&self) {
        if let Some(ref l3) = self.l3 {
            let queries = l3.load_all().await;
            for query in queries {
                self.l1.put(query.clone());
                if let Some(ref l2) = self.l2 {
                    let _ = l2.put(&query);
                }
            }
        }
    }

    pub fn l1_size(&self) -> usize {
        self.l1.len()
    }

    pub fn cache_stats(&self) -> CacheStats {
        CacheStats {
            l1_hits: self.l1_hits.load(Ordering::Relaxed),
            l1_misses: self.l1_misses.load(Ordering::Relaxed),
            l2_hits: self.l2_hits.load(Ordering::Relaxed),
            l2_misses: self.l2_misses.load(Ordering::Relaxed),
            l3_hits: self.l3_hits.load(Ordering::Relaxed),
            l3_misses: self.l3_misses.load(Ordering::Relaxed),
            evictions: self.evictions.load(Ordering::Relaxed),
            l1_size: self.l1_size(),
        }
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
            mql: super::super::CompiledMql::Find {
                filter: doc! {},
                options: None,
            },
            template: None,
            llm_template: None,
            created_at: 0,
        }
    }

    #[tokio::test]
    async fn test_put_and_get_l1_only() {
        let cache = CacheHierarchy::new(None, None);
        let query = make_query("h1");
        cache.put(&query).await;

        let result = cache.get("h1").await;
        assert!(result.is_some());
        assert_eq!(result.unwrap().hash, "h1");
    }

    #[tokio::test]
    async fn test_put_and_get_with_l2() {
        let dir = tempfile::tempdir().unwrap();
        let cache = CacheHierarchy::new(None, Some(dir.path().to_path_buf()));

        let query = make_query("h2");
        cache.put(&query).await;

        // Verify it's in L2 by clearing L1 and fetching again
        cache.l1.clear();
        assert_eq!(cache.l1_size(), 0);

        let result = cache.get("h2").await;
        assert!(result.is_some());
        assert_eq!(result.unwrap().hash, "h2");

        // Should now be promoted back to L1
        assert_eq!(cache.l1_size(), 1);
    }

    #[tokio::test]
    async fn test_remove() {
        let dir = tempfile::tempdir().unwrap();
        let cache = CacheHierarchy::new(None, Some(dir.path().to_path_buf()));

        let query = make_query("h3");
        cache.put(&query).await;
        cache.remove("h3").await;

        assert!(cache.get("h3").await.is_none());
    }
}
