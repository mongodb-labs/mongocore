# Search

MongoCore provides a unified search interface with automatic fallback: vector search, full-text search, and filter-based search. The engine tries the most capable method available and falls back gracefully.

## Fallback Chain

```
Vector Search (Atlas Vector Search + Voyage AI embeddings)
    │ not available or fails
    ▼
Full-Text Search (Atlas Search)
    │ not available or fails
    ▼
Filter Search ($text index, then scan)
```

## Prerequisites

| Search Method | Requirements |
|--------------|--------------|
| Vector Search | Atlas cluster + Vector Search index + `VOYAGE_API_KEY` |
| Full-Text Search | Atlas cluster + Atlas Search index |
| Filter Search | Any MongoDB deployment with a `$text` index |

## Configuration

```toml
# mongocore.toml
voyage_api_key_env = "VOYAGE_API_KEY"  # Enables vector search
```

```bash
# Or via CLI
mongocore --voyage-api-key-env VOYAGE_API_KEY
```

## Vector Search

Embeds your query using Voyage AI, then runs `$vectorSearch` against an Atlas Vector Search index.

### Setting Up

1. Create a vector search index on your Atlas collection:
   ```json
   {
     "type": "vectorSearch",
     "fields": [{
       "type": "vector",
       "path": "embedding",
       "numDimensions": 1024,
       "similarity": "cosine"
     }]
   }
   ```

2. Configure MongoCore with your Voyage AI key
3. Ensure documents have an `embedding` field (or specify a custom field path)

### Python

```python
async with MongoClient() as client:
    articles = client["content"]["articles"]

    # Automatic search (uses best available method)
    results = await articles.search("machine learning optimization techniques", limit=10)
    print(f"Method used: {results.method}")  # "vector", "fulltext", or "filter"

    for doc in results.documents:
        print(doc["title"])

    # Explicit vector search with custom index/field
    results = await articles.vector_search(
        query="neural network architectures",
        index_name="article_vectors",
        field_path="content_embedding",
        limit=5
    )
```

### TypeScript

```typescript
const articles = client.db('content').collection('articles');

// Automatic search
const results = await articles.search('machine learning optimization', { limit: 10 });
console.log(`Method: ${results.method}`);

// Explicit vector search
const vectorResults = await articles.vectorSearch({
  query: 'neural network architectures',
  indexName: 'article_vectors',
  fieldPath: 'content_embedding',
  limit: 5,
});
```

### Go

```go
articles := client.Database("content").Collection("articles")

// Automatic search
results, err := articles.Search(ctx, "machine learning optimization", 10)
fmt.Printf("Method: %s\n", results.Method)

// Explicit vector search
results, err = articles.VectorSearch(ctx, &mongocore.VectorSearchOptions{
    Query:     "neural network architectures",
    IndexName: "article_vectors",
    FieldPath: "content_embedding",
    Limit:     5,
})
```

## Full-Text Search

Uses Atlas Search's `$search` aggregation stage with the `text` operator.

### Setting Up

Create an Atlas Search index on your collection:
```json
{
  "mappings": {
    "dynamic": true
  }
}
```

### Usage

Full-text search is used automatically when vector search is unavailable. The engine builds a `$search` pipeline with the `text` operator targeting all fields.

```python
# If only Atlas Search is available (no Voyage AI key), this uses full-text
results = await articles.search("optimization techniques", limit=10)
assert results.method == "fulltext"
```

## Filter Fallback

When neither Atlas Vector Search nor Atlas Search is available, MongoCore falls back to MongoDB's `$text` operator (requires a text index) or returns all documents up to the limit.

```python
# On a standalone MongoDB with a text index
results = await articles.search("optimization", limit=10)
assert results.method == "filter"
```

## Voyage AI Embeddings

MongoCore uses the Voyage AI REST API for generating embeddings. The client supports batched requests for efficiency.

### Supported Models

- `voyage-3` (default) — General-purpose, 1024 dimensions
- `voyage-3-lite` — Faster, 512 dimensions
- `voyage-code-3` — Optimized for code

### Batch Embedding

```python
from mongocore.voyage import VoyageClient

voyage = VoyageClient(api_key="your-key", model="voyage-3")

# Batch embed documents before inserting
texts = ["doc1 content", "doc2 content", "doc3 content"]
embeddings = await voyage.embed(texts)

# Insert with embeddings
for text, embedding in zip(texts, embeddings):
    await collection.insert_one({
        "content": text,
        "embedding": embedding
    })
```

## Response Metadata

All MongoCore responses include metadata indicating which search method was used:

```protobuf
message ResponseMetadata {
  string search_method = 1;  // "vector", "fulltext", "compiled_query", ""
}
```

This lets your application adapt behavior based on the quality of results (vector search results are semantically ranked; filter results are not).
