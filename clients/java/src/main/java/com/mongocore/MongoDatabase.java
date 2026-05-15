package com.mongocore;

import mongocore.v1.MongoCoreGrpc;
import mongocore.v1.Mongocore;

import java.util.List;

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

    public List<String> listCollections() {
        MongoCoreGrpc.MongoCoreBlockingStub stub = MongoCoreGrpc.newBlockingStub(client.getChannel());
        Mongocore.ListCollectionsResponse resp = stub.listCollections(
                Mongocore.ListCollectionsRequest.newBuilder()
                        .setDatabase(name)
                        .build());
        return resp.getCollectionsList();
    }

    public void createCollection(String collectionName) {
        MongoCoreGrpc.MongoCoreBlockingStub stub = MongoCoreGrpc.newBlockingStub(client.getChannel());
        stub.createCollection(
                Mongocore.CreateCollectionRequest.newBuilder()
                        .setDatabase(name)
                        .setCollection(collectionName)
                        .build());
    }

    public boolean dropCollection(String collectionName) {
        MongoCoreGrpc.MongoCoreBlockingStub stub = MongoCoreGrpc.newBlockingStub(client.getChannel());
        Mongocore.DropCollectionResponse resp = stub.dropCollection(
                Mongocore.DropCollectionRequest.newBuilder()
                        .setDatabase(name)
                        .setCollection(collectionName)
                        .build());
        return resp.getOk();
    }

    MongoClient getClient() {
        return client;
    }
}
