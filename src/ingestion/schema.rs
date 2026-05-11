use polars::prelude::*;
use std::collections::HashMap;

use crate::error::MongoCoreError;
use crate::ingestion::types::{BsonSchema, BsonType, SchemaField};

/// Map a Polars DataType to a BsonType.
pub fn polars_type_to_bson(dtype: &DataType) -> BsonType {
    match dtype {
        DataType::String => BsonType::String,
        DataType::Int8 | DataType::Int16 | DataType::Int32 | DataType::UInt8 | DataType::UInt16 => {
            BsonType::Int32
        }
        DataType::Int64 | DataType::UInt32 | DataType::UInt64 => BsonType::Int64,
        DataType::Float32 | DataType::Float64 => BsonType::Double,
        DataType::Boolean => BsonType::Boolean,
        DataType::Date | DataType::Datetime(_, _) | DataType::Time | DataType::Duration(_) => {
            BsonType::DateTime
        }
        DataType::Null => BsonType::Null,
        DataType::Binary | DataType::BinaryOffset => BsonType::Binary,
        DataType::List(inner) => BsonType::Array(Box::new(polars_type_to_bson(inner))),
        DataType::Struct(fields) => {
            let schema_fields = fields
                .iter()
                .map(|f| SchemaField {
                    name: f.name().to_string(),
                    bson_type: polars_type_to_bson(f.dtype()),
                    nullable: false,
                })
                .collect();
            BsonType::Document(schema_fields)
        }
        _ => BsonType::String,
    }
}

/// Widen two BsonTypes into a compatible type.
pub fn widen_types(a: &BsonType, b: &BsonType) -> BsonType {
    if a == b {
        return a.clone();
    }

    match (a, b) {
        (BsonType::Null, other) | (other, BsonType::Null) => other.clone(),
        (BsonType::Int32, BsonType::Int64) | (BsonType::Int64, BsonType::Int32) => BsonType::Int64,
        (BsonType::Int32, BsonType::Double)
        | (BsonType::Double, BsonType::Int32)
        | (BsonType::Int64, BsonType::Double)
        | (BsonType::Double, BsonType::Int64) => BsonType::Double,
        (BsonType::Array(inner_a), BsonType::Array(inner_b)) => {
            BsonType::Array(Box::new(widen_types(inner_a, inner_b)))
        }
        _ => BsonType::String,
    }
}

/// Infer schema from a collected DataFrame.
pub fn infer_schema(df: &DataFrame) -> Result<BsonSchema, MongoCoreError> {
    let mut fields = Vec::new();

    for col in df.get_columns() {
        let name = col.name().to_string();
        let dtype = col.dtype();
        let bson_type = polars_type_to_bson(dtype);
        let nullable = col.null_count() > 0;

        fields.push(SchemaField {
            name,
            bson_type,
            nullable,
        });
    }

    Ok(BsonSchema { fields })
}

/// Parse a type string into a BsonType.
fn parse_bson_type_str(s: &str) -> Option<BsonType> {
    match s.to_lowercase().as_str() {
        "string" | "str" => Some(BsonType::String),
        "int32" | "int" | "i32" => Some(BsonType::Int32),
        "int64" | "long" | "i64" => Some(BsonType::Int64),
        "double" | "float" | "f64" => Some(BsonType::Double),
        "boolean" | "bool" => Some(BsonType::Boolean),
        "datetime" | "date" | "timestamp" => Some(BsonType::DateTime),
        "binary" | "bytes" => Some(BsonType::Binary),
        "objectid" | "oid" => Some(BsonType::ObjectId),
        _ => None,
    }
}

/// Apply user-provided type overrides to a schema.
pub fn apply_overrides(schema: &mut BsonSchema, overrides: &HashMap<String, String>) {
    for field in schema.fields.iter_mut() {
        if let Some(type_str) = overrides.get(&field.name) {
            if let Some(bson_type) = parse_bson_type_str(type_str) {
                field.bson_type = bson_type;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_polars_string_type() {
        assert_eq!(polars_type_to_bson(&DataType::String), BsonType::String);
    }

    #[test]
    fn test_polars_int32_type() {
        assert_eq!(polars_type_to_bson(&DataType::Int32), BsonType::Int32);
        assert_eq!(polars_type_to_bson(&DataType::Int8), BsonType::Int32);
        assert_eq!(polars_type_to_bson(&DataType::UInt16), BsonType::Int32);
    }

    #[test]
    fn test_polars_int64_type() {
        assert_eq!(polars_type_to_bson(&DataType::Int64), BsonType::Int64);
        assert_eq!(polars_type_to_bson(&DataType::UInt32), BsonType::Int64);
    }

    #[test]
    fn test_polars_float64_type() {
        assert_eq!(polars_type_to_bson(&DataType::Float64), BsonType::Double);
        assert_eq!(polars_type_to_bson(&DataType::Float32), BsonType::Double);
    }

    #[test]
    fn test_polars_boolean_type() {
        assert_eq!(polars_type_to_bson(&DataType::Boolean), BsonType::Boolean);
    }

    #[test]
    fn test_polars_date_type() {
        assert_eq!(polars_type_to_bson(&DataType::Date), BsonType::DateTime);
    }

    #[test]
    fn test_polars_null_type() {
        assert_eq!(polars_type_to_bson(&DataType::Null), BsonType::Null);
    }

    #[test]
    fn test_widen_same_type() {
        assert_eq!(widen_types(&BsonType::String, &BsonType::String), BsonType::String);
        assert_eq!(widen_types(&BsonType::Int32, &BsonType::Int32), BsonType::Int32);
    }

    #[test]
    fn test_widen_int32_int64() {
        assert_eq!(widen_types(&BsonType::Int32, &BsonType::Int64), BsonType::Int64);
        assert_eq!(widen_types(&BsonType::Int64, &BsonType::Int32), BsonType::Int64);
    }

    #[test]
    fn test_widen_int_double() {
        assert_eq!(widen_types(&BsonType::Int32, &BsonType::Double), BsonType::Double);
        assert_eq!(widen_types(&BsonType::Int64, &BsonType::Double), BsonType::Double);
    }

    #[test]
    fn test_widen_incompatible() {
        assert_eq!(
            widen_types(&BsonType::Boolean, &BsonType::Int32),
            BsonType::String
        );
    }

    #[test]
    fn test_widen_null_other() {
        assert_eq!(widen_types(&BsonType::Null, &BsonType::Int64), BsonType::Int64);
        assert_eq!(widen_types(&BsonType::Double, &BsonType::Null), BsonType::Double);
    }

    #[test]
    fn test_infer_schema_multiple_types() {
        let df = df! {
            "name" => &["Alice", "Bob"],
            "age" => &[30i32, 25i32],
            "score" => &[95.5f64, 88.0f64],
            "active" => &[true, false],
        }
        .unwrap();

        let schema = infer_schema(&df).unwrap();
        assert_eq!(schema.fields.len(), 4);
        assert_eq!(schema.fields[0].name, "name");
        assert_eq!(schema.fields[0].bson_type, BsonType::String);
        assert_eq!(schema.fields[1].name, "age");
        assert_eq!(schema.fields[1].bson_type, BsonType::Int32);
        assert_eq!(schema.fields[2].name, "score");
        assert_eq!(schema.fields[2].bson_type, BsonType::Double);
        assert_eq!(schema.fields[3].name, "active");
        assert_eq!(schema.fields[3].bson_type, BsonType::Boolean);
    }

    #[test]
    fn test_infer_schema_nullable_detection() {
        let s = Series::new("value".into(), &[Some(1i64), None, Some(3i64)]);
        let df = DataFrame::new(vec![s.into_column()]).unwrap();

        let schema = infer_schema(&df).unwrap();
        assert_eq!(schema.fields[0].nullable, true);
        assert_eq!(schema.fields[0].bson_type, BsonType::Int64);
    }

    #[test]
    fn test_apply_overrides() {
        let mut schema = BsonSchema {
            fields: vec![
                SchemaField {
                    name: "id".to_string(),
                    bson_type: BsonType::String,
                    nullable: false,
                },
                SchemaField {
                    name: "count".to_string(),
                    bson_type: BsonType::Int32,
                    nullable: false,
                },
            ],
        };

        let mut overrides = HashMap::new();
        overrides.insert("id".to_string(), "objectid".to_string());
        overrides.insert("count".to_string(), "int64".to_string());

        apply_overrides(&mut schema, &overrides);

        assert_eq!(schema.fields[0].bson_type, BsonType::ObjectId);
        assert_eq!(schema.fields[1].bson_type, BsonType::Int64);
    }
}
