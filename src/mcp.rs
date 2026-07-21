//! MCP server over the corpus, built on mcp-stdio.
//!
//! Deliberately dumb: three read-only tools over the corpus. Synthesis
//! happened at ingest time; the calling agent does the reasoning. This
//! module only describes the tools; mcp-stdio owns the transport.

use serde_json::{json, Value};

use mcp_stdio::{serve as serve_stdio, Server, Tool};

use crate::corpus::Corpus;

pub fn serve(c: &Corpus) {
    serve_stdio(&HindsightServer { corpus: c });
}

struct HindsightServer<'a> {
    corpus: &'a Corpus,
}

impl Server for HindsightServer<'_> {
    fn name(&self) -> &str {
        "hindsight"
    }
    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    fn tools(&self) -> Vec<Tool> {
        vec![
            Tool {
                name: "search".into(),
                description: "Search the incident wiki (incidents, failure patterns with playbooks, system histories). Case-insensitive term search; pages matching all terms rank first. Start broad (an error string, a hostname, a symptom), then get_page the hits.".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Search terms, e.g. 'bnxt_en timeout' or 'redis oom'" },
                        "limit": { "type": "integer", "description": "Max results (default 10)" }
                    },
                    "required": ["query"]
                }),
            },
            Tool {
                name: "get_page".into(),
                description: "Read a wiki page by wiki-relative path, e.g. 'index.md', 'patterns/ddos-response.md', 'incidents/20260604-185229.md'. Pages cross-link with relative markdown links; follow them via further get_page calls.".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Wiki-relative path ending in .md" }
                    },
                    "required": ["path"]
                }),
            },
            Tool {
                name: "list".into(),
                description: "List wiki pages. kind 'index' returns the full catalog (one line per page; best starting point). 'incidents', 'patterns', or 'systems' list that directory with page titles.".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "kind": {
                            "type": "string",
                            "enum": ["index", "incidents", "patterns", "systems"],
                            "description": "What to list (default: index)"
                        }
                    }
                }),
            },
        ]
    }

    fn call(&self, name: &str, args: &Value) -> Result<String, String> {
        match name {
            "search" => {
                let q = args["query"].as_str().unwrap_or("");
                let limit = args["limit"].as_u64().unwrap_or(10).max(1) as usize;
                Ok(self.corpus.search(q, limit))
            }
            "get_page" => self.corpus.get_page(args["path"].as_str().unwrap_or("")),
            "list" => {
                let kind = args["kind"].as_str().unwrap_or("index");
                self.corpus.list(kind)
            }
            other => Err(format!("unknown tool: {other}")),
        }
    }
}
