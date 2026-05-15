package com.mongocore;

import com.google.protobuf.ByteString;
import mongocore.v1.MongoCoreGrpc;
import mongocore.v1.Mongocore;
import mongocore.v1.Types;
import org.bson.Document;
import org.bson.codecs.DocumentCodec;
import org.bson.codecs.EncoderContext;
import org.bson.codecs.DecoderContext;
import org.bson.io.BasicOutputBuffer;
import org.bson.BsonBinaryReader;

import java.nio.ByteBuffer;
import java.util.ArrayList;
import java.util.Iterator;
import java.util.List;
import java.util.stream.Collectors;

public class MongoCollection {
    private final MongoClient client;
    private final String database;
    private final String name;
    private static final DocumentCodec CODEC = new DocumentCodec();

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

    private MongoCoreGrpc.MongoCoreBlockingStub getStub() {
        return MongoCoreGrpc.newBlockingStub(client.getChannel());
    }

    private ByteString encodeBson(Document doc) {
        BasicOutputBuffer buffer = new BasicOutputBuffer();
        CODEC.encode(new org.bson.BsonBinaryWriter(buffer), doc, EncoderContext.builder().build());
        return ByteString.copyFrom(buffer.getInternalBuffer(), 0, buffer.getSize());
    }

    private Document decodeBson(ByteString data) {
        byte[] bytes = data.toByteArray();
        BsonBinaryReader reader = new BsonBinaryReader(ByteBuffer.wrap(bytes));
        return CODEC.decode(reader, DecoderContext.builder().build());
    }

    private Types.Filter makeFilter(Document filter) {
        return Types.Filter.newBuilder().setData(encodeBson(filter)).build();
    }

    private Types.Document makeDocument(Document doc) {
        return Types.Document.newBuilder().setData(encodeBson(doc)).build();
    }

    public List<Document> find(Document filter) {
        return find(filter, null);
    }

    public List<Document> find(Document filter, FindOptions options) {
        Mongocore.FindRequest.Builder req = Mongocore.FindRequest.newBuilder()
                .setDatabase(database)
                .setCollection(name)
                .setFilter(makeFilter(filter));

        if (options != null) {
            Types.FindOptions.Builder opts = Types.FindOptions.newBuilder();
            if (options.getLimit() != null) {
                opts.setLimit(options.getLimit().longValue());
            }
            if (options.getSkip() != null) {
                opts.setSkip(options.getSkip().longValue());
            }
            if (options.getSort() != null) {
                opts.setSort(encodeBson(options.getSort()));
            }
            if (options.getProjection() != null) {
                opts.setProjection(encodeBson(options.getProjection()));
            }
            req.setOptions(opts.build());
        }

        Mongocore.FindResponse resp = getStub().find(req.build());
        List<Document> docs = new ArrayList<>(resp.getDocumentsCount());
        for (Types.Document d : resp.getDocumentsList()) {
            docs.add(decodeBson(d.getData()));
        }
        return docs;
    }

    public Document findOne(Document filter) {
        Mongocore.FindOneResponse resp = getStub().findOne(
                Mongocore.FindOneRequest.newBuilder()
                        .setDatabase(database)
                        .setCollection(name)
                        .setFilter(makeFilter(filter))
                        .build());

        if (resp.hasDocument() && !resp.getDocument().getData().isEmpty()) {
            return decodeBson(resp.getDocument().getData());
        }
        return null;
    }

    public InsertResult insertOne(Document document) {
        Mongocore.InsertResponse resp = getStub().insert(
                Mongocore.InsertRequest.newBuilder()
                        .setDatabase(database)
                        .setCollection(name)
                        .setDocument(makeDocument(document))
                        .build());
        return new InsertResult(resp.getInsertedId());
    }

    public InsertManyResult insertMany(List<Document> documents) {
        List<Types.Document> pbDocs = documents.stream()
                .map(this::makeDocument)
                .collect(Collectors.toList());

        Mongocore.InsertManyResponse resp = getStub().insertMany(
                Mongocore.InsertManyRequest.newBuilder()
                        .setDatabase(database)
                        .setCollection(name)
                        .addAllDocuments(pbDocs)
                        .build());
        return new InsertManyResult(resp.getInsertedIdsList());
    }

    public UpdateResult updateOne(Document filter, Document update) {
        Mongocore.UpdateResponse resp = getStub().update(
                Mongocore.UpdateRequest.newBuilder()
                        .setDatabase(database)
                        .setCollection(name)
                        .setFilter(makeFilter(filter))
                        .setUpdate(makeDocument(update))
                        .build());
        return new UpdateResult(resp.getMatchedCount(), resp.getModifiedCount(), "");
    }

    public UpdateResult updateMany(Document filter, Document update) {
        Mongocore.UpdateManyResponse resp = getStub().updateMany(
                Mongocore.UpdateManyRequest.newBuilder()
                        .setDatabase(database)
                        .setCollection(name)
                        .setFilter(makeFilter(filter))
                        .setUpdate(makeDocument(update))
                        .build());
        return new UpdateResult(resp.getMatchedCount(), resp.getModifiedCount(), "");
    }

    public long deleteOne(Document filter) {
        Mongocore.DeleteResponse resp = getStub().delete(
                Mongocore.DeleteRequest.newBuilder()
                        .setDatabase(database)
                        .setCollection(name)
                        .setFilter(makeFilter(filter))
                        .build());
        return resp.getDeletedCount();
    }

    public long deleteMany(Document filter) {
        Mongocore.DeleteManyResponse resp = getStub().deleteMany(
                Mongocore.DeleteManyRequest.newBuilder()
                        .setDatabase(database)
                        .setCollection(name)
                        .setFilter(makeFilter(filter))
                        .build());
        return resp.getDeletedCount();
    }

    public List<Document> aggregate(List<Document> pipeline) {
        List<ByteString> stages = pipeline.stream()
                .map(this::encodeBson)
                .collect(Collectors.toList());

        Mongocore.AggregateResponse resp = getStub().aggregate(
                Mongocore.AggregateRequest.newBuilder()
                        .setDatabase(database)
                        .setCollection(name)
                        .setPipeline(Types.Pipeline.newBuilder().addAllStages(stages).build())
                        .build());

        List<Document> docs = new ArrayList<>(resp.getDocumentsCount());
        for (Types.Document d : resp.getDocumentsList()) {
            docs.add(decodeBson(d.getData()));
        }
        return docs;
    }

    public SearchResult search(String query) {
        return search(query, 10);
    }

    public SearchResult search(String query, long limit) {
        Mongocore.SearchResponse resp = getStub().search(
                Mongocore.SearchRequest.newBuilder()
                        .setDatabase(database)
                        .setCollection(name)
                        .setQuery(query)
                        .setLimit(limit)
                        .build());

        List<Document> docs = new ArrayList<>(resp.getDocumentsCount());
        for (Types.Document d : resp.getDocumentsList()) {
            docs.add(decodeBson(d.getData()));
        }
        return new SearchResult(docs, resp.getMethod(), resp.getTotal());
    }

    public ChangeStream watch() {
        return watch(null);
    }

    public ChangeStream watch(List<Document> pipeline) {
        Mongocore.WatchRequest.Builder req = Mongocore.WatchRequest.newBuilder()
                .setDatabase(database)
                .setCollection(name);

        if (pipeline != null && !pipeline.isEmpty()) {
            List<ByteString> stages = pipeline.stream()
                    .map(this::encodeBson)
                    .collect(Collectors.toList());
            req.setPipeline(Types.Pipeline.newBuilder().addAllStages(stages).build());
        }

        Iterator<Mongocore.WatchEvent> stream = getStub().watch(req.build());
        return new ChangeStream(stream, this);
    }

    public Document findAndModify(Document filter, Document update, boolean returnNew) {
        Types.FindAndModifyOptions.Builder opts = Types.FindAndModifyOptions.newBuilder()
                .setReturnDocument(returnNew ?
                        Types.FindAndModifyOptions.ReturnDocument.AFTER :
                        Types.FindAndModifyOptions.ReturnDocument.BEFORE)
                .setUpsert(false);

        Mongocore.FindAndModifyResponse resp = getStub().findAndModify(
                Mongocore.FindAndModifyRequest.newBuilder()
                        .setDatabase(database)
                        .setCollection(name)
                        .setFilter(makeFilter(filter))
                        .setUpdate(makeDocument(update))
                        .setOptions(opts.build())
                        .build());

        if (resp.hasDocument() && !resp.getDocument().getData().isEmpty()) {
            return decodeBson(resp.getDocument().getData());
        }
        return null;
    }

    public long countDocuments(Document filter) {
        String filterJson = filter != null ? filter.toJson() : "{}";
        Mongocore.CountDocumentsResponse resp = getStub().countDocuments(
                Mongocore.CountDocumentsRequest.newBuilder()
                        .setDatabase(database)
                        .setCollection(name)
                        .setFilter(filterJson)
                        .build());
        return resp.getCount();
    }

    public long countDocuments() {
        return countDocuments(null);
    }

    public boolean drop() {
        Mongocore.DropCollectionResponse resp = getStub().dropCollection(
                Mongocore.DropCollectionRequest.newBuilder()
                        .setDatabase(database)
                        .setCollection(name)
                        .build());
        return resp.getOk();
    }

    public String createIndex(Document keys, boolean unique) {
        Types.IndexOptions.Builder opts = Types.IndexOptions.newBuilder()
                .setUnique(unique);

        Mongocore.CreateIndexResponse resp = getStub().createIndex(
                Mongocore.CreateIndexRequest.newBuilder()
                        .setDatabase(database)
                        .setCollection(name)
                        .setKeys(makeDocument(keys))
                        .setOptions(opts.build())
                        .build());

        return resp.getIndexName();
    }
}
