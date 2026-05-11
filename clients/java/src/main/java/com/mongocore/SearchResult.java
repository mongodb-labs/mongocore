package com.mongocore;

import org.bson.Document;
import java.util.List;

public class SearchResult {
    private final List<Document> documents;
    private final String method;
    private final long total;

    public SearchResult(List<Document> documents, String method, long total) {
        this.documents = documents;
        this.method = method;
        this.total = total;
    }

    public List<Document> getDocuments() {
        return documents;
    }

    public String getMethod() {
        return method;
    }

    public long getTotal() {
        return total;
    }
}
