package com.mongocore.bench;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.mongodb.client.MongoClient;
import com.mongodb.client.MongoClients;
import com.mongodb.client.MongoCollection;
import com.mongodb.client.MongoDatabase;
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

public class BenchNative {
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
            this.mongocore_version = "native";
            this.driver = "mongodb-java-sync";
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

    interface Task {
        void run(MongoDatabase db) throws Exception;
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
            Task setupFn,
            Task beforeTaskFn,
            Task taskFn,
            Task afterTaskFn,
            Task teardownFn,
            int datasetSizeBytes,
            int batchSize,
            Config config
    ) throws Exception {
        try (MongoClient client = MongoClients.create(config.mongodb_uri)) {
            MongoDatabase db = client.getDatabase(config.database);

            setupFn.run(db);

            // Warmup
            int warmup = config.warmup_iterations.get("java");
            for (int i = 0; i < warmup; i++) {
                beforeTaskFn.run(db);
                taskFn.run(db);
                afterTaskFn.run(db);
            }

            // Timed iterations
            List<Double> times = new ArrayList<>();
            double totalTime = 0.0;
            int iteration = 0;

            while (totalTime < config.min_time_secs || iteration < 5) {
                if (iteration >= config.max_iterations || totalTime >= config.max_time_secs) {
                    break;
                }

                beforeTaskFn.run(db);

                long start = System.nanoTime();
                taskFn.run(db);
                double elapsed = (System.nanoTime() - start) / 1_000_000_000.0;

                afterTaskFn.run(db);

                times.add(elapsed);
                totalTime += elapsed;
                iteration++;
            }

            teardownFn.run(db);

            // Calculate metrics
            Collections.sort(times);
            double median = times.get(times.size() / 2);
            double opsPerSec = batchSize / median;
            double mbPerSec = datasetSizeBytes / median / 1_000_000.0;

            BenchResult result = new BenchResult();
            result.benchmark = name;
            result.category = category;
            result.driver = "mongodb-java-sync";
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
    }

    public static void main(String[] args) throws Exception {
        System.out.println("=== MongoDB Java driver (native) benchmarks ===");

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
                db -> {},
                db -> {},
                db -> {
                    for (int i = 0; i < 10_000; i++) {
                        db.runCommand(new Document("hello", 1));
                    }
                },
                db -> {},
                db -> {},
                10_000 * 100, 10_000, config
        ));

        // Find One by ID (batch 10,000 finds per iteration)
        results.add(runBenchmark(
                "find_one_by_id", "single_doc",
                db -> {
                    MongoCollection<Document> coll = db.getCollection("bench_find");
                    coll.drop();
                    Document doc = new Document("_id", new ObjectId("000000000000000000000001"));
                    tweetDoc.forEach((k, v) -> {
                        if (!k.equals("_id")) doc.append(k, v);
                    });
                    coll.insertOne(doc);
                },
                db -> {},
                db -> {
                    MongoCollection<Document> coll = db.getCollection("bench_find");
                    for (int i = 0; i < 10_000; i++) {
                        coll.find(new Document("_id", new ObjectId("000000000000000000000001"))).first();
                    }
                },
                db -> {},
                db -> db.getCollection("bench_find").drop(),
                10_000 * tweetSize, 10_000, config
        ));

        // InsertOne Small (batch 10,000 inserts per iteration)
        results.add(runBenchmark(
                "insert_one_small", "single_doc",
                db -> {},
                db -> db.getCollection("bench_insert_small").drop(),
                db -> {
                    MongoCollection<Document> coll = db.getCollection("bench_insert_small");
                    for (int i = 0; i < 10_000; i++) {
                        Document doc = new Document("_id", new ObjectId());
                        smallDoc.forEach((k, v) -> {
                            if (!k.equals("_id")) doc.append(k, v);
                        });
                        coll.insertOne(doc);
                    }
                },
                db -> {},
                db -> {},
                10_000 * smallSize, 10_000, config
        ));

        // InsertOne Large (batch 10 inserts per iteration)
        results.add(runBenchmark(
                "insert_one_large", "single_doc",
                db -> {},
                db -> db.getCollection("bench_insert_large").drop(),
                db -> {
                    MongoCollection<Document> coll = db.getCollection("bench_insert_large");
                    for (int i = 0; i < 10; i++) {
                        Document doc = new Document("_id", new ObjectId());
                        largeDoc.forEach((k, v) -> {
                            if (!k.equals("_id")) doc.append(k, v);
                        });
                        coll.insertOne(doc);
                    }
                },
                db -> {},
                db -> {},
                10 * largeSize, 10, config
        ));

        // Bulk Insert Small (10,000 docs per iteration)
        results.add(runBenchmark(
                "bulk_insert_small", "multi_doc",
                db -> {},
                db -> db.getCollection("bench_bulk").drop(),
                db -> {
                    List<Document> docs = new ArrayList<>(10_000);
                    for (int i = 0; i < 10_000; i++) {
                        Document doc = new Document("_id", new ObjectId());
                        smallDoc.forEach((k, v) -> {
                            if (!k.equals("_id")) doc.append(k, v);
                        });
                        docs.add(doc);
                    }
                    db.getCollection("bench_bulk").insertMany(docs);
                },
                db -> {},
                db -> {},
                smallSize * 10_000, 10_000, config
        ));

        // Find Many (10,000 docs)
        results.add(runBenchmark(
                "find_many", "multi_doc",
                db -> {
                    MongoCollection<Document> coll = db.getCollection("bench_find_many");
                    coll.drop();
                    List<Document> docs = new ArrayList<>(10_000);
                    for (int i = 0; i < 10_000; i++) {
                        Document doc = new Document("_id", new ObjectId());
                        smallDoc.forEach((k, v) -> {
                            if (!k.equals("_id")) doc.append(k, v);
                        });
                        docs.add(doc);
                    }
                    coll.insertMany(docs);
                },
                db -> {},
                db -> {
                    List<Document> result = new ArrayList<>();
                    db.getCollection("bench_find_many").find().into(result);
                },
                db -> {},
                db -> db.getCollection("bench_find_many").drop(),
                smallSize * 10_000, 10_000, config
        ));

        // Save results
        Path resultsDir = Paths.get("..", "..", "results");
        Files.createDirectories(resultsDir);
        Path outputPath = resultsDir.resolve("java_native.json");
        try (FileWriter writer = new FileWriter(outputPath.toFile())) {
            GSON.toJson(results, writer);
        }
        System.out.println("\nResults saved to " + outputPath);
    }
}
