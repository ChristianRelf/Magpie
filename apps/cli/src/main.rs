//! `magpie` — command-line interface to the Magpie harness.

mod api;
mod fmt;
mod mcp;

use std::io::{IsTerminal, Read, Write};
use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use clap::{Parser, Subcommand};
use futures::StreamExt;
use magpie_runtime::paths::Paths;
use serde_json::{json, Value};

use crate::fmt::*;

#[derive(Parser)]
#[command(name = "magpie", version, about = "Magpie — one harness for every compatible AI model", propagate_version = true)]
struct Cli {
    /// Print raw JSON instead of formatted output.
    #[arg(long, global = true)]
    json: bool,
    /// Do not start the harness automatically.
    #[arg(long, global = true)]
    no_start: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the harness in the foreground.
    Serve {
        /// Override the API port.
        #[arg(long)]
        port: Option<u16>,
    },
    /// Start the harness in the background.
    Start,
    /// Stop the background harness.
    Stop,
    /// Restart the background harness.
    Restart,
    /// Show harness status.
    Status,
    /// List connected providers.
    Providers,
    /// Connect a provider (e.g. `claude_code`, `codex_cli`, `openai`, `ollama`).
    Connect {
        kind: String,
        #[arg(long)]
        label: Option<String>,
        #[arg(long)]
        base_url: Option<String>,
        /// Read the API key from standard input.
        #[arg(long)]
        key_stdin: bool,
    },
    /// Remove a provider connection and its stored credential.
    Disconnect { id: String },
    /// List models.
    Models {
        /// Only show models that are currently available.
        #[arg(long)]
        available: bool,
    },
    /// Show usage statistics.
    Usage {
        /// 1h, 24h, 7d or 30d.
        #[arg(long, default_value = "24h")]
        range: String,
    },
    /// Show provider limits and reset times.
    Limits,
    /// Show recent executions.
    Activity {
        #[arg(long, default_value_t = 20)]
        limit: u32,
    },
    /// Explain which model a prompt would be routed to (no execution).
    Route {
        prompt: Vec<String>,
        #[arg(long)]
        preset: Option<String>,
        #[arg(long)]
        task: Option<String>,
        #[arg(long)]
        model: Option<String>,
    },
    /// Execute a prompt through the harness and stream the output. Use `-`
    /// to read the prompt from standard input.
    Run {
        prompt: Vec<String>,
        #[arg(long, short)]
        model: Option<String>,
        #[arg(long, short)]
        preset: Option<String>,
        #[arg(long, short)]
        task: Option<String>,
        #[arg(long, short)]
        system: Option<String>,
        /// Print routing details to stderr.
        #[arg(long, short)]
        verbose: bool,
    },
    /// Manage API keys for local integrations.
    Keys {
        #[command(subcommand)]
        action: KeysAction,
    },
    /// Print environment variables for OpenAI-compatible tools.
    Env,
    /// Run a Model Context Protocol server on stdio.
    Mcp,
}

#[derive(Subcommand)]
enum KeysAction {
    /// List keys.
    List,
    /// Create a key. Scopes: execute, read, admin.
    Create {
        name: String,
        #[arg(long = "scope", default_values_t = ["execute".to_string(), "read".to_string()])]
        scopes: Vec<String>,
    },
    /// Revoke a key.
    Revoke { id: String },
}

fn main() {
    let cli = Cli::parse();
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().expect("runtime");
    let code = rt.block_on(async {
        match run(cli).await {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("error: {e}");
                1
            }
        }
    });
    std::process::exit(code);
}

fn prompt_text(parts: Vec<String>) -> Result<String> {
    let mut prompt = parts.join(" ");
    let stdin = std::io::stdin();
    if prompt == "-" || (prompt.is_empty() && !stdin.is_terminal()) {
        let mut buf = String::new();
        stdin.lock().read_to_string(&mut buf)?;
        prompt = buf;
    } else if !stdin.is_terminal() {
        let mut buf = String::new();
        stdin.lock().read_to_string(&mut buf)?;
        if !buf.trim().is_empty() {
            prompt = format!("{prompt}\n\n{buf}");
        }
    }
    if prompt.trim().is_empty() {
        bail!("A prompt is required");
    }
    Ok(prompt)
}

fn request_body(prompt: String, model: Option<String>, preset: Option<String>, task: Option<String>, system: Option<String>, stream: bool) -> Value {
    let mut body = json!({"input": prompt, "model": model.unwrap_or_else(|| "auto".into()), "stream": stream});
    if let Some(p) = preset {
        body["preferences"] = json!({"preset": p});
    }
    if let Some(t) = task {
        body["task_type"] = json!(t);
    }
    if let Some(s) = system {
        body["instructions"] = json!(s);
    }
    body
}

async fn run(cli: Cli) -> Result<()> {
    let json_out = cli.json;
    match cli.command {
        Command::Serve { port } => {
            let exe = std::env::current_exe()?;
            magpie_runtime::run_daemon(magpie_runtime::DaemonOptions { launch_command: Some((exe, vec!["serve".into()])), port })
                .await
                .map_err(|e| anyhow!(e.message))
        }
        Command::Start => {
            let api = api::connect(false).await?;
            println!("Harness running at {}", api.url);
            Ok(())
        }
        Command::Stop => {
            let paths = Paths::resolve();
            match magpie_runtime::discover(&paths).await {
                Some(c) => {
                    magpie_runtime::stop(&c).await.map_err(|e| anyhow!(e.message))?;
                    println!("Harness stopped");
                }
                None => println!("Harness is not running"),
            }
            Ok(())
        }
        Command::Restart => {
            let paths = Paths::resolve();
            if let Some(c) = magpie_runtime::discover(&paths).await {
                magpie_runtime::stop(&c).await.map_err(|e| anyhow!(e.message))?;
            }
            let api = api::connect(false).await?;
            println!("Harness running at {}", api.url);
            Ok(())
        }
        Command::Status => {
            let paths = Paths::resolve();
            let Some(c) = magpie_runtime::discover(&paths).await else {
                if json_out {
                    println!("{}", json!({"running": false}));
                } else {
                    println!("{}  not running", dim("harness"));
                    println!("{}  {}", dim("data   "), paths.root.display());
                }
                return Ok(());
            };
            let api = api::Api::new(&c);
            let s = api.get("/v1/status").await?;
            if json_out {
                return print_json(&s);
            }
            kv("harness", &format!("running  {}  pid {}", c.url, s["pid"]));
            kv("version", s["version"].as_str().unwrap_or_default());
            kv("uptime", &duration_secs(s["uptime_secs"].as_i64().unwrap_or(0)));
            kv("providers", &format!("{} connected / {}", s["connected_accounts"], s["accounts"]));
            kv("models", &format!("{} available / {}", s["available_models"], s["models"]));
            kv("active", &s["active_executions"].to_string());
            kv("secrets", s["secret_store"].as_str().unwrap_or_default());
            kv("data", s["data_dir"].as_str().unwrap_or_default());
            Ok(())
        }
        Command::Providers => {
            let api = api::connect(cli.no_start).await?;
            let v = api.get("/v1/providers").await?;
            if json_out {
                return print_json(&v);
            }
            let rows: Vec<Vec<String>> = v["accounts"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .map(|a| {
                    vec![
                        a["id"].as_str().unwrap_or_default().to_string(),
                        a["label"].as_str().unwrap_or_default().to_string(),
                        a["status"].as_str().unwrap_or_default().replace('_', " "),
                        a["billing_mode"].as_str().unwrap_or_default().to_string(),
                        a["plan"].as_str().unwrap_or("-").to_string(),
                        a["model_count"].to_string(),
                        a["limits"]["state"].as_str().unwrap_or("unknown").replace('_', " "),
                    ]
                })
                .collect();
            if rows.is_empty() {
                println!("No providers connected. Try `magpie connect claude_code` or open the Magpie app.");
            } else {
                table(&["ID", "PROVIDER", "STATUS", "BILLING", "PLAN", "MODELS", "LIMITS"], &rows);
            }
            Ok(())
        }
        Command::Connect { kind, label, base_url, key_stdin } => {
            let api = api::connect(cli.no_start).await?;
            let mut body = json!({"kind": kind, "label": label, "base_url": base_url});
            if key_stdin {
                let mut key = String::new();
                std::io::stdin().read_line(&mut key)?;
                body["api_key"] = json!(key.trim());
            }
            let a = api.post("/v1/providers", &body).await?;
            if json_out {
                return print_json(&a);
            }
            println!("Connected {} ({})", a["label"].as_str().unwrap_or_default(), a["id"].as_str().unwrap_or_default());
            if let Some(p) = a["plan"].as_str() {
                kv("plan", p);
            }
            if let Some(i) = a["identity"].as_str() {
                kv("account", i);
            }
            Ok(())
        }
        Command::Disconnect { id } => {
            let api = api::connect(cli.no_start).await?;
            api.delete(&format!("/v1/providers/{id}")).await?;
            println!("Disconnected {id}");
            Ok(())
        }
        Command::Models { available } => {
            let api = api::connect(cli.no_start).await?;
            let v = api.get("/v1/models").await?;
            if json_out {
                return print_json(&v);
            }
            let rows: Vec<Vec<String>> = v["data"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .filter(|m| m["id"] != "auto")
                .filter(|m| !available || m["magpie"]["available"] == true)
                .map(|m| {
                    let i = &m["magpie"];
                    let caps = &i["capabilities"];
                    let flags: Vec<&str> = [("tools", "tools"), ("vision", "vision"), ("reasoning", "reason"), ("structured_output", "json"), ("agentic", "agent")]
                        .iter()
                        .filter(|(k, _)| caps[*k] == true)
                        .map(|(_, l)| *l)
                        .collect();
                    vec![
                        m["id"].as_str().unwrap_or_default().to_string(),
                        i["account_label"].as_str().unwrap_or_default().to_string(),
                        i["tier"].as_str().unwrap_or_default().to_string(),
                        i["context_window"].as_u64().map(compact).unwrap_or_else(|| "-".into()),
                        flags.join(" "),
                        if i["available"] == true { "yes".into() } else { i["unavailable_reason"].as_str().unwrap_or("no").chars().take(40).collect() },
                    ]
                })
                .collect();
            table(&["MODEL", "ACCOUNT", "TIER", "CONTEXT", "CAPABILITIES", "AVAILABLE"], &rows);
            Ok(())
        }
        Command::Usage { range } => {
            let api = api::connect(cli.no_start).await?;
            let v = api.get(&format!("/v1/usage/summary?range={range}")).await?;
            let b = api.get(&format!("/v1/usage/breakdown?range={range}&group_by=model")).await?;
            if json_out {
                return print_json(&json!({"summary": v, "by_model": b}));
            }
            let c = &v["current"];
            println!("{}", dim(&format!("Usage through the harness, last {range}")));
            kv("requests", &format!("{}  ({} ok, {} failed)", c["requests"], c["succeeded"], c["failed"]));
            kv("input", &compact(c["input_tokens"].as_u64().unwrap_or(0)));
            kv("output", &compact(c["output_tokens"].as_u64().unwrap_or(0)));
            kv("cached", &compact(c["cached_tokens"].as_u64().unwrap_or(0)));
            kv("reported $", &format!("{:.4}", c["cost_reported_usd"].as_f64().unwrap_or(0.0)));
            kv("calculated $", &format!("{:.4}", c["cost_calculated_usd"].as_f64().unwrap_or(0.0)));
            kv("estimated $", &format!("{:.4}", c["cost_estimated_usd"].as_f64().unwrap_or(0.0)));
            kv("api-equiv $", &format!("{:.4}  (subscription usage, not charged)", c["api_equivalent_usd"].as_f64().unwrap_or(0.0)));
            if let Some(ms) = c["avg_duration_ms"].as_f64() {
                kv("avg latency", &format!("{:.0} ms", ms));
            }
            let rows: Vec<Vec<String>> = b["rows"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .map(|r| {
                    vec![
                        r["label"].as_str().unwrap_or_default().to_string(),
                        r["provider"].as_str().unwrap_or("-").to_string(),
                        r["requests"].to_string(),
                        compact(r["input_tokens"].as_u64().unwrap_or(0)),
                        compact(r["output_tokens"].as_u64().unwrap_or(0)),
                    ]
                })
                .collect();
            if !rows.is_empty() {
                println!();
                table(&["MODEL", "PROVIDER", "REQUESTS", "INPUT", "OUTPUT"], &rows);
            }
            Ok(())
        }
        Command::Limits => {
            let api = api::connect(cli.no_start).await?;
            let v = api.get("/v1/limits").await?;
            if json_out {
                return print_json(&v);
            }
            let providers = api.get("/v1/providers").await?;
            let label_of = |id: &str| {
                providers["accounts"].as_array().and_then(|a| a.iter().find(|x| x["id"] == id)).and_then(|x| x["label"].as_str()).unwrap_or(id).to_string()
            };
            let mut rows = Vec::new();
            for acc in v["accounts"].as_array().cloned().unwrap_or_default() {
                let label = label_of(acc["account_id"].as_str().unwrap_or_default());
                let windows = acc["windows"].as_array().cloned().unwrap_or_default();
                if windows.is_empty() {
                    rows.push(vec![label, "-".into(), "unknown".into(), "-".into(), "-".into(), "not reported".into()]);
                    continue;
                }
                for w in windows {
                    let used = w["used_percent"].as_f64().map(|p| format!("{p:.0}%")).or_else(|| match (w["limit"].as_f64(), w["remaining"].as_f64()) {
                        (Some(l), Some(r)) => Some(format!("{}/{}", compact((l - r) as u64), compact(l as u64))),
                        (None, Some(r)) => Some(format!("{r:.2} left")),
                        _ => None,
                    });
                    rows.push(vec![
                        label.clone(),
                        w["label"].as_str().unwrap_or_default().to_string(),
                        if w["exhausted"] == true { "exhausted".into() } else { "ok".into() },
                        used.unwrap_or_else(|| "-".into()),
                        w["resets_at"].as_str().map(relative_time).unwrap_or_else(|| "-".into()),
                        w["provenance"].as_str().unwrap_or_default().to_string(),
                    ]);
                }
            }
            table(&["PROVIDER", "WINDOW", "STATE", "USED", "RESETS", "SOURCE"], &rows);
            Ok(())
        }
        Command::Activity { limit } => {
            let api = api::connect(cli.no_start).await?;
            let v = api.get(&format!("/v1/executions?limit={limit}")).await?;
            if json_out {
                return print_json(&v);
            }
            let rows: Vec<Vec<String>> = v["data"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .map(|e| {
                    vec![
                        e["created_at"].as_str().map(short_time).unwrap_or_default(),
                        e["model"]["display_name"].as_str().unwrap_or("-").to_string(),
                        e["task"].as_str().unwrap_or_default().replace('_', " "),
                        format!(
                            "{} / {}",
                            e["usage"]["input_tokens"].as_u64().map(compact).unwrap_or_else(|| "-".into()),
                            e["usage"]["output_tokens"].as_u64().map(compact).unwrap_or_else(|| "-".into())
                        ),
                        e["duration_ms"].as_u64().map(|d| format!("{:.1}s", d as f64 / 1000.0)).unwrap_or_else(|| "-".into()),
                        e["status"].as_str().unwrap_or_default().to_string(),
                    ]
                })
                .collect();
            table(&["TIME", "MODEL", "TASK", "TOKENS IN/OUT", "DURATION", "STATUS"], &rows);
            Ok(())
        }
        Command::Route { prompt, preset, task, model } => {
            let api = api::connect(cli.no_start).await?;
            let body = request_body(prompt_text(prompt)?, model, preset, task, None, false);
            let d = api.post("/v1/route", &body).await?;
            if json_out {
                return print_json(&d);
            }
            let c = &d["classification"];
            kv("task", &format!("{} ({} complexity)", c["task"].as_str().unwrap_or_default().replace('_', " "), c["complexity"].as_str().unwrap_or_default()));
            kv("preset", &d["preset"].as_str().unwrap_or_default().replace('_', " "));
            kv("tokens", &format!("~{} in, ~{} out (estimated)", compact(c["estimated_input_tokens"].as_u64().unwrap_or(0)), compact(c["estimated_output_tokens"].as_u64().unwrap_or(0))));
            println!();
            for (i, cand) in d["candidates"].as_array().cloned().unwrap_or_default().iter().enumerate() {
                let marker = if i == 0 { "selected " } else { "fallback " };
                println!("{}{}  {}", dim(marker), cand["model"]["display_name"].as_str().unwrap_or_default(), dim(&format!("{:.3}", cand["score"].as_f64().unwrap_or(0.0))));
                if i == 0 {
                    for r in cand["reasons"].as_array().cloned().unwrap_or_default() {
                        println!("           - {}", r.as_str().unwrap_or_default());
                    }
                }
            }
            let rejected = d["rejected"].as_array().cloned().unwrap_or_default();
            if !rejected.is_empty() {
                println!();
                for r in rejected.iter().take(8) {
                    println!("{}{}: {}", dim("excluded "), r["display_name"].as_str().unwrap_or_default(), r["reason"].as_str().unwrap_or_default());
                }
            }
            Ok(())
        }
        Command::Run { prompt, model, preset, task, system, verbose } => {
            let api = api::connect(cli.no_start).await?;
            let body = request_body(prompt_text(prompt)?, model, preset, task, system, !json_out);
            if json_out {
                let r = api.post("/v1/responses", &body).await?;
                return print_json(&r);
            }
            stream_run(&api, &body, verbose).await
        }
        Command::Keys { action } => {
            let api = api::connect(cli.no_start).await?;
            match action {
                KeysAction::List => {
                    let v = api.get("/v1/keys").await?;
                    if json_out {
                        return print_json(&v);
                    }
                    let rows: Vec<Vec<String>> = v["keys"]
                        .as_array()
                        .cloned()
                        .unwrap_or_default()
                        .iter()
                        .map(|k| {
                            vec![
                                k["id"].as_str().unwrap_or_default().to_string(),
                                k["name"].as_str().unwrap_or_default().to_string(),
                                format!("{}…", k["prefix"].as_str().unwrap_or_default()),
                                k["scopes"].as_array().map(|s| s.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(",")).unwrap_or_default(),
                                k["last_used_at"].as_str().map(relative_time).unwrap_or_else(|| "never".into()),
                                if k["revoked_at"].is_null() { "active".into() } else { "revoked".into() },
                            ]
                        })
                        .collect();
                    table(&["ID", "NAME", "KEY", "SCOPES", "LAST USED", "STATE"], &rows);
                }
                KeysAction::Create { name, scopes } => {
                    let v = api.post("/v1/keys", &json!({"name": name, "scopes": scopes})).await?;
                    if json_out {
                        return print_json(&v);
                    }
                    println!("{}", v["token"].as_str().unwrap_or_default());
                    eprintln!("{}", dim("Store this key now; it cannot be shown again."));
                }
                KeysAction::Revoke { id } => {
                    api.delete(&format!("/v1/keys/{id}")).await?;
                    println!("Revoked {id}");
                }
            }
            Ok(())
        }
        Command::Env => {
            let api = api::connect(cli.no_start).await?;
            println!("export OPENAI_BASE_URL={}/v1", api.url);
            println!("export OPENAI_API_BASE={}/v1", api.url);
            println!("export MAGPIE_URL={}", api.url);
            eprintln!("{}", dim("Create a key with `magpie keys create <name>` and set OPENAI_API_KEY / MAGPIE_API_KEY to it."));
            Ok(())
        }
        Command::Mcp => {
            let api = api::connect(cli.no_start).await?;
            mcp::serve(api).await
        }
    }
}

async fn stream_run(api: &api::Api, body: &Value, verbose: bool) -> Result<()> {
    let resp = api.request(reqwest::Method::POST, "/v1/responses").json(body).timeout(Duration::from_secs(3600)).send().await?;
    if !resp.status().is_success() {
        let v: Value = resp.json().await.unwrap_or(Value::Null);
        bail!("{}", v["error"]["message"].as_str().unwrap_or("request failed"));
    }
    let mut stream = resp.bytes_stream();
    let mut buf = String::new();
    let mut out = std::io::stdout();
    let mut ended_with_newline = true;
    while let Some(chunk) = stream.next().await {
        buf.push_str(&String::from_utf8_lossy(&chunk?));
        while let Some(pos) = buf.find("\n\n") {
            let block: String = buf.drain(..pos + 2).collect();
            let data: String = block.lines().filter_map(|l| l.strip_prefix("data:")).map(|l| l.trim_start()).collect::<Vec<_>>().join("\n");
            if data.is_empty() {
                continue;
            }
            let ev: Value = match serde_json::from_str(&data) {
                Ok(v) => v,
                Err(_) => continue,
            };
            match ev["type"].as_str().unwrap_or_default() {
                "started" => {
                    if verbose {
                        eprintln!("{}", dim(&format!("→ {} · {}", ev["model"]["display_name"].as_str().unwrap_or_default(), ev["task"].as_str().unwrap_or_default().replace('_', " "))));
                        for r in ev["reasons"].as_array().cloned().unwrap_or_default() {
                            eprintln!("{}", dim(&format!("  - {}", r.as_str().unwrap_or_default())));
                        }
                    }
                }
                "routing_changed" => {
                    eprintln!("{}", dim(&format!("↳ switched to {}: {}", ev["to"]["display_name"].as_str().unwrap_or_default(), ev["reason"].as_str().unwrap_or_default())));
                }
                "text_delta" => {
                    let t = ev["text"].as_str().unwrap_or_default();
                    out.write_all(t.as_bytes())?;
                    out.flush()?;
                    ended_with_newline = t.ends_with('\n');
                }
                "tool_call" => {
                    eprintln!("{}", dim(&format!("[tool call] {} {}", ev["call"]["name"].as_str().unwrap_or_default(), ev["call"]["arguments"].as_str().unwrap_or_default())));
                }
                "completed" => {
                    if !ended_with_newline {
                        println!();
                    }
                    if verbose {
                        let r = &ev["result"];
                        let cost = r["cost"]["usd"].as_f64().map(|c| format!(" · ${c:.4} {}", r["cost"]["provenance"].as_str().unwrap_or_default())).unwrap_or_default();
                        eprintln!(
                            "{}",
                            dim(&format!(
                                "{} · {} in / {} out ({}) · {:.1}s{cost}",
                                r["model"]["display_name"].as_str().unwrap_or_default(),
                                r["usage"]["input_tokens"].as_u64().map(compact).unwrap_or_else(|| "-".into()),
                                r["usage"]["output_tokens"].as_u64().map(compact).unwrap_or_else(|| "-".into()),
                                r["usage"]["provenance"].as_str().unwrap_or_default(),
                                r["duration_ms"].as_u64().unwrap_or(0) as f64 / 1000.0,
                            ))
                        );
                    }
                    return Ok(());
                }
                "failed" => {
                    if !ended_with_newline {
                        println!();
                    }
                    bail!("{}", ev["error"]["message"].as_str().unwrap_or("execution failed"));
                }
                _ => {}
            }
        }
    }
    bail!("connection closed before the execution completed")
}

fn print_json(v: &Value) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(v)?);
    Ok(())
}
