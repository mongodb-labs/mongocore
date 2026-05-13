//! Integration tests for MCP stdio transport.
//! These tests spawn MongoCore as a child process with --stdio flag
//! and verify JSON-RPC communication over stdin/stdout.
//!
//! NOTE: These tests require MongoDB running on localhost:27017.
//! Run `just docker-up` before running these tests.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use serde_json::{json, Value};

fn spawn_mongocore_stdio() -> std::process::Child {
    Command::new(env!("CARGO_BIN_EXE_mongocore"))
        .args(["--stdio", "--connection-uri", "mongodb://localhost:27017"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to start mongocore")
}

fn send_request(stdin: &mut impl Write, request: &Value) {
    writeln!(stdin, "{}", serde_json::to_string(request).unwrap()).unwrap();
    stdin.flush().unwrap();
}

fn read_response(reader: &mut impl BufRead) -> Value {
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    serde_json::from_str(&line).expect("Failed to parse response JSON")
}

#[test]
fn test_stdio_initialize() {
    let mut child = spawn_mongocore_stdio();
    let stdin = child.stdin.as_mut().unwrap();
    let mut reader = BufReader::new(child.stdout.take().unwrap());

    // Send initialize request
    send_request(stdin, &json!({
        "jsonrpc": "2.0",
        "method": "initialize",
        "id": 1
    }));

    let response = read_response(&mut reader);
    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], 1);
    assert!(response["result"]["protocolVersion"].is_string());
    assert_eq!(response["result"]["serverInfo"]["name"], "mongocore");
    // Stdio mode should advertise prompts capability
    assert!(response["result"]["capabilities"]["prompts"].is_object());

    child.kill().unwrap();
}

#[test]
fn test_stdio_tools_list() {
    let mut child = spawn_mongocore_stdio();
    let stdin = child.stdin.as_mut().unwrap();
    let mut reader = BufReader::new(child.stdout.take().unwrap());

    // Initialize first
    send_request(stdin, &json!({"jsonrpc":"2.0","method":"initialize","id":1}));
    let _ = read_response(&mut reader);

    // Request tools list
    send_request(stdin, &json!({"jsonrpc":"2.0","method":"tools/list","id":2}));
    let response = read_response(&mut reader);

    assert_eq!(response["id"], 2);
    let tools = response["result"]["tools"].as_array().unwrap();
    assert!(tools.len() >= 24); // 21 existing + collection_schema + ask + explain_query

    // Verify new tools are present
    let tool_names: Vec<&str> = tools.iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(tool_names.contains(&"collection_schema"), "Missing collection_schema tool");
    assert!(tool_names.contains(&"ask"), "Missing ask tool");
    assert!(tool_names.contains(&"explain_query"), "Missing explain_query tool");

    child.kill().unwrap();
}

#[test]
fn test_stdio_invalid_json() {
    let mut child = spawn_mongocore_stdio();
    let stdin = child.stdin.as_mut().unwrap();
    let mut reader = BufReader::new(child.stdout.take().unwrap());

    // Send invalid JSON
    writeln!(stdin, "not valid json").unwrap();
    stdin.flush().unwrap();

    let response = read_response(&mut reader);
    assert_eq!(response["jsonrpc"], "2.0");
    assert!(response["error"].is_object());
    assert_eq!(response["error"]["code"], -32700); // Parse error

    child.kill().unwrap();
}

#[test]
fn test_stdio_unknown_method() {
    let mut child = spawn_mongocore_stdio();
    let stdin = child.stdin.as_mut().unwrap();
    let mut reader = BufReader::new(child.stdout.take().unwrap());

    send_request(stdin, &json!({
        "jsonrpc": "2.0",
        "method": "nonexistent/method",
        "id": 99
    }));

    let response = read_response(&mut reader);
    assert_eq!(response["id"], 99);
    assert!(response["error"].is_object());
    assert_eq!(response["error"]["code"], -32601); // Method not found

    child.kill().unwrap();
}
