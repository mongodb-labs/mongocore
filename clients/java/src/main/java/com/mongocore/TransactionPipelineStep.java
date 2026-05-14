package com.mongocore;

import com.google.protobuf.ByteString;
import mongocore.v1.Mongocore;
import mongocore.v1.Types;
import org.bson.Document;
import org.bson.codecs.DocumentCodec;
import org.bson.codecs.EncoderContext;
import org.bson.io.BasicOutputBuffer;

import java.util.List;
import java.util.stream.Collectors;

/**
 * Represents a single step in a transaction pipeline.
 * Each step has a name, targets a database/collection, and performs one operation.
 * Use the static factory methods to construct steps.
 */
public class TransactionPipelineStep {
    private static final DocumentCodec CODEC = new DocumentCodec();

    private final String name;
    private final String database;
    private final String collection;
    private final Mongocore.TransactionStep protoStep;

    private TransactionPipelineStep(String name, String database, String collection,
                                    Mongocore.TransactionStep protoStep) {
        this.name = name;
        this.database = database;
        this.collection = collection;
        this.protoStep = protoStep;
    }

    public String getName() {
        return name;
    }

    public String getDatabase() {
        return database;
    }

    public String getCollection() {
        return collection;
    }

    /**
     * Returns the underlying proto TransactionStep for use in gRPC calls.
     */
    public Mongocore.TransactionStep toProto() {
        return protoStep;
    }

    // --- Factory methods ---

    public static TransactionPipelineStep findOne(String name, String database, String collection, Document filter) {
        Mongocore.TransactionStep step = Mongocore.TransactionStep.newBuilder()
                .setName(name)
                .setDatabase(database)
                .setCollection(collection)
                .setFindOne(Mongocore.FindOneRequest.newBuilder()
                        .setDatabase(database)
                        .setCollection(collection)
                        .setFilter(makeFilter(filter))
                        .build())
                .build();
        return new TransactionPipelineStep(name, database, collection, step);
    }

    public static TransactionPipelineStep find(String name, String database, String collection, Document filter) {
        Mongocore.TransactionStep step = Mongocore.TransactionStep.newBuilder()
                .setName(name)
                .setDatabase(database)
                .setCollection(collection)
                .setFind(Mongocore.FindRequest.newBuilder()
                        .setDatabase(database)
                        .setCollection(collection)
                        .setFilter(makeFilter(filter))
                        .build())
                .build();
        return new TransactionPipelineStep(name, database, collection, step);
    }

    public static TransactionPipelineStep insert(String name, String database, String collection, Document document) {
        Mongocore.TransactionStep step = Mongocore.TransactionStep.newBuilder()
                .setName(name)
                .setDatabase(database)
                .setCollection(collection)
                .setInsert(Mongocore.InsertRequest.newBuilder()
                        .setDatabase(database)
                        .setCollection(collection)
                        .setDocument(makeDocument(document))
                        .build())
                .build();
        return new TransactionPipelineStep(name, database, collection, step);
    }

    public static TransactionPipelineStep insertMany(String name, String database, String collection,
                                                     List<Document> documents) {
        List<Types.Document> pbDocs = documents.stream()
                .map(TransactionPipelineStep::makeDocument)
                .collect(Collectors.toList());

        Mongocore.TransactionStep step = Mongocore.TransactionStep.newBuilder()
                .setName(name)
                .setDatabase(database)
                .setCollection(collection)
                .setInsertMany(Mongocore.InsertManyRequest.newBuilder()
                        .setDatabase(database)
                        .setCollection(collection)
                        .addAllDocuments(pbDocs)
                        .build())
                .build();
        return new TransactionPipelineStep(name, database, collection, step);
    }

    public static TransactionPipelineStep update(String name, String database, String collection,
                                                 Document filter, Document update) {
        Mongocore.TransactionStep step = Mongocore.TransactionStep.newBuilder()
                .setName(name)
                .setDatabase(database)
                .setCollection(collection)
                .setUpdate(Mongocore.UpdateRequest.newBuilder()
                        .setDatabase(database)
                        .setCollection(collection)
                        .setFilter(makeFilter(filter))
                        .setUpdate(makeDocument(update))
                        .build())
                .build();
        return new TransactionPipelineStep(name, database, collection, step);
    }

    public static TransactionPipelineStep updateMany(String name, String database, String collection,
                                                     Document filter, Document update) {
        Mongocore.TransactionStep step = Mongocore.TransactionStep.newBuilder()
                .setName(name)
                .setDatabase(database)
                .setCollection(collection)
                .setUpdateMany(Mongocore.UpdateManyRequest.newBuilder()
                        .setDatabase(database)
                        .setCollection(collection)
                        .setFilter(makeFilter(filter))
                        .setUpdate(makeDocument(update))
                        .build())
                .build();
        return new TransactionPipelineStep(name, database, collection, step);
    }

    public static TransactionPipelineStep delete(String name, String database, String collection, Document filter) {
        Mongocore.TransactionStep step = Mongocore.TransactionStep.newBuilder()
                .setName(name)
                .setDatabase(database)
                .setCollection(collection)
                .setDelete(Mongocore.DeleteRequest.newBuilder()
                        .setDatabase(database)
                        .setCollection(collection)
                        .setFilter(makeFilter(filter))
                        .build())
                .build();
        return new TransactionPipelineStep(name, database, collection, step);
    }

    public static TransactionPipelineStep deleteMany(String name, String database, String collection, Document filter) {
        Mongocore.TransactionStep step = Mongocore.TransactionStep.newBuilder()
                .setName(name)
                .setDatabase(database)
                .setCollection(collection)
                .setDeleteMany(Mongocore.DeleteManyRequest.newBuilder()
                        .setDatabase(database)
                        .setCollection(collection)
                        .setFilter(makeFilter(filter))
                        .build())
                .build();
        return new TransactionPipelineStep(name, database, collection, step);
    }

    public static TransactionPipelineStep findAndModify(String name, String database, String collection,
                                                        Document filter, Document update, boolean returnNew) {
        Types.FindAndModifyOptions.Builder opts = Types.FindAndModifyOptions.newBuilder()
                .setReturnDocument(returnNew ?
                        Types.FindAndModifyOptions.ReturnDocument.AFTER :
                        Types.FindAndModifyOptions.ReturnDocument.BEFORE)
                .setUpsert(false);

        Mongocore.TransactionStep step = Mongocore.TransactionStep.newBuilder()
                .setName(name)
                .setDatabase(database)
                .setCollection(collection)
                .setFindAndModify(Mongocore.FindAndModifyRequest.newBuilder()
                        .setDatabase(database)
                        .setCollection(collection)
                        .setFilter(makeFilter(filter))
                        .setUpdate(makeDocument(update))
                        .setOptions(opts.build())
                        .build())
                .build();
        return new TransactionPipelineStep(name, database, collection, step);
    }

    public static TransactionPipelineStep aggregate(String name, String database, String collection,
                                                    List<Document> pipeline) {
        List<ByteString> stages = pipeline.stream()
                .map(TransactionPipelineStep::encodeBson)
                .collect(Collectors.toList());

        Mongocore.TransactionStep step = Mongocore.TransactionStep.newBuilder()
                .setName(name)
                .setDatabase(database)
                .setCollection(collection)
                .setAggregate(Mongocore.AggregateRequest.newBuilder()
                        .setDatabase(database)
                        .setCollection(collection)
                        .setPipeline(Types.Pipeline.newBuilder().addAllStages(stages).build())
                        .build())
                .build();
        return new TransactionPipelineStep(name, database, collection, step);
    }

    // --- BSON helpers ---

    private static ByteString encodeBson(Document doc) {
        BasicOutputBuffer buffer = new BasicOutputBuffer();
        CODEC.encode(new org.bson.BsonBinaryWriter(buffer), doc, EncoderContext.builder().build());
        return ByteString.copyFrom(buffer.getInternalBuffer(), 0, buffer.getSize());
    }

    private static Types.Filter makeFilter(Document filter) {
        return Types.Filter.newBuilder().setData(encodeBson(filter)).build();
    }

    private static Types.Document makeDocument(Document doc) {
        return Types.Document.newBuilder().setData(encodeBson(doc)).build();
    }
}
