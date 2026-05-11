package com.mongocore;

import java.util.List;

public class InsertManyResult {
    private final List<String> insertedIds;

    public InsertManyResult(List<String> insertedIds) {
        this.insertedIds = insertedIds;
    }

    public List<String> getInsertedIds() { return insertedIds; }
    public int getInsertedCount() { return insertedIds.size(); }
}
