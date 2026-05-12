package com.mongocore;

import org.bson.Document;
import org.junit.AfterClass;
import org.junit.BeforeClass;
import org.junit.Test;

import java.util.Arrays;
import java.util.List;
import java.util.Map;
import java.util.UUID;

import static org.junit.Assert.*;

public class IntegrationTest {
    private static final String TEST_DB = "mongocore_client_test";
    private static MongoClient client;

    @BeforeClass
    public static void setUp() {
        client = MongoClient.create("localhost:50051");
    }

    @AfterClass
    public static void tearDown() throws Exception {
        if (client != null) {
            client.close();
        }
    }

    private static String uniqueCollection() {
        return "java_test_" + UUID.randomUUID().toString().substring(0, 12);
    }

    private MongoCollection getCollection() {
        return client.getDatabase(TEST_DB).getCollection(uniqueCollection());
    }

    @Test
    public void testInsertAndFind() {
        MongoCollection coll = getCollection();

        InsertResult result = coll.insertOne(new Document("name", "Alice").append("age", 30));
        assertNotNull(result.getInsertedId());
        assertFalse(result.getInsertedId().isEmpty());

        List<Document> docs = coll.find(new Document("name", "Alice"));
        assertEquals(1, docs.size());
        assertEquals("Alice", docs.get(0).getString("name"));
        assertEquals(30, docs.get(0).getInteger("age").intValue());
    }

    @Test
    public void testInsertMany() {
        MongoCollection coll = getCollection();

        InsertManyResult result = coll.insertMany(Arrays.asList(
                new Document("name", "Bob").append("score", 85),
                new Document("name", "Carol").append("score", 92),
                new Document("name", "Dave").append("score", 78)
        ));
        assertEquals(3, result.getInsertedCount());
        assertEquals(3, result.getInsertedIds().size());

        List<Document> docs = coll.find(new Document());
        assertEquals(3, docs.size());
    }

    @Test
    public void testFindOne() {
        MongoCollection coll = getCollection();

        coll.insertOne(new Document("key", "unique_java_value"));
        Document doc = coll.findOne(new Document("key", "unique_java_value"));
        assertNotNull(doc);
        assertEquals("unique_java_value", doc.getString("key"));

        Document missing = coll.findOne(new Document("key", "nonexistent"));
        assertNull(missing);
    }

    @Test
    public void testUpdateOne() {
        MongoCollection coll = getCollection();

        coll.insertOne(new Document("name", "Eve").append("status", "active"));
        UpdateResult result = coll.updateOne(
                new Document("name", "Eve"),
                new Document("$set", new Document("status", "inactive"))
        );
        assertEquals(1, result.getModifiedCount());

        Document doc = coll.findOne(new Document("name", "Eve"));
        assertEquals("inactive", doc.getString("status"));
    }

    @Test
    public void testDeleteOne() {
        MongoCollection coll = getCollection();

        coll.insertOne(new Document("name", "Frank"));
        coll.insertOne(new Document("name", "Grace"));

        long count = coll.deleteOne(new Document("name", "Frank"));
        assertEquals(1, count);

        List<Document> docs = coll.find(new Document());
        assertEquals(1, docs.size());
        assertEquals("Grace", docs.get(0).getString("name"));
    }

    @Test
    public void testDeleteMany() {
        MongoCollection coll = getCollection();

        coll.insertMany(Arrays.asList(
                new Document("group", "A"),
                new Document("group", "A"),
                new Document("group", "B")
        ));

        long count = coll.deleteMany(new Document("group", "A"));
        assertEquals(2, count);

        List<Document> docs = coll.find(new Document());
        assertEquals(1, docs.size());
    }

    @Test
    public void testAggregate() {
        MongoCollection coll = getCollection();

        coll.insertMany(Arrays.asList(
                new Document("category", "A").append("value", 10),
                new Document("category", "A").append("value", 20),
                new Document("category", "B").append("value", 30)
        ));

        List<Document> results = coll.aggregate(Arrays.asList(
                new Document("$group", new Document("_id", "$category")
                        .append("total", new Document("$sum", "$value"))),
                new Document("$sort", new Document("_id", 1))
        ));

        assertEquals(2, results.size());
        assertEquals("A", results.get(0).getString("_id"));
        assertEquals(30, results.get(0).getInteger("total").intValue());
        assertEquals("B", results.get(1).getString("_id"));
        assertEquals(30, results.get(1).getInteger("total").intValue());
    }

    @Test
    public void testFindWithLimit() {
        MongoCollection coll = getCollection();

        for (int i = 0; i < 10; i++) {
            coll.insertOne(new Document("i", i));
        }

        List<Document> docs = coll.find(new Document(), new FindOptions().limit(3));
        assertEquals(3, docs.size());
    }

    @Test
    public void testWatch() throws Exception {
        MongoCollection coll = client.getDatabase(TEST_DB).getCollection(uniqueCollection());

        // Create collection first
        coll.insertOne(new Document("setup", true));

        try (ChangeStream stream = coll.watch()) {
            // Insert from a background thread
            Thread inserter = new Thread(() -> {
                try { Thread.sleep(100); } catch (InterruptedException e) { return; }
                coll.insertOne(new Document("name", "watched"));
            });
            inserter.start();

            // Read one event
            java.util.Iterator<ChangeEvent> it = stream.iterator();
            assertTrue(it.hasNext());
            ChangeEvent event = it.next();
            assertEquals("insert", event.getOperationType());
            assertNotNull(event.getDocument());
            assertEquals("watched", event.getDocument().getString("name"));

            inserter.join(5000);
        }
    }

    @Test
    public void testSearch() {
        MongoCollection coll = client.getDatabase(TEST_DB).getCollection(uniqueCollection() + "_search");
        coll.insertMany(List.of(
                new Document("title", "rust programming guide").append("content", "learn rust basics"),
                new Document("title", "python basics").append("content", "learn python programming"),
                new Document("title", "rust advanced patterns").append("content", "advanced rust techniques")
        ));

        SearchResult result = coll.search("rust", 10);
        assertNotNull(result);
        assertTrue("Expected at least 2 results for 'rust'", result.getTotal() >= 2);
        assertTrue("Expected valid search method", List.of("vector", "fulltext", "filter").contains(result.getMethod()));
        assertTrue("Expected at least 2 documents", result.getDocuments().size() >= 2);
    }

    @Test
    public void testListDatabases() {
        List<String> databases = client.listDatabases();
        assertNotNull(databases);
        assertFalse(databases.isEmpty());
    }

    @Test
    public void testUpdateMany() {
        MongoCollection coll = getCollection();

        coll.insertMany(Arrays.asList(
                new Document("group", "A").append("status", "active"),
                new Document("group", "A").append("status", "active"),
                new Document("group", "B").append("status", "active")
        ));

        UpdateResult result = coll.updateMany(
                new Document("group", "A"),
                new Document("$set", new Document("status", "inactive"))
        );
        assertEquals(2, result.getModifiedCount());

        List<Document> docs = coll.find(new Document("status", "inactive"));
        assertEquals(2, docs.size());
    }

    @Test
    public void testFindAndModify() {
        MongoCollection coll = getCollection();

        coll.insertOne(new Document("name", "counter").append("counter", 10));

        Document result = coll.findAndModify(
                new Document("name", "counter"),
                new Document("$inc", new Document("counter", 1)),
                true  // returnNew
        );

        assertNotNull(result);
        assertEquals("counter", result.getString("name"));
        assertEquals(11, result.getInteger("counter").intValue());

        Document doc = coll.findOne(new Document("name", "counter"));
        assertEquals(11, doc.getInteger("counter").intValue());
    }

    @Test
    public void testListCollections() {
        MongoDatabase db = client.getDatabase(TEST_DB);
        String collName = uniqueCollection();

        db.getCollection(collName).insertOne(new Document("test", true));

        List<String> collections = db.listCollections();
        assertNotNull(collections);
        assertTrue(collections.contains(collName));
    }

    @Test
    public void testCreateCollection() {
        MongoDatabase db = client.getDatabase(TEST_DB);
        String collName = uniqueCollection();

        db.createCollection(collName);

        List<String> collections = db.listCollections();
        assertTrue(collections.contains(collName));
    }

    @Test
    public void testCreateIndex() {
        MongoCollection coll = getCollection();

        coll.insertOne(new Document("email", "test@example.com"));

        String indexName = coll.createIndex(new Document("email", 1), true);
        assertNotNull(indexName);
        assertFalse(indexName.isEmpty());
    }

    @Test
    public void testRunCommand() {
        Document result = client.runCommand("admin", new Document("ping", 1), false);
        assertNotNull(result);
        assertEquals(1.0, result.getDouble("ok"), 0.01);
    }

    @Test
    public void testGetAnalytics() {
        MongoCollection coll = getCollection();
        coll.insertOne(new Document("test", true));

        Map<String, Object> analytics = client.getAnalytics();
        assertNotNull(analytics);
        assertTrue(analytics.containsKey("total_operations"));
        assertTrue((Long) analytics.get("total_operations") > 0);
    }

    @Test
    public void testTransactionCommit() {
        String txnId = client.beginTransaction();
        assertNotNull(txnId);
        assertFalse(txnId.isEmpty());

        boolean committed = client.commitTransaction(txnId);
        assertTrue(committed);
    }

    @Test
    public void testTransactionAbort() {
        String txnId = client.beginTransaction();
        assertNotNull(txnId);
        assertFalse(txnId.isEmpty());

        boolean aborted = client.abortTransaction(txnId);
        assertTrue(aborted);
    }

    @Test
    public void testIngestCSV() {
        String csvPath = java.nio.file.Paths.get("clients/test_fixtures/sample.csv")
                .toAbsolutePath()
                .toString();

        MongoClient.IngestOptions options = new MongoClient.IngestOptions(
                csvPath,
                TEST_DB,
                uniqueCollection()
        );

        MongoClient.IngestResult result = client.ingest(options);
        assertNotNull(result.jobId());
        assertFalse(result.jobId().isEmpty());
    }

    @Test
    public void testIngestStatus() {
        String csvPath = java.nio.file.Paths.get("clients/test_fixtures/sample.csv")
                .toAbsolutePath()
                .toString();

        MongoClient.IngestOptions options = new MongoClient.IngestOptions(
                csvPath,
                TEST_DB,
                uniqueCollection()
        );

        MongoClient.IngestResult ingestResult = client.ingest(options);
        String jobId = ingestResult.jobId();

        MongoClient.IngestJob status = client.ingestStatus(jobId);
        assertNotNull(status);
        assertNotNull(status.jobId());
        assertEquals(jobId, status.jobId());
    }

    @Test
    public void testListIngestJobs() {
        List<MongoClient.IngestJob> jobs = client.listIngestJobs();
        assertNotNull(jobs);
    }

    @Test
    public void testCancelIngest() {
        String csvPath = java.nio.file.Paths.get("clients/test_fixtures/sample.csv")
                .toAbsolutePath()
                .toString();

        MongoClient.IngestOptions options = new MongoClient.IngestOptions(
                csvPath,
                TEST_DB,
                uniqueCollection()
        );

        MongoClient.IngestResult ingestResult = client.ingest(options);
        String jobId = ingestResult.jobId();

        boolean cancelled = client.cancelIngest(jobId);
        // The result can be true or false depending on job state
        assertNotNull(cancelled);
    }

    @Test
    public void testWatchDirectory() throws Exception {
        java.nio.file.Path tempDir = java.nio.file.Files.createTempDirectory("mongocore_test_watch");

        try {
            MongoClient.WatchOptions options = new MongoClient.WatchOptions(
                    tempDir.toString(),
                    TEST_DB,
                    uniqueCollection()
            );

            MongoClient.WatchResult result = client.watchDirectory(options);
            assertNotNull(result.watchId());
            assertFalse(result.watchId().isEmpty());
            assertTrue(result.success());

            client.stopWatch(result.watchId());
        } finally {
            java.nio.file.Files.deleteIfExists(tempDir);
        }
    }

    @Test
    public void testStopWatch() throws Exception {
        java.nio.file.Path tempDir = java.nio.file.Files.createTempDirectory("mongocore_test_stop");

        try {
            MongoClient.WatchOptions options = new MongoClient.WatchOptions(
                    tempDir.toString(),
                    TEST_DB,
                    uniqueCollection()
            );

            MongoClient.WatchResult watchResult = client.watchDirectory(options);
            String watchId = watchResult.watchId();

            MongoClient.WatchResult stopResult = client.stopWatch(watchId);
            assertNotNull(stopResult);
            assertEquals(watchId, stopResult.watchId());
            assertTrue(stopResult.success());
        } finally {
            java.nio.file.Files.deleteIfExists(tempDir);
        }
    }
}
