package com.mongocore;

public class UpdateResult {
    private final long matchedCount;
    private final long modifiedCount;
    private final String upsertedId;

    public UpdateResult(long matchedCount, long modifiedCount, String upsertedId) {
        this.matchedCount = matchedCount;
        this.modifiedCount = modifiedCount;
        this.upsertedId = upsertedId;
    }

    public long getMatchedCount() { return matchedCount; }
    public long getModifiedCount() { return modifiedCount; }
    public String getUpsertedId() { return upsertedId; }
}
