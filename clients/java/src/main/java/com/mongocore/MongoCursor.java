package com.mongocore;

import com.google.protobuf.ByteString;
import mongocore.v1.Types;
import org.bson.BsonBinaryReader;
import org.bson.Document;
import org.bson.codecs.DecoderContext;
import org.bson.codecs.DocumentCodec;

import java.io.Closeable;
import java.nio.ByteBuffer;
import java.util.ArrayList;
import java.util.Iterator;
import java.util.List;
import java.util.NoSuchElementException;
import java.util.stream.Stream;
import java.util.stream.StreamSupport;

public class MongoCursor implements AutoCloseable, Iterator<Document>, Iterable<Document> {
    private static final DocumentCodec CODEC = new DocumentCodec();

    private final Iterator<Types.DocumentBatch> stream;
    private List<Document> buffer = new ArrayList<>();
    private int bufferIndex = 0;
    private boolean exhausted = false;

    MongoCursor(Iterator<Types.DocumentBatch> stream) {
        this.stream = stream;
    }

    private Document decodeBson(ByteString data) {
        byte[] bytes = data.toByteArray();
        BsonBinaryReader reader = new BsonBinaryReader(ByteBuffer.wrap(bytes));
        return CODEC.decode(reader, DecoderContext.builder().build());
    }

    @Override
    public boolean hasNext() {
        if (bufferIndex < buffer.size()) {
            return true;
        }
        if (exhausted) {
            return false;
        }
        fetchNextBatch();
        return bufferIndex < buffer.size();
    }

    @Override
    public Document next() {
        if (!hasNext()) {
            throw new NoSuchElementException("Cursor exhausted");
        }
        return buffer.get(bufferIndex++);
    }

    private void fetchNextBatch() {
        if (!stream.hasNext()) {
            exhausted = true;
            return;
        }
        Types.DocumentBatch batch = stream.next();
        buffer = new ArrayList<>(batch.getDocumentsCount());
        for (Types.Document d : batch.getDocumentsList()) {
            buffer.add(decodeBson(d.getData()));
        }
        bufferIndex = 0;
        if (!batch.getHasMore()) {
            exhausted = true;
        }
    }

    @Override
    public Iterator<Document> iterator() {
        return this;
    }

    public Stream<Document> stream() {
        Iterable<Document> iterable = this;
        return StreamSupport.stream(iterable.spliterator(), false).onClose(this::close);
    }

    public List<Document> toList() {
        List<Document> results = new ArrayList<>();
        while (hasNext()) {
            results.add(next());
        }
        return results;
    }

    @Override
    public void close() {
        exhausted = true;
        if (stream instanceof Closeable) {
            try {
                ((Closeable) stream).close();
            } catch (Exception ignored) {
            }
        }
    }
}
