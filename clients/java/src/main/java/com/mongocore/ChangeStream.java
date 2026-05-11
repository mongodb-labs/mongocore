package com.mongocore;

import com.google.protobuf.ByteString;
import mongocore.v1.Mongocore;
import org.bson.BsonBinaryReader;
import org.bson.Document;
import org.bson.codecs.DecoderContext;
import org.bson.codecs.DocumentCodec;

import java.nio.ByteBuffer;
import java.util.Iterator;
import java.util.NoSuchElementException;

public class ChangeStream implements AutoCloseable, Iterable<ChangeEvent> {
    private static final DocumentCodec CODEC = new DocumentCodec();
    private static final String[] OP_TYPE_NAMES = {"insert", "update", "delete", "replace", "invalidate"};

    private final Iterator<Mongocore.WatchEvent> stream;
    private final MongoCollection collection;
    private volatile boolean closed = false;

    ChangeStream(Iterator<Mongocore.WatchEvent> stream, MongoCollection collection) {
        this.stream = stream;
        this.collection = collection;
    }

    private Document decodeBson(ByteString data) {
        byte[] bytes = data.toByteArray();
        BsonBinaryReader reader = new BsonBinaryReader(ByteBuffer.wrap(bytes));
        return CODEC.decode(reader, DecoderContext.builder().build());
    }

    @Override
    public void close() {
        closed = true;
    }

    @Override
    public Iterator<ChangeEvent> iterator() {
        return new Iterator<ChangeEvent>() {
            @Override
            public boolean hasNext() {
                if (closed) return false;
                return stream.hasNext();
            }

            @Override
            public ChangeEvent next() {
                if (closed || !stream.hasNext()) {
                    throw new NoSuchElementException("Change stream is closed or exhausted");
                }
                Mongocore.WatchEvent event = stream.next();
                String opType = event.getOperationTypeValue() < OP_TYPE_NAMES.length
                        ? OP_TYPE_NAMES[event.getOperationTypeValue()]
                        : "unknown";

                ChangeEvent ce = new ChangeEvent(opType, event.getDatabase(), event.getCollection());

                if (event.hasDocument() && !event.getDocument().getData().isEmpty()) {
                    ce.setDocument(decodeBson(event.getDocument().getData()));
                }
                if (event.hasUpdateDescription() && !event.getUpdateDescription().getData().isEmpty()) {
                    ce.setUpdateDescription(decodeBson(event.getUpdateDescription().getData()));
                }
                if (event.hasDocumentKey() && !event.getDocumentKey().getData().isEmpty()) {
                    ce.setDocumentKey(decodeBson(event.getDocumentKey().getData()));
                }
                return ce;
            }
        };
    }
}
