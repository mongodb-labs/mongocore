package com.mongocore;

import mongocore.v1.Mongocore;
import mongocore.v1.Types;
import org.bson.Document;
import org.bson.codecs.DocumentCodec;
import org.bson.codecs.DecoderContext;
import org.bson.BsonBinaryReader;
import com.google.protobuf.ByteString;

import java.nio.ByteBuffer;
import java.util.ArrayList;
import java.util.List;

/**
 * Wraps a PipelineResult proto message.
 * Provides typed accessors for different operation result types.
 */
public class PipelineResult {
    private static final DocumentCodec CODEC = new DocumentCodec();
    private final Mongocore.PipelineResult result;

    public PipelineResult(Mongocore.PipelineResult result) {
        this.result = result;
    }

    public int getIndex() {
        return result.getIndex();
    }

    public boolean isSuccess() {
        return !result.hasError();
    }

    public String getError() {
        if (result.hasError()) {
            return result.getError().getMessage();
        }
        return null;
    }

    // Typed accessors for different result types

    public FindResult asFind() {
        if (!result.hasFind()) {
            throw new IllegalStateException("Result is not a find operation");
        }
        Mongocore.FindResponse resp = result.getFind();
        List<Document> docs = new ArrayList<>(resp.getDocumentsCount());
        for (Types.Document d : resp.getDocumentsList()) {
            docs.add(decodeBson(d.getData()));
        }
        return new FindResult(docs);
    }

    public FindOneResult asFindOne() {
        if (!result.hasFindOne()) {
            throw new IllegalStateException("Result is not a findOne operation");
        }
        Mongocore.FindOneResponse resp = result.getFindOne();
        if (resp.hasDocument() && !resp.getDocument().getData().isEmpty()) {
            return new FindOneResult(decodeBson(resp.getDocument().getData()));
        }
        return new FindOneResult(null);
    }

    public InsertResult asInsert() {
        if (!result.hasInsert()) {
            throw new IllegalStateException("Result is not an insert operation");
        }
        Mongocore.InsertResponse resp = result.getInsert();
        return new InsertResult(resp.getInsertedId());
    }

    public InsertManyResult asInsertMany() {
        if (!result.hasInsertMany()) {
            throw new IllegalStateException("Result is not an insertMany operation");
        }
        Mongocore.InsertManyResponse resp = result.getInsertMany();
        return new InsertManyResult(resp.getInsertedIdsList());
    }

    public UpdateResult asUpdate() {
        if (!result.hasUpdate()) {
            throw new IllegalStateException("Result is not an update operation");
        }
        Mongocore.UpdateResponse resp = result.getUpdate();
        return new UpdateResult(resp.getMatchedCount(), resp.getModifiedCount(), "");
    }

    public UpdateResult asUpdateMany() {
        if (!result.hasUpdateMany()) {
            throw new IllegalStateException("Result is not an updateMany operation");
        }
        Mongocore.UpdateManyResponse resp = result.getUpdateMany();
        return new UpdateResult(resp.getMatchedCount(), resp.getModifiedCount(), "");
    }

    public DeleteResult asDelete() {
        if (!result.hasDelete()) {
            throw new IllegalStateException("Result is not a delete operation");
        }
        Mongocore.DeleteResponse resp = result.getDelete();
        return new DeleteResult(resp.getDeletedCount());
    }

    public DeleteResult asDeleteMany() {
        if (!result.hasDeleteMany()) {
            throw new IllegalStateException("Result is not a deleteMany operation");
        }
        Mongocore.DeleteManyResponse resp = result.getDeleteMany();
        return new DeleteResult(resp.getDeletedCount());
    }

    public AggregateResult asAggregate() {
        if (!result.hasAggregate()) {
            throw new IllegalStateException("Result is not an aggregate operation");
        }
        Mongocore.AggregateResponse resp = result.getAggregate();
        List<Document> docs = new ArrayList<>(resp.getDocumentsCount());
        for (Types.Document d : resp.getDocumentsList()) {
            docs.add(decodeBson(d.getData()));
        }
        return new AggregateResult(docs);
    }

    public RunCommandResult asRunCommand() {
        if (!result.hasRunCommand()) {
            throw new IllegalStateException("Result is not a runCommand operation");
        }
        Mongocore.RunCommandResponse resp = result.getRunCommand();
        return new RunCommandResult(decodeBson(resp.getResult().getData()));
    }

    public ListDatabasesResult asListDatabases() {
        if (!result.hasListDatabases()) {
            throw new IllegalStateException("Result is not a listDatabases operation");
        }
        Mongocore.ListDatabasesResponse resp = result.getListDatabases();
        return new ListDatabasesResult(resp.getDatabasesList());
    }

    public ListCollectionsResult asListCollections() {
        if (!result.hasListCollections()) {
            throw new IllegalStateException("Result is not a listCollections operation");
        }
        Mongocore.ListCollectionsResponse resp = result.getListCollections();
        return new ListCollectionsResult(resp.getCollectionsList());
    }

    private static Document decodeBson(ByteString data) {
        byte[] bytes = data.toByteArray();
        BsonBinaryReader reader = new BsonBinaryReader(ByteBuffer.wrap(bytes));
        return CODEC.decode(reader, DecoderContext.builder().build());
    }

    // Result wrapper classes

    public static class FindResult {
        private final List<Document> documents;

        public FindResult(List<Document> documents) {
            this.documents = documents;
        }

        public List<Document> getDocuments() {
            return documents;
        }
    }

    public static class FindOneResult {
        private final Document document;

        public FindOneResult(Document document) {
            this.document = document;
        }

        public Document getDocument() {
            return document;
        }
    }

    public static class DeleteResult {
        private final long deletedCount;

        public DeleteResult(long deletedCount) {
            this.deletedCount = deletedCount;
        }

        public long getDeletedCount() {
            return deletedCount;
        }
    }

    public static class AggregateResult {
        private final List<Document> documents;

        public AggregateResult(List<Document> documents) {
            this.documents = documents;
        }

        public List<Document> getDocuments() {
            return documents;
        }
    }

    public static class RunCommandResult {
        private final Document result;

        public RunCommandResult(Document result) {
            this.result = result;
        }

        public Document getResult() {
            return result;
        }
    }

    public static class ListDatabasesResult {
        private final List<String> databases;

        public ListDatabasesResult(List<String> databases) {
            this.databases = databases;
        }

        public List<String> getDatabases() {
            return databases;
        }
    }

    public static class ListCollectionsResult {
        private final List<String> collections;

        public ListCollectionsResult(List<String> collections) {
            this.collections = collections;
        }

        public List<String> getCollections() {
            return collections;
        }
    }
}
