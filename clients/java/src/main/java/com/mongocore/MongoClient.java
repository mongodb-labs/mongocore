package com.mongocore;

import io.grpc.*;
import io.grpc.stub.ClientCalls;
import mongocore.v1.MongoCoreGrpc;
import mongocore.v1.Mongocore;
import mongocore.v1.Ingestion;
import org.bson.Document;

import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.TimeUnit;

public class MongoClient implements AutoCloseable {
    private static final Metadata.Key<String> CLIENT_LANG_KEY =
            Metadata.Key.of("x-client-language", Metadata.ASCII_STRING_MARSHALLER);

    private final ClientInterceptor langInterceptor = new ClientInterceptor() {
        @Override
        public <ReqT, RespT> ClientCall<ReqT, RespT> interceptCall(
                MethodDescriptor<ReqT, RespT> method, CallOptions options, Channel next) {
            return new ForwardingClientCall.SimpleForwardingClientCall<>(next.newCall(method, options)) {
                @Override
                public void start(Listener<RespT> listener, Metadata headers) {
                    headers.put(CLIENT_LANG_KEY, "java");
                    super.start(listener, headers);
                }
            };
        }
    };

    private static final String DEFAULT_SOCKET_PATH = "/tmp/mongocore.sock";
    private static final String DEFAULT_ADDRESS = "localhost:50051";
    private static final int MAX_MESSAGE_SIZE = 64 * 1024 * 1024;

    private final ManagedChannel channel;
    private final String address;
    private final String transport;

    private MongoClient(String target, String transport) {
        this.address = target;
        this.transport = transport;
        this.channel = ManagedChannelBuilder.forTarget(target)
                .usePlaintext()
                .maxInboundMessageSize(MAX_MESSAGE_SIZE)
                .intercept(langInterceptor)
                .build();
    }

    public static MongoClient create(String address) {
        return new MongoClient(address, "tcp");
    }

    public static MongoClient createWithSocket(String socketPath) {
        return new MongoClient("unix://" + socketPath, "uds");
    }

    public static MongoClient create() {
        String envSocket = System.getenv("MONGOCORE_SOCKET_PATH");
        if (envSocket != null && !envSocket.isEmpty() && isUdsSupported()) {
            return new MongoClient("unix://" + envSocket, "uds");
        }
        java.io.File socketFile = new java.io.File(DEFAULT_SOCKET_PATH);
        if (socketFile.exists() && isUdsSupported()) {
            return new MongoClient("unix://" + DEFAULT_SOCKET_PATH, "uds");
        }
        String envAddr = System.getenv("MONGOCORE_ADDRESS");
        if (envAddr != null && !envAddr.isEmpty()) {
            return new MongoClient(envAddr, "tcp");
        }
        return new MongoClient(DEFAULT_ADDRESS, "tcp");
    }

    private static boolean isUdsSupported() {
        try {
            Class.forName("io.netty.channel.epoll.EpollDomainSocketChannel");
            return true;
        } catch (ClassNotFoundException e) {
            // Fall through
        }
        try {
            Class.forName("io.netty.channel.kqueue.KQueueDomainSocketChannel");
            return true;
        } catch (ClassNotFoundException e) {
            return false;
        }
    }

    public String getTransport() {
        return transport;
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

    public record IngestOptions(String filePath, String database, String collection, Ingestion.FileFormat format) {
        public IngestOptions(String filePath, String database, String collection) {
            this(filePath, database, collection, Ingestion.FileFormat.FILE_FORMAT_AUTO);
        }
    }

    public record IngestResult(String jobId, Ingestion.IngestJobStatus status, long totalRows) {}

    public record IngestJob(String jobId, String filePath, String database, String collection,
                            Ingestion.IngestJobStatus status, long totalRows, long rowsProcessed) {}

    public record WatchOptions(String path, String database, String collection) {}

    public record WatchResult(String watchId, boolean success) {}

    public IngestResult ingest(IngestOptions options) {
        MongoCoreGrpc.MongoCoreBlockingStub stub = MongoCoreGrpc.newBlockingStub(channel);
        Ingestion.IngestResponse resp = stub.ingest(
                Ingestion.IngestRequest.newBuilder()
                        .setFilePath(options.filePath())
                        .setDatabase(options.database())
                        .setCollection(options.collection())
                        .setFormat(options.format())
                        .build());
        return new IngestResult(resp.getJobId(), resp.getStatus(), resp.getTotalRows());
    }

    public IngestJob ingestStatus(String jobId) {
        MongoCoreGrpc.MongoCoreBlockingStub stub = MongoCoreGrpc.newBlockingStub(channel);
        Ingestion.GetIngestStatusResponse resp = stub.getIngestStatus(
                Ingestion.GetIngestStatusRequest.newBuilder()
                        .setJobId(jobId)
                        .build());
        return new IngestJob(resp.getJobId(), "", "", "",
                resp.getStatus(), resp.getTotalRows(), resp.getRowsProcessed());
    }

    public List<IngestJob> listIngestJobs() {
        MongoCoreGrpc.MongoCoreBlockingStub stub = MongoCoreGrpc.newBlockingStub(channel);
        Ingestion.ListIngestJobsResponse resp = stub.listIngestJobs(
                Ingestion.ListIngestJobsRequest.newBuilder().build());
        return resp.getJobsList().stream()
                .map(j -> new IngestJob(j.getJobId(), j.getFilePath(), j.getDatabase(),
                        j.getCollection(), j.getStatus(), j.getTotalRows(), j.getRowsProcessed()))
                .toList();
    }

    public boolean cancelIngest(String jobId) {
        MongoCoreGrpc.MongoCoreBlockingStub stub = MongoCoreGrpc.newBlockingStub(channel);
        Ingestion.CancelIngestResponse resp = stub.cancelIngest(
                Ingestion.CancelIngestRequest.newBuilder()
                        .setJobId(jobId)
                        .build());
        return resp.getSuccess();
    }

    public WatchResult watchDirectory(WatchOptions options) {
        MongoCoreGrpc.MongoCoreBlockingStub stub = MongoCoreGrpc.newBlockingStub(channel);
        Ingestion.WatchDirectoryResponse resp = stub.watchDirectory(
                Ingestion.WatchDirectoryRequest.newBuilder()
                        .setPath(options.path())
                        .setDatabase(options.database())
                        .setCollection(options.collection())
                        .build());
        return new WatchResult(resp.getWatchId(), true);
    }

    public WatchResult stopWatch(String watchId) {
        MongoCoreGrpc.MongoCoreBlockingStub stub = MongoCoreGrpc.newBlockingStub(channel);
        Ingestion.StopWatchResponse resp = stub.stopWatch(
                Ingestion.StopWatchRequest.newBuilder()
                        .setWatchId(watchId)
                        .build());
        return new WatchResult(watchId, resp.getSuccess());
    }

    // --- Embed & Search Methods ---

    public record EmbedAndStoreResult(long documentsStored, long embeddingsGenerated, int embeddingDimensions) {}

    public EmbedAndStoreResult embedAndStore(String database, String collection, String documents,
                                              String embedField, String embeddingField) {
        MongoCoreGrpc.MongoCoreBlockingStub stub = MongoCoreGrpc.newBlockingStub(channel);
        Mongocore.EmbedAndStoreResponse resp = stub.embedAndStore(
                Mongocore.EmbedAndStoreRequest.newBuilder()
                        .setDatabase(database)
                        .setCollection(collection)
                        .setDocuments(documents)
                        .setEmbedField(embedField)
                        .setEmbeddingField(embeddingField != null ? embeddingField : "")
                        .build());
        return new EmbedAndStoreResult(resp.getDocumentsStored(),
                resp.getEmbeddingsGenerated(), resp.getEmbeddingDimensions());
    }

    public EmbedAndStoreResult embedAndStore(String database, String collection, String documents, String embedField) {
        return embedAndStore(database, collection, documents, embedField, "");
    }

    public record SemanticSearchResult(String results, long count) {}

    public SemanticSearchResult semanticSearch(String database, String collection, String query,
                                               String indexName, int limit) {
        MongoCoreGrpc.MongoCoreBlockingStub stub = MongoCoreGrpc.newBlockingStub(channel);
        Mongocore.SemanticSearchResponse resp = stub.semanticSearch(
                Mongocore.SemanticSearchRequest.newBuilder()
                        .setDatabase(database)
                        .setCollection(collection)
                        .setQuery(query)
                        .setIndexName(indexName != null ? indexName : "")
                        .setLimit(limit)
                        .build());
        return new SemanticSearchResult(resp.getResults(), resp.getCount());
    }

    public SemanticSearchResult semanticSearch(String database, String collection, String query) {
        return semanticSearch(database, collection, query, "", 10);
    }

    // --- Transaction Methods ---

    public String beginTransaction() {
        MongoCoreGrpc.MongoCoreBlockingStub stub = MongoCoreGrpc.newBlockingStub(channel);
        Mongocore.BeginTransactionResponse resp = stub.beginTransaction(
                Mongocore.BeginTransactionRequest.newBuilder()
                        .setDatabase("")
                        .build());
        return resp.getTransactionId();
    }

    public boolean commitTransaction(String transactionId) {
        MongoCoreGrpc.MongoCoreBlockingStub stub = MongoCoreGrpc.newBlockingStub(channel);
        try {
            stub.commitTransaction(
                    Mongocore.CommitTransactionRequest.newBuilder()
                            .setTransactionId(transactionId)
                            .build());
            return true;
        } catch (Exception e) {
            return false;
        }
    }

    public boolean abortTransaction(String transactionId) {
        MongoCoreGrpc.MongoCoreBlockingStub stub = MongoCoreGrpc.newBlockingStub(channel);
        try {
            stub.abortTransaction(
                    Mongocore.AbortTransactionRequest.newBuilder()
                            .setTransactionId(transactionId)
                            .build());
            return true;
        } catch (Exception e) {
            return false;
        }
    }

    // --- Transaction Pipeline Methods ---

    public record TransactionPipelineOptions(String readConcern, String writeConcern, Long maxTimeMs) {
        public TransactionPipelineOptions() {
            this(null, null, null);
        }
    }

    public record TransactionPipelineResult(List<TransactionStepResult> steps,
                                            int totalSteps, int stepsCompleted, long elapsedMs) {}

    public record TransactionStepResult(String name, boolean success,
                                        PipelineResult result) {}

    public TransactionPipelineResult transactionPipeline(TransactionPipelineStep... steps) {
        return transactionPipeline(null, steps);
    }

    public TransactionPipelineResult transactionPipeline(TransactionPipelineOptions options,
                                                         TransactionPipelineStep... steps) {
        MongoCoreGrpc.MongoCoreBlockingStub stub = MongoCoreGrpc.newBlockingStub(channel);

        Mongocore.TransactionPipelineRequest.Builder reqBuilder =
                Mongocore.TransactionPipelineRequest.newBuilder();

        for (TransactionPipelineStep step : steps) {
            reqBuilder.addSteps(step.toProto());
        }

        if (options != null) {
            Mongocore.TransactionPipelineOptions.Builder optsBuilder =
                    Mongocore.TransactionPipelineOptions.newBuilder();
            if (options.readConcern() != null) {
                optsBuilder.setReadConcern(options.readConcern());
            }
            if (options.writeConcern() != null) {
                optsBuilder.setWriteConcern(options.writeConcern());
            }
            if (options.maxTimeMs() != null) {
                optsBuilder.setMaxTimeMs(options.maxTimeMs());
            }
            reqBuilder.setOptions(optsBuilder.build());
        }

        Mongocore.TransactionPipelineResponse resp = stub.transactionPipeline(reqBuilder.build());

        List<TransactionStepResult> stepResults = new java.util.ArrayList<>();
        for (Mongocore.TransactionStepResult stepResult : resp.getStepsList()) {
            stepResults.add(new TransactionStepResult(
                    stepResult.getName(), stepResult.getSuccess(), null));
        }

        int totalSteps = resp.hasSummary() ? resp.getSummary().getTotalSteps() : steps.length;
        int stepsCompleted = resp.hasSummary() ? resp.getSummary().getStepsCompleted() : stepResults.size();
        long elapsedMs = resp.hasSummary() ? resp.getSummary().getElapsedMs() : 0;

        return new TransactionPipelineResult(stepResults, totalSteps, stepsCompleted, elapsedMs);
    }

    // --- Pipeline Methods ---

    public List<PipelineResult> pipeline(Mongocore.PipelineOperation... operations) {
        MongoCoreGrpc.MongoCoreBlockingStub stub = MongoCoreGrpc.newBlockingStub(channel);
        Mongocore.PipelineResponse resp = stub.pipeline(
                Mongocore.PipelineRequest.newBuilder()
                        .addAllOperations(java.util.Arrays.asList(operations))
                        .build());

        return resp.getResultsList().stream()
                .map(PipelineResult::new)
                .collect(java.util.stream.Collectors.toList());
    }

    // --- Analytics Methods ---

    public Map<String, Object> getAnalytics() {
        MongoCoreGrpc.MongoCoreBlockingStub stub = MongoCoreGrpc.newBlockingStub(channel);
        Mongocore.GetAnalyticsResponse resp = stub.getAnalytics(
                Mongocore.GetAnalyticsRequest.newBuilder()
                        .setWindowSeconds(60)
                        .build());

        Map<String, Object> analytics = new HashMap<>();
        analytics.put("total_operations", resp.getTotalOperations());
        analytics.put("total_errors", resp.getTotalErrors());
        analytics.put("error_rate", resp.getErrorRate());
        analytics.put("p50_latency_ms", resp.getP50LatencyMs());
        analytics.put("p95_latency_ms", resp.getP95LatencyMs());
        analytics.put("p99_latency_ms", resp.getP99LatencyMs());

        List<Map<String, Object>> topOperations = resp.getTopOperationsList().stream()
                .map(op -> {
                    Map<String, Object> m = new HashMap<>();
                    m.put("operation", op.getOperation());
                    m.put("count", op.getCount());
                    return m;
                })
                .toList();
        analytics.put("top_operations", topOperations);

        List<Map<String, Object>> topCollections = resp.getTopCollectionsList().stream()
                .map(col -> {
                    Map<String, Object> m = new HashMap<>();
                    m.put("collection", col.getCollection());
                    m.put("count", col.getCount());
                    return m;
                })
                .toList();
        analytics.put("top_collections", topCollections);

        return analytics;
    }

    ManagedChannel getChannel() {
        return channel;
    }

    @Override
    public void close() throws Exception {
        channel.shutdown().awaitTermination(5, TimeUnit.SECONDS);
    }
}
