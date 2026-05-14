package com.mongocore.bench;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.mongocore.MongoClient;
import com.mongocore.Ops;
import mongocore.v1.Mongocore;
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

public class BenchPipeline {
    private static final Gson GSON = new GsonBuilder().setPrettyPrinting().create();
    private static final int[] BATCH_SIZES = {100, 1000, 10000};
    private static final int TOTAL_OPS = 10000;

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

    interface TaskFn {
        void run(MongoClient client) throws Exception;
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
            MongoClient client,
            TaskFn setupFn,
            TaskFn beforeTaskFn,
            TaskFn taskFn,
            TaskFn afterTaskFn,
            TaskFn teardownFn,
            int datasetSizeBytes,
            int batchSize,
            Config config
    ) throws Exception {
        setupFn.run(client);

        int warmup = config.warmup_iterations.get("java");
        for (int i = 0; i < warmup; i++) {
            beforeTaskFn.run(client);
            taskFn.run(client);
            afterTaskFn.run(client);
        }

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
        System.out.println("=== MongoCore+Java Pipeline benchmarks ===");

        Path configPath = Paths.get("..", "common.json");
        Config config = GSON.fromJson(new FileReader(configPath.toFile()), Config.class);

        Path dataDir = Paths.get("..", "..", "data");
        String smallDocJson = Files.readString(dataDir.resolve("small_doc.json"));
        String tweetDocJson = Files.readString(dataDir.resolve("tweet.json"));

        Document smallDoc = Document.parse(smallDocJson);
        Document tweetDoc = Document.parse(tweetDocJson);

        int smallSize = smallDocJson.getBytes().length;
        int tweetSize = tweetDocJson.getBytes().length;

        MongoClient client = MongoClient.create(config.mongocore_address);
        List<BenchResult> results = new ArrayList<>();

        for (int bs : BATCH_SIZES) {
            int callsPerIter = TOTAL_OPS / bs;

            // --- pipeline_run_command ---
            final int fbs = bs;
            final int fcalls = callsPerIter;
            results.add(runBenchmark(
                    "pipeline_run_command_" + bs, "pipeline", client,
                    c -> {},
                    c -> {},
                    c -> {
                        for (int call = 0; call < fcalls; call++) {
                            Mongocore.PipelineOperation[] ops = new Mongocore.PipelineOperation[fbs];
                            for (int i = 0; i < fbs; i++) {
                                ops[i] = Ops.runCommand(config.database, new Document("hello", 1), false);
                            }
                            c.pipeline(ops);
                        }
                    },
                    c -> {},
                    c -> {},
                    TOTAL_OPS * 100, TOTAL_OPS, config
            ));

            // --- pipeline_insert_one_small ---
            String collInsert = "bench_pipeline_insert_java";
            results.add(runBenchmark(
                    "pipeline_insert_one_small_" + bs, "pipeline", client,
                    c -> {},
                    c -> {
                        try { c.runCommand(config.database, new Document("drop", collInsert), false); } catch (Exception ignored) {}
                    },
                    c -> {
                        for (int call = 0; call < fcalls; call++) {
                            Mongocore.PipelineOperation[] ops = new Mongocore.PipelineOperation[fbs];
                            for (int i = 0; i < fbs; i++) {
                                Document doc = new Document(smallDoc);
                                doc.put("_id", new ObjectId().toHexString());
                                ops[i] = Ops.insert(config.database, collInsert, doc);
                            }
                            c.pipeline(ops);
                        }
                    },
                    c -> {},
                    c -> {},
                    TOTAL_OPS * smallSize, TOTAL_OPS, config
            ));

            // --- pipeline_find_one_by_id ---
            String collFind = "bench_pipeline_find_java";
            results.add(runBenchmark(
                    "pipeline_find_one_by_id_" + bs, "pipeline", client,
                    c -> {
                        try { c.runCommand(config.database, new Document("drop", collFind), false); } catch (Exception ignored) {}
                        Document doc = new Document(tweetDoc);
                        doc.put("_id", "bench_find_001");
                        c.getDatabase(config.database).getCollection(collFind).insertOne(doc);
                    },
                    c -> {},
                    c -> {
                        for (int call = 0; call < fcalls; call++) {
                            Mongocore.PipelineOperation[] ops = new Mongocore.PipelineOperation[fbs];
                            for (int i = 0; i < fbs; i++) {
                                ops[i] = Ops.findOne(config.database, collFind, new Document("_id", "bench_find_001"));
                            }
                            c.pipeline(ops);
                        }
                    },
                    c -> {},
                    c -> {
                        c.runCommand(config.database, new Document("drop", collFind), false);
                    },
                    TOTAL_OPS * tweetSize, TOTAL_OPS, config
            ));
        }

        client.close();

        Path resultsDir = Paths.get("..", "..", "results");
        Files.createDirectories(resultsDir);
        Path outputPath = resultsDir.resolve("java_pipeline.json");
        try (FileWriter writer = new FileWriter(outputPath.toFile())) {
            GSON.toJson(results, writer);
        }
        System.out.printf("%nResults saved to %s%n", outputPath);
    }
}
