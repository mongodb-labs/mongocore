use super::Language;

/// Generate a typed data model from collection schema fields.
///
/// Takes a collection name and list of (field_name, bson_type) pairs,
/// returns language-specific model definition as a string.
pub fn generate_model(
    language: Language,
    collection_name: &str,
    fields: &[(String, String)],
) -> String {
    let struct_name = to_pascal_case(collection_name);
    match language {
        Language::Python => generate_python_model(&struct_name, fields),
        Language::TypeScript => generate_typescript_model(&struct_name, fields),
        Language::Go => generate_go_model(&struct_name, fields),
        Language::Java => generate_java_model(&struct_name, fields),
    }
}

/// Generate a Pydantic BaseModel for Python.
fn generate_python_model(struct_name: &str, fields: &[(String, String)]) -> String {
    let mut imports = vec!["from pydantic import BaseModel"];
    let mut has_optional = false;
    let mut has_datetime = false;
    let mut has_field = false;

    // Check what imports we need
    for (field_name, bson_type) in fields {
        if field_name == "_id" {
            has_optional = true;
            has_field = true;
        }
        if bson_type == "DateTime" {
            has_datetime = true;
        }
    }

    if has_optional {
        imports.push("from typing import Optional");
    }
    if has_field {
        imports.push("from pydantic import Field");
    }
    if has_datetime {
        imports.push("from datetime import datetime");
    }

    let mut result = imports.join("\n");
    result.push_str("\n\n\n");
    result.push_str(&format!("class {}(BaseModel):\n", struct_name));

    if fields.is_empty() {
        result.push_str("    pass\n");
        return result;
    }

    for (field_name, bson_type) in fields {
        let py_type = bson_to_python_type(bson_type);
        if field_name == "_id" {
            result.push_str(&format!(
                "    {}: Optional[{}] = Field(None, alias=\"_id\")\n",
                to_snake_case(field_name),
                py_type
            ));
        } else {
            result.push_str(&format!(
                "    {}: {}\n",
                to_snake_case(field_name),
                py_type
            ));
        }
    }

    result
}

/// Generate a TypeScript interface.
fn generate_typescript_model(struct_name: &str, fields: &[(String, String)]) -> String {
    let mut result = format!("interface {} {{\n", struct_name);

    if fields.is_empty() {
        result.push_str("}\n");
        return result;
    }

    for (field_name, bson_type) in fields {
        let ts_type = bson_to_typescript_type(bson_type);
        result.push_str(&format!("  {}: {};\n", to_camel_case(field_name), ts_type));
    }

    result.push_str("}\n");
    result
}

/// Generate a Go struct with BSON and JSON tags.
fn generate_go_model(struct_name: &str, fields: &[(String, String)]) -> String {
    let mut result = format!("type {} struct {{\n", struct_name);

    if fields.is_empty() {
        result.push_str("}\n");
        return result;
    }

    for (field_name, bson_type) in fields {
        let go_type = bson_to_go_type(bson_type);
        let field_name_pascal = to_pascal_case(field_name);
        result.push_str(&format!(
            "\t{} {} `bson:\"{}\" json:\"{}\"`\n",
            field_name_pascal,
            go_type,
            field_name,
            field_name
        ));
    }

    result.push_str("}\n");
    result
}

/// Generate a Java record.
fn generate_java_model(struct_name: &str, fields: &[(String, String)]) -> String {
    if fields.is_empty() {
        return format!("public record {}() {{}}\n", struct_name);
    }

    let field_list = fields
        .iter()
        .map(|(field_name, bson_type)| {
            format!(
                "{} {}",
                bson_to_java_type(bson_type),
                to_camel_case(field_name)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");

    format!("public record {}({}) {{}}\n", struct_name, field_list)
}

/// Map BSON type string to Python type.
fn bson_to_python_type(bson_type: &str) -> &str {
    match bson_type {
        "String" | "ObjectId" => "str",
        "Int32" | "Int64" => "int",
        "Double" => "float",
        "Boolean" => "bool",
        "DateTime" => "datetime",
        "Array" => "list",
        "Document" => "dict",
        _ => "str", // fallback
    }
}

/// Map BSON type string to TypeScript type.
fn bson_to_typescript_type(bson_type: &str) -> &str {
    match bson_type {
        "String" | "ObjectId" => "string",
        "Int32" | "Int64" | "Double" => "number",
        "Boolean" => "boolean",
        "DateTime" => "Date",
        "Array" => "unknown[]",
        "Document" => "Record<string, unknown>",
        _ => "string", // fallback
    }
}

/// Map BSON type string to Go type.
fn bson_to_go_type(bson_type: &str) -> &str {
    match bson_type {
        "String" | "ObjectId" => "string",
        "Int32" => "int32",
        "Int64" => "int64",
        "Double" => "float64",
        "Boolean" => "bool",
        "DateTime" => "time.Time",
        "Array" => "[]interface{}",
        "Document" => "bson.M",
        _ => "string", // fallback
    }
}

/// Map BSON type string to Java type.
fn bson_to_java_type(bson_type: &str) -> &str {
    match bson_type {
        "String" | "ObjectId" => "String",
        "Int32" => "Integer",
        "Int64" => "Long",
        "Double" => "Double",
        "Boolean" => "Boolean",
        "DateTime" => "Instant",
        "Array" => "List<Object>",
        "Document" => "Map<String, Object>",
        _ => "String", // fallback
    }
}

/// Convert a string to PascalCase.
///
/// Splits on `_`, `-`, ` ` and capitalizes each word.
pub fn to_pascal_case(s: &str) -> String {
    s.split(|c| c == '_' || c == '-' || c == ' ')
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect()
}

/// Convert a string to snake_case.
///
/// Inserts `_` before uppercase letters and converts to lowercase.
pub fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(c.to_lowercase().next().unwrap_or(c));
    }
    result
}

/// Convert a string to camelCase.
///
/// Pascal case with first character lowered.
pub fn to_camel_case(s: &str) -> String {
    let pascal = to_pascal_case(s);
    if pascal.is_empty() {
        return pascal;
    }
    let mut chars = pascal.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_lowercase().collect::<String>() + chars.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_pascal_case() {
        assert_eq!(to_pascal_case("hello_world"), "HelloWorld");
        assert_eq!(to_pascal_case("user-profile"), "UserProfile");
        assert_eq!(to_pascal_case("product name"), "ProductName");
        assert_eq!(to_pascal_case("simple"), "Simple");
        assert_eq!(to_pascal_case("multi_word_test"), "MultiWordTest");
    }

    #[test]
    fn test_to_snake_case() {
        assert_eq!(to_snake_case("HelloWorld"), "hello_world");
        assert_eq!(to_snake_case("UserProfile"), "user_profile");
        assert_eq!(to_snake_case("simple"), "simple");
        assert_eq!(to_snake_case("_id"), "_id");
        assert_eq!(to_snake_case("XMLParser"), "x_m_l_parser");
    }

    #[test]
    fn test_to_camel_case() {
        assert_eq!(to_camel_case("hello_world"), "helloWorld");
        assert_eq!(to_camel_case("user-profile"), "userProfile");
        assert_eq!(to_camel_case("product name"), "productName");
        assert_eq!(to_camel_case("simple"), "simple");
        assert_eq!(to_camel_case("_id"), "id");
    }

    #[test]
    fn test_python_model_generation() {
        let fields = vec![
            ("name".to_string(), "String".to_string()),
            ("age".to_string(), "Int32".to_string()),
            ("active".to_string(), "Boolean".to_string()),
        ];

        let result = generate_python_model("User", &fields);
        assert!(result.contains("class User(BaseModel):"));
        assert!(result.contains("name: str"));
        assert!(result.contains("age: int"));
        assert!(result.contains("active: bool"));
    }

    #[test]
    fn test_python_model_with_id_field() {
        let fields = vec![
            ("_id".to_string(), "ObjectId".to_string()),
            ("title".to_string(), "String".to_string()),
        ];

        let result = generate_python_model("Document", &fields);
        assert!(result.contains("from typing import Optional"));
        assert!(result.contains("from pydantic import Field"));
        assert!(result.contains("_id: Optional[str] = Field(None, alias=\"_id\")"));
        assert!(result.contains("title: str"));
    }

    #[test]
    fn test_python_model_with_datetime() {
        let fields = vec![
            ("created_at".to_string(), "DateTime".to_string()),
            ("name".to_string(), "String".to_string()),
        ];

        let result = generate_python_model("Event", &fields);
        assert!(result.contains("from datetime import datetime"));
        assert!(result.contains("created_at: datetime"));
    }

    #[test]
    fn test_typescript_model_generation() {
        let fields = vec![
            ("name".to_string(), "String".to_string()),
            ("age".to_string(), "Int32".to_string()),
            ("active".to_string(), "Boolean".to_string()),
        ];

        let result = generate_typescript_model("User", &fields);
        assert!(result.contains("interface User {"));
        assert!(result.contains("name: string;"));
        assert!(result.contains("age: number;"));
        assert!(result.contains("active: boolean;"));
    }

    #[test]
    fn test_go_model_generation() {
        let fields = vec![
            ("name".to_string(), "String".to_string()),
            ("age".to_string(), "Int32".to_string()),
            ("score".to_string(), "Double".to_string()),
        ];

        let result = generate_go_model("User", &fields);
        assert!(result.contains("type User struct {"));
        assert!(result.contains("Name string `bson:\"name\" json:\"name\"`"));
        assert!(result.contains("Age int32 `bson:\"age\" json:\"age\"`"));
        assert!(result.contains("Score float64 `bson:\"score\" json:\"score\"`"));
    }

    #[test]
    fn test_java_model_generation() {
        let fields = vec![
            ("name".to_string(), "String".to_string()),
            ("age".to_string(), "Int32".to_string()),
            ("active".to_string(), "Boolean".to_string()),
        ];

        let result = generate_java_model("User", &fields);
        assert!(result.contains("public record User("));
        assert!(result.contains("String name"));
        assert!(result.contains("Integer age"));
        assert!(result.contains("Boolean active"));
    }

    #[test]
    fn test_generate_model_all_languages() {
        let fields = vec![
            ("user_name".to_string(), "String".to_string()),
            ("user_age".to_string(), "Int32".to_string()),
        ];

        let python = generate_model(Language::Python, "user_profile", &fields);
        assert!(python.contains("class UserProfile(BaseModel):"));
        assert!(python.contains("user_name: str"));

        let typescript = generate_model(Language::TypeScript, "user_profile", &fields);
        assert!(typescript.contains("interface UserProfile {"));
        assert!(typescript.contains("userName: string;"));

        let go = generate_model(Language::Go, "user_profile", &fields);
        assert!(go.contains("type UserProfile struct {"));
        assert!(go.contains("UserName string"));

        let java = generate_model(Language::Java, "user_profile", &fields);
        assert!(java.contains("public record UserProfile("));
        assert!(java.contains("String userName"));
    }

    #[test]
    fn test_empty_fields() {
        let fields = vec![];

        let python = generate_model(Language::Python, "Empty", &fields);
        assert!(python.contains("class Empty(BaseModel):"));
        assert!(python.contains("pass"));

        let typescript = generate_model(Language::TypeScript, "Empty", &fields);
        assert!(typescript.contains("interface Empty {"));

        let go = generate_model(Language::Go, "Empty", &fields);
        assert!(go.contains("type Empty struct {"));

        let java = generate_model(Language::Java, "Empty", &fields);
        assert!(java.contains("public record Empty() {}"));
    }

    #[test]
    fn test_type_mappings() {
        let fields = vec![
            ("str_field".to_string(), "String".to_string()),
            ("obj_id".to_string(), "ObjectId".to_string()),
            ("int32_field".to_string(), "Int32".to_string()),
            ("int64_field".to_string(), "Int64".to_string()),
            ("double_field".to_string(), "Double".to_string()),
            ("bool_field".to_string(), "Boolean".to_string()),
            ("date_field".to_string(), "DateTime".to_string()),
            ("array_field".to_string(), "Array".to_string()),
            ("doc_field".to_string(), "Document".to_string()),
        ];

        // Python
        let python = generate_model(Language::Python, "TypeTest", &fields);
        assert!(python.contains("str_field: str"));
        assert!(python.contains("obj_id: str"));
        assert!(python.contains("int32_field: int"));
        assert!(python.contains("int64_field: int"));
        assert!(python.contains("double_field: float"));
        assert!(python.contains("bool_field: bool"));
        assert!(python.contains("date_field: datetime"));
        assert!(python.contains("array_field: list"));
        assert!(python.contains("doc_field: dict"));

        // TypeScript
        let typescript = generate_model(Language::TypeScript, "TypeTest", &fields);
        assert!(typescript.contains("strField: string;"));
        assert!(typescript.contains("objId: string;"));
        assert!(typescript.contains("int32Field: number;"));
        assert!(typescript.contains("int64Field: number;"));
        assert!(typescript.contains("doubleField: number;"));
        assert!(typescript.contains("boolField: boolean;"));
        assert!(typescript.contains("dateField: Date;"));
        assert!(typescript.contains("arrayField: unknown[];"));
        assert!(typescript.contains("docField: Record<string, unknown>;"));

        // Go
        let go = generate_model(Language::Go, "TypeTest", &fields);
        assert!(go.contains("StrField string"));
        assert!(go.contains("ObjId string"));
        assert!(go.contains("Int32Field int32"));
        assert!(go.contains("Int64Field int64"));
        assert!(go.contains("DoubleField float64"));
        assert!(go.contains("BoolField bool"));
        assert!(go.contains("DateField time.Time"));
        assert!(go.contains("ArrayField []interface{}"));
        assert!(go.contains("DocField bson.M"));

        // Java
        let java = generate_model(Language::Java, "TypeTest", &fields);
        assert!(java.contains("String strField"));
        assert!(java.contains("String objId"));
        assert!(java.contains("Integer int32Field"));
        assert!(java.contains("Long int64Field"));
        assert!(java.contains("Double doubleField"));
        assert!(java.contains("Boolean boolField"));
        assert!(java.contains("Instant dateField"));
        assert!(java.contains("List<Object> arrayField"));
        assert!(java.contains("Map<String, Object> docField"));
    }
}
