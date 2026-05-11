use bson::doc;

use super::super::CompiledQuery;
use crate::connection::pool::ConnectionPool;

const CACHE_DB: &str = "__mongocore";
const CACHE_COLLECTION: &str = "compiled_queries";

pub struct AtlasCache {
    pool: ConnectionPool,
}

impl AtlasCache {
    pub fn new(pool: ConnectionPool) -> Self {
        Self { pool }
    }

    pub async fn get(&self, hash: &str) -> Option<CompiledQuery> {
        let coll = self
            .pool
            .database(CACHE_DB)
            .collection::<bson::Document>(CACHE_COLLECTION);
        let doc = coll.find_one(doc! { "hash": hash }).await.ok()??;
        bson::from_document(doc).ok()
    }

    pub async fn put(&self, query: &CompiledQuery) {
        let coll = self
            .pool
            .database(CACHE_DB)
            .collection::<bson::Document>(CACHE_COLLECTION);
        if let Ok(doc) = bson::to_document(query) {
            let _ = coll
                .replace_one(doc! { "hash": &query.hash }, doc)
                .upsert(true)
                .await;
        }
    }

    pub async fn remove(&self, hash: &str) {
        let coll = self
            .pool
            .database(CACHE_DB)
            .collection::<bson::Document>(CACHE_COLLECTION);
        let _ = coll.delete_one(doc! { "hash": hash }).await;
    }

    pub async fn load_all(&self) -> Vec<CompiledQuery> {
        let coll = self
            .pool
            .database(CACHE_DB)
            .collection::<bson::Document>(CACHE_COLLECTION);
        let mut cursor = match coll.find(doc! {}).await {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
        let mut results = Vec::new();
        while cursor.advance().await.unwrap_or(false) {
            if let Ok(doc) = cursor.deserialize_current() {
                if let Ok(query) = bson::from_document(doc) {
                    results.push(query);
                }
            }
        }
        results
    }
}
