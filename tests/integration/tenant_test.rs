use mongocore::tenant::context::TenantContext;
use mongocore::tenant::isolation::PartitionedCache;
use mongocore::tenant::quota::QuotaManager;
use mongocore::tenant::registry::{TenantConfig, TenantRegistry};

#[tokio::test]
async fn test_tenant_registry_lifecycle() {
    let registry = TenantRegistry::new();

    registry.register(TenantConfig {
        tenant_id: "acme".to_string(),
        max_connections: 10,
        max_cache_entries: 1000,
        rate_limit_ops_per_sec: 100,
        connection_uri_override: None,
    });

    registry.register(TenantConfig {
        tenant_id: "beta".to_string(),
        max_connections: 5,
        max_cache_entries: 500,
        rate_limit_ops_per_sec: 50,
        connection_uri_override: Some("mongodb://other:27017".to_string()),
    });

    assert!(registry.get("acme").is_some());
    assert!(registry.get("beta").is_some());
    assert!(registry.get("unknown").is_none());
    assert_eq!(registry.list().len(), 2);

    registry.remove("acme");
    assert!(registry.get("acme").is_none());
    assert_eq!(registry.list().len(), 1);
}

#[tokio::test]
async fn test_cache_isolation_between_tenants() {
    let cache = PartitionedCache::new(100);

    cache.insert("tenant-a", "key1", "value-a".to_string());
    cache.insert("tenant-b", "key1", "value-b".to_string());

    assert_eq!(cache.get("tenant-a", "key1"), Some("value-a".to_string()));
    assert_eq!(cache.get("tenant-b", "key1"), Some("value-b".to_string()));

    // Removing one tenant doesn't affect the other
    cache.remove_tenant("tenant-a");
    assert_eq!(cache.get("tenant-a", "key1"), None);
    assert_eq!(cache.get("tenant-b", "key1"), Some("value-b".to_string()));
}

#[tokio::test]
async fn test_cache_capacity_per_tenant() {
    let cache = PartitionedCache::new(2);

    assert!(cache.insert("t1", "k1", "v1".to_string()));
    assert!(cache.insert("t1", "k2", "v2".to_string()));
    assert!(!cache.insert("t1", "k3", "v3".to_string())); // Over capacity

    // Other tenant still has room
    assert!(cache.insert("t2", "k1", "v1".to_string()));
}

#[tokio::test]
async fn test_quota_enforcement() {
    let mgr = QuotaManager::new();
    mgr.set_limit("limited", 3);

    assert!(mgr.try_acquire("limited"));
    assert!(mgr.try_acquire("limited"));
    assert!(mgr.try_acquire("limited"));
    assert!(!mgr.try_acquire("limited")); // Exceeded

    // Unknown tenant has no limit
    assert!(mgr.try_acquire("unknown"));
}

#[tokio::test]
async fn test_tenant_context_extraction() {
    let mut metadata = tonic::metadata::MetadataMap::new();
    metadata.insert("x-tenant-id", "test-tenant".parse().unwrap());

    let ctx = TenantContext::from_grpc_metadata(&metadata);
    assert_eq!(ctx.tenant_id(), Some("test-tenant"));

    let empty = tonic::metadata::MetadataMap::new();
    let ctx2 = TenantContext::from_grpc_metadata(&empty);
    assert_eq!(ctx2.tenant_id(), None);
}
