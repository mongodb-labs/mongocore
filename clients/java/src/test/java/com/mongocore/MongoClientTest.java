package com.mongocore;

import org.junit.Test;
import static org.junit.Assert.*;

public class MongoClientTest {
    @Test
    public void testClientCreation() {
        MongoClient client = MongoClient.create("localhost:50051");
        assertNotNull(client);
    }

    @Test
    public void testDatabaseAccess() {
        MongoClient client = MongoClient.create();
        MongoDatabase db = client.getDatabase("testdb");
        assertEquals("testdb", db.getName());
    }

    @Test
    public void testCollectionAccess() {
        MongoClient client = MongoClient.create();
        MongoDatabase db = client.getDatabase("testdb");
        MongoCollection coll = db.getCollection("users");
        assertEquals("users", coll.getName());
        assertEquals("testdb", coll.getDatabase());
    }

    @Test
    public void testFindOptionsBuilder() {
        FindOptions opts = new FindOptions()
                .limit(10)
                .skip(5);
        assertEquals(Integer.valueOf(10), opts.getLimit());
        assertEquals(Integer.valueOf(5), opts.getSkip());
    }

    @Test
    public void testDefaultAddress() {
        MongoClient client = MongoClient.create();
        assertNotNull(client);
    }

    @Test
    public void testMetadataInterceptor() {
        MongoClient client = MongoClient.create("localhost:50051");
        assertNotNull(client);
    }
}
