use std::pin::Pin;

use bson::Document;
use dashmap::DashMap;
use futures::StreamExt;
use tonic::{Request, Response, Status};
use uuid::Uuid;

use crate::connection::pool::ConnectionPool;
use crate::error::MongoCoreError;
use crate::operations::{
    FindAndModifyOptions, FindOptions, IndexOptions, Operations, ReturnDocumentOption, Transaction,
};

use super::proto::{self, mongo_core_server::MongoCore};

/// The gRPC service implementation for MongoCore.
pub struct MongoCoreService {
    operations: Operations,
    pool: ConnectionPool,
    transactions: DashMap<String, Transaction>,
}

impl MongoCoreService {
    /// Create a new MongoCoreService from a ConnectionPool.
    pub fn new(pool: ConnectionPool) -> Self {
        let operations = Operations::new(pool.clone());
        Self {
            operations,
            pool,
            transactions: DashMap::new(),
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
    }
}

#[tonic::async_trait]
impl MongoCore for MongoCoreService {
    // === CRUD ===

    async fn find(
        &self,
        request: Request<proto::FindRequest>,
    ) -> Result<Response<proto::FindResponse>, Status> {
        let req = request.into_inner();
        let filter = proto_filter_to_bson(&req.filter)?;
        let options = convert_find_options(&req.options)?;

        // Check if this is a transactional operation
        let docs = if let Some(ref txn_id) = req.transaction_id {
            let mut txn = self
                .transactions
                .get_mut(txn_id)
                .ok_or_else(|| Status::not_found(format!("Transaction not found: {}", txn_id)))?;
            txn.find(&req.database, &req.collection, filter)
                .await
                .map_err(to_status)?
        } else {
            self.operations
                .find(&req.database, &req.collection, filter, options)
                .await
                .map_err(to_status)?
        };

        let documents: Result<Vec<proto::Document>, Status> =
            docs.iter().map(bson_to_proto_doc).collect();

        Ok(Response::new(proto::FindResponse {
            documents: documents?,
            metadata: Some(proto::ResponseMetadata {
                search_method: String::new(),
            }),
        }))
    }

    async fn find_one(
        &self,
        request: Request<proto::FindOneRequest>,
    ) -> Result<Response<proto::FindOneResponse>, Status> {
        let req = request.into_inner();
        let filter = proto_filter_to_bson(&req.filter)?;

        let doc = self
            .operations
            .find_one(&req.database, &req.collection, filter)
            .await
            .map_err(to_status)?;

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

    async fn insert(
        &self,
        request: Request<proto::InsertRequest>,
    ) -> Result<Response<proto::InsertResponse>, Status> {
        let req = request.into_inner();
        let doc = req
            .document
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing document"))?;
        let bson_doc = proto_doc_to_bson(doc)?;

        // Check if this is a transactional operation
        let inserted_id = if let Some(ref txn_id) = req.transaction_id {
            let mut txn = self
                .transactions
                .get_mut(txn_id)
                .ok_or_else(|| Status::not_found(format!("Transaction not found: {}", txn_id)))?;
            let result = txn
                .insert(&req.database, &req.collection, bson_doc)
                .await
                .map_err(to_status)?;
            result.inserted_id.to_string()
        } else {
            let result = self
                .operations
                .insert(&req.database, &req.collection, bson_doc)
                .await
                .map_err(to_status)?;
            result.inserted_id.to_string()
        };

        Ok(Response::new(proto::InsertResponse { inserted_id }))
    }

    async fn insert_many(
        &self,
        request: Request<proto::InsertManyRequest>,
    ) -> Result<Response<proto::InsertManyResponse>, Status> {
        let req = request.into_inner();
        let docs: Result<Vec<bson::Document>, Status> =
            req.documents.iter().map(proto_doc_to_bson).collect();
        let docs = docs?;

        let result = self
            .operations
            .insert_many(&req.database, &req.collection, docs)
            .await
            .map_err(to_status)?;

        let inserted_ids: Vec<String> = result
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

    async fn update(
        &self,
        request: Request<proto::UpdateRequest>,
    ) -> Result<Response<proto::UpdateResponse>, Status> {
        let req = request.into_inner();
        let filter = proto_filter_to_bson(&req.filter)?;
        let update_doc = req
            .update
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing update document"))?;
        let update = proto_doc_to_bson(update_doc)?;

        // Check if this is a transactional operation
        let (matched_count, modified_count, upserted_id) =
            if let Some(ref txn_id) = req.transaction_id {
                let mut txn = self.transactions.get_mut(txn_id).ok_or_else(|| {
                    Status::not_found(format!("Transaction not found: {}", txn_id))
                })?;
                let result = txn
                    .update(&req.database, &req.collection, filter, update)
                    .await
                    .map_err(to_status)?;
                (
                    result.matched_count as i64,
                    result.modified_count as i64,
                    result.upserted_id.map(|id| id.to_string()),
                )
            } else {
                let result = self
                    .operations
                    .update(&req.database, &req.collection, filter, update)
                    .await
                    .map_err(to_status)?;
                (
                    result.matched_count as i64,
                    result.modified_count as i64,
                    result.upserted_id.map(|id| id.to_string()),
                )
            };

        Ok(Response::new(proto::UpdateResponse {
            matched_count,
            modified_count,
            upserted_id,
        }))
    }

    async fn update_many(
        &self,
        request: Request<proto::UpdateManyRequest>,
    ) -> Result<Response<proto::UpdateManyResponse>, Status> {
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
            .await
            .map_err(to_status)?;

        Ok(Response::new(proto::UpdateManyResponse {
            matched_count: result.matched_count as i64,
            modified_count: result.modified_count as i64,
            upserted_id: result.upserted_id.map(|id| id.to_string()),
        }))
    }

    async fn delete(
        &self,
        request: Request<proto::DeleteRequest>,
    ) -> Result<Response<proto::DeleteResponse>, Status> {
        let req = request.into_inner();
        let filter = proto_filter_to_bson(&req.filter)?;

        // Check if this is a transactional operation
        let deleted_count = if let Some(ref txn_id) = req.transaction_id {
            let mut txn = self
                .transactions
                .get_mut(txn_id)
                .ok_or_else(|| Status::not_found(format!("Transaction not found: {}", txn_id)))?;
            let result = txn
                .delete(&req.database, &req.collection, filter)
                .await
                .map_err(to_status)?;
            result.deleted_count as i64
        } else {
            let result = self
                .operations
                .delete(&req.database, &req.collection, filter)
                .await
                .map_err(to_status)?;
            result.deleted_count as i64
        };

        Ok(Response::new(proto::DeleteResponse { deleted_count }))
    }

    async fn delete_many(
        &self,
        request: Request<proto::DeleteManyRequest>,
    ) -> Result<Response<proto::DeleteManyResponse>, Status> {
        let req = request.into_inner();
        let filter = proto_filter_to_bson(&req.filter)?;

        let result = self
            .operations
            .delete_many(&req.database, &req.collection, filter)
            .await
            .map_err(to_status)?;

        Ok(Response::new(proto::DeleteManyResponse {
            deleted_count: result.deleted_count as i64,
        }))
    }

    async fn find_and_modify(
        &self,
        request: Request<proto::FindAndModifyRequest>,
    ) -> Result<Response<proto::FindAndModifyResponse>, Status> {
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
            .await
            .map_err(to_status)?;

        let document = match result {
            Some(ref d) => Some(bson_to_proto_doc(d)?),
            None => None,
        };

        Ok(Response::new(proto::FindAndModifyResponse { document }))
    }

    // === Aggregation ===

    async fn aggregate(
        &self,
        request: Request<proto::AggregateRequest>,
    ) -> Result<Response<proto::AggregateResponse>, Status> {
        let req = request.into_inner();

        let pipeline: Vec<bson::Document> = match req.pipeline {
            Some(p) => p
                .stages
                .iter()
                .map(|stage| decode_bson(stage))
                .collect::<Result<Vec<_>, _>>()?,
            None => Vec::new(),
        };

        let docs = self
            .operations
            .aggregate(&req.database, &req.collection, pipeline)
            .await
            .map_err(to_status)?;

        let documents: Result<Vec<proto::Document>, Status> =
            docs.iter().map(bson_to_proto_doc).collect();

        Ok(Response::new(proto::AggregateResponse {
            documents: documents?,
            metadata: Some(proto::ResponseMetadata {
                search_method: String::new(),
            }),
        }))
    }

    // === Transactions ===

    async fn begin_transaction(
        &self,
        _request: Request<proto::BeginTransactionRequest>,
    ) -> Result<Response<proto::BeginTransactionResponse>, Status> {
        let txn = Transaction::begin(&self.pool).await.map_err(to_status)?;
        let txn_id = Uuid::new_v4().to_string();
        self.transactions.insert(txn_id.clone(), txn);

        Ok(Response::new(proto::BeginTransactionResponse {
            transaction_id: txn_id,
        }))
    }

    async fn commit_transaction(
        &self,
        request: Request<proto::CommitTransactionRequest>,
    ) -> Result<Response<proto::CommitTransactionResponse>, Status> {
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

    async fn abort_transaction(
        &self,
        request: Request<proto::AbortTransactionRequest>,
    ) -> Result<Response<proto::AbortTransactionResponse>, Status> {
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

    async fn create_collection(
        &self,
        request: Request<proto::CreateCollectionRequest>,
    ) -> Result<Response<proto::CreateCollectionResponse>, Status> {
        let req = request.into_inner();

        self.operations
            .create_collection(&req.database, &req.collection)
            .await
            .map_err(to_status)?;

        Ok(Response::new(proto::CreateCollectionResponse {}))
    }

    async fn create_index(
        &self,
        request: Request<proto::CreateIndexRequest>,
    ) -> Result<Response<proto::CreateIndexResponse>, Status> {
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

    async fn list_databases(
        &self,
        _request: Request<proto::ListDatabasesRequest>,
    ) -> Result<Response<proto::ListDatabasesResponse>, Status> {
        let databases = self
            .pool
            .client()
            .list_database_names()
            .await
            .map_err(|e| Status::internal(format!("Failed to list databases: {}", e)))?;

        Ok(Response::new(proto::ListDatabasesResponse { databases }))
    }

    async fn list_collections(
        &self,
        request: Request<proto::ListCollectionsRequest>,
    ) -> Result<Response<proto::ListCollectionsResponse>, Status> {
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

    async fn watch(
        &self,
        request: Request<proto::WatchRequest>,
    ) -> Result<Response<Self::WatchStream>, Status> {
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
}
