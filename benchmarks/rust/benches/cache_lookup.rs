use criterion::{criterion_group, criterion_main, Criterion};
use mongocore::compiled::hasher::QueryHasher;

fn bench_query_hash(c: &mut Criterion) {
    c.bench_function("query_hash", |b| {
        b.iter(|| {
            QueryHasher::hash(
                "find Italian restaurants in Manhattan",
                "sample_restaurants",
                "restaurants",
                None,
            )
        })
    });
}

fn bench_query_hash_with_schema(c: &mut Criterion) {
    c.bench_function("query_hash_with_schema", |b| {
        b.iter(|| {
            QueryHasher::hash(
                "find Italian restaurants in Manhattan",
                "sample_restaurants",
                "restaurants",
                Some("schema_v1"),
            )
        })
    });
}

criterion_group!(benches, bench_query_hash, bench_query_hash_with_schema);
criterion_main!(benches);
