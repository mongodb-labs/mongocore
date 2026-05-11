use dashmap::DashMap;

/// Configuration for a single tenant
#[derive(Debug, Clone)]
pub struct TenantConfig {
    pub tenant_id: String,
    pub max_connections: usize,
    pub max_cache_entries: usize,
    pub rate_limit_ops_per_sec: u64,
    pub connection_uri_override: Option<String>,
}

/// Thread-safe registry for managing tenant configurations
pub struct TenantRegistry {
    configs: DashMap<String, TenantConfig>,
}

impl TenantRegistry {
    /// Creates a new empty tenant registry
    pub fn new() -> Self {
        Self {
            configs: DashMap::new(),
        }
    }

    /// Registers a tenant configuration, replacing any existing configuration for the same tenant_id
    pub fn register(&self, config: TenantConfig) {
        self.configs.insert(config.tenant_id.clone(), config);
    }

    /// Retrieves a tenant configuration by tenant_id
    pub fn get(&self, tenant_id: &str) -> Option<TenantConfig> {
        self.configs.get(tenant_id).map(|entry| entry.value().clone())
    }

    /// Removes a tenant configuration
    pub fn remove(&self, tenant_id: &str) {
        self.configs.remove(tenant_id);
    }

    /// Lists all registered tenant IDs
    pub fn list(&self) -> Vec<String> {
        self.configs.iter().map(|entry| entry.key().clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_tenant() {
        let registry = TenantRegistry::new();
        let config = TenantConfig {
            tenant_id: "tenant1".to_string(),
            max_connections: 100,
            max_cache_entries: 1000,
            rate_limit_ops_per_sec: 500,
            connection_uri_override: None,
        };

        registry.register(config.clone());
        let retrieved = registry.get("tenant1");

        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.tenant_id, "tenant1");
        assert_eq!(retrieved.max_connections, 100);
        assert_eq!(retrieved.max_cache_entries, 1000);
        assert_eq!(retrieved.rate_limit_ops_per_sec, 500);
        assert_eq!(retrieved.connection_uri_override, None);
    }

    #[test]
    fn test_unknown_tenant_returns_none() {
        let registry = TenantRegistry::new();
        let result = registry.get("nonexistent");
        assert!(result.is_none());
    }

    #[test]
    fn test_remove_tenant() {
        let registry = TenantRegistry::new();
        let config = TenantConfig {
            tenant_id: "tenant1".to_string(),
            max_connections: 100,
            max_cache_entries: 1000,
            rate_limit_ops_per_sec: 500,
            connection_uri_override: Some("mongodb://localhost:27017".to_string()),
        };

        registry.register(config);
        assert!(registry.get("tenant1").is_some());

        registry.remove("tenant1");
        assert!(registry.get("tenant1").is_none());
    }

    #[test]
    fn test_list_tenants() {
        let registry = TenantRegistry::new();

        let config1 = TenantConfig {
            tenant_id: "tenant1".to_string(),
            max_connections: 100,
            max_cache_entries: 1000,
            rate_limit_ops_per_sec: 500,
            connection_uri_override: None,
        };

        let config2 = TenantConfig {
            tenant_id: "tenant2".to_string(),
            max_connections: 200,
            max_cache_entries: 2000,
            rate_limit_ops_per_sec: 1000,
            connection_uri_override: Some("mongodb://localhost:27018".to_string()),
        };

        registry.register(config1);
        registry.register(config2);

        let tenant_ids = registry.list();
        assert_eq!(tenant_ids.len(), 2);
        assert!(tenant_ids.contains(&"tenant1".to_string()));
        assert!(tenant_ids.contains(&"tenant2".to_string()));
    }
}
