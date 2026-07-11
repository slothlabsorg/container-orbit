//! A minimal MCP (Model Context Protocol) server over stdio, so AI assistants
//! can drive orbit in plain language.
//!
//! MCP's stdio transport is newline-delimited JSON-RPC 2.0. We keep stdout
//! exclusively for protocol messages and route each tool call to the orbit CLI
//! as a subprocess (capturing its output) — that way the human-facing `println!`
//! output of the commands never corrupts the JSON channel.

use anyhow::Result;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

const PROTOCOL_VERSION: &str = "2024-11-05";

const INSTRUCTIONS: &str = "\
orbit delegates the user's local Docker to a beefier machine on their LAN over SSH, \
and forwards published container ports back to their localhost so it feels local.

Typical flow:
- `status` — is orbit linked/up? what's forwarded? remote CPU/RAM?
- `up` / `down` — start/stop delegating Docker to the host.
- `link` — link to a host (needs the SSH key already authorized; otherwise tell the \
user to run `orbit setup` in a terminal — it's interactive).
- `add_forward` / `remove_forward` / `list_forwards` — manage TCP port tunnels.
- `doctor` — diagnose SSH, the remote daemon, the forwarded socket and the docker context.

For first-time setup, host authorization, or anything interactive, tell the user to run \
`orbit setup` in their terminal — do not try to proxy interactive prompts.";

/// Serve MCP over stdio until stdin closes.
pub async fn serve() -> Result<()> {
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);
    let mut stdout = tokio::io::stdout();
    let mut line = String::new();

    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            break; // EOF
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let id = msg.get("id").cloned();
        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = msg.get("params").cloned().unwrap_or(Value::Null);

        // Notifications (no id) get no response.
        let Some(id) = id else {
            continue;
        };

        let response = match method {
            "initialize" => ok(id, initialize_result()),
            "tools/list" => ok(id, json!({ "tools": tool_specs() })),
            "tools/call" => match call_tool(&params).await {
                Ok(text) => ok(id, tool_text(&text, false)),
                Err(e) => ok(id, tool_text(&format!("{e:#}"), true)),
            },
            "ping" => ok(id, json!({})),
            _ => err(id, -32601, "method not found"),
        };

        let mut buf = serde_json::to_string(&response)?;
        buf.push('\n');
        stdout.write_all(buf.as_bytes()).await?;
        stdout.flush().await?;
    }
    Ok(())
}

fn initialize_result() -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": "orbit", "version": env!("CARGO_PKG_VERSION") },
        "instructions": INSTRUCTIONS,
    })
}

fn ok(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn err(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn tool_text(text: &str, is_error: bool) -> Value {
    json!({
        "content": [ { "type": "text", "text": text } ],
        "isError": is_error,
    })
}

fn no_args() -> Value {
    json!({ "type": "object", "properties": {}, "additionalProperties": false })
}

fn tool_specs() -> Value {
    json!([
        { "name": "status", "description": "Show orbit link, connection state, forwarded ports, and remote CPU/RAM/image counts.", "inputSchema": no_args() },
        { "name": "up", "description": "Delegate Docker to the linked host and start forwarding published ports. Detached.", "inputSchema": no_args() },
        { "name": "down", "description": "Stop forwarding, close the SSH connection, and restore the previous docker context.", "inputSchema": no_args() },
        { "name": "doctor", "description": "Diagnose SSH, the remote docker daemon, the forwarded socket, and the docker context.", "inputSchema": no_args() },
        { "name": "list_forwards", "description": "List the ports currently forwarded from the host to localhost.", "inputSchema": no_args() },
        {
            "name": "link",
            "description": "Link this machine to a host. The SSH key must already be authorized (else tell the user to run `orbit setup`).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "target": { "type": "string", "description": "user@host or just host" },
                    "port": { "type": "integer", "description": "SSH port (default 22)" }
                },
                "required": ["target"]
            }
        },
        {
            "name": "add_forward",
            "description": "Forward an extra TCP port from the host to localhost.",
            "inputSchema": { "type": "object", "properties": { "port": { "type": "integer" } }, "required": ["port"] }
        },
        {
            "name": "remove_forward",
            "description": "Stop forwarding a previously added TCP port.",
            "inputSchema": { "type": "object", "properties": { "port": { "type": "integer" } }, "required": ["port"] }
        },
        {
            "name": "setup_hint",
            "description": "How to run first-time interactive setup. Returns the exact command for the user to run in a terminal.",
            "inputSchema": no_args()
        }
    ])
}

/// Dispatch a tool call to the orbit CLI subprocess and return its output.
async fn call_tool(params: &Value) -> Result<String> {
    let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or(Value::Null);

    let argv: Vec<String> = match name {
        "status" => vec!["status".into()],
        "up" => vec!["up".into()],
        "down" => vec!["down".into()],
        "doctor" => vec!["doctor".into()],
        "list_forwards" => vec!["ports".into()],
        "link" => {
            let target = args
                .get("target")
                .and_then(|t| t.as_str())
                .ok_or_else(|| anyhow::anyhow!("link requires a 'target'"))?;
            let mut v = vec!["link".to_string(), target.to_string()];
            if let Some(p) = args.get("port").and_then(|p| p.as_u64()) {
                v.push("--port".into());
                v.push(p.to_string());
            }
            v
        }
        "add_forward" => {
            let port = args
                .get("port")
                .and_then(|p| p.as_u64())
                .ok_or_else(|| anyhow::anyhow!("add_forward requires a 'port'"))?;
            vec!["ports".into(), "add".into(), port.to_string()]
        }
        "remove_forward" => {
            let port = args
                .get("port")
                .and_then(|p| p.as_u64())
                .ok_or_else(|| anyhow::anyhow!("remove_forward requires a 'port'"))?;
            vec!["ports".into(), "rm".into(), port.to_string()]
        }
        "setup_hint" => {
            return Ok("Run `orbit setup` in a terminal. It's interactive: it discovers \
                the host on your LAN, authorizes the SSH key (asking for the host password \
                once if needed), links, and runs an end-to-end self-test."
                .to_string())
        }
        other => anyhow::bail!("unknown tool: {other}"),
    };

    run_orbit(&argv).await
}

/// Invoke this same binary with the given args, capturing combined output.
async fn run_orbit(args: &[String]) -> Result<String> {
    let exe = std::env::current_exe()?;
    let out = tokio::process::Command::new(exe)
        .args(args)
        .output()
        .await?;
    let mut text = String::new();
    text.push_str(&strip_ansi(&String::from_utf8_lossy(&out.stdout)));
    let errs = strip_ansi(&String::from_utf8_lossy(&out.stderr));
    if !errs.trim().is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&errs);
    }
    if !out.status.success() {
        anyhow::bail!("{}", text.trim());
    }
    Ok(text.trim().to_string())
}

/// Strip ANSI color codes so the AI sees clean text.
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            // skip until a letter (end of CSI sequence)
            for n in chars.by_ref() {
                if n.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}
