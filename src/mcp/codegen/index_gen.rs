use serde_json::Value;
use super::Language;

pub struct IndexSuggestion {
    pub index_spec: String,
    pub fields: Vec<String>,
    pub code: String,
    pub explanation: String,
}

pub fn suggest_index(
    language: Language,
    database: &str,
    collection: &str,
    filter: &Value,
) -> IndexSuggestion {
    let fields = extract_index_fields(filter);
    let index_spec = fields.iter()
        .map(|f| format!("\"{}\": 1", f))
        .collect::<Vec<_>>()
        .join(", ");

    let code = generate_index_code(language, database, collection, &fields);
    let explanation = if fields.is_empty() {
        "No filterable fields detected in query.".to_string()
    } else {
        format!(
            "Create a compound index on [{}] to support this query pattern.",
            fields.join(", ")
        )
    };

    IndexSuggestion {
        index_spec: format!("{{{}}}", index_spec),
        fields,
        code,
        explanation,
    }
}

fn extract_index_fields(filter: &Value) -> Vec<String> {
    let mut fields = Vec::new();
    if let Some(obj) = filter.as_object() {
        for (key, _) in obj {
            if !key.starts_with('$') {
                fields.push(key.clone());
            }
        }
    }
    fields.sort();
    fields
}

fn generate_index_code(language: Language, database: &str, collection: &str, fields: &[String]) -> String {
    if fields.is_empty() {
        return "// No index needed for this query pattern.".to_string();
    }

    let keys_json = fields.iter()
        .map(|f| format!("\"{}\": 1", f))
        .collect::<Vec<_>>()
        .join(", ");

    match language {
        Language::Python => format!(
r#"import asyncio
from mongocore import MongoCore

async def main():
    async with MongoCore("localhost:50051") as client:
        await client.create_index(
            database="{}",
            collection="{}",
            keys={{{}}},
        )
        print("Index created successfully")

if __name__ == "__main__":
    asyncio.run(main())
"#, database, collection, keys_json),
        Language::TypeScript => format!(
r#"import {{ MongoCore }} from "mongocore-client";

async function main() {{
  const client = new MongoCore("localhost:50051");
  try {{
    await client.createIndex({{
      database: "{}",
      collection: "{}",
      keys: {{{}}},
    }});
    console.log("Index created successfully");
  }} finally {{
    await client.close();
  }}
}}

main();
"#, database, collection, keys_json),
        Language::Go => format!(
r#"package main

import (
    "context"
    "fmt"
    "log"

    "github.com/mongodb/mongocore/clients/go/mongocore"
)

func main() {{
    client, err := mongocore.NewClient("localhost:50051")
    if err != nil {{
        log.Fatal(err)
    }}
    defer client.Close()

    err = client.CreateIndex(context.Background(), "{}", "{}", `{{{}}}`)
    if err != nil {{
        log.Fatal(err)
    }}
    fmt.Println("Index created successfully")
}}
"#, database, collection, keys_json),
        Language::Java => format!(
r#"import com.mongodb.mongocore.MongoCore;

public class CreateIndex {{
    public static void main(String[] args) throws Exception {{
        try (var client = new MongoCore("localhost:50051")) {{
            client.createIndex("{}", "{}", "{{{}}}");
            System.out.println("Index created successfully");
        }}
    }}
}}
"#, database, collection, keys_json),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_index_fields() {
        let filter = json!({"status": "active", "age": {"$gt": 25}});
        let fields = extract_index_fields(&filter);
        assert_eq!(fields, vec!["age", "status"]);
    }

    #[test]
    fn test_extract_ignores_operators() {
        let filter = json!({"$and": [{"a": 1}]});
        let fields = extract_index_fields(&filter);
        assert!(fields.is_empty());
    }

    #[test]
    fn test_suggest_index_python() {
        let filter = json!({"borough": "Brooklyn", "cuisine": "Italian"});
        let suggestion = suggest_index(Language::Python, "sample_restaurants", "restaurants", &filter);
        assert_eq!(suggestion.fields, vec!["borough", "cuisine"]);
        assert!(suggestion.code.contains("create_index"));
        assert!(suggestion.explanation.contains("compound index"));
    }

    #[test]
    fn test_suggest_index_empty() {
        let filter = json!({});
        let suggestion = suggest_index(Language::Go, "mydb", "coll", &filter);
        assert!(suggestion.fields.is_empty());
        assert!(suggestion.explanation.contains("No filterable"));
    }
}
