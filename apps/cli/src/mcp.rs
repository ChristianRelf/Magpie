//! Model Context Protocol server (stdio transport). Exposes the harness to
//! MCP-capable agents as tools: route a sub-task to the best model, list
//! models, and read usage and limits.

use anyhow::Result;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::api::Api;

const PROTOCOL_VERSION: &str = "2025-06-18";

fn tools() -> Value {
    json!([
        {
            "name": "magpie_generate",
            "description": "Send a prompt to the best available model through the Magpie harness. Magpie selects a model based on the task, the user's routing preferences and current provider limits, and falls back automatically when a provider is unavailable.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "prompt": {"type": "string", "description": "The full prompt to execute."},
                    "system": {"type": "string", "description": "Optional system instructions."},
                    "model": {"type": "string", "description": "Optional model id (e.g. 'anthropic/claude-sonnet-5'). Defaults to automatic routing."},
                    "task_type": {"type": "string", "description": "Optional task hint: simple_question, code_generation, debugging, repository_analysis, math_reasoning, planning, summarisation, data_extraction."},
                    "preset": {"type": "string", "enum": ["automatic", "best_quality", "fastest", "economical", "preserve_limits"]}
                },
                "required": ["prompt"]
            }
        },
        {
            "name": "magpie_list_models",
            "description": "List models available through the Magpie harness with their capabilities and availability.",
            "inputSchema": {"type": "object", "properties": {}}
        },
        {
            "name": "magpie_usage",
            "description": "Summarise token usage and cost through the harness for a time range.",
            "inputSchema": {"type": "object", "properties": {"range": {"type": "string", "enum": ["1h", "24h", "7d", "30d"]}}}
        },
        {
            "name": "magpie_limits",
            "description": "Show current provider limit windows, usage percentages and reset times.",
            "inputSchema": {"type": "object", "properties": {}}
        }
    ])
}

async fn call_tool(api: &Api, name: &str, args: &Value) -> Result<String> {
    match name {
        "magpie_generate" => {
            let mut body = json!({"input": args["prompt"].as_str().unwrap_or_default(), "model": args["model"].as_str().unwrap_or("auto")});
            if let Some(s) = args["system"].as_str() {
                body["instructions"] = json!(s);
            }
            if let Some(t) = args["task_type"].as_str() {
                body["task_type"] = json!(t);
            }
            if let Some(p) = args["preset"].as_str() {
                body["preferences"] = json!({"preset": p});
            }
            let r = api.post("/v1/responses", &body).await?;
            Ok(r["output_text"].as_str().unwrap_or_default().to_string())
        }
        "magpie_list_models" => {
            let v = api.get("/v1/models").await?;
            let lines: Vec<String> = v["data"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .filter(|m| m["id"] != "auto")
                .map(|m| {
                    let i = &m["magpie"];
                    format!(
                        "{} — {} tier, context {}, {}",
                        m["id"].as_str().unwrap_or_default(),
                        i["tier"].as_str().unwrap_or_default(),
                        i["context_window"].as_u64().map(|c| c.to_string()).unwrap_or_else(|| "unknown".into()),
                        if i["available"] == true { "available" } else { "unavailable" }
                    )
                })
                .collect();
            Ok(if lines.is_empty() { "No models connected.".into() } else { lines.join("\n") })
        }
        "magpie_usage" => {
            let range = args["range"].as_str().unwrap_or("24h");
            let v = api.get(&format!("/v1/usage/summary?range={range}")).await?;
            Ok(serde_json::to_string_pretty(&v["current"])?)
        }
        "magpie_limits" => {
            let v = api.get("/v1/limits").await?;
            Ok(serde_json::to_string_pretty(&v)?)
        }
        other => anyhow::bail!("Unknown tool: {other}"),
    }
}

pub async fn serve(api: Api) -> Result<()> {
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();
    while let Some(line) = lines.next_line().await? {
        let Ok(msg) = serde_json::from_str::<Value>(line.trim()) else { continue };
        let Some(id) = msg.get("id").cloned() else { continue }; // notifications need no reply
        let method = msg["method"].as_str().unwrap_or_default();
        let result: Result<Value, (i64, String)> = match method {
            "initialize" => Ok(json!({
                "protocolVersion": msg["params"]["protocolVersion"].as_str().unwrap_or(PROTOCOL_VERSION),
                "capabilities": {"tools": {"listChanged": false}},
                "serverInfo": {"name": "magpie", "title": "Magpie", "version": env!("CARGO_PKG_VERSION")},
                "instructions": "Magpie routes prompts to the user's connected AI providers. Use magpie_generate to delegate a self-contained sub-task to another model."
            })),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({"tools": tools()})),
            "tools/call" => {
                let name = msg["params"]["name"].as_str().unwrap_or_default();
                let args = msg["params"]["arguments"].clone();
                match call_tool(&api, name, &args).await {
                    Ok(text) => Ok(json!({"content": [{"type": "text", "text": text}], "isError": false})),
                    Err(e) => Ok(json!({"content": [{"type": "text", "text": e.to_string()}], "isError": true})),
                }
            }
            _ => Err((-32601, format!("Method not found: {method}"))),
        };
        let reply = match result {
            Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
            Err((code, message)) => json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}),
        };
        let mut out = reply.to_string();
        out.push('\n');
        stdout.write_all(out.as_bytes()).await?;
        stdout.flush().await?;
    }
    Ok(())
}
