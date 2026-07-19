//! MCP server over stdio: newline-delimited JSON-RPC 2.0.
//!
//! Deliberately dumb: three read-only tools over the corpus. Synthesis
//! happened at ingest time; the calling agent does the reasoning.

use std::io::{self, BufRead, Write};

use serde_json::{json, Value};

use crate::corpus::Corpus;

pub fn serve(c: &Corpus) {
    let stdin = io::stdin();
    let stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let id = msg.get("id").cloned();
        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");

        let result = match method {
            "initialize" => {
                let pv = msg["params"]["protocolVersion"]
                    .as_str()
                    .unwrap_or("2024-11-05")
                    .to_string();
                Some(json!({
                    "protocolVersion": pv,
                    "capabilities": { "tools": {} },
                    "serverInfo": {
                        "name": "hindsight",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }))
            }
            "ping" => Some(json!({})),
            "tools/list" => Some(tools_list()),
            "tools/call" => Some(tools_call(c, &msg["params"])),
            _ => None,
        };

        // Notifications (no id) never get a response.
        let id = match id {
            Some(id) => id,
            None => continue,
        };
        let out = match result {
            Some(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            None => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": format!("method not found: {}", method) }
            }),
        };

        let mut lock = stdout.lock();
        let _ = writeln!(lock, "{}", out);
        let _ = lock.flush();
    }
}

fn tools_list() -> Value {
    json!({
        "tools": [
            {
                "name": "search",
                "description": "Search the incident wiki (incidents, failure patterns with playbooks, system histories). Case-insensitive term search; pages matching all terms rank first. Start broad (an error string, a hostname, a symptom), then get_page the hits.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Search terms, e.g. 'bnxt_en timeout' or 'redis oom'" },
                        "limit": { "type": "integer", "description": "Max results (default 10)" }
                    },
                    "required": ["query"]
                }
            },
            {
                "name": "get_page",
                "description": "Read a wiki page by wiki-relative path, e.g. 'index.md', 'patterns/ddos-response.md', 'incidents/20260604-185229.md'. Pages cross-link with relative markdown links; follow them via further get_page calls.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Wiki-relative path ending in .md" }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "list",
                "description": "List wiki pages. kind 'index' returns the full catalog (one line per page; best starting point). 'incidents', 'patterns', or 'systems' list that directory with page titles.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "kind": {
                            "type": "string",
                            "enum": ["index", "incidents", "patterns", "systems"],
                            "description": "What to list (default: index)"
                        }
                    }
                }
            }
        ]
    })
}

fn tools_call(c: &Corpus, params: &Value) -> Value {
    let name = params["name"].as_str().unwrap_or("");
    let args = &params["arguments"];

    let result: Result<String, String> = match name {
        "search" => {
            let q = args["query"].as_str().unwrap_or("");
            let limit = args["limit"].as_u64().unwrap_or(10).max(1) as usize;
            Ok(c.search(q, limit))
        }
        "get_page" => c.get_page(args["path"].as_str().unwrap_or("")),
        "list" => {
            let kind = args["kind"].as_str().unwrap_or("index");
            c.list(kind)
        }
        other => Err(format!("unknown tool: {}", other)),
    };

    match result {
        Ok(text) => json!({ "content": [{ "type": "text", "text": text }] }),
        Err(e) => json!({ "content": [{ "type": "text", "text": e }], "isError": true }),
    }
}
