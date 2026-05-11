use sha2::{Digest, Sha256};

pub struct QueryHasher;

impl QueryHasher {
    /// Compute a deterministic hash from intent, database, collection, and optional schema fingerprint.
    pub fn hash(
        intent: &str,
        database: &str,
        collection: &str,
        schema_fingerprint: Option<&str>,
    ) -> String {
        let mut hasher = Sha256::new();
        // Normalize: lowercase, trim whitespace
        let normalized_intent = intent.trim().to_lowercase();
        hasher.update(normalized_intent.as_bytes());
        hasher.update(b"|");
        hasher.update(database.as_bytes());
        hasher.update(b"|");
        hasher.update(collection.as_bytes());
        if let Some(fp) = schema_fingerprint {
            hasher.update(b"|");
            hasher.update(fp.as_bytes());
        }
        format!("{:x}", hasher.finalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_intent_produces_same_hash() {
        let h1 = QueryHasher::hash("find all users", "mydb", "users", None);
        let h2 = QueryHasher::hash("find all users", "mydb", "users", None);
        assert_eq!(h1, h2);
    }

    #[test]
    fn different_intent_produces_different_hash() {
        let h1 = QueryHasher::hash("find all users", "mydb", "users", None);
        let h2 = QueryHasher::hash("find active users", "mydb", "users", None);
        assert_ne!(h1, h2);
    }

    #[test]
    fn different_collection_produces_different_hash() {
        let h1 = QueryHasher::hash("find all", "mydb", "users", None);
        let h2 = QueryHasher::hash("find all", "mydb", "orders", None);
        assert_ne!(h1, h2);
    }

    #[test]
    fn intent_normalization_trim_and_lowercase() {
        let h1 = QueryHasher::hash("Find All Users", "mydb", "users", None);
        let h2 = QueryHasher::hash("  find all users  ", "mydb", "users", None);
        assert_eq!(h1, h2);
    }

    #[test]
    fn schema_fingerprint_changes_hash() {
        let h1 = QueryHasher::hash("find all", "mydb", "users", None);
        let h2 = QueryHasher::hash("find all", "mydb", "users", Some("v1"));
        assert_ne!(h1, h2);
    }
}
