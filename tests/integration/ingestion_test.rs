use mongocore::ingestion::reader;
use mongocore::ingestion::schema;
use mongocore::ingestion::transform;
use mongocore::ingestion::writer;
use mongocore::ingestion::{BsonType, CsvOptions, FileFormat};
use std::collections::HashMap;
use std::path::Path;

#[test]
fn test_csv_schema_inference() {
    let path = Path::new("tests/fixtures/sample.csv");
    let lf = reader::read_lazy(path, FileFormat::Csv, &CsvOptions::default()).unwrap();
    let df = lf.collect().unwrap();
    let inferred = schema::infer_schema(&df).unwrap();

    assert_eq!(inferred.fields.len(), 5);
    assert_eq!(inferred.fields[0].name, "name");
    assert_eq!(inferred.fields[0].bson_type, BsonType::String);
    // Polars infers CSV integers as Int64
    assert_eq!(inferred.fields[1].name, "age");
    assert_eq!(inferred.fields[1].bson_type, BsonType::Int64);
    assert_eq!(inferred.fields[2].name, "email");
    assert_eq!(inferred.fields[2].bson_type, BsonType::String);
    assert_eq!(inferred.fields[3].name, "score");
    assert_eq!(inferred.fields[3].bson_type, BsonType::Double);
}

#[test]
fn test_csv_to_bson_documents() {
    let path = Path::new("tests/fixtures/sample.csv");
    let lf = reader::read_lazy(path, FileFormat::Csv, &CsvOptions::default()).unwrap();
    let df = lf.collect().unwrap();
    let inferred = schema::infer_schema(&df).unwrap();
    let docs = writer::dataframe_to_documents(&df, &inferred).unwrap();

    assert_eq!(docs.len(), 5);
    assert_eq!(docs[0].get_str("name"), Ok("Alice"));
    assert_eq!(docs[0].get_i64("age"), Ok(30));
    assert_eq!(docs[0].get_f64("score"), Ok(95.5));
    assert_eq!(docs[1].get_str("name"), Ok("Bob"));
    assert_eq!(docs[4].get_str("name"), Ok("Eve"));
}

#[test]
fn test_ndjson_end_to_end() {
    let path = Path::new("tests/fixtures/sample.ndjson");
    let lf = reader::read_lazy(path, FileFormat::NdJson, &CsvOptions::default()).unwrap();
    let df = lf.collect().unwrap();
    let inferred = schema::infer_schema(&df).unwrap();
    let docs = writer::dataframe_to_documents(&df, &inferred).unwrap();

    assert_eq!(docs.len(), 3);
    assert_eq!(docs[0].get_str("name"), Ok("Alice"));
    assert_eq!(docs[2].get_str("name"), Ok("Charlie"));
}

#[test]
fn test_transform_filter_and_drop() {
    let path = Path::new("tests/fixtures/sample.csv");
    let lf = reader::read_lazy(path, FileFormat::Csv, &CsvOptions::default()).unwrap();
    let transformed = transform::apply_expressions(
        lf,
        &[
            "filter(age > 28)".to_string(),
            "drop(active)".to_string(),
        ],
    )
    .unwrap();
    let df = transformed.collect().unwrap();

    // age > 28 filters to Alice(30), Charlie(35), Eve(32) = 3 rows
    assert_eq!(df.height(), 3);
    assert!(df.column("active").is_err());
    assert!(df.column("name").is_ok());
}

#[test]
fn test_schema_overrides() {
    let path = Path::new("tests/fixtures/sample.csv");
    let lf = reader::read_lazy(path, FileFormat::Csv, &CsvOptions::default()).unwrap();
    let df = lf.collect().unwrap();
    let mut inferred = schema::infer_schema(&df).unwrap();

    let mut overrides = HashMap::new();
    overrides.insert("age".to_string(), "Double".to_string());
    schema::apply_overrides(&mut inferred, &overrides);

    let age_field = inferred.fields.iter().find(|f| f.name == "age").unwrap();
    assert_eq!(age_field.bson_type, BsonType::Double);
}

#[test]
fn test_format_detection() {
    assert_eq!(
        reader::detect_format(Path::new("data.csv")).unwrap(),
        FileFormat::Csv
    );
    assert_eq!(
        reader::detect_format(Path::new("data.json")).unwrap(),
        FileFormat::Json
    );
    assert_eq!(
        reader::detect_format(Path::new("data.ndjson")).unwrap(),
        FileFormat::NdJson
    );
    assert_eq!(
        reader::detect_format(Path::new("data.parquet")).unwrap(),
        FileFormat::Parquet
    );
    assert!(reader::detect_format(Path::new("data.xyz")).is_err());
}

#[test]
fn test_count_rows() {
    let path = Path::new("tests/fixtures/sample.csv");
    let count = reader::count_rows(path, FileFormat::Csv, &CsvOptions::default()).unwrap();
    assert_eq!(count, 5);
}

#[test]
fn test_row_count_ndjson() {
    let path = Path::new("tests/fixtures/sample.ndjson");
    let count = reader::count_rows(path, FileFormat::NdJson, &CsvOptions::default()).unwrap();
    assert_eq!(count, 3);
}
