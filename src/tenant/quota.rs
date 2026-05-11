use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;

/// Per-tenant rate limiter using a sliding window approach
pub struct RateLimiter {
    max_per_second: u64,
    count: AtomicU64,
    window_start: Mutex<Instant>,
}

impl RateLimiter {
    pub fn new(max_per_second: u64) -> Self {
        Self {
            max_per_second,
            count: AtomicU64::new(0),
            window_start: Mutex::new(Instant::now()),
        }
    }

    pub fn try_acquire(&self) -> bool {
        let mut window_start = self.window_start.lock().unwrap();
        let now = Instant::now();

        // Check if we need to reset the window (>= 1 second elapsed)
        if now.duration_since(*window_start).as_secs() >= 1 {
            self.count.store(0, Ordering::Relaxed);
            *window_start = now;
        }

        // Try to increment the count
        let current = self.count.fetch_add(1, Ordering::Relaxed);

        // Check if we're over the limit
        if current >= self.max_per_second {
            // Undo the increment
            self.count.fetch_sub(1, Ordering::Relaxed);
            return false;
        }

        true
    }
}

/// Manages rate limiters for multiple tenants
pub struct QuotaManager {
    limiters: DashMap<String, RateLimiter>,
}

impl QuotaManager {
    pub fn new() -> Self {
        Self {
            limiters: DashMap::new(),
        }
    }

    pub fn set_limit(&self, tenant_id: &str, max_per_second: u64) {
        self.limiters.insert(tenant_id.to_string(), RateLimiter::new(max_per_second));
    }

    pub fn try_acquire(&self, tenant_id: &str) -> bool {
        match self.limiters.get(tenant_id) {
            Some(limiter) => limiter.try_acquire(),
            None => true, // No limit set for this tenant
        }
    }

    pub fn remove(&self, tenant_id: &str) {
        self.limiters.remove(tenant_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_rate_limiter_allows_within_limit() {
        let limiter = RateLimiter::new(10);

        for _ in 0..10 {
            assert!(limiter.try_acquire(), "Should allow all 10 requests within limit");
        }
    }

    #[test]
    fn test_rate_limiter_blocks_over_limit() {
        let limiter = RateLimiter::new(2);

        assert!(limiter.try_acquire(), "First request should succeed");
        assert!(limiter.try_acquire(), "Second request should succeed");
        assert!(!limiter.try_acquire(), "Third request should be blocked");
    }

    #[test]
    fn test_quota_manager_no_limit_allows() {
        let manager = QuotaManager::new();

        // Unknown tenant should always be allowed
        assert!(manager.try_acquire("unknown_tenant"));
        assert!(manager.try_acquire("unknown_tenant"));
        assert!(manager.try_acquire("unknown_tenant"));
    }

    #[test]
    fn test_quota_manager_with_limit() {
        let manager = QuotaManager::new();
        manager.set_limit("tenant1", 2);

        assert!(manager.try_acquire("tenant1"), "First request should succeed");
        assert!(manager.try_acquire("tenant1"), "Second request should succeed");
        assert!(!manager.try_acquire("tenant1"), "Third request should be blocked");
    }

    #[test]
    fn test_rate_limiter_window_reset() {
        let limiter = RateLimiter::new(2);

        // Exhaust the limit
        assert!(limiter.try_acquire());
        assert!(limiter.try_acquire());
        assert!(!limiter.try_acquire());

        // Wait for window to reset
        thread::sleep(Duration::from_secs(1));

        // Should be able to acquire again
        assert!(limiter.try_acquire(), "Should succeed after window reset");
    }

    #[test]
    fn test_quota_manager_remove() {
        let manager = QuotaManager::new();
        manager.set_limit("tenant1", 1);

        // Exhaust the limit
        assert!(manager.try_acquire("tenant1"));
        assert!(!manager.try_acquire("tenant1"));

        // Remove the limiter
        manager.remove("tenant1");

        // Should now be unlimited
        assert!(manager.try_acquire("tenant1"));
        assert!(manager.try_acquire("tenant1"));
    }
}
