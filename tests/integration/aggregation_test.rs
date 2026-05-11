use bson::doc;
use uuid::Uuid;

use mongocore::operations::crud::Operations;

#[path = "../harness/mod.rs"]
mod harness;

fn unique_collection() -> String {
    format!("test_agg_{}", Uuid::new_v4().to_string().replace('-', ""))
}

#[tokio::test]
async fn test_aggregation_match_and_group() {
    let pool = harness::get_test_pool().await;
    let ops = Operations::new(pool);
    let coll = unique_collection();

    let docs = vec![
        doc! { "department": "engineering", "salary": 100000 },
        doc! { "department": "engineering", "salary": 120000 },
        doc! { "department": "marketing", "salary": 90000 },
        doc! { "department": "marketing", "salary": 85000 },
        doc! { "department": "sales", "salary": 75000 },
    ];
    ops.insert_many(harness::TEST_DB, &coll, docs)
        .await
        .unwrap();

    let pipeline = vec![
        doc! { "$match": { "department": { "$in": ["engineering", "marketing"] } } },
        doc! { "$group": {
            "_id": "$department",
            "avg_salary": { "$avg": "$salary" },
            "count": { "$sum": 1 }
        }},
        doc! { "$sort": { "_id": 1 } },
    ];

    let results = ops
        .aggregate(harness::TEST_DB, &coll, pipeline)
        .await
        .unwrap();
    assert_eq!(results.len(), 2);

    let eng = &results[0];
    assert_eq!(eng.get_str("_id").unwrap(), "engineering");
    assert_eq!(eng.get_i32("count").unwrap(), 2);
    let eng_avg = eng
        .get_f64("avg_salary")
        .unwrap_or_else(|_| eng.get_i32("avg_salary").unwrap() as f64);
    assert!((eng_avg - 110000.0).abs() < 1.0);

    let mkt = &results[1];
    assert_eq!(mkt.get_str("_id").unwrap(), "marketing");
    assert_eq!(mkt.get_i32("count").unwrap(), 2);
    let mkt_avg = mkt
        .get_f64("avg_salary")
        .unwrap_or_else(|_| mkt.get_i32("avg_salary").unwrap() as f64);
    assert!((mkt_avg - 87500.0).abs() < 1.0);
}
