use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::collections::HashSet;

use tokio::sync::Semaphore;

use bson::Document;
use dashmap::DashMap;
use futures::StreamExt;
use tonic::{Request, Response, Status};
use uuid::Uuid;

use crate::analytics::{AnalyticsCollector, AnalyticsEvent, OperationKind};
use crate::defaults::{DEFAULT_PIPELINE_MAX_OPS, MAX_STREAM_BATCH_SIZE, MIN_STREAM_BATCH_SIZE};
use crate::connection::pool::ConnectionPool;
use crate::error::MongoCoreError;
use crate::operations::{
    FindAndModifyOptions, FindOptions, IndexOptions, Operations, ReturnDocumentOption, Transaction,
};
use crate::operations::raw::{run_command, RawCommandOptions};
use crate::operations::raw_validator::ValidationMode;
use crate::search::SearchEngine;
use crate::tenant::{TenantContext, TenantRegistry};
use crate::tenant::quota::QuotaManager;
use crate::voyage::VoyageClient;

use super::proto::{self, mongo_core_server::MongoCore};

/// The gRPC service implementation for MongoCore.
#[allow(dead_code)]
pub struct MongoCoreService {
    operations: Operations,
    pool: ConnectionPool,
    transactions: DashMap<String, Transaction>,
    search_engine: SearchEngine,
    analytics: Option<Arc<AnalyticsCollector>>,
    tenant_registry: Option<Arc<TenantRegistry>>,
    quota_manager: Option<Arc<QuotaManager>>,
    ingestion_engine: Option<Arc<crate::ingestion::IngestionEngine>>,
    directory_watcher: Option<Arc<crate::ingestion::DirectoryWatcher>>,
    client: Option<mongodb::Client>,
    stream_idle_timeout: Duration,
    pipeline_timeout: Duration,
    pipeline_semaphore: Arc<Semaphore>,
    appended_languages: Mutex<HashSet<String>>,
}

impl MongoCoreService {
    /// Create a new MongoCoreService from a ConnectionPool.
    pub fn new(
        pool: ConnectionPool,
        analytics: Option<Arc<AnalyticsCollector>>,
        tenant_registry: Option<Arc<TenantRegistry>>,
        quota_manager: Option<Arc<QuotaManager>>,
        stream_idle_timeout: Duration,
    ) -> Self {
        let operations = Operations::new(pool.clone());
        let search_engine = SearchEngine::new(pool.clone(), None);
        Self {
            operations,
            pool,
            transactions: DashMap::new(),
            search_engine,
            analytics,
            tenant_registry,
            quota_manager,
            ingestion_engine: None,
            directory_watcher: None,
            client: None,
            stream_idle_timeout,
            pipeline_timeout: Duration::from_secs(crate::defaults::DEFAULT_PIPELINE_TIMEOUT_SECS),
            pipeline_semaphore: Arc::new(Semaphore::new(crate::defaults::DEFAULT_PIPELINE_MAX_CONCURRENCY)),
            appended_languages: Mutex::new(HashSet::new()),
        }
    }

    /// Create a new MongoCoreService with Voyage AI enabled for vector search.
    pub fn with_voyage(
        pool: ConnectionPool,
        voyage_api_key: &str,
        analytics: Option<Arc<AnalyticsCollector>>,
        tenant_registry: Option<Arc<TenantRegistry>>,
        quota_manager: Option<Arc<QuotaManager>>,
        stream_idle_timeout: Duration,
    ) -> Self {
        let operations = Operations::new(pool.clone());
        let voyage_client = Arc::new(VoyageClient::new(voyage_api_key.to_string()));
        let search_engine = SearchEngine::new(pool.clone(), Some(voyage_client));
        Self {
            operations,
            pool,
            transactions: DashMap::new(),
            search_engine,
            analytics,
            tenant_registry,
            quota_manager,
            ingestion_engine: None,
            directory_watcher: None,
            client: None,
            stream_idle_timeout,
            pipeline_timeout: Duration::from_secs(crate::defaults::DEFAULT_PIPELINE_TIMEOUT_SECS),
            pipeline_semaphore: Arc::new(Semaphore::new(crate::defaults::DEFAULT_PIPELINE_MAX_CONCURRENCY)),
            appended_languages: Mutex::new(HashSet::new()),
        }
    }

    /// Configure pipeline timeout and concurrency.
    pub fn with_pipeline_config(mut self, timeout: Duration, max_concurrency: usize) -> Self {
        self.pipeline_timeout = timeout;
        self.pipeline_semaphore = Arc::new(Semaphore::new(max_concurrency));
        self
    }

    /// Configure ingestion support on this service.
    pub fn with_ingestion(
        mut self,
        engine: Arc<crate::ingestion::IngestionEngine>,
        watcher: Arc<crate::ingestion::DirectoryWatcher>,
        client: mongodb::Client,
    ) -> Self {
        self.ingestion_engine = Some(engine);
        self.directory_watcher = Some(watcher);
        self.client = Some(client);
        self
    }

    /// Record an analytics event if analytics is enabled.
    fn record_analytics(&self, op: OperationKind, db: &str, coll: &str, latency: std::time::Duration, success: bool) {
        if let Some(ref analytics) = self.analytics {
            analytics.record(AnalyticsEvent::new(op, db.to_string(), coll.to_string(), latency, success));
        }
    }

    /// Check tenant quota before processing request.
    fn check_tenant_quota(&self, metadata: &tonic::metadata::MetadataMap) -> Result<(), Status> {
        if let Some(ref quota) = self.quota_manager {
            let tenant = TenantContext::from_grpc_metadata(metadata);
            if let Some(tid) = tenant.tenant_id() {
                if !quota.try_acquire(tid) {
                    return Err(Status::resource_exhausted(
                        format!("Rate limit exceeded for tenant '{}'", tid)
                    ));
                }
            }
        }
        Ok(())
    }

    /// Append client language metadata from gRPC request metadata.
    fn append_client_language(&self, request_metadata: &tonic::metadata::MetadataMap) {
        if let Some(lang) = request_metadata.get("x-client-language") {
            if let Ok(lang_str) = lang.to_str() {
                let mut seen = self.appended_languages.lock().unwrap();
                if !seen.contains(lang_str) {
                    self.pool.append_interface_metadata(lang_str);
                    seen.insert(lang_str.to_string());
                }
            }
        }
    }
}

// === Helper functions ===

/// Decode raw bytes into a bson::Document.
fn decode_bson(data: &[u8]) -> Result<bson::Document, Status> {
    bson::Document::from_reader(data)
        .map_err(|e| Status::invalid_argument(format!("Invalid BSON: {}", e)))
}

/// Encode a bson::Document into raw bytes.
fn encode_bson(doc: &bson::Document) -> Result<Vec<u8>, Status> {
    let mut buf = Vec::new();
    doc.to_writer(&mut buf)
        .map_err(|e| Status::internal(format!("Failed to encode BSON: {}", e)))?;
    Ok(buf)
}

/// Convert a proto Document to a bson::Document.
fn proto_doc_to_bson(doc: &proto::Document) -> Result<bson::Document, Status> {
    decode_bson(&doc.data)
}

/// Convert a bson::Document to a proto Document.
fn bson_to_proto_doc(doc: &bson::Document) -> Result<proto::Document, Status> {
    Ok(proto::Document {
        data: encode_bson(doc)?,
    })
}

/// Convert a proto Filter to a bson::Document.
fn proto_filter_to_bson(filter: &Option<proto::Filter>) -> Result<bson::Document, Status> {
    match filter {
        Some(f) if !f.data.is_empty() => decode_bson(&f.data),
        _ => Ok(bson::Document::new()),
    }
}

/// Convert proto FindOptions to our internal FindOptions.
fn convert_find_options(opts: &Option<proto::FindOptions>) -> Result<Option<FindOptions>, Status> {
    match opts {
        None => Ok(None),
        Some(o) => {
            let sort = match &o.sort {
                Some(data) if !data.is_empty() => Some(decode_bson(data)?),
                _ => None,
            };
            let projection = match &o.projection {
                Some(data) if !data.is_empty() => Some(decode_bson(data)?),
                _ => None,
            };
            Ok(Some(FindOptions {
                limit: o.limit,
                skip: o.skip.map(|s| s as u64),
                sort,
                projection,
            }))
        }
    }
}

/// Map a MongoCoreError to a tonic Status.
fn to_status(err: MongoCoreError) -> Status {
    match &err {
        MongoCoreError::ConfigError(_) => Status::internal(err.to_string()),
        MongoCoreError::ConnectionError(_) => Status::unavailable(err.to_string()),
        MongoCoreError::OperationError(_) => Status::internal(err.to_string()),
        MongoCoreError::ValidationError(_) => Status::invalid_argument(err.to_string()),
        MongoCoreError::TimeoutError(_) => Status::deadline_exceeded(err.to_string()),
        MongoCoreError::IngestionError(_) => Status::internal(err.to_string()),
    }
}

#[tonic::async_trait]
impl MongoCore for MongoCoreService {
    // === CRUD ===

    #[tracing::instrument(skip(self, request))]
    async fn find(
        &self,
        request: Request<proto::FindRequest>,
    ) -> Result<Response<proto::FindResponse>, Status> {
        self.append_client_language(request.metadata());
        self.check_tenant_quota(request.metadata())?;
        let start = std::time::Instant::now();
        let req = request.into_inner();
        let filter = proto_filter_to_bson(&req.filter)?;
        let options = convert_find_options(&req.options)?;

        // Check if this is a transactional operation
        let result = if let Some(ref txn_id) = req.transaction_id {
            let mut txn = self
                .transactions
                .get_mut(txn_id)
                .ok_or_else(|| Status::not_found(format!("Transaction not found: {}", txn_id)))?;
            txn.find(&req.database, &req.collection, filter)
                .await
        } else {
            self.operations
                .find(&req.database, &req.collection, filter, options)
                .await
        };

        self.record_analytics(OperationKind::Find, &req.database, &req.collection, start.elapsed(), result.is_ok());
        let docs = result.map_err(to_status)?;

        let documents: Result<Vec<proto::Document>, Status> =
            docs.iter().map(bson_to_proto_doc).collect();

        Ok(Response::new(proto::FindResponse {
            documents: documents?,
            metadata: Some(proto::ResponseMetadata {
                search_method: String::new(),
            }),
        }))
    }

    #[tracing::instrument(skip(self, request))]
    async fn find_one(
        &self,
        request: Request<proto::FindOneRequest>,
    ) -> Result<Response<proto::FindOneResponse>, Status> {
        self.append_client_language(request.metadata());
        self.check_tenant_quota(request.metadata())?;
        let start = std::time::Instant::now();
        let req = request.into_inner();
        let filter = proto_filter_to_bson(&req.filter)?;

        let result = self
            .operations
            .find_one(&req.database, &req.collection, filter)
            .await;

        self.record_analytics(OperationKind::FindOne, &req.database, &req.collection, start.elapsed(), result.is_ok());
        let doc = result.map_err(to_status)?;

        let document = match doc {
            Some(ref d) => Some(bson_to_proto_doc(d)?),
            None => None,
        };

        Ok(Response::new(proto::FindOneResponse {
            document,
            metadata: Some(proto::ResponseMetadata {
                search_method: String::new(),
            }),
        }))
    }

    #[tracing::instrument(skip(self, request))]
    async fn insert(
        &self,
        request: Request<proto::InsertRequest>,
    ) -> Result<Response<proto::InsertResponse>, Status> {
        self.append_client_language(request.metadata());
        self.check_tenant_quota(request.metadata())?;
        let start = std::time::Instant::now();
        let req = request.into_inner();
        let doc = req
            .document
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing document"))?;
        let bson_doc = proto_doc_to_bson(doc)?;

        // Check if this is a transactional operation
        let result = if let Some(ref txn_id) = req.transaction_id {
            let mut txn = self
                .transactions
                .get_mut(txn_id)
                .ok_or_else(|| Status::not_found(format!("Transaction not found: {}", txn_id)))?;
            txn.insert(&req.database, &req.collection, bson_doc)
                .await
        } else {
            self.operations
                .insert(&req.database, &req.collection, bson_doc)
                .await
        };

        self.record_analytics(OperationKind::Insert, &req.database, &req.collection, start.elapsed(), result.is_ok());
        let insert_result = result.map_err(to_status)?;
        let inserted_id = insert_result.inserted_id.to_string();

        Ok(Response::new(proto::InsertResponse { inserted_id }))
    }

    #[tracing::instrument(skip(self, request))]
    async fn insert_many(
        &self,
        request: Request<proto::InsertManyRequest>,
    ) -> Result<Response<proto::InsertManyResponse>, Status> {
        self.append_client_language(request.metadata());
        self.check_tenant_quota(request.metadata())?;
        let start = std::time::Instant::now();
        let req = request.into_inner();
        let docs: Result<Vec<bson::Document>, Status> =
            req.documents.iter().map(proto_doc_to_bson).collect();
        let docs = docs?;

        let result = self
            .operations
            .insert_many(&req.database, &req.collection, docs)
            .await;

        self.record_analytics(OperationKind::InsertMany, &req.database, &req.collection, start.elapsed(), result.is_ok());
        let insert_result = result.map_err(to_status)?;

        let inserted_ids: Vec<String> = insert_result
            .inserted_ids
            .values()
            .map(|id| id.to_string())
            .collect();
        let inserted_count = inserted_ids.len() as i64;

        Ok(Response::new(proto::InsertManyResponse {
            inserted_ids,
            inserted_count,
        }))
    }

    #[tracing::instrument(skip(self, request))]
    async fn update(
        &self,
        request: Request<proto::UpdateRequest>,
    ) -> Result<Response<proto::UpdateResponse>, Status> {
        self.append_client_language(request.metadata());
        self.check_tenant_quota(request.metadata())?;
        let start = std::time::Instant::now();
        let req = request.into_inner();
        let filter = proto_filter_to_bson(&req.filter)?;
        let update_doc = req
            .update
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing update document"))?;
        let update = proto_doc_to_bson(update_doc)?;

        // Check if this is a transactional operation
        let result = if let Some(ref txn_id) = req.transaction_id {
            let mut txn = self.transactions.get_mut(txn_id).ok_or_else(|| {
                Status::not_found(format!("Transaction not found: {}", txn_id))
            })?;
            txn.update(&req.database, &req.collection, filter, update)
                .await
        } else {
            self.operations
                .update(&req.database, &req.collection, filter, update)
                .await
        };

        self.record_analytics(OperationKind::Update, &req.database, &req.collection, start.elapsed(), result.is_ok());
        let update_result = result.map_err(to_status)?;

        Ok(Response::new(proto::UpdateResponse {
            matched_count: update_result.matched_count as i64,
            modified_count: update_result.modified_count as i64,
            upserted_id: update_result.upserted_id.map(|id| id.to_string()),
        }))
    }

    #[tracing::instrument(skip(self, request))]
    async fn update_many(
        &self,
        request: Request<proto::UpdateManyRequest>,
    ) -> Result<Response<proto::UpdateManyResponse>, Status> {
        self.append_client_language(request.metadata());
        self.check_tenant_quota(request.metadata())?;
        let start = std::time::Instant::now();
        let req = request.into_inner();
        let filter = proto_filter_to_bson(&req.filter)?;
        let update_doc = req
            .update
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing update document"))?;
        let update = proto_doc_to_bson(update_doc)?;

        let result = self
            .operations
            .update_many(&req.database, &req.collection, filter, update)
            .await;

        self.record_analytics(OperationKind::UpdateMany, &req.database, &req.collection, start.elapsed(), result.is_ok());
        let update_result = result.map_err(to_status)?;

        Ok(Response::new(proto::UpdateManyResponse {
            matched_count: update_result.matched_count as i64,
            modified_count: update_result.modified_count as i64,
            upserted_id: update_result.upserted_id.map(|id| id.to_string()),
        }))
    }

    #[tracing::instrument(skip(self, request))]
    async fn delete(
        &self,
        request: Request<proto::DeleteRequest>,
    ) -> Result<Response<proto::DeleteResponse>, Status> {
        self.append_client_language(request.metadata());
        self.check_tenant_quota(request.metadata())?;
        let start = std::time::Instant::now();
        let req = request.into_inner();
        let filter = proto_filter_to_bson(&req.filter)?;

        // Check if this is a transactional operation
        let result = if let Some(ref txn_id) = req.transaction_id {
            let mut txn = self
                .transactions
                .get_mut(txn_id)
                .ok_or_else(|| Status::not_found(format!("Transaction not found: {}", txn_id)))?;
            txn.delete(&req.database, &req.collection, filter)
                .await
        } else {
            self.operations
                .delete(&req.database, &req.collection, filter)
                .await
        };

        self.record_analytics(OperationKind::Delete, &req.database, &req.collection, start.elapsed(), result.is_ok());
        let delete_result = result.map_err(to_status)?;

        Ok(Response::new(proto::DeleteResponse {
            deleted_count: delete_result.deleted_count as i64
        }))
    }

    #[tracing::instrument(skip(self, request))]
    async fn delete_many(
        &self,
        request: Request<proto::DeleteManyRequest>,
    ) -> Result<Response<proto::DeleteManyResponse>, Status> {
        self.append_client_language(request.metadata());
        self.check_tenant_quota(request.metadata())?;
        let start = std::time::Instant::now();
        let req = request.into_inner();
        let filter = proto_filter_to_bson(&req.filter)?;

        let result = self
            .operations
            .delete_many(&req.database, &req.collection, filter)
            .await;

        self.record_analytics(OperationKind::DeleteMany, &req.database, &req.collection, start.elapsed(), result.is_ok());
        let delete_result = result.map_err(to_status)?;

        Ok(Response::new(proto::DeleteManyResponse {
            deleted_count: delete_result.deleted_count as i64,
        }))
    }

    #[tracing::instrument(skip(self, request))]
    async fn find_and_modify(
        &self,
        request: Request<proto::FindAndModifyRequest>,
    ) -> Result<Response<proto::FindAndModifyResponse>, Status> {
        self.append_client_language(request.metadata());
        self.check_tenant_quota(request.metadata())?;
        let start = std::time::Instant::now();
        let req = request.into_inner();
        let filter = proto_filter_to_bson(&req.filter)?;
        let update_doc = req
            .update
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing update document"))?;
        let update = proto_doc_to_bson(update_doc)?;

        let options = req.options.map(|opts| {
            let return_document =
                if opts.return_document == proto::find_and_modify_options::ReturnDocument::Before as i32 {
                    ReturnDocumentOption::Before
                } else {
                    ReturnDocumentOption::After
                };
            let sort = opts
                .sort
                .as_ref()
                .and_then(|data| if data.is_empty() { None } else { decode_bson(data).ok() });
            FindAndModifyOptions {
                return_document,
                upsert: opts.upsert,
                sort,
            }
        });

        let result = self
            .operations
            .find_and_modify(&req.database, &req.collection, filter, update, options)
            .await;

        self.record_analytics(OperationKind::FindAndModify, &req.database, &req.collection, start.elapsed(), result.is_ok());
        let doc_result = result.map_err(to_status)?;

        let document = match doc_result {
            Some(ref d) => Some(bson_to_proto_doc(d)?),
            None => None,
        };

        Ok(Response::new(proto::FindAndModifyResponse { document }))
    }

    // === Aggregation ===

    #[tracing::instrument(skip(self, request))]
    async fn aggregate(
        &self,
        request: Request<proto::AggregateRequest>,
    ) -> Result<Response<proto::AggregateResponse>, Status> {
        self.append_client_language(request.metadata());
        self.check_tenant_quota(request.metadata())?;
        let start = std::time::Instant::now();
        let req = request.into_inner();

        let pipeline: Vec<bson::Document> = match req.pipeline {
            Some(p) => p
                .stages
                .iter()
                .map(|stage| decode_bson(stage))
                .collect::<Result<Vec<_>, _>>()?,
            None => Vec::new(),
        };

        let result = self
            .operations
            .aggregate(&req.database, &req.collection, pipeline)
            .await;

        self.record_analytics(OperationKind::Aggregate, &req.database, &req.collection, start.elapsed(), result.is_ok());
        let docs = result.map_err(to_status)?;

        let documents: Result<Vec<proto::Document>, Status> =
            docs.iter().map(bson_to_proto_doc).collect();

        Ok(Response::new(proto::AggregateResponse {
            documents: documents?,
            metadata: Some(proto::ResponseMetadata {
                search_method: String::new(),
            }),
        }))
    }

    // === Search ===

    #[tracing::instrument(skip(self, request))]
    async fn search(
        &self,
        request: Request<proto::SearchRequest>,
    ) -> Result<Response<proto::SearchResponse>, Status> {
        self.append_client_language(request.metadata());
        self.check_tenant_quota(request.metadata())?;
        let start = std::time::Instant::now();
        let req = request.into_inner();

        let limit = if req.limit > 0 { req.limit } else { 10 };

        let result = self
            .search_engine
            .search(&req.database, &req.collection, &req.query, limit)
            .await;

        self.record_analytics(OperationKind::Search, &req.database, &req.collection, start.elapsed(), result.is_ok());
        let search_result = result.map_err(|e| Status::internal(format!("Search error: {}", e)))?;

        let documents: Result<Vec<proto::Document>, Status> =
            search_result.documents.iter().map(bson_to_proto_doc).collect();

        let method = match search_result.method {
            crate::search::SearchMethod::Vector => "vector",
            crate::search::SearchMethod::Fulltext => "fulltext",
            crate::search::SearchMethod::Filter => "filter",
        };

        Ok(Response::new(proto::SearchResponse {
            documents: documents?,
            method: method.to_string(),
            total: search_result.total as i64,
        }))
    }

    // === Transactions ===

    #[tracing::instrument(skip(self, request))]
    async fn begin_transaction(
        &self,
        request: Request<proto::BeginTransactionRequest>,
    ) -> Result<Response<proto::BeginTransactionResponse>, Status> {
        self.append_client_language(request.metadata());
        let txn = Transaction::begin(&self.pool).await.map_err(to_status)?;
        let txn_id = Uuid::new_v4().to_string();
        self.transactions.insert(txn_id.clone(), txn);

        Ok(Response::new(proto::BeginTransactionResponse {
            transaction_id: txn_id,
        }))
    }

    #[tracing::instrument(skip(self, request))]
    async fn commit_transaction(
        &self,
        request: Request<proto::CommitTransactionRequest>,
    ) -> Result<Response<proto::CommitTransactionResponse>, Status> {
        self.append_client_language(request.metadata());
        let req = request.into_inner();
        let (_, mut txn) = self
            .transactions
            .remove(&req.transaction_id)
            .ok_or_else(|| {
                Status::not_found(format!("Transaction not found: {}", req.transaction_id))
            })?;

        txn.commit().await.map_err(to_status)?;

        Ok(Response::new(proto::CommitTransactionResponse {}))
    }

    #[tracing::instrument(skip(self, request))]
    async fn abort_transaction(
        &self,
        request: Request<proto::AbortTransactionRequest>,
    ) -> Result<Response<proto::AbortTransactionResponse>, Status> {
        self.append_client_language(request.metadata());
        let req = request.into_inner();
        let (_, mut txn) = self
            .transactions
            .remove(&req.transaction_id)
            .ok_or_else(|| {
                Status::not_found(format!("Transaction not found: {}", req.transaction_id))
            })?;

        txn.abort().await.map_err(to_status)?;

        Ok(Response::new(proto::AbortTransactionResponse {}))
    }

    // === Admin ===

    #[tracing::instrument(skip(self, request))]
    async fn create_collection(
        &self,
        request: Request<proto::CreateCollectionRequest>,
    ) -> Result<Response<proto::CreateCollectionResponse>, Status> {
        self.append_client_language(request.metadata());
        let req = request.into_inner();

        self.operations
            .create_collection(&req.database, &req.collection)
            .await
            .map_err(to_status)?;

        Ok(Response::new(proto::CreateCollectionResponse {}))
    }

    #[tracing::instrument(skip(self, request))]
    async fn create_index(
        &self,
        request: Request<proto::CreateIndexRequest>,
    ) -> Result<Response<proto::CreateIndexResponse>, Status> {
        self.append_client_language(request.metadata());
        let req = request.into_inner();
        let keys_doc = req
            .keys
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing keys document"))?;
        let keys = proto_doc_to_bson(keys_doc)?;

        let options = req.options.map(|opts| IndexOptions {
            name: opts.name,
            unique: opts.unique,
            sparse: opts.sparse,
        });

        let index_name = self
            .operations
            .create_index(&req.database, &req.collection, keys, options)
            .await
            .map_err(to_status)?;

        Ok(Response::new(proto::CreateIndexResponse { index_name }))
    }

    // === Introspection ===

    #[tracing::instrument(skip(self, request))]
    async fn list_databases(
        &self,
        request: Request<proto::ListDatabasesRequest>,
    ) -> Result<Response<proto::ListDatabasesResponse>, Status> {
        self.append_client_language(request.metadata());
        let databases = self
            .pool
            .client()
            .list_database_names()
            .await
            .map_err(|e| Status::internal(format!("Failed to list databases: {}", e)))?;

        Ok(Response::new(proto::ListDatabasesResponse { databases }))
    }

    #[tracing::instrument(skip(self, request))]
    async fn list_collections(
        &self,
        request: Request<proto::ListCollectionsRequest>,
    ) -> Result<Response<proto::ListCollectionsResponse>, Status> {
        self.append_client_language(request.metadata());
        let req = request.into_inner();

        let collections = self
            .pool
            .database(&req.database)
            .list_collection_names()
            .await
            .map_err(|e| Status::internal(format!("Failed to list collections: {}", e)))?;

        Ok(Response::new(proto::ListCollectionsResponse {
            collections,
        }))
    }

    // === Streaming ===

    type WatchStream = Pin<
        Box<dyn tokio_stream::Stream<Item = Result<proto::WatchEvent, Status>> + Send + 'static>,
    >;

    type FindStreamStream = Pin<
        Box<dyn tokio_stream::Stream<Item = Result<proto::DocumentBatch, Status>> + Send + 'static>,
    >;

    type AggregateStreamStream = Pin<
        Box<dyn tokio_stream::Stream<Item = Result<proto::DocumentBatch, Status>> + Send + 'static>,
    >;

    type InsertManyBidiStream = Pin<
        Box<dyn tokio_stream::Stream<Item = Result<proto::InsertBatchAck, Status>> + Send + 'static>,
    >;

    #[tracing::instrument(skip(self, request))]
    async fn watch(
        &self,
        request: Request<proto::WatchRequest>,
    ) -> Result<Response<Self::WatchStream>, Status> {
        self.append_client_language(request.metadata());
        let req = request.into_inner();

        let pipeline: Vec<Document> = if let Some(p) = req.pipeline {
            p.stages
                .iter()
                .map(|s| bson::from_slice(s))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| Status::invalid_argument(format!("Invalid pipeline: {}", e)))?
        } else {
            vec![]
        };

        let mut change_stream = if let Some(ref collection) = req.collection {
            self.pool
                .collection(&req.database, collection)
                .watch()
                .pipeline(pipeline)
                .await
                .map_err(|e| Status::internal(format!("Failed to open change stream: {}", e)))?
        } else {
            self.pool
                .database(&req.database)
                .watch()
                .pipeline(pipeline)
                .await
                .map_err(|e| Status::internal(format!("Failed to open change stream: {}", e)))?
        };

        let stream = async_stream::stream! {
            while let Some(result) = change_stream.next().await {
                match result {
                    Ok(event) => {
                        let op_type = match event.operation_type {
                            mongodb::change_stream::event::OperationType::Insert => proto::watch_event::OperationType::Insert,
                            mongodb::change_stream::event::OperationType::Update => proto::watch_event::OperationType::Update,
                            mongodb::change_stream::event::OperationType::Delete => proto::watch_event::OperationType::Delete,
                            mongodb::change_stream::event::OperationType::Replace => proto::watch_event::OperationType::Replace,
                            mongodb::change_stream::event::OperationType::Invalidate => proto::watch_event::OperationType::Invalidate,
                            _ => proto::watch_event::OperationType::Insert,
                        };

                        let (database, collection) = match &event.ns {
                            Some(ns) => (
                                ns.db.clone(),
                                ns.coll.clone().unwrap_or_default(),
                            ),
                            None => (String::new(), String::new()),
                        };

                        let document = event.full_document.map(|doc| {
                            let bytes = bson::to_vec(&doc).unwrap_or_default();
                            proto::Document { data: bytes }
                        });

                        let update_description = event.update_description.map(|ud| {
                            let doc = bson::doc! {
                                "updatedFields": &ud.updated_fields,
                                "removedFields": &ud.removed_fields,
                            };
                            let bytes = bson::to_vec(&doc).unwrap_or_default();
                            proto::Document { data: bytes }
                        });

                        let document_key = event.document_key.map(|dk| {
                            let bytes = bson::to_vec(&dk).unwrap_or_default();
                            proto::Document { data: bytes }
                        });

                        yield Ok(proto::WatchEvent {
                            operation_type: op_type as i32,
                            database,
                            collection,
                            document,
                            update_description,
                            document_key,
                        });
                    }
                    Err(e) => {
                        yield Err(Status::internal(format!("Change stream error: {}", e)));
                        break;
                    }
                }
            }
        };

        Ok(Response::new(Box::pin(stream) as Self::WatchStream))
    }

    // === Streaming Bulk ===

    async fn find_stream(
        &self,
        request: Request<proto::FindStreamRequest>,
    ) -> Result<Response<Self::FindStreamStream>, Status> {
        self.append_client_language(request.metadata());
        self.check_tenant_quota(request.metadata())?;
        let req = request.into_inner();
        let filter = proto_filter_to_bson(&req.filter)?;
        let options = convert_find_options(&req.options)?;

        let batch_size = if req.batch_size == 0 {
            crate::defaults::DEFAULT_STREAM_BATCH_SIZE
        } else {
            req.batch_size.clamp(MIN_STREAM_BATCH_SIZE, MAX_STREAM_BATCH_SIZE)
        };

        let cursor = self
            .operations
            .find_cursor(&req.database, &req.collection, filter, options)
            .await
            .map_err(to_status)?;

        let idle_timeout = self.stream_idle_timeout;
        let stream = async_stream::stream! {
            let mut cursor = cursor;
            let mut batch_index: u32 = 0;
            let mut batch: Vec<proto::Document> = Vec::with_capacity(batch_size as usize);

            loop {
                match tokio::time::timeout(idle_timeout, cursor.advance()).await {
                    Ok(Ok(true)) => {
                        match cursor.deserialize_current() {
                            Ok(doc) => {
                                let bytes = bson::to_vec(&doc).unwrap_or_default();
                                batch.push(proto::Document { data: bytes });

                                if batch.len() >= batch_size as usize {
                                    yield Ok(proto::DocumentBatch {
                                        documents: std::mem::take(&mut batch),
                                        batch_index,
                                        has_more: true,
                                    });
                                    batch_index += 1;
                                    batch = Vec::with_capacity(batch_size as usize);
                                }
                            }
                            Err(e) => {
                                yield Err(Status::internal(format!("Cursor error: {}", e)));
                                return;
                            }
                        }
                    }
                    Ok(Ok(false)) => break, // cursor exhausted
                    Ok(Err(e)) => {
                        yield Err(Status::internal(format!("Cursor error: {}", e)));
                        return;
                    }
                    Err(_) => {
                        yield Err(Status::deadline_exceeded("Stream idle timeout"));
                        return;
                    }
                }
            }

            // Final batch
            yield Ok(proto::DocumentBatch {
                documents: batch,
                batch_index,
                has_more: false,
            });
        };

        Ok(Response::new(Box::pin(stream)))
    }

    async fn aggregate_stream(
        &self,
        request: Request<proto::AggregateStreamRequest>,
    ) -> Result<Response<Self::AggregateStreamStream>, Status> {
        self.append_client_language(request.metadata());
        self.check_tenant_quota(request.metadata())?;
        let req = request.into_inner();

        let pipeline: Vec<bson::Document> = match req.pipeline {
            Some(p) => p
                .stages
                .iter()
                .map(|stage| decode_bson(stage))
                .collect::<Result<Vec<_>, _>>()?,
            None => Vec::new(),
        };

        let batch_size = if req.batch_size == 0 {
            crate::defaults::DEFAULT_STREAM_BATCH_SIZE
        } else {
            req.batch_size.clamp(MIN_STREAM_BATCH_SIZE, MAX_STREAM_BATCH_SIZE)
        };

        let cursor = self
            .operations
            .aggregate_cursor(&req.database, &req.collection, pipeline)
            .await
            .map_err(to_status)?;

        let idle_timeout = self.stream_idle_timeout;
        let stream = async_stream::stream! {
            let mut cursor = cursor;
            let mut batch_index: u32 = 0;
            let mut batch: Vec<proto::Document> = Vec::with_capacity(batch_size as usize);

            loop {
                match tokio::time::timeout(idle_timeout, cursor.advance()).await {
                    Ok(Ok(true)) => {
                        match cursor.deserialize_current() {
                            Ok(doc) => {
                                let bytes = bson::to_vec(&doc).unwrap_or_default();
                                batch.push(proto::Document { data: bytes });

                                if batch.len() >= batch_size as usize {
                                    yield Ok(proto::DocumentBatch {
                                        documents: std::mem::take(&mut batch),
                                        batch_index,
                                        has_more: true,
                                    });
                                    batch_index += 1;
                                    batch = Vec::with_capacity(batch_size as usize);
                                }
                            }
                            Err(e) => {
                                yield Err(Status::internal(format!("Cursor error: {}", e)));
                                return;
                            }
                        }
                    }
                    Ok(Ok(false)) => break, // cursor exhausted
                    Ok(Err(e)) => {
                        yield Err(Status::internal(format!("Cursor error: {}", e)));
                        return;
                    }
                    Err(_) => {
                        yield Err(Status::deadline_exceeded("Stream idle timeout"));
                        return;
                    }
                }
            }

            yield Ok(proto::DocumentBatch {
                documents: batch,
                batch_index,
                has_more: false,
            });
        };

        Ok(Response::new(Box::pin(stream)))
    }

    async fn insert_many_stream(
        &self,
        request: Request<tonic::Streaming<proto::InsertBatch>>,
    ) -> Result<Response<proto::InsertManyStreamResponse>, Status> {
        self.append_client_language(request.metadata());
        let mut stream = request.into_inner();
        let mut total_inserted: u64 = 0;
        let mut errors: Vec<proto::InsertError> = Vec::new();
        let mut database = String::new();
        let mut collection = String::new();
        let mut global_index: u32 = 0;

        while let Some(batch) = stream
            .message()
            .await
            .map_err(|e| Status::internal(format!("Stream error: {}", e)))?
        {
            if database.is_empty() {
                database.clone_from(&batch.database);
                collection.clone_from(&batch.collection);
            }

            let docs: Result<Vec<bson::Document>, Status> =
                batch.documents.iter().map(|d| proto_doc_to_bson(d)).collect();
            let docs = docs?;
            let batch_len = docs.len() as u32;

            match self
                .operations
                .insert_many(&database, &collection, docs)
                .await
            {
                Ok(result) => {
                    total_inserted += result.inserted_ids.len() as u64;
                }
                Err(e) => {
                    errors.push(proto::InsertError {
                        index: global_index,
                        message: e.to_string(),
                        code: 0,
                    });
                }
            }
            global_index += batch_len;
        }

        Ok(Response::new(proto::InsertManyStreamResponse {
            total_inserted,
            errors,
        }))
    }

    async fn insert_many_bidi(
        &self,
        request: Request<tonic::Streaming<proto::InsertBatch>>,
    ) -> Result<Response<Self::InsertManyBidiStream>, Status> {
        self.append_client_language(request.metadata());
        let mut inbound = request.into_inner();
        let operations = self.operations.clone();

        let stream = async_stream::stream! {
            let mut database = String::new();
            let mut collection = String::new();
            let mut batch_index: u32 = 0;

            loop {
                match inbound.message().await {
                    Ok(Some(batch)) => {
                        if database.is_empty() {
                            database.clone_from(&batch.database);
                            collection.clone_from(&batch.collection);
                        }

                        let docs: std::result::Result<Vec<bson::Document>, Status> =
                            batch.documents.iter().map(|d| proto_doc_to_bson(d)).collect();
                        match docs {
                            Ok(docs) => {
                                match operations.insert_many(&database, &collection, docs).await {
                                    Ok(result) => {
                                        yield Ok(proto::InsertBatchAck {
                                            batch_index,
                                            inserted_count: result.inserted_ids.len() as u32,
                                            errors: vec![],
                                        });
                                    }
                                    Err(e) => {
                                        yield Ok(proto::InsertBatchAck {
                                            batch_index,
                                            inserted_count: 0,
                                            errors: vec![proto::InsertError {
                                                index: 0,
                                                message: e.to_string(),
                                                code: 0,
                                            }],
                                        });
                                    }
                                }
                            }
                            Err(e) => {
                                yield Err(e);
                                return;
                            }
                        }
                        batch_index += 1;
                    }
                    Ok(None) => break,
                    Err(e) => {
                        yield Err(Status::internal(format!("Stream error: {}", e)));
                        return;
                    }
                }
            }
        };

        Ok(Response::new(Box::pin(stream)))
    }

    // === Raw Passthrough ===

    #[tracing::instrument(skip(self, request))]
    async fn run_command(
        &self,
        request: Request<proto::RunCommandRequest>,
    ) -> Result<Response<proto::RunCommandResponse>, Status> {
        self.append_client_language(request.metadata());
        self.check_tenant_quota(request.metadata())?;
        let start = std::time::Instant::now();
        let req = request.into_inner();

        // Decode the command from proto format
        let command_doc = req
            .command
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing command document"))?;
        let command = proto_doc_to_bson(command_doc)?;

        // Map the allow_all field to ValidationMode
        let validation_mode = if req.allow_all {
            ValidationMode::AllowAll
        } else {
            ValidationMode::BlockDangerous
        };

        let options = RawCommandOptions { validation_mode };

        // Execute the command
        let result = run_command(&self.pool, &req.database, command, &options)
            .await;

        self.record_analytics(OperationKind::RunCommand, &req.database, "", start.elapsed(), result.is_ok());
        let cmd_result = result.map_err(to_status)?;

        // Encode the result back to proto format
        let result_doc = bson_to_proto_doc(&cmd_result)?;

        Ok(Response::new(proto::RunCommandResponse {
            result: Some(result_doc),
        }))
    }

    // === Analytics ===

    #[tracing::instrument(skip(self, request))]
    async fn get_analytics(
        &self,
        request: Request<proto::GetAnalyticsRequest>,
    ) -> Result<Response<proto::GetAnalyticsResponse>, Status> {
        self.append_client_language(request.metadata());
        let analytics = self.analytics.as_ref()
            .ok_or_else(|| Status::unavailable("Analytics not enabled"))?;

        let events = analytics.snapshot();
        let summary = crate::analytics::aggregator::aggregate(&events);

        let top_operations = summary.top_operations.iter().map(|(op, count)| {
            proto::OperationCount {
                operation: format!("{:?}", op),
                count: *count as i64,
            }
        }).collect();

        let top_collections = summary.top_collections.iter().map(|(coll, count)| {
            proto::CollectionCount {
                collection: coll.clone(),
                count: *count as i64,
            }
        }).collect();

        Ok(Response::new(proto::GetAnalyticsResponse {
            total_operations: summary.total_operations as i64,
            total_errors: summary.total_errors as i64,
            error_rate: summary.error_rate,
            p50_latency_ms: summary.p50_latency_ms,
            p95_latency_ms: summary.p95_latency_ms,
            p99_latency_ms: summary.p99_latency_ms,
            top_operations,
            top_collections,
        }))
    }

    // === Ingestion ===

    #[tracing::instrument(skip(self, request))]
    async fn ingest(
        &self,
        request: Request<proto::IngestRequest>,
    ) -> Result<Response<proto::IngestResponse>, Status> {
        self.append_client_language(request.metadata());
        let engine = self.ingestion_engine.as_ref()
            .ok_or_else(|| Status::unavailable("Ingestion not enabled"))?;
        let client = self.client.as_ref()
            .ok_or_else(|| Status::unavailable("Client not configured"))?;
        let req = request.into_inner();

        let csv_options = req.csv_options.map(|opts| {
            crate::ingestion::CsvOptions {
                delimiter: opts.delimiter.bytes().next(),
                quote_char: opts.quote_char.bytes().next(),
                has_header: Some(opts.has_header),
                comment_char: opts.comment_char.bytes().next(),
            }
        }).unwrap_or_default();

        let options = crate::ingestion::IngestOptions {
            file_path: req.file_path,
            database: req.database,
            collection: req.collection,
            format: proto_format_to_internal(req.format),
            dedup_key: req.dedup_key,
            conflict_strategy: proto_conflict_to_internal(req.conflict_strategy),
            batch_size: if req.batch_size > 0 { req.batch_size as u32 } else { 1000 },
            concurrency: if req.concurrency > 0 { req.concurrency as u32 } else { 4 },
            expressions: req.expressions,
            schema_overrides: req.schema_overrides,
            sample_size: if req.sample_size > 0 { req.sample_size as u32 } else { 1000 },
            csv_options,
        };

        match engine.ingest(client, options).await {
            Ok(job) => {
                let mut schema_map = std::collections::HashMap::new();
                for field in &job.inferred_schema.fields {
                    schema_map.insert(field.name.clone(), format!("{:?}", field.bson_type));
                }
                Ok(Response::new(proto::IngestResponse {
                    job_id: job.job_id,
                    status: proto::IngestJobStatus::Running as i32,
                    inferred_schema: schema_map,
                    total_rows: job.total_rows,
                }))
            }
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    #[tracing::instrument(skip(self, request))]
    async fn get_ingest_status(
        &self,
        request: Request<proto::GetIngestStatusRequest>,
    ) -> Result<Response<proto::GetIngestStatusResponse>, Status> {
        self.append_client_language(request.metadata());
        let engine = self.ingestion_engine.as_ref()
            .ok_or_else(|| Status::unavailable("Ingestion not enabled"))?;
        let job_id = request.into_inner().job_id;

        match engine.get_status(&job_id).await {
            Ok(Some(job)) => {
                let elapsed = chrono::Utc::now()
                    .signed_duration_since(job.started_at)
                    .num_milliseconds();
                let estimated_remaining = if job.rows_processed > 0 {
                    ((job.total_rows - job.rows_processed) as f64
                        * (elapsed as f64 / job.rows_processed as f64)) as i64
                } else {
                    0
                };
                Ok(Response::new(proto::GetIngestStatusResponse {
                    job_id: job.job_id,
                    status: status_to_proto(job.status) as i32,
                    total_rows: job.total_rows,
                    rows_processed: job.rows_processed,
                    rows_inserted: job.rows_inserted,
                    rows_skipped: job.rows_skipped,
                    rows_failed: job.rows_failed,
                    elapsed_ms: elapsed,
                    estimated_remaining_ms: estimated_remaining,
                }))
            }
            Ok(None) => Err(Status::not_found(format!("Job '{}' not found", job_id))),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    #[tracing::instrument(skip(self, request))]
    async fn list_ingest_jobs(
        &self,
        request: Request<proto::ListIngestJobsRequest>,
    ) -> Result<Response<proto::ListIngestJobsResponse>, Status> {
        self.append_client_language(request.metadata());
        let engine = self.ingestion_engine.as_ref()
            .ok_or_else(|| Status::unavailable("Ingestion not enabled"))?;

        match engine.list_jobs().await {
            Ok(jobs) => {
                let summaries = jobs.iter().map(|j| proto::IngestJobSummary {
                    job_id: j.job_id.clone(),
                    file_path: j.file_path.clone(),
                    database: j.database.clone(),
                    collection: j.collection.clone(),
                    status: status_to_proto(j.status.clone()) as i32,
                    total_rows: j.total_rows,
                    rows_processed: j.rows_processed,
                }).collect();
                Ok(Response::new(proto::ListIngestJobsResponse { jobs: summaries }))
            }
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    #[tracing::instrument(skip(self, request))]
    async fn cancel_ingest(
        &self,
        request: Request<proto::CancelIngestRequest>,
    ) -> Result<Response<proto::CancelIngestResponse>, Status> {
        self.append_client_language(request.metadata());
        let engine = self.ingestion_engine.as_ref()
            .ok_or_else(|| Status::unavailable("Ingestion not enabled"))?;
        let job_id = request.into_inner().job_id;

        match engine.cancel(&job_id).await {
            Ok(()) => Ok(Response::new(proto::CancelIngestResponse { success: true })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    #[tracing::instrument(skip(self, request))]
    async fn watch_directory(
        &self,
        request: Request<proto::WatchDirectoryRequest>,
    ) -> Result<Response<proto::WatchDirectoryResponse>, Status> {
        self.append_client_language(request.metadata());
        let watcher = self.directory_watcher.as_ref()
            .ok_or_else(|| Status::unavailable("Ingestion not enabled"))?;
        let req = request.into_inner();

        let config = crate::ingestion::watch::WatchConfig {
            path: std::path::PathBuf::from(req.path),
            file_pattern: if req.file_pattern.is_empty() { "*".to_string() } else { req.file_pattern },
            database: req.database,
            collection: req.collection,
            conflict_strategy: proto_conflict_to_internal(req.conflict_strategy),
            dedup_key: req.dedup_key,
            debounce_ms: 1000,
        };

        match watcher.start_watch(config).await {
            Ok(watch_id) => Ok(Response::new(proto::WatchDirectoryResponse { watch_id })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    #[tracing::instrument(skip(self, request))]
    async fn stop_watch(
        &self,
        request: Request<proto::StopWatchRequest>,
    ) -> Result<Response<proto::StopWatchResponse>, Status> {
        self.append_client_language(request.metadata());
        let watcher = self.directory_watcher.as_ref()
            .ok_or_else(|| Status::unavailable("Ingestion not enabled"))?;
        let watch_id = request.into_inner().watch_id;

        match watcher.stop_watch(&watch_id).await {
            Ok(()) => Ok(Response::new(proto::StopWatchResponse { success: true })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    // === Pipeline ===

    #[tracing::instrument(skip(self, request))]
    async fn pipeline(
        &self,
        request: Request<proto::PipelineRequest>,
    ) -> Result<Response<proto::PipelineResponse>, Status> {
        self.append_client_language(request.metadata());
        self.check_tenant_quota(request.metadata())?;
        let start = std::time::Instant::now();
        let req = request.into_inner();

        if req.operations.is_empty() {
            return Err(Status::invalid_argument("Pipeline must contain at least one operation"));
        }
        if req.operations.len() > DEFAULT_PIPELINE_MAX_OPS {
            return Err(Status::invalid_argument(format!(
                "Pipeline exceeds maximum of {} operations",
                DEFAULT_PIPELINE_MAX_OPS
            )));
        }

        let semaphore = self.pipeline_semaphore.clone();
        let futures_vec: Vec<_> = req.operations.into_iter().enumerate().map(|(i, op)| {
            let sem = semaphore.clone();
            let svc = self;
            async move {
                let _permit = sem.acquire().await.unwrap();
                let result = svc.execute_pipeline_op(op).await;
                proto::PipelineResult {
                    index: i as u32,
                    result: Some(result),
                }
            }
        }).collect();

        let results = match tokio::time::timeout(
            self.pipeline_timeout,
            futures::future::join_all(futures_vec),
        ).await {
            Ok(results) => results,
            Err(_) => return Err(Status::deadline_exceeded("Pipeline timeout exceeded")),
        };

        let succeeded = results.iter().filter(|r| !pipeline_result_is_error(r)).count() as u32;
        let failed = results.iter().filter(|r| pipeline_result_is_error(r)).count() as u32;

        self.record_analytics(OperationKind::Pipeline, "", "", start.elapsed(), failed == 0);

        Ok(Response::new(proto::PipelineResponse {
            results,
            succeeded,
            failed,
        }))
    }
}

// === Pipeline helpers ===

/// Check if a pipeline result is an error.
fn pipeline_result_is_error(result: &proto::PipelineResult) -> bool {
    matches!(result.result, Some(proto::pipeline_result::Result::Error(_)))
}

impl MongoCoreService {
    /// Dispatch a single pipeline operation.
    async fn execute_pipeline_op(&self, op: proto::PipelineOperation) -> proto::pipeline_result::Result {
        match op.operation {
            None => proto::pipeline_result::Result::Error(proto::PipelineError {
                code: 3,
                message: "Operation not specified".to_string(),
            }),
            Some(operation) => match operation {
                proto::pipeline_operation::Operation::Find(req) => self.pipeline_find(req).await,
                proto::pipeline_operation::Operation::FindOne(req) => self.pipeline_find_one(req).await,
                proto::pipeline_operation::Operation::Insert(req) => self.pipeline_insert(req).await,
                proto::pipeline_operation::Operation::InsertMany(req) => self.pipeline_insert_many(req).await,
                proto::pipeline_operation::Operation::Update(req) => self.pipeline_update(req).await,
                proto::pipeline_operation::Operation::UpdateMany(req) => self.pipeline_update_many(req).await,
                proto::pipeline_operation::Operation::Delete(req) => self.pipeline_delete(req).await,
                proto::pipeline_operation::Operation::DeleteMany(req) => self.pipeline_delete_many(req).await,
                proto::pipeline_operation::Operation::Aggregate(req) => self.pipeline_aggregate(req).await,
                proto::pipeline_operation::Operation::FindAndModify(req) => self.pipeline_find_and_modify(req).await,
                proto::pipeline_operation::Operation::RunCommand(req) => self.pipeline_run_command(req).await,
                proto::pipeline_operation::Operation::Search(req) => self.pipeline_search(req).await,
                proto::pipeline_operation::Operation::CreateCollection(req) => self.pipeline_create_collection(req).await,
                proto::pipeline_operation::Operation::CreateIndex(req) => self.pipeline_create_index(req).await,
                proto::pipeline_operation::Operation::ListDatabases(req) => self.pipeline_list_databases(req).await,
                proto::pipeline_operation::Operation::ListCollections(req) => self.pipeline_list_collections(req).await,
                proto::pipeline_operation::Operation::BeginTransaction(req) => self.pipeline_begin_transaction(req).await,
                proto::pipeline_operation::Operation::CommitTransaction(req) => self.pipeline_commit_transaction(req).await,
                proto::pipeline_operation::Operation::AbortTransaction(req) => self.pipeline_abort_transaction(req).await,
                proto::pipeline_operation::Operation::GetAnalytics(req) => self.pipeline_get_analytics(req).await,
            },
        }
    }

    async fn pipeline_find(&self, req: proto::FindRequest) -> proto::pipeline_result::Result {
        let filter = match proto_filter_to_bson(&req.filter) {
            Ok(f) => f,
            Err(e) => return pipeline_err(e),
        };
        let options = match convert_find_options(&req.options) {
            Ok(o) => o,
            Err(e) => return pipeline_err(e),
        };

        let result = if let Some(ref txn_id) = req.transaction_id {
            match self.transactions.get_mut(txn_id) {
                Some(mut txn) => txn.find(&req.database, &req.collection, filter).await,
                None => return pipeline_err(Status::not_found(format!("Transaction not found: {}", txn_id))),
            }
        } else {
            self.operations.find(&req.database, &req.collection, filter, options).await
        };

        match result {
            Ok(docs) => {
                let documents: Result<Vec<proto::Document>, Status> = docs.iter().map(bson_to_proto_doc).collect();
                match documents {
                    Ok(documents) => proto::pipeline_result::Result::Find(proto::FindResponse {
                        documents,
                        metadata: Some(proto::ResponseMetadata { search_method: String::new() }),
                    }),
                    Err(e) => pipeline_err(e),
                }
            }
            Err(e) => pipeline_err(to_status(e)),
        }
    }

    async fn pipeline_find_one(&self, req: proto::FindOneRequest) -> proto::pipeline_result::Result {
        let filter = match proto_filter_to_bson(&req.filter) {
            Ok(f) => f,
            Err(e) => return pipeline_err(e),
        };

        let result = self.operations.find_one(&req.database, &req.collection, filter).await;

        match result {
            Ok(doc) => {
                let document = match doc {
                    Some(ref d) => match bson_to_proto_doc(d) {
                        Ok(pd) => Some(pd),
                        Err(e) => return pipeline_err(e),
                    },
                    None => None,
                };
                proto::pipeline_result::Result::FindOne(proto::FindOneResponse {
                    document,
                    metadata: Some(proto::ResponseMetadata { search_method: String::new() }),
                })
            }
            Err(e) => pipeline_err(to_status(e)),
        }
    }

    async fn pipeline_insert(&self, req: proto::InsertRequest) -> proto::pipeline_result::Result {
        let doc = match req.document.as_ref() {
            Some(d) => match proto_doc_to_bson(d) {
                Ok(d) => d,
                Err(e) => return pipeline_err(e),
            },
            None => return pipeline_err(Status::invalid_argument("Missing document")),
        };

        let result = if let Some(ref txn_id) = req.transaction_id {
            match self.transactions.get_mut(txn_id) {
                Some(mut txn) => txn.insert(&req.database, &req.collection, doc).await,
                None => return pipeline_err(Status::not_found(format!("Transaction not found: {}", txn_id))),
            }
        } else {
            self.operations.insert(&req.database, &req.collection, doc).await
        };

        match result {
            Ok(r) => proto::pipeline_result::Result::Insert(proto::InsertResponse {
                inserted_id: r.inserted_id.to_string(),
            }),
            Err(e) => pipeline_err(to_status(e)),
        }
    }

    async fn pipeline_insert_many(&self, req: proto::InsertManyRequest) -> proto::pipeline_result::Result {
        let docs: Result<Vec<bson::Document>, Status> = req.documents.iter().map(proto_doc_to_bson).collect();
        let docs = match docs {
            Ok(d) => d,
            Err(e) => return pipeline_err(e),
        };

        let result = self.operations.insert_many(&req.database, &req.collection, docs).await;

        match result {
            Ok(r) => {
                let inserted_ids: Vec<String> = r.inserted_ids.values().map(|id| id.to_string()).collect();
                let inserted_count = inserted_ids.len() as i64;
                proto::pipeline_result::Result::InsertMany(proto::InsertManyResponse {
                    inserted_ids,
                    inserted_count,
                })
            }
            Err(e) => pipeline_err(to_status(e)),
        }
    }

    async fn pipeline_update(&self, req: proto::UpdateRequest) -> proto::pipeline_result::Result {
        let filter = match proto_filter_to_bson(&req.filter) {
            Ok(f) => f,
            Err(e) => return pipeline_err(e),
        };
        let update = match req.update.as_ref() {
            Some(d) => match proto_doc_to_bson(d) {
                Ok(d) => d,
                Err(e) => return pipeline_err(e),
            },
            None => return pipeline_err(Status::invalid_argument("Missing update document")),
        };

        let result = if let Some(ref txn_id) = req.transaction_id {
            match self.transactions.get_mut(txn_id) {
                Some(mut txn) => txn.update(&req.database, &req.collection, filter, update).await,
                None => return pipeline_err(Status::not_found(format!("Transaction not found: {}", txn_id))),
            }
        } else {
            self.operations.update(&req.database, &req.collection, filter, update).await
        };

        match result {
            Ok(r) => proto::pipeline_result::Result::Update(proto::UpdateResponse {
                matched_count: r.matched_count as i64,
                modified_count: r.modified_count as i64,
                upserted_id: r.upserted_id.map(|id| id.to_string()),
            }),
            Err(e) => pipeline_err(to_status(e)),
        }
    }

    async fn pipeline_update_many(&self, req: proto::UpdateManyRequest) -> proto::pipeline_result::Result {
        let filter = match proto_filter_to_bson(&req.filter) {
            Ok(f) => f,
            Err(e) => return pipeline_err(e),
        };
        let update = match req.update.as_ref() {
            Some(d) => match proto_doc_to_bson(d) {
                Ok(d) => d,
                Err(e) => return pipeline_err(e),
            },
            None => return pipeline_err(Status::invalid_argument("Missing update document")),
        };

        let result = self.operations.update_many(&req.database, &req.collection, filter, update).await;

        match result {
            Ok(r) => proto::pipeline_result::Result::UpdateMany(proto::UpdateManyResponse {
                matched_count: r.matched_count as i64,
                modified_count: r.modified_count as i64,
                upserted_id: r.upserted_id.map(|id| id.to_string()),
            }),
            Err(e) => pipeline_err(to_status(e)),
        }
    }

    async fn pipeline_delete(&self, req: proto::DeleteRequest) -> proto::pipeline_result::Result {
        let filter = match proto_filter_to_bson(&req.filter) {
            Ok(f) => f,
            Err(e) => return pipeline_err(e),
        };

        let result = if let Some(ref txn_id) = req.transaction_id {
            match self.transactions.get_mut(txn_id) {
                Some(mut txn) => txn.delete(&req.database, &req.collection, filter).await,
                None => return pipeline_err(Status::not_found(format!("Transaction not found: {}", txn_id))),
            }
        } else {
            self.operations.delete(&req.database, &req.collection, filter).await
        };

        match result {
            Ok(r) => proto::pipeline_result::Result::Delete(proto::DeleteResponse {
                deleted_count: r.deleted_count as i64,
            }),
            Err(e) => pipeline_err(to_status(e)),
        }
    }

    async fn pipeline_delete_many(&self, req: proto::DeleteManyRequest) -> proto::pipeline_result::Result {
        let filter = match proto_filter_to_bson(&req.filter) {
            Ok(f) => f,
            Err(e) => return pipeline_err(e),
        };

        let result = self.operations.delete_many(&req.database, &req.collection, filter).await;

        match result {
            Ok(r) => proto::pipeline_result::Result::DeleteMany(proto::DeleteManyResponse {
                deleted_count: r.deleted_count as i64,
            }),
            Err(e) => pipeline_err(to_status(e)),
        }
    }

    async fn pipeline_aggregate(&self, req: proto::AggregateRequest) -> proto::pipeline_result::Result {
        let pipeline: Vec<bson::Document> = match req.pipeline {
            Some(p) => match p.stages.iter().map(|stage| decode_bson(stage)).collect::<Result<Vec<_>, _>>() {
                Ok(stages) => stages,
                Err(e) => return pipeline_err(e),
            },
            None => Vec::new(),
        };

        let result = self.operations.aggregate(&req.database, &req.collection, pipeline).await;

        match result {
            Ok(docs) => {
                let documents: Result<Vec<proto::Document>, Status> = docs.iter().map(bson_to_proto_doc).collect();
                match documents {
                    Ok(documents) => proto::pipeline_result::Result::Aggregate(proto::AggregateResponse {
                        documents,
                        metadata: Some(proto::ResponseMetadata { search_method: String::new() }),
                    }),
                    Err(e) => pipeline_err(e),
                }
            }
            Err(e) => pipeline_err(to_status(e)),
        }
    }

    async fn pipeline_find_and_modify(&self, req: proto::FindAndModifyRequest) -> proto::pipeline_result::Result {
        let filter = match proto_filter_to_bson(&req.filter) {
            Ok(f) => f,
            Err(e) => return pipeline_err(e),
        };
        let update = match req.update.as_ref() {
            Some(d) => match proto_doc_to_bson(d) {
                Ok(d) => d,
                Err(e) => return pipeline_err(e),
            },
            None => return pipeline_err(Status::invalid_argument("Missing update document")),
        };

        let options = req.options.map(|opts| {
            let return_document =
                if opts.return_document == proto::find_and_modify_options::ReturnDocument::Before as i32 {
                    ReturnDocumentOption::Before
                } else {
                    ReturnDocumentOption::After
                };
            let sort = opts
                .sort
                .as_ref()
                .and_then(|data| if data.is_empty() { None } else { decode_bson(data).ok() });
            FindAndModifyOptions {
                return_document,
                upsert: opts.upsert,
                sort,
            }
        });

        let result = self.operations.find_and_modify(&req.database, &req.collection, filter, update, options).await;

        match result {
            Ok(doc) => {
                let document = match doc {
                    Some(ref d) => match bson_to_proto_doc(d) {
                        Ok(pd) => Some(pd),
                        Err(e) => return pipeline_err(e),
                    },
                    None => None,
                };
                proto::pipeline_result::Result::FindAndModify(proto::FindAndModifyResponse { document })
            }
            Err(e) => pipeline_err(to_status(e)),
        }
    }

    async fn pipeline_run_command(&self, req: proto::RunCommandRequest) -> proto::pipeline_result::Result {
        let command = match req.command.as_ref() {
            Some(d) => match proto_doc_to_bson(d) {
                Ok(d) => d,
                Err(e) => return pipeline_err(e),
            },
            None => return pipeline_err(Status::invalid_argument("Missing command document")),
        };

        let validation_mode = if req.allow_all {
            ValidationMode::AllowAll
        } else {
            ValidationMode::BlockDangerous
        };

        let options = RawCommandOptions { validation_mode };
        let result = run_command(&self.pool, &req.database, command, &options).await;

        match result {
            Ok(doc) => match bson_to_proto_doc(&doc) {
                Ok(result_doc) => proto::pipeline_result::Result::RunCommand(proto::RunCommandResponse {
                    result: Some(result_doc),
                }),
                Err(e) => pipeline_err(e),
            },
            Err(e) => pipeline_err(to_status(e)),
        }
    }

    async fn pipeline_search(&self, req: proto::SearchRequest) -> proto::pipeline_result::Result {
        let limit = if req.limit > 0 { req.limit } else { 10 };

        let result = self.search_engine.search(&req.database, &req.collection, &req.query, limit).await;

        match result {
            Ok(search_result) => {
                let documents: Result<Vec<proto::Document>, Status> =
                    search_result.documents.iter().map(bson_to_proto_doc).collect();
                match documents {
                    Ok(documents) => {
                        let method = match search_result.method {
                            crate::search::SearchMethod::Vector => "vector",
                            crate::search::SearchMethod::Fulltext => "fulltext",
                            crate::search::SearchMethod::Filter => "filter",
                        };
                        proto::pipeline_result::Result::Search(proto::SearchResponse {
                            documents,
                            method: method.to_string(),
                            total: search_result.total as i64,
                        })
                    }
                    Err(e) => pipeline_err(e),
                }
            }
            Err(e) => pipeline_err(Status::internal(format!("Search error: {}", e))),
        }
    }

    async fn pipeline_create_collection(&self, req: proto::CreateCollectionRequest) -> proto::pipeline_result::Result {
        match self.operations.create_collection(&req.database, &req.collection).await {
            Ok(()) => proto::pipeline_result::Result::CreateCollection(proto::CreateCollectionResponse {}),
            Err(e) => pipeline_err(to_status(e)),
        }
    }

    async fn pipeline_create_index(&self, req: proto::CreateIndexRequest) -> proto::pipeline_result::Result {
        let keys = match req.keys.as_ref() {
            Some(d) => match proto_doc_to_bson(d) {
                Ok(d) => d,
                Err(e) => return pipeline_err(e),
            },
            None => return pipeline_err(Status::invalid_argument("Missing keys document")),
        };

        let options = req.options.map(|opts| IndexOptions {
            name: opts.name,
            unique: opts.unique,
            sparse: opts.sparse,
        });

        match self.operations.create_index(&req.database, &req.collection, keys, options).await {
            Ok(index_name) => proto::pipeline_result::Result::CreateIndex(proto::CreateIndexResponse { index_name }),
            Err(e) => pipeline_err(to_status(e)),
        }
    }

    async fn pipeline_list_databases(&self, _req: proto::ListDatabasesRequest) -> proto::pipeline_result::Result {
        match self.pool.client().list_database_names().await {
            Ok(databases) => proto::pipeline_result::Result::ListDatabases(proto::ListDatabasesResponse { databases }),
            Err(e) => pipeline_err(Status::internal(format!("Failed to list databases: {}", e))),
        }
    }

    async fn pipeline_list_collections(&self, req: proto::ListCollectionsRequest) -> proto::pipeline_result::Result {
        match self.pool.database(&req.database).list_collection_names().await {
            Ok(collections) => proto::pipeline_result::Result::ListCollections(proto::ListCollectionsResponse { collections }),
            Err(e) => pipeline_err(Status::internal(format!("Failed to list collections: {}", e))),
        }
    }

    async fn pipeline_begin_transaction(&self, _req: proto::BeginTransactionRequest) -> proto::pipeline_result::Result {
        match Transaction::begin(&self.pool).await {
            Ok(txn) => {
                let txn_id = Uuid::new_v4().to_string();
                self.transactions.insert(txn_id.clone(), txn);
                proto::pipeline_result::Result::BeginTransaction(proto::BeginTransactionResponse {
                    transaction_id: txn_id,
                })
            }
            Err(e) => pipeline_err(to_status(e)),
        }
    }

    async fn pipeline_commit_transaction(&self, req: proto::CommitTransactionRequest) -> proto::pipeline_result::Result {
        match self.transactions.remove(&req.transaction_id) {
            Some((_, mut txn)) => match txn.commit().await {
                Ok(()) => proto::pipeline_result::Result::CommitTransaction(proto::CommitTransactionResponse {}),
                Err(e) => pipeline_err(to_status(e)),
            },
            None => pipeline_err(Status::not_found(format!("Transaction not found: {}", req.transaction_id))),
        }
    }

    async fn pipeline_abort_transaction(&self, req: proto::AbortTransactionRequest) -> proto::pipeline_result::Result {
        match self.transactions.remove(&req.transaction_id) {
            Some((_, mut txn)) => match txn.abort().await {
                Ok(()) => proto::pipeline_result::Result::AbortTransaction(proto::AbortTransactionResponse {}),
                Err(e) => pipeline_err(to_status(e)),
            },
            None => pipeline_err(Status::not_found(format!("Transaction not found: {}", req.transaction_id))),
        }
    }

    async fn pipeline_get_analytics(&self, _req: proto::GetAnalyticsRequest) -> proto::pipeline_result::Result {
        let analytics = match self.analytics.as_ref() {
            Some(a) => a,
            None => return pipeline_err(Status::unavailable("Analytics not enabled")),
        };

        let events = analytics.snapshot();
        let summary = crate::analytics::aggregator::aggregate(&events);

        let top_operations = summary.top_operations.iter().map(|(op, count)| {
            proto::OperationCount {
                operation: format!("{:?}", op),
                count: *count as i64,
            }
        }).collect();

        let top_collections = summary.top_collections.iter().map(|(coll, count)| {
            proto::CollectionCount {
                collection: coll.clone(),
                count: *count as i64,
            }
        }).collect();

        proto::pipeline_result::Result::GetAnalytics(proto::GetAnalyticsResponse {
            total_operations: summary.total_operations as i64,
            total_errors: summary.total_errors as i64,
            error_rate: summary.error_rate,
            p50_latency_ms: summary.p50_latency_ms,
            p95_latency_ms: summary.p95_latency_ms,
            p99_latency_ms: summary.p99_latency_ms,
            top_operations,
            top_collections,
        })
    }
}

/// Convert a tonic Status into a PipelineError result.
fn pipeline_err(status: Status) -> proto::pipeline_result::Result {
    proto::pipeline_result::Result::Error(proto::PipelineError {
        code: status.code() as i32,
        message: status.message().to_string(),
    })
}

// === Ingestion helper functions ===

fn proto_format_to_internal(format: i32) -> crate::ingestion::FileFormat {
    match proto::FileFormat::try_from(format).unwrap_or(proto::FileFormat::Auto) {
        proto::FileFormat::Auto => crate::ingestion::FileFormat::Auto,
        proto::FileFormat::Csv => crate::ingestion::FileFormat::Csv,
        proto::FileFormat::Json => crate::ingestion::FileFormat::Json,
        proto::FileFormat::Ndjson => crate::ingestion::FileFormat::NdJson,
        proto::FileFormat::Parquet => crate::ingestion::FileFormat::Parquet,
    }
}

fn proto_conflict_to_internal(strategy: i32) -> crate::ingestion::ConflictStrategy {
    match proto::ConflictStrategy::try_from(strategy).unwrap_or(proto::ConflictStrategy::Skip) {
        proto::ConflictStrategy::Skip => crate::ingestion::ConflictStrategy::Skip,
        proto::ConflictStrategy::Overwrite => crate::ingestion::ConflictStrategy::Overwrite,
        proto::ConflictStrategy::Merge => crate::ingestion::ConflictStrategy::Merge,
    }
}

fn status_to_proto(status: crate::ingestion::IngestStatus) -> proto::IngestJobStatus {
    match status {
        crate::ingestion::IngestStatus::Running => proto::IngestJobStatus::Running,
        crate::ingestion::IngestStatus::Completed => proto::IngestJobStatus::Completed,
        crate::ingestion::IngestStatus::Failed => proto::IngestJobStatus::Failed,
        crate::ingestion::IngestStatus::Cancelled => proto::IngestJobStatus::Cancelled,
    }
}
