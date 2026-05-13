use serde_json::Value;
use tera::Context;

use super::templates::render_query;
use super::Language;

pub fn generate_query_code(
    language: Language,
    database: &str,
    collection: &str,
    method: &str,
    mql: &Value,
    host: &str,
) -> Result<String, String> {
    let mut ctx = Context::new();
    ctx.insert("host", host);
    ctx.insert("database", database);
    ctx.insert("collection", collection);

    let operation = match method {
        "filter" | "find" | "geo" => {
            let filter = mql.get("filter")
                .map(|f| serde_json::to_string_pretty(f).unwrap_or_default())
                .unwrap_or_else(|| "{}".to_string());
            ctx.insert("filter", &filter);
            "find"
        }
        "aggregate" => {
            let pipeline = mql.get("pipeline")
                .map(|p| serde_json::to_string_pretty(p).unwrap_or_default())
                .unwrap_or_else(|| "[]".to_string());
            ctx.insert("pipeline", &pipeline);
            "aggregate"
        }
        _ => {
            let filter = serde_json::to_string_pretty(mql).unwrap_or_else(|_| "{}".to_string());
            ctx.insert("filter", &filter);
            "find"
        }
    };

    render_query(language, operation, &ctx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_generate_find_code_python() {
        let mql = json!({"filter": {"status": "active"}});
        let code = generate_query_code(
            Language::Python, "mydb", "users", "filter", &mql, "localhost:50051"
        ).unwrap();
        assert!(code.contains("MongoCore"));
        assert!(code.contains("mydb"));
        assert!(code.contains("users"));
    }

    #[test]
    fn test_generate_aggregate_code_typescript() {
        let mql = json!({"pipeline": [{"$match": {"type": "click"}}]});
        let code = generate_query_code(
            Language::TypeScript, "analytics", "events", "aggregate", &mql, "localhost:50051"
        ).unwrap();
        assert!(code.contains("aggregate"));
    }

    #[test]
    fn test_generate_find_go() {
        let mql = json!({"filter": {"price": {"$lt": 50}}});
        let code = generate_query_code(
            Language::Go, "shop", "products", "find", &mql, "localhost:50051"
        ).unwrap();
        assert!(code.contains("Find"));
    }
}
