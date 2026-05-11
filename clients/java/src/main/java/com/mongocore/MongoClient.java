package com.mongocore;

import io.grpc.ManagedChannel;
import io.grpc.ManagedChannelBuilder;
import mongocore.v1.MongoCoreGrpc;
import mongocore.v1.Mongocore;
import org.bson.Document;

import java.util.List;
import java.util.concurrent.TimeUnit;

public class MongoClient implements AutoCloseable {
    private final ManagedChannel channel;
    private final String address;

    private MongoClient(String address) {
        this.address = address;
        this.channel = ManagedChannelBuilder.forTarget(address)
                .usePlaintext()
                .build();
    }

    public static MongoClient create(String address) {
        return new MongoClient(address);
    }

    public static MongoClient create() {
        return create("localhost:50051");
    }

    public MongoDatabase getDatabase(String name) {
        return new MongoDatabase(this, name);
    }

    public List<String> listDatabases() {
        MongoCoreGrpc.MongoCoreBlockingStub stub = MongoCoreGrpc.newBlockingStub(channel);
        Mongocore.ListDatabasesResponse resp = stub.listDatabases(
                Mongocore.ListDatabasesRequest.newBuilder().build());
        return resp.getDatabasesList();
    }

    public Document runCommand(String database, Document command, boolean allowAll) {
        MongoCoreGrpc.MongoCoreBlockingStub stub = MongoCoreGrpc.newBlockingStub(channel);
        Mongocore.RunCommandResponse resp = stub.runCommand(
                Mongocore.RunCommandRequest.newBuilder()
                        .setDatabase(database)
                        .setCommand(encodeDocument(command))
                        .setAllowAll(allowAll)
                        .build());
        return decodeDocument(resp.getResult());
    }

    private mongocore.v1.Types.Document encodeDocument(Document doc) {
        return mongocore.v1.Types.Document.newBuilder()
                .setData(encodeBson(doc))
                .build();
    }

    private Document decodeDocument(mongocore.v1.Types.Document pbDoc) {
        return decodeBson(pbDoc.getData());
    }

    private com.google.protobuf.ByteString encodeBson(Document doc) {
        org.bson.io.BasicOutputBuffer buffer = new org.bson.io.BasicOutputBuffer();
        org.bson.codecs.DocumentCodec codec = new org.bson.codecs.DocumentCodec();
        codec.encode(new org.bson.BsonBinaryWriter(buffer), doc,
                org.bson.codecs.EncoderContext.builder().build());
        return com.google.protobuf.ByteString.copyFrom(buffer.getInternalBuffer(), 0, buffer.getSize());
    }

    private Document decodeBson(com.google.protobuf.ByteString data) {
        byte[] bytes = data.toByteArray();
        org.bson.BsonBinaryReader reader = new org.bson.BsonBinaryReader(java.nio.ByteBuffer.wrap(bytes));
        org.bson.codecs.DocumentCodec codec = new org.bson.codecs.DocumentCodec();
        return codec.decode(reader, org.bson.codecs.DecoderContext.builder().build());
    }

    // --- Ingestion Methods ---

    public record IngestOptions(String source, String database, String collection, String format) {
        public IngestOptions(String source, String database, String collection) {
            this(source, database, collection, "auto");
        }
    }

    public record IngestResult(String jobId, String status) {}

    public record IngestJob(String jobId, String status, String source, String database, String collection) {}

    public record WatchOptions(String directory, String database, String collection, String format) {
        public WatchOptions(String directory, String database, String collection) {
            this(directory, database, collection, "auto");
        }
    }

    public record WatchResult(String watchId, String status) {}

    public IngestResult ingest(IngestOptions options) {
        MongoCoreGrpc.MongoCoreBlockingStub stub = MongoCoreGrpc.newBlockingStub(channel);
        Mongocore.IngestResponse resp = stub.ingest(
                Mongocore.IngestRequest.newBuilder()
                        .setSource(options.source())
                        .setDatabase(options.database())
                        .setCollection(options.collection())
                        .setFormat(options.format())
                        .build());
        return new IngestResult(resp.getJobId(), resp.getStatus());
    }

    public IngestJob ingestStatus(String jobId) {
        MongoCoreGrpc.MongoCoreBlockingStub stub = MongoCoreGrpc.newBlockingStub(channel);
        Mongocore.IngestStatusResponse resp = stub.ingestStatus(
                Mongocore.IngestStatusRequest.newBuilder()
                        .setJobId(jobId)
                        .build());
        return new IngestJob(resp.getJobId(), resp.getStatus(), resp.getSource(),
                resp.getDatabase(), resp.getCollection());
    }

    public List<IngestJob> listIngestJobs() {
        MongoCoreGrpc.MongoCoreBlockingStub stub = MongoCoreGrpc.newBlockingStub(channel);
        Mongocore.ListIngestJobsResponse resp = stub.listIngestJobs(
                Mongocore.ListIngestJobsRequest.newBuilder().build());
        return resp.getJobsList().stream()
                .map(j -> new IngestJob(j.getJobId(), j.getStatus(), j.getSource(),
                        j.getDatabase(), j.getCollection()))
                .toList();
    }

    public IngestResult cancelIngest(String jobId) {
        MongoCoreGrpc.MongoCoreBlockingStub stub = MongoCoreGrpc.newBlockingStub(channel);
        Mongocore.CancelIngestResponse resp = stub.cancelIngest(
                Mongocore.CancelIngestRequest.newBuilder()
                        .setJobId(jobId)
                        .build());
        return new IngestResult(resp.getJobId(), resp.getStatus());
    }

    public WatchResult watchDirectory(WatchOptions options) {
        MongoCoreGrpc.MongoCoreBlockingStub stub = MongoCoreGrpc.newBlockingStub(channel);
        Mongocore.WatchDirectoryResponse resp = stub.watchDirectory(
                Mongocore.WatchDirectoryRequest.newBuilder()
                        .setDirectory(options.directory())
                        .setDatabase(options.database())
                        .setCollection(options.collection())
                        .setFormat(options.format())
                        .build());
        return new WatchResult(resp.getWatchId(), resp.getStatus());
    }

    public WatchResult stopWatch(String watchId) {
        MongoCoreGrpc.MongoCoreBlockingStub stub = MongoCoreGrpc.newBlockingStub(channel);
        Mongocore.StopWatchResponse resp = stub.stopWatch(
                Mongocore.StopWatchRequest.newBuilder()
                        .setWatchId(watchId)
                        .build());
        return new WatchResult(resp.getWatchId(), resp.getStatus());
    }

    ManagedChannel getChannel() {
        return channel;
    }

    @Override
    public void close() throws Exception {
        channel.shutdown().awaitTermination(5, TimeUnit.SECONDS);
    }
}
