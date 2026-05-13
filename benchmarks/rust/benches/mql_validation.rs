use criterion::{criterion_group, criterion_main, Criterion};
use bson::doc;
use mongocore::compiled::validator::MqlValidator;

fn bench_validate_simple_filter(c: &mut Criterion) {
    let filter = doc! { "cuisine": "Italian", "borough": "Manhattan" };

    c.bench_function("validate_simple_filter", |b| {
        b.iter(|| MqlValidator::validate_filter(&filter))
    });
}

fn bench_validate_nested_filter(c: &mut Criterion) {
    let filter = doc! {
        "$and": [
            { "cuisine": "Italian" },
            { "$or": [
                { "borough": "Manhattan" },
                { "borough": "Brooklyn" }
            ]},
            { "grades.score": { "$gt": 80 } }
        ]
    };

    c.bench_function("validate_nested_filter", |b| {
        b.iter(|| MqlValidator::validate_filter(&filter))
    });
}

fn bench_validate_deeply_nested_filter(c: &mut Criterion) {
    let filter = doc! {
        "$and": [
            {
                "$or": [
                    { "status": "active" },
                    {
                        "$and": [
                            { "status": "pending" },
                            { "priority": { "$gt": 5 } }
                        ]
                    }
                ]
            },
            {
                "$or": [
                    { "category": "A" },
                    { "category": "B" }
                ]
            },
            { "created_at": { "$gte": "2024-01-01" } }
        ]
    };

    c.bench_function("validate_deeply_nested_filter", |b| {
        b.iter(|| MqlValidator::validate_filter(&filter))
    });
}

fn bench_validate_simple_pipeline(c: &mut Criterion) {
    let pipeline = vec![
        doc! { "$match": { "cuisine": "Italian" } },
        doc! { "$group": { "_id": "$borough", "count": { "$sum": 1 } } },
        doc! { "$sort": { "count": -1 } },
        doc! { "$limit": 10 },
    ];

    c.bench_function("validate_simple_pipeline", |b| {
        b.iter(|| MqlValidator::validate_pipeline(&pipeline))
    });
}

fn bench_validate_complex_pipeline(c: &mut Criterion) {
    let pipeline = vec![
        doc! { "$match": { "status": "active", "created_at": { "$gte": "2024-01-01" } } },
        doc! { "$lookup": {
            "from": "users",
            "localField": "user_id",
            "foreignField": "_id",
            "as": "user"
        }},
        doc! { "$unwind": "$user" },
        doc! { "$addFields": { "full_name": { "$concat": ["$user.first_name", " ", "$user.last_name"] } } },
        doc! { "$group": {
            "_id": "$category",
            "count": { "$sum": 1 },
            "avg_score": { "$avg": "$score" }
        }},
        doc! { "$sort": { "count": -1 } },
        doc! { "$limit": 100 },
    ];

    c.bench_function("validate_complex_pipeline", |b| {
        b.iter(|| MqlValidator::validate_pipeline(&pipeline))
    });
}

fn bench_validate_vectorsearch_pipeline(c: &mut Criterion) {
    let pipeline = vec![
        doc! { "$vectorSearch": {
            "index": "vector_index",
            "path": "embedding",
            "queryVector": [0.1, 0.2, 0.3],
            "numCandidates": 100,
            "limit": 10
        }},
        doc! { "$project": { "score": { "$meta": "vectorSearchScore" }, "title": 1, "description": 1 } },
        doc! { "$limit": 5 },
    ];

    c.bench_function("validate_vectorsearch_pipeline", |b| {
        b.iter(|| MqlValidator::validate_pipeline(&pipeline))
    });
}

criterion_group!(
    benches,
    bench_validate_simple_filter,
    bench_validate_nested_filter,
    bench_validate_deeply_nested_filter,
    bench_validate_simple_pipeline,
    bench_validate_complex_pipeline,
    bench_validate_vectorsearch_pipeline
);
criterion_main!(benches);
