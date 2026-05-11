package com.mongocore;

import org.bson.Document;
import org.junit.AfterClass;
import org.junit.BeforeClass;
import org.junit.Test;

import java.util.Arrays;
import java.util.List;
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
}
