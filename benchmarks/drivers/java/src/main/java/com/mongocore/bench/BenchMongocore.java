package com.mongocore.bench;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonObject;
import com.mongocore.MongoClient;
import org.bson.Document;
import org.bson.types.ObjectId;

import java.io.FileReader;
import java.io.FileWriter;
import java.lang.management.ManagementFactory;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.time.Instant;
import java.util.*;
import java.util.stream.Collectors;

public class BenchMongocore {
    private static final Gson GSON = new GsonBuilder().setPrettyPrinting().create();

    static class Config {
        String mongodb_uri;
        String mongocore_address;
        String database;
        int min_time_secs;
        int max_iterations;
        int max_time_secs;
        Map<String, Integer> warmup_iterations;
    }

    static class SystemInfo {
        String os;
        String arch;
        int cpus;
        double ram_gb;
        String mongocore_version;
        String driver;

        SystemInfo() {
            this.os = System.getProperty("os.name").toLowerCase();
            this.arch = System.getProperty("os.arch");
            this.cpus = Runtime.getRuntime().availableProcessors();
            long memory = ((com.sun.management.OperatingSystemMXBean) ManagementFactory.getOperatingSystemMXBean()).getTotalMemorySize();
            this.ram_gb = Math.round(memory / (1024.0 * 1024.0 * 1024.0) * 10) / 10.0;
            this.mongocore_version = "0.6.0";
            this.driver = "mongocore+java";
        }
    }

    static class Percentiles {
        double p10, p25, p50, p75, p90, p95, p99;
    }

    static class BenchResult {
        String benchmark;
        String category;
        String driver;
        int dataset_size_bytes;
        int batch_size;
        int iterations;
        double total_time_secs;
        double ops_per_sec;
        double mb_per_sec;
        Percentiles percentiles;
        String timestamp;
        SystemInfo system;
    }


    interface Setup {
        void run(com.mongocore.MongoClient client) throws Exception;
    }

    interface BeforeTask {
        void run(com.mongocore.MongoClient client) throws Exception;
    }

    interface Task {
        void run(com.mongocore.MongoClient client) throws Exception;
    }

    interface AfterTask {
        void run(com.mongocore.MongoClient client) throws Exception;
    }

    interface Teardown {
        void run(com.mongocore.MongoClient client) throws Exception;
    }

    private static double percentile(List<Double> data, int pct) {
        int idx = (int) Math.ceil(data.size() * pct / 100.0) - 1;
        if (idx < 0) idx = 0;
        if (idx >= data.size()) idx = data.size() - 1;
        return data.get(idx);
    }

    private static BenchResult runBenchmark(
            String name,
            String category,
            Setup setupFn,
            BeforeTask beforeTaskFn,
            Task taskFn,
            AfterTask afterTaskFn,
            Teardown teardownFn,
            int datasetSizeBytes,
            int batchSize,
            Config config
    ) throws Exception {
        com.mongocore.MongoClient client = com.mongocore.MongoClient.create(config.mongocore_address);

        setupFn.run(client);

        // Warmup
        int warmup = config.warmup_iterations.get("java");
        for (int i = 0; i < warmup; i++) {
            beforeTaskFn.run(client);
            taskFn.run(client);
            afterTaskFn.run(client);
        }

        // Timed iterations
        List<Double> times = new ArrayList<>();
        double totalTime = 0.0;
        int iteration = 0;

        while (totalTime < config.min_time_secs || iteration < 5) {
            if (iteration >= config.max_iterations || totalTime >= config.max_time_secs) {
                break;
            }

            beforeTaskFn.run(client);

            long start = System.nanoTime();
            taskFn.run(client);
            double elapsed = (System.nanoTime() - start) / 1_000_000_000.0;

            afterTaskFn.run(client);

            times.add(elapsed);
            totalTime += elapsed;
            iteration++;
        }

        teardownFn.run(client);
        client.close();

        // Calculate metrics
        Collections.sort(times);
        double median = times.get(times.size() / 2);
        double opsPerSec = batchSize / median;
        double mbPerSec = datasetSizeBytes / median / 1_000_000.0;

        BenchResult result = new BenchResult();
        result.benchmark = name;
        result.category = category;
        result.driver = "mongocore+java";
        result.dataset_size_bytes = datasetSizeBytes;
        result.batch_size = batchSize;
        result.iterations = times.size();
        result.total_time_secs = Math.round(totalTime * 1000) / 1000.0;
        result.ops_per_sec = Math.round(opsPerSec * 10) / 10.0;
        result.mb_per_sec = Math.round(mbPerSec * 1000) / 1000.0;

        Percentiles pct = new Percentiles();
        pct.p10 = Math.round(percentile(times, 10) * 1_000_000) / 1_000_000.0;
        pct.p25 = Math.round(percentile(times, 25) * 1_000_000) / 1_000_000.0;
        pct.p50 = Math.round(median * 1_000_000) / 1_000_000.0;
        pct.p75 = Math.round(percentile(times, 75) * 1_000_000) / 1_000_000.0;
        pct.p90 = Math.round(percentile(times, 90) * 1_000_000) / 1_000_000.0;
        pct.p95 = Math.round(percentile(times, 95) * 1_000_000) / 1_000_000.0;
        pct.p99 = Math.round(percentile(times, 99) * 1_000_000) / 1_000_000.0;
        result.percentiles = pct;

        result.timestamp = Instant.now().toString();
        result.system = new SystemInfo();

        System.out.printf("  %s: %.0f ops/s, %.2f MB/s (%d iterations)%n",
                name, opsPerSec, mbPerSec, times.size());
        return result;
    }

    public static void main(String[] args) throws Exception {
        System.out.println("=== MongoCore+Java benchmarks ===");

        // Load config
        Path configPath = Paths.get("..", "common.json");
        Config config = GSON.fromJson(new FileReader(configPath.toFile()), Config.class);

        // Load test documents
        Path dataDir = Paths.get("..", "..", "data");
        String smallDocJson = Files.readString(dataDir.resolve("small_doc.json"));
        String tweetDocJson = Files.readString(dataDir.resolve("tweet.json"));
        String largeDocJson = Files.readString(dataDir.resolve("large_doc.json"));

        Document smallDoc = Document.parse(smallDocJson);
        Document tweetDoc = Document.parse(tweetDocJson);
        Document largeDoc = Document.parse(largeDocJson);

        int smallSize = smallDocJson.getBytes().length;
        int tweetSize = tweetDocJson.getBytes().length;
        int largeSize = largeDocJson.getBytes().length;

        List<BenchResult> results = new ArrayList<>();

        // Run Command (batch 10,000 hello commands per iteration)
        results.add(runBenchmark(
                "run_command", "single_doc",
                client -> {},
                client -> {},
                client -> {
                    for (int i = 0; i < 10_000; i++) {
                        client.runCommand(config.database, new Document("hello", 1), false);
                    }
                },
                client -> {},
                client -> {},
                10_000 * 100, 10_000, config
        ));

        // Find One by ID (batch 10,000 finds per iteration)
        results.add(runBenchmark(
                "find_one_by_id", "single_doc",
                client -> {
                    try {
                        client.runCommand(config.database, new Document("drop", "bench_find_mc"), false);
                    } catch (Exception ignored) {}
                    Document doc = new Document(tweetDoc);
                    doc.put("_id", "bench_find_001");
                    client.getDatabase(config.database).getCollection("bench_find_mc").insertOne(doc);
                },
                client -> {},
                client -> {
                    for (int i = 0; i < 10_000; i++) {
                        client.getDatabase(config.database).getCollection("bench_find_mc")
                                .findOne(new Document("_id", "bench_find_001"));
                    }
                },
                client -> {},
                client -> {
                    client.runCommand(config.database, new Document("drop", "bench_find_mc"), false);
                },
                10_000 * tweetSize, 10_000, config
        ));

        // InsertOne Small (batch 10,000 inserts per iteration)
        results.add(runBenchmark(
                "insert_one_small", "single_doc",
                client -> {},
                client -> {
                    try {
                        client.runCommand(config.database, new Document("drop", "bench_insert_small_mc"), false);
                    } catch (Exception ignored) {}
                },
                client -> {
                    for (int i = 0; i < 10_000; i++) {
                        Document doc = new Document(smallDoc);
                        doc.put("_id", new ObjectId().toHexString());
                        client.getDatabase(config.database).getCollection("bench_insert_small_mc").insertOne(doc);
                    }
                },
                client -> {},
                client -> {},
                10_000 * smallSize, 10_000, config
        ));

        // InsertOne Large (batch 10 inserts per iteration, large docs ~2.75MB each)
        results.add(runBenchmark(
                "insert_one_large", "single_doc",
                client -> {},
                client -> {
                    try {
                        client.runCommand(config.database, new Document("drop", "bench_insert_large_mc"), false);
                    } catch (Exception ignored) {}
                },
                client -> {
                    for (int i = 0; i < 10; i++) {
                        Document doc = new Document(largeDoc);
                        doc.put("_id", new ObjectId().toHexString());
                        client.getDatabase(config.database).getCollection("bench_insert_large_mc").insertOne(doc);
                    }
                },
                client -> {},
                client -> {},
                10 * largeSize, 10, config
        ));

        // Bulk Insert Small (10K per iteration)
        results.add(runBenchmark(
                "bulk_insert_small", "multi_doc",
                client -> {},
                client -> {
                    try {
                        client.runCommand(config.database, new Document("drop", "bench_bulk_mc"), false);
                    } catch (Exception ignored) {}
                },
                client -> {
                    List<Document> docs = new ArrayList<>(10_000);
                    for (int i = 0; i < 10_000; i++) {
                        Document doc = new Document(smallDoc);
                        doc.put("_id", new ObjectId().toHexString());
                        docs.add(doc);
                    }
                    client.getDatabase(config.database).getCollection("bench_bulk_mc").insertMany(docs);
                },
                client -> {},
                client -> {},
                smallSize * 10_000, 10_000, config
        ));

        // Find Many (2K docs — limited by gRPC 4MB message size)
        // NOTE: Native drivers do 10K but proto-encoded response exceeds 4MB at higher counts
        // TODO: Increase gRPC max_receive_message_length or implement response streaming
        results.add(runBenchmark(
                "find_many", "multi_doc",
                client -> {
                    try {
                        client.runCommand(config.database, new Document("drop", "bench_find_many_mc"), false);
                    } catch (Exception ignored) {}
                    List<Document> docs = new ArrayList<>(2_000);
                    for (int i = 0; i < 2_000; i++) {
                        Document doc = new Document(smallDoc);
                        doc.put("_id", new ObjectId().toHexString());
                        docs.add(doc);
                    }
                    client.getDatabase(config.database).getCollection("bench_find_many_mc").insertMany(docs);
                },
                client -> {},
                client -> {
                    client.getDatabase(config.database).getCollection("bench_find_many_mc")
                            .find(new Document());
                },
                client -> {},
                client -> {},
                smallSize * 2_000, 2_000, config
        ));

        // Bulk Insert Large — SKIPPED: exceeds gRPC default 4MB message limit (10 x 2.75MB = 27.5MB)
        // TODO: Increase gRPC max_message_size in MongoCore config to enable this benchmark
        System.out.println("  bulk_insert_large: SKIPPED (exceeds gRPC 4MB message limit)");

        // Find Many Large — SKIPPED: 10 x 2.75MB = 27.5MB response exceeds gRPC 4MB limit
        // TODO: Implement streaming/pagination in MongoCore Find RPC to handle large result sets
        System.out.println("  find_many_large: SKIPPED (response exceeds gRPC 4MB message limit)");

        // Save results
        Path resultsDir = Paths.get("..", "..", "results");
        Files.createDirectories(resultsDir);
        Path outputPath = resultsDir.resolve("java_mongocore.json");
        try (FileWriter writer = new FileWriter(outputPath.toFile())) {
            GSON.toJson(results, writer);
        }
        System.out.println("\nResults saved to " + outputPath);
    }
}
