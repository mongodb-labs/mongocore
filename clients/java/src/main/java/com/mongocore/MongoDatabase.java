package com.mongocore;

public class MongoDatabase {
    private final MongoClient client;
    private final String name;

    MongoDatabase(MongoClient client, String name) {
        this.client = client;
        this.name = name;
    }

    public String getName() {
        return name;
    }

    public MongoCollection getCollection(String name) {
        return new MongoCollection(client, this.name, name);
    }

    MongoClient getClient() {
        return client;
    }
}
