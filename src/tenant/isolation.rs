use dashmap::DashMap;
use std::hash::Hash;

/// A cache key that includes tenant partitioning information.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TenantCacheKey {
    partition: String,
    key: String,
}

impl TenantCacheKey {
    /// Creates a new TenantCacheKey with the given tenant ID and key.
    /// If tenant_id is None, uses "__default__" as the partition.
    pub fn new(tenant_id: Option<&str>, key: &str) -> Self {
        Self {
            partition: tenant_id.unwrap_or("__default__").to_string(),
            key: key.to_string(),
        }
    }
}

/// A thread-safe cache that partitions entries by tenant with per-tenant capacity limits.
pub struct PartitionedCache {
    max_per_tenant: usize,
    entries: DashMap<TenantCacheKey, String>,
    counts: DashMap<String, usize>,
}

impl PartitionedCache {
    /// Creates a new PartitionedCache with the specified maximum entries per tenant.
    pub fn new(max_per_tenant: usize) -> Self {
        Self {
            max_per_tenant,
            entries: DashMap::new(),
            counts: DashMap::new(),
        }
    }

    /// Inserts a value into the cache for the specified tenant.
    /// Returns false if the tenant has reached capacity and the key doesn't already exist.
    /// Increments the count only for new keys.
    pub fn insert(&self, tenant_id: &str, key: &str, value: String) -> bool {
        let cache_key = TenantCacheKey::new(Some(tenant_id), key);

        // Check if key already exists
        if self.entries.contains_key(&cache_key) {
            // Update existing entry without incrementing count
            self.entries.insert(cache_key, value);
            return true;
        }

        // Check capacity for new entries
        let current_count = self.counts.get(tenant_id).map(|v| *v).unwrap_or(0);
        if current_count >= self.max_per_tenant {
            return false;
        }

        // Insert new entry and increment count
        self.entries.insert(cache_key, value);
        self.counts
            .entry(tenant_id.to_string())
            .and_modify(|count| *count += 1)
            .or_insert(1);

        true
    }

    /// Retrieves a value from the cache for the specified tenant.
    /// Returns a cloned copy of the value if it exists.
    pub fn get(&self, tenant_id: &str, key: &str) -> Option<String> {
        let cache_key = TenantCacheKey::new(Some(tenant_id), key);
        self.entries.get(&cache_key).map(|v| v.value().clone())
    }

    /// Removes all entries for the specified tenant and clears the count.
    pub fn remove_tenant(&self, tenant_id: &str) {
        // Remove all entries for this tenant
        self.entries.retain(|k, _| k.partition != tenant_id);

        // Remove the count entry
        self.counts.remove(tenant_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tenant_cache_key_includes_tenant() {
        let key1 = TenantCacheKey::new(Some("tenant1"), "key1");
        let key2 = TenantCacheKey::new(Some("tenant2"), "key1");

        assert_ne!(key1, key2, "Different tenants should produce different cache keys");
    }

    #[test]
    fn test_no_tenant_uses_default_partition() {
        let key = TenantCacheKey::new(None, "key1");

        assert_eq!(key.partition, "__default__", "None tenant should use __default__ partition");
    }

    #[test]
    fn test_partitioned_cache_isolates_tenants() {
        let cache = PartitionedCache::new(10);

        cache.insert("tenant1", "key1", "value1".to_string());
        cache.insert("tenant2", "key1", "value2".to_string());

        assert_eq!(cache.get("tenant1", "key1"), Some("value1".to_string()));
        assert_eq!(cache.get("tenant2", "key1"), Some("value2".to_string()));
    }

    #[test]
    fn test_cache_respects_max_per_tenant() {
        let cache = PartitionedCache::new(2);

        assert!(cache.insert("tenant1", "key1", "value1".to_string()));
        assert!(cache.insert("tenant1", "key2", "value2".to_string()));
        assert!(!cache.insert("tenant1", "key3", "value3".to_string()),
                "Third insert should fail when max is 2");
    }

    #[test]
    fn test_remove_tenant_clears_entries() {
        let cache = PartitionedCache::new(10);

        cache.insert("tenant1", "key1", "value1".to_string());
        cache.insert("tenant1", "key2", "value2".to_string());

        cache.remove_tenant("tenant1");

        assert_eq!(cache.get("tenant1", "key1"), None);
        assert_eq!(cache.get("tenant1", "key2"), None);
    }
}
