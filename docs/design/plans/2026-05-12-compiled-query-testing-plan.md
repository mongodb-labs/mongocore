# Compiled Query LLM Testing — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **For implementers:** Read and follow `AGENTS.md` at the project root.
> Before committing: `cargo test --lib` must pass AND `cargo test --test integration` must compile.
> If modifying client libraries: verify imports work and run `just test-clients`.
> If modifying shared structs (like `Config`): update ALL struct literals in `src/` AND `tests/`.

**Goal:** Add real LLM integration tests that validate compiled queries (NL→MQL) execute correctly against MongoDB sample data.

**Architecture:** Conditional tests that skip without API key. Docker-compose loads sample data when `ANTHROPIC_API_KEY` is set. Tests use `sample_mflix.movies` for NL queries, validate MQL parses and returns plausible results.

**Tech Stack:** Rust (tokio test), mongodb driver, existing `CompiledQueryTranslator`, `ClaudeProvider`/`OpenAiProvider`, `sample_mflix` dataset.

---

## File Structure

| Action | Path | Responsibility |
|--------|------|----------------|
| Modify | `docker-compose.test.yml` | Add conditional sample data loading |
| Create | `tests/integration/compiled_llm_test.rs` | 7 LLM integration tests |
| Modify | `tests/integration.rs` | Add module reference |
| Modify | `justfile` | Add `test-llm` recipe |
| Modify | `AGENTS.md` | Add `test-llm` to testing table |

---

## Task 1: Enable Conditional Sample Data in Docker Compose

**Files:**
- Modify: `docker-compose.test.yml`

- [ ] **Step 1: Add environment variable passthrough**

Change `docker-compose.test.yml` from:

```yaml
services:
  mongodb:
    image: mongodb/mongodb-atlas-local:latest
    hostname: localhost
    ports:
      - "27017:27017"
```

To:

```yaml
services:
  mongodb:
    image: mongodb/mongodb-atlas-local:latest
    hostname: localhost
    ports:
      - "27017:27017"
    environment:
      MONGODB_LOAD_SAMPLE_DATA: "${ANTHROPIC_API_KEY:+true}"
```

- [ ] **Step 2: Verify existing tests still work without API key**

```bash
unset ANTHROPIC_API_KEY
just docker-down
just docker-up
cargo test --test integration compiled_test -- --nocapture
```

Expected: All existing compiled tests pass (they use mock LLM, don't need sample data).

- [ ] **Step 3: Commit**

```bash
git add docker-compose.test.yml
git commit -m "feat: conditionally load sample data in Docker when ANTHROPIC_API_KEY is set"
```

---

## Task 2: Create LLM Integration Test File

**Files:**
- Create: `tests/integration/compiled_llm_test.rs`
- Modify: `tests/integration.rs`

- [ ] **Step 1: Add module to integration test root**

Add to the end of `tests/integration.rs`:

```rust
#[path = "integration/compiled_llm_test.rs"]
mod compiled_llm_test;
```

- [ ] **Step 2: Create the test file with helpers and first test**

Create `tests/integration/compiled_llm_test.rs`:

```rust
use bson::{doc, Document};
use mongocore::compiled::providers::{ClaudeProvider, LlmProvider, OpenAiProvider, TranslationContext};
use mongocore::compiled::translator::CompiledQueryTranslator;
use mongocore::compiled::CompiledMql;
use mongocore::connection::pool::ConnectionPool;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[allow(unused_imports)]
#[path = "../harness/mod.rs"]
mod harness;

/// Returns a real LLM provider if an API key is available, or None to skip.
fn get_llm_provider() -> Option<Box<dyn LlmProvider>> {
    if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
        Some(Box::new(ClaudeProvider::new(key)))
    } else if let Ok(key) = std::env::var("OPENAI_API_KEY") {
        Some(Box::new(OpenAiProvider::new(key)))
    } else {
        None
    }
}

/// Verify sample data is loaded. Panics with a clear message if not.
async fn ensure_sample_data(pool: &ConnectionPool) {
    let count = pool
        .database("sample_mflix")
        .collection::<Document>("movies")
        .count_documents(doc! {})
        .await
        .expect("Failed to count sample_mflix.movies");
    assert!(
        count > 1000,
        "sample_mflix.movies has {} docs — expected >1000. \
         Did you start Docker with ANTHROPIC_API_KEY set? \
         (enables MONGODB_LOAD_SAMPLE_DATA in docker-compose)",
        count
    );
}

/// Build a TranslationContext with the sample_mflix.movies schema.
fn movies_context() -> TranslationContext {
    TranslationContext {
        schema_hint: Some(
            "Fields: title (String), year (Int), runtime (Int), genres ([String]), \
             countries ([String]), cast ([String]), directors ([String]), \
             plot (String), rated (String), imdb.rating (Double), imdb.votes (Int), \
             tomatoes.viewer.rating (Double)"
                .to_string(),
        ),
        sample_documents: vec![],
        available_indexes: vec![],
    }
}

/// Execute a CompiledMql against the movies collection and return results.
async fn execute_mql(pool: &ConnectionPool, mql: &CompiledMql) -> Vec<Document> {
    let coll = pool
        .database("sample_mflix")
        .collection::<Document>("movies");

    match mql {
        CompiledMql::Find { filter, options } => {
            use futures::StreamExt;
            let mut find = coll.find(filter.clone());
            if let Some(opts) = options {
                if let Ok(limit) = opts.get_i64("limit") {
                    find = find.limit(limit);
                }
                if let Some(sort) = opts.get_document("sort").ok() {
                    find = find.sort(sort.clone());
                }
            }
            find.await
                .expect("find failed")
                .collect::<Vec<_>>()
                .await
                .into_iter()
                .filter_map(|r| r.ok())
                .take(20)
                .collect()
        }
        CompiledMql::Aggregate { pipeline } => {
            use futures::StreamExt;
            coll.aggregate(pipeline.clone())
                .await
                .expect("aggregate failed")
                .collect::<Vec<_>>()
                .await
                .into_iter()
                .filter_map(|r| r.ok())
                .take(20)
                .collect()
        }
    }
}

// ==================== Tests ====================

#[tokio::test]
async fn test_llm_find_short_films() {
    let Some(provider) = get_llm_provider() else {
        eprintln!("Skipping test_llm_find_short_films: no API key set");
        return;
    };
    let pool = harness::get_test_pool().await;
    ensure_sample_data(&pool).await;

    let translator = CompiledQueryTranslator::new(None, Some(provider), None);
    let context = movies_context();

    let result = translator
        .translate("find movies shorter than 60 minutes", "sample_mflix", "movies", &context)
        .await
        .expect("translation should succeed");

    eprintln!("MQL: {:?}", result.mql);

    let docs = execute_mql(&pool, &result.mql).await;
    assert!(!docs.is_empty(), "Expected results for short films query");

    // Plausibility: all returned docs should have runtime < 60
    for doc in &docs {
        if let Ok(runtime) = doc.get_i32("runtime") {
            assert!(runtime < 60, "Expected runtime < 60, got {}", runtime);
        }
    }
}

#[tokio::test]
async fn test_llm_find_genre_and_year() {
    let Some(provider) = get_llm_provider() else {
        eprintln!("Skipping test_llm_find_genre_and_year: no API key set");
        return;
    };
    let pool = harness::get_test_pool().await;
    ensure_sample_data(&pool).await;

    let translator = CompiledQueryTranslator::new(None, Some(provider), None);
    let context = movies_context();

    let result = translator
        .translate("find comedy movies from 2010", "sample_mflix", "movies", &context)
        .await
        .expect("translation should succeed");

    eprintln!("MQL: {:?}", result.mql);

    let docs = execute_mql(&pool, &result.mql).await;
    assert!(!docs.is_empty(), "Expected results for comedy 2010 query");

    // Plausibility: results should have Comedy in genres and year == 2010
    for doc in &docs {
        if let Ok(year) = doc.get_i32("year") {
            assert_eq!(year, 2010, "Expected year 2010, got {}", year);
        }
        if let Ok(genres) = doc.get_array("genres") {
            let genre_strs: Vec<&str> = genres
                .iter()
                .filter_map(|g| g.as_str())
                .collect();
            assert!(
                genre_strs.contains(&"Comedy"),
                "Expected Comedy in genres, got {:?}",
                genre_strs
            );
        }
    }
}

#[tokio::test]
async fn test_llm_sort_by_rating() {
    let Some(provider) = get_llm_provider() else {
        eprintln!("Skipping test_llm_sort_by_rating: no API key set");
        return;
    };
    let pool = harness::get_test_pool().await;
    ensure_sample_data(&pool).await;

    let translator = CompiledQueryTranslator::new(None, Some(provider), None);
    let context = movies_context();

    let result = translator
        .translate("find the highest rated movies", "sample_mflix", "movies", &context)
        .await
        .expect("translation should succeed");

    eprintln!("MQL: {:?}", result.mql);

    let docs = execute_mql(&pool, &result.mql).await;
    assert!(!docs.is_empty(), "Expected results for highest rated query");

    // Plausibility: first result should have a high rating
    if let Some(first) = docs.first() {
        if let Ok(imdb) = first.get_document("imdb") {
            if let Ok(rating) = imdb.get_f64("rating") {
                assert!(rating > 7.0, "Expected high rating, got {}", rating);
            }
        }
    }
}

#[tokio::test]
async fn test_llm_count_by_genre() {
    let Some(provider) = get_llm_provider() else {
        eprintln!("Skipping test_llm_count_by_genre: no API key set");
        return;
    };
    let pool = harness::get_test_pool().await;
    ensure_sample_data(&pool).await;

    let translator = CompiledQueryTranslator::new(None, Some(provider), None);
    let context = movies_context();

    let result = translator
        .translate("count movies by genre", "sample_mflix", "movies", &context)
        .await
        .expect("translation should succeed");

    eprintln!("MQL: {:?}", result.mql);

    // This should be an aggregate
    match &result.mql {
        CompiledMql::Aggregate { pipeline } => {
            assert!(!pipeline.is_empty(), "Pipeline should not be empty");
        }
        CompiledMql::Find { .. } => {
            eprintln!("WARN: LLM returned Find instead of Aggregate for count query — acceptable");
        }
    }

    let docs = execute_mql(&pool, &result.mql).await;
    assert!(!docs.is_empty(), "Expected results for count by genre query");
}

#[tokio::test]
async fn test_llm_average_runtime() {
    let Some(provider) = get_llm_provider() else {
        eprintln!("Skipping test_llm_average_runtime: no API key set");
        return;
    };
    let pool = harness::get_test_pool().await;
    ensure_sample_data(&pool).await;

    let translator = CompiledQueryTranslator::new(None, Some(provider), None);
    let context = movies_context();

    let result = translator
        .translate("average runtime of action movies", "sample_mflix", "movies", &context)
        .await
        .expect("translation should succeed");

    eprintln!("MQL: {:?}", result.mql);

    let docs = execute_mql(&pool, &result.mql).await;
    assert!(!docs.is_empty(), "Expected results for average runtime query");

    // Plausibility: average runtime should be between 60-180 minutes
    if let Some(first) = docs.first() {
        // The aggregation result might have various field names for the average
        for key in first.keys() {
            if key.contains("avg") || key.contains("average") || key.contains("runtime") {
                if let Ok(val) = first.get_f64(key) {
                    assert!(
                        val > 60.0 && val < 180.0,
                        "Expected average runtime between 60-180, got {}",
                        val
                    );
                }
            }
        }
    }
}

#[tokio::test]
async fn test_llm_cache_reuse() {
    let Some(provider) = get_llm_provider() else {
        eprintln!("Skipping test_llm_cache_reuse: no API key set");
        return;
    };
    let pool = harness::get_test_pool().await;
    ensure_sample_data(&pool).await;

    let translator = CompiledQueryTranslator::new(None, Some(provider), None);
    let context = movies_context();

    // First call — hits LLM
    let result1 = translator
        .translate("find movies shorter than 60 minutes", "sample_mflix", "movies", &context)
        .await
        .expect("first translation should succeed");

    // Second call — should be cache hit
    let result2 = translator
        .translate("find movies shorter than 60 minutes", "sample_mflix", "movies", &context)
        .await
        .expect("second translation should succeed");

    // Same hash = cache hit
    assert_eq!(result1.hash, result2.hash, "Second call should be a cache hit");
    // Cache should have 1 entry
    assert_eq!(translator.cache_size(), 1);
}

#[tokio::test]
async fn test_llm_template_cache_reuse() {
    let Some(provider) = get_llm_provider() else {
        eprintln!("Skipping test_llm_template_cache_reuse: no API key set");
        return;
    };
    let pool = harness::get_test_pool().await;
    ensure_sample_data(&pool).await;

    let translator = CompiledQueryTranslator::new(None, Some(provider), None);
    let context = movies_context();

    // First call with 2010 — hits LLM
    let result1 = translator
        .translate("find comedy movies from 2010", "sample_mflix", "movies", &context)
        .await
        .expect("first translation should succeed");

    eprintln!("First MQL: {:?}", result1.mql);
    eprintln!("Template: {:?}", result1.template);

    // Second call with 2020 — should reuse template (no LLM call)
    let result2 = translator
        .translate("find comedy movies from 2020", "sample_mflix", "movies", &context)
        .await
        .expect("second translation should succeed");

    eprintln!("Second MQL: {:?}", result2.mql);

    // Execute second query and verify year == 2020
    let docs = execute_mql(&pool, &result2.mql).await;
    assert!(!docs.is_empty(), "Expected results for comedy 2020 query");

    for doc in &docs {
        if let Ok(year) = doc.get_i32("year") {
            assert_eq!(year, 2020, "Expected year 2020 from template reuse, got {}", year);
        }
    }
}
```

- [ ] **Step 3: Verify it compiles**

```bash
cargo test --test integration compiled_llm -- --list 2>&1 | head -10
```

Expected: Lists 7 test functions (they'll skip at runtime without API key).

- [ ] **Step 4: Commit**

```bash
git add tests/integration/compiled_llm_test.rs tests/integration.rs
git commit -m "test(compiled): add real LLM integration tests for NL→MQL with sample data"
```

---

## Task 3: Add Justfile Recipe and Update AGENTS.md

**Files:**
- Modify: `justfile`
- Modify: `AGENTS.md`

- [ ] **Step 1: Add test-llm recipe to justfile**

Add after the existing `test-all` recipe:

```
# Run compiled query LLM tests (requires ANTHROPIC_API_KEY or OPENAI_API_KEY + sample data)
test-llm:
    cargo test --test integration compiled_llm -- --nocapture
```

- [ ] **Step 2: Update AGENTS.md testing table**

In `AGENTS.md`, find the testing table and add a row:

```
| `just test-llm` | Compiled query LLM tests (7) | Docker MongoDB with sample data + ANTHROPIC_API_KEY |
```

Also add a note below the table:

```markdown
**Sample data:** When `ANTHROPIC_API_KEY` is set, `just docker-up` automatically loads MongoDB Atlas sample datasets (sample_mflix, etc.) for LLM integration tests. Without the key, Docker starts fast with no sample data.
```

- [ ] **Step 3: Commit**

```bash
git add justfile AGENTS.md
git commit -m "chore: add test-llm justfile recipe and document in AGENTS.md"
```

---

## Task 4: Verification

- [ ] **Step 1: Verify tests skip without API key**

```bash
unset ANTHROPIC_API_KEY
unset OPENAI_API_KEY
cargo test --test integration compiled_llm -- --nocapture 2>&1 | grep -i "skip"
```

Expected: All 7 tests print "Skipping" messages and pass (no failures).

- [ ] **Step 2: Verify full test suite still passes**

```bash
cargo test --lib
cargo test --test integration
```

Expected: All unit tests (208) and integration tests pass. The new `compiled_llm` tests skip gracefully.

- [ ] **Step 3: (If API key available) Run LLM tests**

```bash
export ANTHROPIC_API_KEY=<your-key>
just docker-down
just docker-up
# Wait ~30-60s for sample data to load on first start
sleep 60
just test-llm
```

Expected: All 7 tests pass with real LLM calls. Output shows MQL generated for each query.

- [ ] **Step 4: Commit any fixes**

If any test required adjustment, fix and commit.

---

## Implementation Order

```
Task 1: docker-compose.test.yml (independent)
Task 2: Test file + integration.rs (independent of Task 1 for compilation, needs Task 1 for execution)
Task 3: Justfile + AGENTS.md (depends on Task 2 existing)
Task 4: Verification (depends on all above)
```

Tasks 1 and 2 can be done in parallel.

---

## Definition of Done

- [ ] `docker-compose.test.yml` has `MONGODB_LOAD_SAMPLE_DATA: "${ANTHROPIC_API_KEY:+true}"`
- [ ] `tests/integration/compiled_llm_test.rs` exists with 7 tests
- [ ] `tests/integration.rs` includes `mod compiled_llm_test`
- [ ] All 7 tests skip cleanly when no API key is set
- [ ] `just test-llm` runs the tests
- [ ] AGENTS.md documents the `test-llm` command and sample data behavior
- [ ] `cargo test --test integration` compiles and passes (existing tests unaffected)
- [ ] With API key + sample data: all 7 tests pass and validate MQL against real MongoDB
