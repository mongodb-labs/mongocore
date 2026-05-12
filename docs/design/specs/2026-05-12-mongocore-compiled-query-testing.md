# MongoCore: Compiled Query Testing — Real LLM Integration

## Overview

Add real LLM integration tests for the compiled query system (NL→MQL) that call Claude/OpenAI, validate the generated MQL, execute it against MongoDB with sample data, and verify results are plausible. Tests skip when no API key is configured.

## Motivation

The compiled query system has 35 unit tests + 7 integration tests using a mock LLM provider. These verify the cache, hasher, validator, template, and translator mechanics work correctly. What's missing is end-to-end validation that a real LLM produces MQL that actually works — i.e., parses correctly, executes against MongoDB, and returns sensible results.

## Existing Test Coverage

**Unit tests (35):** Cache L1/L2 mechanics, hasher consistency, template extraction, validator safety checks, translator parsing.

**Integration tests (7, mock LLM):** Basic translation, cache hit verification, different intents, aggregate translation, Atlas L3 cache, disk L2 cache, template extraction.

**Gap:** No test calls a real LLM and validates the MQL executes correctly against real data.

## Design

### Sample Data via Atlas Local

Enable the Atlas Local sample dataset by adding `MONGODB_LOAD_SAMPLE_DATA: "true"` to `docker-compose.test.yml`. This loads ~9 databases including `sample_mflix` which has a `movies` collection (~23k documents) with rich fields:

- Structured: `year` (int), `runtime` (int), `genres` (array), `countries` (array), `rated` (string)
- Text: `title`, `plot`, `fullplot`
- Nested: `imdb.rating`, `imdb.votes`, `tomatoes.viewer.rating`
- Array: `cast`, `directors`, `writers`

This provides realistic, varied data for NL queries without any test-specific seed data insertion.

### docker-compose.test.yml Change

```yaml
services:
  mongodb:
    image: mongodb/mongodb-atlas-local:latest
    hostname: localhost
    ports:
      - "27017:27017"
    environment:
      MONGODB_LOAD_SAMPLE_DATA: "true"
```

Note: First startup is slower (~30-60s) as sample data loads. Subsequent starts are fast (data persists in the container volume). This impacts all `just docker-up` users — existing integration tests will experience a one-time slower first startup but no behavioral change after data is loaded.

### Test File

`tests/integration/compiled_llm_test.rs`

### Skip Condition

Tests check for `ANTHROPIC_API_KEY` environment variable (or `OPENAI_API_KEY`). If neither is set, each test is skipped with a clear message:

```rust
fn get_llm_provider() -> Option<Box<dyn LlmProvider>> {
    if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
        Some(Box::new(ClaudeProvider::new(key)))
    } else if let Ok(key) = std::env::var("OPENAI_API_KEY") {
        Some(Box::new(OpenAiProvider::new(key)))
    } else {
        None
    }
}

macro_rules! skip_without_llm {
    ($provider:ident) => {
        let Some($provider) = get_llm_provider() else {
            eprintln!("Skipping: no ANTHROPIC_API_KEY or OPENAI_API_KEY set");
            return;
        };
    };
}
```

### Test Cases

All tests query `sample_mflix.movies`:

| Test | NL Query | Expected MQL | Result Validation |
|------|----------|-------------|-------------------|
| `test_llm_find_short_films` | "find movies shorter than 60 minutes" | Find with `runtime` filter | All results have `runtime < 60` |
| `test_llm_find_genre_and_year` | "find comedy movies from 2010" | Find with `genres` + `year` filter | Results have "Comedy" in genres AND year == 2010 |
| `test_llm_sort_by_rating` | "find the highest rated movies" | Find with sort or Aggregate with $sort | Results have `imdb.rating` in descending order |
| `test_llm_count_by_genre` | "count movies by genre" | Aggregate with $group | Results have `_id` and a count field |
| `test_llm_average_runtime` | "average runtime of action movies" | Aggregate with $match + $group/$avg | Result has an average value between 60-180 (plausible) |
| `test_llm_cache_reuse` | Same query twice | Second call is cache hit | LLM called only once, both results identical |
| `test_llm_template_cache_reuse` | "find comedy movies from 2010" then "find comedy movies from 2020" | Second call uses cached template with new parameter | LLM called only once, second result has year == 2020 |

### Assertion Strategy

LLM output is non-deterministic. Assertions are intentionally loose:

1. **MQL parses:** `CompiledMql::Find` or `CompiledMql::Aggregate` — no parse error
2. **MQL type is reasonable:** "find ... shorter than" → likely Find; "count by" → likely Aggregate
3. **Execution succeeds:** Running the MQL against MongoDB returns results (no error)
4. **Results are non-empty:** At least 1 document returned
5. **Results are plausible:** Spot-check one field value matches the constraint (e.g., `runtime < 60`)

If an assertion on plausibility fails, the test logs the LLM's response for debugging but doesn't hard-fail on type mismatch (Find vs Aggregate) — only on execution failure or empty results.

### Template Cache Reuse

The compiled query system extracts parameterized templates from NL queries. Numbers like `2010` in "find comedy movies from 2010" become placeholders (`{num_0}`). When a second query like "find comedy movies from 2020" arrives, the template pattern matches the cached entry and the new parameter value (2020) is substituted into the compiled MQL — no LLM call needed.

The `test_llm_template_cache_reuse` test validates this by:
1. Translating "find comedy movies from 2010" (cold — calls LLM)
2. Translating "find comedy movies from 2020" (warm — should reuse template)
3. Asserting the LLM was called only once
4. Asserting the second result filters by year == 2020 (parameter substituted correctly)

### Pre-flight Check

Before running any LLM test, verify sample data is available:

```rust
async fn ensure_sample_data(pool: &ConnectionPool) {
    let count = pool.database("sample_mflix")
        .collection::<Document>("movies")
        .count_documents(doc! {})
        .await
        .expect("Failed to count sample_mflix.movies");
    assert!(count > 1000, "sample_mflix.movies has {} docs — expected >1000. Did you start Docker with MONGODB_LOAD_SAMPLE_DATA=true?", count);
}
```

This fails fast with a clear message if sample data isn't loaded.

### Execution

Each test:
1. Checks for LLM API key (skip if absent)
2. Verifies sample data is loaded (fail-fast if not)
3. Creates a `CompiledQueryTranslator` with a real provider + in-memory cache
4. Provides a `TranslationContext` with schema hints from `sample_mflix.movies`
5. Calls `translator.translate(query, "sample_mflix", "movies", &context)`
6. Matches on `CompiledMql::Find` or `CompiledMql::Aggregate`
7. Executes the filter/pipeline against the real `sample_mflix.movies` collection
8. Validates results

### TranslationContext

The `TranslationContext` should include schema/sample info so the LLM knows what fields are available:

```rust
let context = TranslationContext {
    schema: Some(vec![
        "title: String", "year: Int", "runtime: Int", "genres: [String]",
        "imdb.rating: Double", "imdb.votes: Int", "cast: [String]",
        "plot: String", "countries: [String]", "rated: String",
    ]),
    sample_documents: None, // optional, schema is sufficient
};
```

### Justfile

```
# Run compiled query LLM tests (requires ANTHROPIC_API_KEY or OPENAI_API_KEY)
test-llm:
    cargo test --test integration compiled_llm -- --nocapture
```

## Implementation Scope

| File | Change |
|------|--------|
| `docker-compose.test.yml` | Add `MONGODB_LOAD_SAMPLE_DATA: "true"` |
| `tests/integration/compiled_llm_test.rs` | Create with 7 test functions |
| `tests/integration.rs` | Add `mod compiled_llm_test;` |
| `justfile` | Add `test-llm` recipe |
| `AGENTS.md` | Add `test-llm` to testing table and note sample data requirement |

## Won't Build

- No changes to the compiled query system itself
- No changes to LLM providers
- No new mock infrastructure (existing mock tests are sufficient)
- No recorded/replay fixtures (conditional skip is simpler)

## Success Criteria

- [ ] `docker-compose.test.yml` loads sample data on startup
- [ ] `sample_mflix.movies` is available with data after `just docker-up`
- [ ] Tests skip cleanly when no API key is set (no failures in CI)
- [ ] With `ANTHROPIC_API_KEY` set, all 7 tests pass
- [ ] Each test validates: MQL parses, executes, returns plausible results
- [ ] Cache reuse test confirms LLM called only once for repeated query
- [ ] `just test-llm` runs the LLM tests specifically
