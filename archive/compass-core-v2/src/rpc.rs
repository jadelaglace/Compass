//! JSON-RPC server over stdin/stdout.
//!
//! Handles two methods:
//! - `compute_score` — delegates to ScoringEngine
//! - `parse_refs` — delegates to ReferenceParser

use std::io::Read;

use crate::models::{ReferenceInput, ScoringInput};
use crate::scoring::ScoringEngine;
use crate::reference::ReferenceParser;
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
struct RpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    method: String,
    params: Value,
    id: Value,
}

const JSONRPC_VERSION: &str = "2.0";

/// Read JSON-RPC request from stdin, write response to stdout.
pub fn run() {
    // Read entire stdin
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        // Return parse error JSON instead of panicking
        let err = serde_json::json!({
            "jsonrpc": "2.0",
            "error": { "code": -32700, "message": "Failed to read stdin" },
            "id": serde_json::Value::Null
        });
        println!("{}", err);
        return;
    }

    let trimmed = input.trim();
    if trimmed.is_empty() {
        // Empty input — silent exit (e.g., pipe with no data)
        return;
    }

    // Parse the outer envelope to get method and id
    let envelope: RpcRequest = match serde_json::from_str(trimmed) {
        Ok(req) => req,
        Err(e) => {
            let id = serde_json::from_str::<Value>(trimmed)
                .ok()
                .and_then(|v| v.get("id").cloned())
                .unwrap_or(serde_json::Value::Null);
            let err = serde_json::json!({
                "jsonrpc": JSONRPC_VERSION,
                "error": {
                    "code": -32700,
                    "message": format!("Parse error: {}", e)
                },
                "id": id
            });
            println!("{}", err);
            return;
        }
    };

    // Dispatch by method name
    let response: Value = match envelope.method.as_str() {
        "compute_score" => {
            match serde_json::from_value::<ScoringInput>(envelope.params) {
                Ok(p) => {
                    let output = ScoringEngine::compute(p);
                    serde_json::json!({
                        "jsonrpc": JSONRPC_VERSION,
                        "result": output,
                        "id": envelope.id
                    })
                }
                Err(msg) => {
                    serde_json::json!({
                        "jsonrpc": JSONRPC_VERSION,
                        "error": { "code": -32602, "message": msg.to_string() },
                        "id": envelope.id
                    })
                }
            }
        }
        "parse_refs" => {
            match serde_json::from_value::<ReferenceInput>(envelope.params) {
                Ok(p) => {
                    let output = ReferenceParser::parse(p);
                    serde_json::json!({
                        "jsonrpc": JSONRPC_VERSION,
                        "result": output,
                        "id": envelope.id
                    })
                }
                Err(msg) => {
                    serde_json::json!({
                        "jsonrpc": JSONRPC_VERSION,
                        "error": { "code": -32602, "message": msg.to_string() },
                        "id": envelope.id
                    })
                }
            }
        }
        _ => {
            serde_json::json!({
                "jsonrpc": JSONRPC_VERSION,
                "error": {
                    "code": -32601,
                    "message": format!("Method not found: {}", envelope.method)
                },
                "id": envelope.id
            })
        }
    };

    println!("{}", response);
}
