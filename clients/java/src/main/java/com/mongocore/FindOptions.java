package com.mongocore;

import org.bson.Document;

public class FindOptions {
    private Integer limit;
    private Integer skip;
    private Document sort;
    private Document projection;
    private Integer batchSize;

    public FindOptions limit(int limit) {
        this.limit = limit;
        return this;
    }

    public FindOptions skip(int skip) {
        this.skip = skip;
        return this;
    }

    public FindOptions sort(Document sort) {
        this.sort = sort;
        return this;
    }

    public FindOptions projection(Document projection) {
        this.projection = projection;
        return this;
    }

    public FindOptions batchSize(int batchSize) {
        this.batchSize = batchSize;
        return this;
    }

    public Integer getLimit() { return limit; }
    public Integer getSkip() { return skip; }
    public Document getSort() { return sort; }
    public Document getProjection() { return projection; }
    public Integer getBatchSize() { return batchSize; }
}
