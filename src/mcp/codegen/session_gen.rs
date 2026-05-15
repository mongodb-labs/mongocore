use serde_json::Value;
use std::collections::HashMap;

use super::Language;
use super::crud_gen::{capitalize, generate_crud_code};
use super::ingest_gen::generate_ingest_code;
use super::search_gen::generate_search_code;
use crate::mcp::session::OperationRecord;

const CRUD_TOOLS: &[&str] = &[
    "find", "find_one", "insert", "insert_one", "insert_many",
    "update", "update_one", "update_many", "delete", "delete_one", "delete_many",
];

const INGEST_TOOLS: &[&str] = &["ingest", "watch_directory"];

const SEARCH_TOOLS: &[&str] = &["embed_and_store", "semantic_search"];

/// Generate code for a single operation, routing to the appropriate sub-generator.
pub fn generate_single_operation_code(
    language: Language,
    tool_name: &str,
    params: &Value,
) -> Result<String, String> {
    if CRUD_TOOLS.contains(&tool_name) {
        generate_crud_code(language, tool_name, params)
    } else if INGEST_TOOLS.contains(&tool_name) {
        generate_ingest_code(language, tool_name, params)
    } else if SEARCH_TOOLS.contains(&tool_name) {
        generate_search_code(language, tool_name, params)
    } else {
        generate_passthrough(language, tool_name, params)
    }
}

/// Generate a complete session script from multiple operations.
pub fn generate_session_script(
    language: Language,
    operations: &[OperationRecord],
) -> Result<String, String> {
    if operations.is_empty() {
        return Err("No operations to generate code for".to_string());
    }

    let mut function_bodies = Vec::new();
    let mut function_names = Vec::new();
    let mut name_counts: HashMap<String, usize> = HashMap::new();

    for op in operations {
        let base_name = make_function_name(&op.tool_name, &op.params);
        let count = name_counts.entry(base_name.clone()).or_insert(0);
        *count += 1;
        let func_name = if *count > 1 {
            format!("{}_{}", base_name, count)
        } else {
            base_name
        };

        let db = op.params.get("database").and_then(|v| v.as_str()).unwrap_or("mydb");
        let coll = op.params.get("collection").and_then(|v| v.as_str()).unwrap_or("docs");
        let docstring = format!("{} on {} in {}", op.tool_name, coll, db);

        if op.success {
            let code = generate_single_operation_code(language, &op.tool_name, &op.params)
                .unwrap_or_else(|e| format!("# Error generating code: {}\npass\n", e));
            function_bodies.push((func_name.clone(), docstring, code, true, None));
        } else {
            let error_msg = op.error_message.clone().unwrap_or_else(|| "Unknown error".to_string());
            let code = generate_single_operation_code(language, &op.tool_name, &op.params)
                .unwrap_or_else(|_| "pass\n".to_string());
            function_bodies.push((func_name.clone(), docstring, code, false, Some(error_msg)));
        }

        function_names.push((func_name, op.success));
    }

    let script = assemble_script(language, &function_bodies, &function_names);
    Ok(script)
}

fn make_function_name(tool_name: &str, params: &Value) -> String {
    let coll = params
        .get("collection")
        .and_then(|v| v.as_str())
        .unwrap_or("docs");
    format!("{}_{}", tool_name, coll)
}

fn assemble_script(
    language: Language,
    functions: &[(String, String, String, bool, Option<String>)],
    call_names: &[(String, bool)],
) -> String {
    let mut output = String::new();

    // Header/imports
    match language {
        Language::Python => {
            output.push_str("import asyncio\nfrom mongocore import MongoCore\n\n\n");
        }
        Language::TypeScript => {
            output.push_str("import { MongoCore } from '@mongocore/client';\n\n");
        }
        Language::Go => {
            output.push_str("package main\n\nimport (\n\t\"context\"\n\t\"fmt\"\n\n\tpb \"github.com/mongodb/mongocore/proto\"\n\t\"github.com/mongodb/mongocore/client\"\n)\n\n");
        }
        Language::Java => {
            output.push_str("import com.mongodb.mongocore.*;\nimport com.mongodb.mongocore.proto.*;\nimport java.util.*;\n\npublic class MongoSession {\n\n");
        }
    }

    // Function definitions
    for (_name, docstring, code, success, error_msg) in functions {
        if !*success {
            let comment_char = match language {
                Language::Python => "#",
                _ => "//",
            };
            output.push_str(&format!(
                "{} FAILED: {} (error: {})\n",
                comment_char,
                docstring,
                error_msg.as_deref().unwrap_or("unknown")
            ));
            // Comment out the code
            for line in code.lines() {
                output.push_str(&format!("{} {}\n", comment_char, line));
            }
            output.push('\n');
        } else {
            match language {
                Language::Python => {
                    // Wrap the generated code in a named function with docstring
                    output.push_str(&format!(
                        "# {}\n{}\n\n",
                        docstring, code
                    ));
                }
                Language::TypeScript => {
                    output.push_str(&format!(
                        "// {}\n{}\n\n",
                        docstring, code
                    ));
                }
                Language::Go => {
                    output.push_str(&format!(
                        "// {}\n{}\n\n",
                        docstring, code
                    ));
                }
                Language::Java => {
                    output.push_str(&format!(
                        "    // {}\n    {}\n\n",
                        docstring,
                        code.replace('\n', "\n    ")
                    ));
                }
            }
        }
    }

    // Main function that calls them in order
    match language {
        Language::Python => {
            output.push_str("async def main():\n");
            output.push_str("    client = MongoCore()\n");
            output.push_str("    await client.connect()\n\n");
            for (name, success) in call_names {
                if *success {
                    output.push_str(&format!("    await {}(client)\n", name));
                } else {
                    output.push_str(&format!("    # await {}(client)  # skipped: failed\n", name));
                }
            }
            output.push_str("\n\nasyncio.run(main())\n");
        }
        Language::TypeScript => {
            output.push_str("async function main() {\n");
            output.push_str("  const client = new MongoCore();\n");
            output.push_str("  await client.connect();\n\n");
            for (name, success) in call_names {
                let ts_name = to_ts_function_name(name);
                if *success {
                    output.push_str(&format!("  await {}(client);\n", ts_name));
                } else {
                    output.push_str(&format!("  // await {}(client);  // skipped: failed\n", ts_name));
                }
            }
            output.push_str("}\n\nmain();\n");
        }
        Language::Go => {
            output.push_str("func main() {\n");
            output.push_str("\tclient, err := mongocore.NewClient(\"localhost:50051\")\n");
            output.push_str("\tif err != nil {\n\t\tpanic(err)\n\t}\n\tdefer client.Close()\n\n");
            for (name, success) in call_names {
                let go_name = to_go_function_name(name);
                if *success {
                    output.push_str(&format!("\t_, _ = {}(client, \"\", \"\", nil)\n", go_name));
                } else {
                    output.push_str(&format!("\t// _, _ = {}(client, \"\", \"\", nil)  // skipped: failed\n", go_name));
                }
            }
            output.push_str("}\n");
        }
        Language::Java => {
            output.push_str("    public static void main(String[] args) {\n");
            output.push_str("        MongoClient client = MongoClient.create(\"localhost:50051\");\n\n");
            for (name, success) in call_names {
                let java_name = to_java_method_name(name);
                if *success {
                    output.push_str(&format!("        new MongoSession().{}(client, \"\", \"\", null);\n", java_name));
                } else {
                    output.push_str(&format!("        // new MongoSession().{}(client, \"\", \"\", null);  // skipped: failed\n", java_name));
                }
            }
            output.push_str("    }\n}\n");
        }
    }

    output
}

fn to_ts_function_name(name: &str) -> String {
    // find_users -> findUsers
    let parts: Vec<&str> = name.split('_').collect();
    if parts.len() <= 1 {
        return name.to_string();
    }
    let mut result = parts[0].to_string();
    for part in &parts[1..] {
        result.push_str(&capitalize(part));
    }
    result
}

fn to_go_function_name(name: &str) -> String {
    // find_users -> findUsers (unexported)
    let parts: Vec<&str> = name.split('_').collect();
    if parts.len() <= 1 {
        return name.to_string();
    }
    let mut result = parts[0].to_string();
    for part in &parts[1..] {
        result.push_str(&capitalize(part));
    }
    result
}

fn to_java_method_name(name: &str) -> String {
    // find_users -> findUsers
    let parts: Vec<&str> = name.split('_').collect();
    if parts.len() <= 1 {
        return name.to_string();
    }
    let mut result = parts[0].to_string();
    for part in &parts[1..] {
        result.push_str(&capitalize(part));
    }
    result
}

fn generate_passthrough(language: Language, tool_name: &str, params: &Value) -> Result<String, String> {
    let params_str = serde_json::to_string_pretty(params).unwrap_or_else(|_| "{}".to_string());
    let db = params.get("database").and_then(|v| v.as_str()).unwrap_or("mydb");
    let coll = params.get("collection").and_then(|v| v.as_str()).unwrap_or("docs");

    match language {
        Language::Python => Ok(format!(
            r#"async def {tool_name}_{coll}(client, db_name: str = "{db}", collection: str = "{coll}"):
    """{tool_name} on {coll} in {db}."""
    result = await client.run_tool(
        "{tool_name}",
        {params_str},
    )
    return result
"#
        )),
        Language::TypeScript => Ok(format!(
            r#"async function {func_name}(client: MongoCore, dbName = "{db}", collection = "{coll}") {{
  // {tool_name} on {coll} in {db}
  const result = await client.runTool(
    "{tool_name}",
    {params_str},
  );
  return result;
}}
"#,
            func_name = format!("{}_{}", tool_name, coll)
        )),
        Language::Go => Ok(format!(
            r#"func {func_name}(client *mongocore.Client, dbName string, collection string) (interface{{}}, error) {{
	// {tool_name} on {coll} in {db}
	return client.RunTool(context.Background(), "{tool_name}", []byte(`{params_str}`))
}}
"#,
            func_name = format!("{}_{}", tool_name, coll)
        )),
        Language::Java => Ok(format!(
            r#"public Object {method_name}(MongoClient client, String dbName, String collection) {{
    // {tool_name} on {coll} in {db}
    return client.runTool("{tool_name}", "{params_escaped}");
}}
"#,
            method_name = format!("{}_{}", tool_name, coll),
            params_escaped = params_str.replace('"', "\\\"").replace('\n', " ")
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use serde_json::json;

    fn make_op(tool_name: &str, params: Value, success: bool, error: Option<&str>) -> OperationRecord {
        OperationRecord {
            index: 0,
            tool_name: tool_name.to_string(),
            params,
            context: json!({}),
            success,
            error_message: error.map(|s| s.to_string()),
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn test_generate_session_script_python() {
        let ops = vec![
            make_op("insert_many", json!({"database": "mydb", "collection": "users", "documents": [{"name": "Alice"}]}), true, None),
            make_op("find", json!({"database": "mydb", "collection": "users", "filter": {"name": "Alice"}}), true, None),
        ];
        let code = generate_session_script(Language::Python, &ops).unwrap();
        assert!(code.contains("import asyncio"));
        assert!(code.contains("insert_many_users"));
        assert!(code.contains("find_users"));
        assert!(code.contains("async def main()"));
        assert!(code.contains("asyncio.run(main())"));
    }

    #[test]
    fn test_failed_operations_commented_out() {
        let ops = vec![
            make_op("find", json!({"database": "db", "collection": "items", "filter": {}}), true, None),
            make_op("delete", json!({"database": "db", "collection": "items", "filter": {"x": 1}}), false, Some("permission denied")),
        ];
        let code = generate_session_script(Language::Python, &ops).unwrap();
        assert!(code.contains("# FAILED"));
        assert!(code.contains("permission denied"));
        assert!(code.contains("# skipped: failed"));
    }

    #[test]
    fn test_empty_session_error() {
        let result = generate_session_script(Language::Python, &[]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No operations"));
    }

    #[test]
    fn test_duplicate_function_names() {
        let ops = vec![
            make_op("find", json!({"database": "db", "collection": "users", "filter": {}}), true, None),
            make_op("find", json!({"database": "db", "collection": "users", "filter": {"active": true}}), true, None),
        ];
        let code = generate_session_script(Language::Python, &ops).unwrap();
        assert!(code.contains("find_users"));
        assert!(code.contains("find_users_2"));
    }

    #[test]
    fn test_passthrough_operation() {
        let params = json!({"database": "admin", "collection": "system"});
        let code = generate_single_operation_code(Language::TypeScript, "list_collections", &params).unwrap();
        assert!(code.contains("list_collections"));
        assert!(code.contains("runTool"));
    }

    #[test]
    fn test_session_script_typescript() {
        let ops = vec![
            make_op("insert", json!({"database": "app", "collection": "events", "document": {"type": "click"}}), true, None),
        ];
        let code = generate_session_script(Language::TypeScript, &ops).unwrap();
        assert!(code.contains("import { MongoCore }"));
        assert!(code.contains("async function main()"));
        assert!(code.contains("main();"));
    }
}
