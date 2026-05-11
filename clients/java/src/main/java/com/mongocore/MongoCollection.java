package com.mongocore;

import org.bson.Document;
import java.util.List;

public class MongoCollection {
    private final MongoClient client;
    private final String database;
    private final String name;

    MongoCollection(MongoClient client, String database, String name) {
        this.client = client;
        this.database = database;
        this.name = name;
    }

    public String getName() {
        return name;
    }

    public String getDatabase() {
        return database;
    }

    /**
     * Find documents matching the filter.
     */
    public List<Document> find(Document filter) {
        // Will use generated gRPC stub to call Find RPC
        // Encodes filter as raw BSON bytes, sends via proto, decodes response
        throw new UnsupportedOperationException("Requires generated gRPC stubs. Run: mvn generate-sources");
    }

    /**
     * Find documents with options.
     */
    public List<Document> find(Document filter, FindOptions options) {
        throw new UnsupportedOperationException("Requires generated gRPC stubs. Run: mvn generate-sources");
    }

    /**
     * Find a single document.
     */
    public Document findOne(Document filter) {
        throw new UnsupportedOperationException("Requires generated gRPC stubs. Run: mvn generate-sources");
    }

    /**
     * Insert a single document.
     */
    public InsertResult insertOne(Document document) {
        throw new UnsupportedOperationException("Requires generated gRPC stubs. Run: mvn generate-sources");
    }

    /**
     * Insert multiple documents.
     */
    public InsertManyResult insertMany(List<Document> documents) {
        throw new UnsupportedOperationException("Requires generated gRPC stubs. Run: mvn generate-sources");
    }

    /**
     * Update a single document.
     */
    public UpdateResult updateOne(Document filter, Document update) {
        throw new UnsupportedOperationException("Requires generated gRPC stubs. Run: mvn generate-sources");
    }

    /**
     * Update multiple documents.
     */
    public UpdateResult updateMany(Document filter, Document update) {
        throw new UnsupportedOperationException("Requires generated gRPC stubs. Run: mvn generate-sources");
    }

    /**
     * Delete a single document.
     */
    public long deleteOne(Document filter) {
        throw new UnsupportedOperationException("Requires generated gRPC stubs. Run: mvn generate-sources");
    }

    /**
     * Delete multiple documents.
     */
    public long deleteMany(Document filter) {
        throw new UnsupportedOperationException("Requires generated gRPC stubs. Run: mvn generate-sources");
    }

    /**
     * Run an aggregation pipeline.
     */
    public List<Document> aggregate(List<Document> pipeline) {
        throw new UnsupportedOperationException("Requires generated gRPC stubs. Run: mvn generate-sources");
    }
}
