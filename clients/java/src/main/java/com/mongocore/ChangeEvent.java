package com.mongocore;

import org.bson.Document;

public class ChangeEvent {
    private final String operationType;
    private final String database;
    private final String collection;
    private Document document;
    private Document updateDescription;
    private Document documentKey;

    public ChangeEvent(String operationType, String database, String collection) {
        this.operationType = operationType;
        this.database = database;
        this.collection = collection;
    }

    public String getOperationType() { return operationType; }
    public String getDatabase() { return database; }
    public String getCollection() { return collection; }
    public Document getDocument() { return document; }
    public Document getUpdateDescription() { return updateDescription; }
    public Document getDocumentKey() { return documentKey; }

    void setDocument(Document document) { this.document = document; }
    void setUpdateDescription(Document updateDescription) { this.updateDescription = updateDescription; }
    void setDocumentKey(Document documentKey) { this.documentKey = documentKey; }
}
