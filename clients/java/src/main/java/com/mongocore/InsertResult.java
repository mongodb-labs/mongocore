package com.mongocore;

public class InsertResult {
    private final String insertedId;

    public InsertResult(String insertedId) {
        this.insertedId = insertedId;
    }

    public String getInsertedId() { return insertedId; }
}
