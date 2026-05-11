package com.mongocore;

import io.grpc.ManagedChannel;
import io.grpc.ManagedChannelBuilder;
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

    ManagedChannel getChannel() {
        return channel;
    }

    @Override
    public void close() throws Exception {
        channel.shutdown().awaitTermination(5, TimeUnit.SECONDS);
    }
}
