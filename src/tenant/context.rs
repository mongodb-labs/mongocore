const TENANT_HEADER: &str = "x-tenant-id";

/// Tenant context extracted from request headers or metadata
#[derive(Debug, Clone)]
pub struct TenantContext {
    tenant_id: Option<String>,
}

impl TenantContext {
    /// Create a new TenantContext with the given tenant ID
    pub fn new(tenant_id: Option<String>) -> Self {
        Self { tenant_id }
    }

    /// Create a default tenant context with no tenant ID
    pub fn default_tenant() -> Self {
        Self { tenant_id: None }
    }

    /// Get the tenant ID as a string slice
    pub fn tenant_id(&self) -> Option<&str> {
        self.tenant_id.as_deref()
    }

    /// Extract tenant context from gRPC metadata
    pub fn from_grpc_metadata(metadata: &tonic::metadata::MetadataMap) -> Self {
        let tenant_id = metadata
            .get(TENANT_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(String::from);
        Self::new(tenant_id)
    }

    /// Extract tenant context from HTTP headers
    pub fn from_http_headers(headers: &http::HeaderMap) -> Self {
        let tenant_id = headers
            .get(TENANT_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(String::from);
        Self::new(tenant_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_tenant_from_metadata() {
        let mut metadata = tonic::metadata::MetadataMap::new();
        metadata.insert(
            "x-tenant-id",
            "test-tenant-123".parse().unwrap(),
        );

        let context = TenantContext::from_grpc_metadata(&metadata);
        assert_eq!(context.tenant_id(), Some("test-tenant-123"));
    }

    #[test]
    fn test_no_tenant_returns_none() {
        let metadata = tonic::metadata::MetadataMap::new();
        let context = TenantContext::from_grpc_metadata(&metadata);
        assert_eq!(context.tenant_id(), None);
    }

    #[test]
    fn test_extract_tenant_from_headers() {
        let mut headers = http::HeaderMap::new();
        headers.insert(
            "x-tenant-id",
            "test-tenant-456".parse().unwrap(),
        );

        let context = TenantContext::from_http_headers(&headers);
        assert_eq!(context.tenant_id(), Some("test-tenant-456"));
    }

    #[test]
    fn test_default_tenant() {
        let context = TenantContext::default_tenant();
        assert_eq!(context.tenant_id(), None);
    }
}
