package com.mongocore;

import mongocore.v1.Mongocore;
import mongocore.v1.Types;
import org.bson.Document;
import com.google.protobuf.ByteString;
import org.bson.codecs.DocumentCodec;
import org.bson.codecs.EncoderContext;
import org.bson.io.BasicOutputBuffer;

import java.util.List;
import java.util.stream.Collectors;

/**
 * Operation builders for pipeline requests.
 * Each method returns a PipelineOperation proto message.
 */
public class Ops {
    private static final DocumentCodec CODEC = new DocumentCodec();

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

    public static Mongocore.PipelineOperation find(String database, String collection, Document filter) {
        return Mongocore.PipelineOperation.newBuilder()
                .setFind(Mongocore.FindRequest.newBuilder()
                        .setDatabase(database)
                        .setCollection(collection)
                        .setFilter(makeFilter(filter))
                        .build())
                .build();
    }

    public static Mongocore.PipelineOperation findOne(String database, String collection, Document filter) {
        return Mongocore.PipelineOperation.newBuilder()
                .setFindOne(Mongocore.FindOneRequest.newBuilder()
                        .setDatabase(database)
                        .setCollection(collection)
                        .setFilter(makeFilter(filter))
                        .build())
                .build();
    }

    public static Mongocore.PipelineOperation insert(String database, String collection, Document document) {
        return Mongocore.PipelineOperation.newBuilder()
                .setInsert(Mongocore.InsertRequest.newBuilder()
                        .setDatabase(database)
                        .setCollection(collection)
                        .setDocument(makeDocument(document))
                        .build())
                .build();
    }

    public static Mongocore.PipelineOperation insertMany(String database, String collection, List<Document> documents) {
        List<Types.Document> pbDocs = documents.stream()
                .map(Ops::makeDocument)
                .collect(Collectors.toList());

        return Mongocore.PipelineOperation.newBuilder()
                .setInsertMany(Mongocore.InsertManyRequest.newBuilder()
                        .setDatabase(database)
                        .setCollection(collection)
                        .addAllDocuments(pbDocs)
                        .build())
                .build();
    }

    public static Mongocore.PipelineOperation update(String database, String collection, Document filter, Document update) {
        return Mongocore.PipelineOperation.newBuilder()
                .setUpdate(Mongocore.UpdateRequest.newBuilder()
                        .setDatabase(database)
                        .setCollection(collection)
                        .setFilter(makeFilter(filter))
                        .setUpdate(makeDocument(update))
                        .build())
                .build();
    }

    public static Mongocore.PipelineOperation updateMany(String database, String collection, Document filter, Document update) {
        return Mongocore.PipelineOperation.newBuilder()
                .setUpdateMany(Mongocore.UpdateManyRequest.newBuilder()
                        .setDatabase(database)
                        .setCollection(collection)
                        .setFilter(makeFilter(filter))
                        .setUpdate(makeDocument(update))
                        .build())
                .build();
    }

    public static Mongocore.PipelineOperation delete(String database, String collection, Document filter) {
        return Mongocore.PipelineOperation.newBuilder()
                .setDelete(Mongocore.DeleteRequest.newBuilder()
                        .setDatabase(database)
                        .setCollection(collection)
                        .setFilter(makeFilter(filter))
                        .build())
                .build();
    }

    public static Mongocore.PipelineOperation deleteMany(String database, String collection, Document filter) {
        return Mongocore.PipelineOperation.newBuilder()
                .setDeleteMany(Mongocore.DeleteManyRequest.newBuilder()
                        .setDatabase(database)
                        .setCollection(collection)
                        .setFilter(makeFilter(filter))
                        .build())
                .build();
    }

    public static Mongocore.PipelineOperation aggregate(String database, String collection, List<Document> pipeline) {
        List<ByteString> stages = pipeline.stream()
                .map(Ops::encodeBson)
                .collect(Collectors.toList());

        return Mongocore.PipelineOperation.newBuilder()
                .setAggregate(Mongocore.AggregateRequest.newBuilder()
                        .setDatabase(database)
                        .setCollection(collection)
                        .setPipeline(Types.Pipeline.newBuilder().addAllStages(stages).build())
                        .build())
                .build();
    }

    public static Mongocore.PipelineOperation runCommand(String database, Document command, boolean allowAll) {
        return Mongocore.PipelineOperation.newBuilder()
                .setRunCommand(Mongocore.RunCommandRequest.newBuilder()
                        .setDatabase(database)
                        .setCommand(makeDocument(command))
                        .setAllowAll(allowAll)
                        .build())
                .build();
    }

    public static Mongocore.PipelineOperation listDatabases() {
        return Mongocore.PipelineOperation.newBuilder()
                .setListDatabases(Mongocore.ListDatabasesRequest.newBuilder().build())
                .build();
    }

    public static Mongocore.PipelineOperation listCollections(String database) {
        return Mongocore.PipelineOperation.newBuilder()
                .setListCollections(Mongocore.ListCollectionsRequest.newBuilder()
                        .setDatabase(database)
                        .build())
                .build();
    }
}
