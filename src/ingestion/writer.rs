use bson::{Bson, Document};
use polars::prelude::*;

use crate::error::MongoCoreError;
use crate::ingestion::types::{BsonSchema, BsonType};

/// Convert a DataFrame into a Vec of BSON Documents according to the schema.
pub fn dataframe_to_documents(
    df: &DataFrame,
    schema: &BsonSchema,
) -> Result<Vec<Document>, MongoCoreError> {
    let num_rows = df.height();
    let mut documents = Vec::with_capacity(num_rows);

    for row in 0..num_rows {
        let mut doc = Document::new();
        for field in &schema.fields {
            let series = df.column(&field.name).map_err(|e| {
                MongoCoreError::IngestionError(format!(
                    "Column '{}' not found: {}",
                    field.name, e
                ))
            })?;
            let value = series_value_to_bson(series, row, &field.bson_type)?;
            doc.insert(field.name.clone(), value);
        }
        documents.push(doc);
    }

    Ok(documents)
}

/// Convert a single value from a Series at a given row to Bson.
fn series_value_to_bson(
    series: &Column,
    row: usize,
    bson_type: &BsonType,
) -> Result<Bson, MongoCoreError> {
    let series = series.as_materialized_series();

    // Check for null
    let null_mask = series.is_null();
    if null_mask.get(row).unwrap_or(false) {
        return Ok(Bson::Null);
    }

    match bson_type {
        BsonType::String => {
            let ca = series.str().map_err(polars_to_ingestion_error)?;
            match ca.get(row) {
                Some(v) => Ok(Bson::String(v.to_string())),
                None => Ok(Bson::Null),
            }
        }
        BsonType::Int32 => {
            let ca = series.i32().map_err(polars_to_ingestion_error)?;
            match ca.get(row) {
                Some(v) => Ok(Bson::Int32(v)),
                None => Ok(Bson::Null),
            }
        }
        BsonType::Int64 => {
            let ca = series.i64().map_err(polars_to_ingestion_error)?;
            match ca.get(row) {
                Some(v) => Ok(Bson::Int64(v)),
                None => Ok(Bson::Null),
            }
        }
        BsonType::Double => {
            let ca = series.f64().map_err(polars_to_ingestion_error)?;
            match ca.get(row) {
                Some(v) => Ok(Bson::Double(v)),
                None => Ok(Bson::Null),
            }
        }
        BsonType::Boolean => {
            let ca = series.bool().map_err(polars_to_ingestion_error)?;
            match ca.get(row) {
                Some(v) => Ok(Bson::Boolean(v)),
                None => Ok(Bson::Null),
            }
        }
        BsonType::DateTime => {
            let millis = match series.dtype() {
                DataType::Date => {
                    let ca = series.i32().map_err(polars_to_ingestion_error)?;
                    match ca.get(row) {
                        Some(days) => days as i64 * 86_400_000,
                        None => return Ok(Bson::Null),
                    }
                }
                DataType::Datetime(tu, _) => {
                    let ca = series.i64().map_err(polars_to_ingestion_error)?;
                    match ca.get(row) {
                        Some(v) => match tu {
                            TimeUnit::Nanoseconds => v / 1_000_000,
                            TimeUnit::Microseconds => v / 1_000,
                            TimeUnit::Milliseconds => v,
                        },
                        None => return Ok(Bson::Null),
                    }
                }
                _ => {
                    // Try to interpret as i64 millis
                    let ca = series.i64().map_err(polars_to_ingestion_error)?;
                    match ca.get(row) {
                        Some(v) => v,
                        None => return Ok(Bson::Null),
                    }
                }
            };
            Ok(Bson::DateTime(bson::DateTime::from_millis(millis)))
        }
        BsonType::Binary => {
            let ca = series.binary().map_err(polars_to_ingestion_error)?;
            match ca.get(row) {
                Some(bytes) => Ok(Bson::Binary(bson::Binary {
                    subtype: bson::spec::BinarySubtype::Generic,
                    bytes: bytes.to_vec(),
                })),
                None => Ok(Bson::Null),
            }
        }
        BsonType::ObjectId => {
            let ca = series.str().map_err(polars_to_ingestion_error)?;
            match ca.get(row) {
                Some(v) => match bson::oid::ObjectId::parse_str(v) {
                    Ok(oid) => Ok(Bson::ObjectId(oid)),
                    Err(_) => Ok(Bson::String(v.to_string())),
                },
                None => Ok(Bson::Null),
            }
        }
        BsonType::Array(inner_type) => {
            let ca = series.list().map_err(polars_to_ingestion_error)?;
            match ca.get_as_series(row) {
                Some(inner_series) => {
                    let mut arr = Vec::new();
                    for i in 0..inner_series.len() {
                        let col = Column::from(inner_series.clone());
                        let val = series_value_to_bson(&col, i, inner_type)?;
                        arr.push(val);
                    }
                    Ok(Bson::Array(arr))
                }
                None => Ok(Bson::Null),
            }
        }
        BsonType::Document(fields) => {
            let ca = series.struct_().map_err(polars_to_ingestion_error)?;
            let mut doc = Document::new();
            for field in fields {
                let field_series = ca.field_by_name(&field.name).map_err(|e| {
                    MongoCoreError::IngestionError(format!(
                        "Struct field '{}' not found: {}",
                        field.name, e
                    ))
                })?;
                let col = Column::from(field_series);
                let val = series_value_to_bson(&col, row, &field.bson_type)?;
                doc.insert(field.name.clone(), val);
            }
            Ok(Bson::Document(doc))
        }
        BsonType::Null => Ok(Bson::Null),
    }
}

fn polars_to_ingestion_error(e: PolarsError) -> MongoCoreError {
    MongoCoreError::IngestionError(format!("Polars conversion error: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingestion::types::SchemaField;

    #[test]
    fn test_simple_dataframe_to_documents() {
        let df = df! {
            "name" => &["Alice", "Bob", "Charlie"],
            "age" => &[30i32, 25i32, 35i32],
            "score" => &[95.5f64, 82.3f64, 91.0f64],
        }
        .unwrap();

        let schema = BsonSchema {
            fields: vec![
                SchemaField {
                    name: "name".to_string(),
                    bson_type: BsonType::String,
                    nullable: false,
                },
                SchemaField {
                    name: "age".to_string(),
                    bson_type: BsonType::Int32,
                    nullable: false,
                },
                SchemaField {
                    name: "score".to_string(),
                    bson_type: BsonType::Double,
                    nullable: false,
                },
            ],
        };

        let docs = dataframe_to_documents(&df, &schema).unwrap();
        assert_eq!(docs.len(), 3);

        assert_eq!(docs[0].get_str("name").unwrap(), "Alice");
        assert_eq!(docs[0].get_i32("age").unwrap(), 30);
        assert_eq!(docs[0].get_f64("score").unwrap(), 95.5);

        assert_eq!(docs[1].get_str("name").unwrap(), "Bob");
        assert_eq!(docs[1].get_i32("age").unwrap(), 25);
    }

    #[test]
    fn test_nullable_values() {
        let df = df! {
            "name" => &[Some("Alice"), None, Some("Charlie")],
            "age" => &[Some(30i32), Some(25i32), None],
        }
        .unwrap();

        let schema = BsonSchema {
            fields: vec![
                SchemaField {
                    name: "name".to_string(),
                    bson_type: BsonType::String,
                    nullable: true,
                },
                SchemaField {
                    name: "age".to_string(),
                    bson_type: BsonType::Int32,
                    nullable: true,
                },
            ],
        };

        let docs = dataframe_to_documents(&df, &schema).unwrap();
        assert_eq!(docs.len(), 3);

        assert_eq!(docs[0].get_str("name").unwrap(), "Alice");
        assert_eq!(docs[1].get("name").unwrap(), &Bson::Null);
        assert_eq!(docs[2].get_i32("age"), Err(bson::document::ValueAccessError::UnexpectedType));
    }

    #[test]
    fn test_boolean_conversion() {
        let df = df! {
            "active" => &[true, false, true],
        }
        .unwrap();

        let schema = BsonSchema {
            fields: vec![SchemaField {
                name: "active".to_string(),
                bson_type: BsonType::Boolean,
                nullable: false,
            }],
        };

        let docs = dataframe_to_documents(&df, &schema).unwrap();
        assert_eq!(docs[0].get_bool("active").unwrap(), true);
        assert_eq!(docs[1].get_bool("active").unwrap(), false);
        assert_eq!(docs[2].get_bool("active").unwrap(), true);
    }

    #[test]
    fn test_int32_conversion() {
        let df = df! {
            "count" => &[1i32, 2i32, 3i32],
        }
        .unwrap();

        let schema = BsonSchema {
            fields: vec![SchemaField {
                name: "count".to_string(),
                bson_type: BsonType::Int32,
                nullable: false,
            }],
        };

        let docs = dataframe_to_documents(&df, &schema).unwrap();
        assert_eq!(docs[0].get_i32("count").unwrap(), 1);
        assert_eq!(docs[1].get_i32("count").unwrap(), 2);
        assert_eq!(docs[2].get_i32("count").unwrap(), 3);
    }
}
