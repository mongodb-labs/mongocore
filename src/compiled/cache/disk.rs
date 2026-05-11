use std::path::PathBuf;

use super::super::CompiledQuery;

pub struct DiskCache {
    directory: PathBuf,
}

impl DiskCache {
    pub fn new(directory: PathBuf) -> std::io::Result<Self> {
        std::fs::create_dir_all(&directory)?;
        Ok(Self { directory })
    }

    pub fn get(&self, hash: &str) -> Option<CompiledQuery> {
        let path = self.path_for(hash);
        let data = std::fs::read_to_string(&path).ok()?;
        serde_json::from_str(&data).ok()
    }

    pub fn put(&self, query: &CompiledQuery) -> std::io::Result<()> {
        let path = self.path_for(&query.hash);
        let data = serde_json::to_string(query)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(path, data)?;
        Ok(())
    }

    pub fn remove(&self, hash: &str) -> std::io::Result<()> {
        let path = self.path_for(hash);
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }

    fn path_for(&self, hash: &str) -> PathBuf {
        self.directory.join(format!("{}.json", hash))
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
    fn test_put_and_get_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().to_path_buf()).unwrap();

        let query = make_query("hash1");
        cache.put(&query).unwrap();

        let result = cache.get("hash1");
        assert!(result.is_some());
        let result = result.unwrap();
        assert_eq!(result.hash, "hash1");
        assert_eq!(result.intent, "test");
    }

    #[test]
    fn test_get_missing_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().to_path_buf()).unwrap();

        assert!(cache.get("nonexistent").is_none());
    }

    #[test]
    fn test_remove() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().to_path_buf()).unwrap();

        let query = make_query("to_remove");
        cache.put(&query).unwrap();
        assert!(cache.get("to_remove").is_some());

        cache.remove("to_remove").unwrap();
        assert!(cache.get("to_remove").is_none());
    }
}
