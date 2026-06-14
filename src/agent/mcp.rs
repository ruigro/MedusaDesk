//! Model Context Protocol server over stdio (newline-delimited JSON-RPC 2.0).
//!
//! Register with an MCP-capable agent, e.g.:
//!   claude mcp add medusadesk -- medusadesk agent mcp
//! Tools take a `peer` argument; sessions are pooled per (peer, conn-type).

use std::sync::Arc;
use std::time::Duration;

use hbb_common::tokio;
use serde_json::{json, Value};

use super::proto;
use super::{Kind, SessionPool};

const PROTOCOL_VERSION: &str = "2024-11-05";
const DEFAULT_EXEC_TIMEOUT_SECS: u64 = 60;
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(3600);

pub async fn serve(pool: Arc<SessionPool>) {
    let stdin = std::io::stdin();
    loop {
        let mut line = String::new();
        let read = tokio::task::block_in_place(|| stdin.read_line(&mut line));
        match read {
            Ok(0) => break, // EOF: client disconnected
            Ok(_) => {}
            Err(_) => break,
        }
        let line = line.trim_start_matches('\u{feff}').trim();
        if line.is_empty() {
            continue;
        }
        if let Some(resp) = handle_line(line, &pool).await {
            use std::io::Write;
            let mut stdout = std::io::stdout();
            if writeln!(stdout, "{}", resp).and_then(|_| stdout.flush()).is_err() {
                break;
            }
        }
    }
}

async fn handle_line(line: &str, pool: &Arc<SessionPool>) -> Option<Value> {
    let req: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => {
            return Some(json!({
                "jsonrpc": "2.0", "id": null,
                "error": { "code": -32700, "message": format!("parse error: {e}") }
            }))
        }
    };
    let id = req.get("id").cloned();
    let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let is_notification = id.is_none() || id == Some(Value::Null);

    let result = match method {
        "initialize" => json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "medusadesk-agent", "version": crate::VERSION }
        }),
        "notifications/initialized" | "notifications/cancelled" => return None,
        "ping" => json!({}),
        "tools/list" => json!({ "tools": tool_definitions() }),
        "tools/call" => {
            let name = req
                .pointer("/params/name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let args = req
                .pointer("/params/arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            match call_tool(name, args, pool).await {
                Ok(content) => json!({ "content": content }),
                Err(e) => json!({
                    "content": [ { "type": "text", "text": e } ],
                    "isError": true
                }),
            }
        }
        _ => {
            if is_notification {
                return None;
            }
            return Some(json!({
                "jsonrpc": "2.0", "id": id,
                "error": { "code": -32601, "message": format!("method not found: {method}") }
            }));
        }
    };

    if is_notification {
        return None;
    }
    Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

fn peer_prop() -> Value {
    json!({ "type": "string", "description": "Remote peer ID (see medusa_list_peers)" })
}

fn tool_definitions() -> Value {
    json!([
        {
            "name": "medusa_list_peers",
            "description": "List saved MedusaDesk peers (remote machines) with their IDs, hostnames and platforms.",
            "inputSchema": { "type": "object", "properties": {}, "required": [] }
        },
        {
            "name": "medusa_screenshot",
            "description": "Capture the remote machine's screen and return it as a PNG image.",
            "inputSchema": { "type": "object", "properties": {
                "peer": peer_prop(),
                "display": { "type": "integer", "description": "Display index, default 0" }
            }, "required": ["peer"] }
        },
        {
            "name": "medusa_click",
            "description": "Click the mouse at absolute screen coordinates on the remote machine.",
            "inputSchema": { "type": "object", "properties": {
                "peer": peer_prop(),
                "x": { "type": "integer" },
                "y": { "type": "integer" },
                "button": { "type": "string", "enum": ["left", "right", "middle"], "description": "Default left" },
                "double": { "type": "boolean", "description": "Double-click" }
            }, "required": ["peer", "x", "y"] }
        },
        {
            "name": "medusa_move_mouse",
            "description": "Move the mouse cursor on the remote machine without clicking.",
            "inputSchema": { "type": "object", "properties": {
                "peer": peer_prop(), "x": { "type": "integer" }, "y": { "type": "integer" }
            }, "required": ["peer", "x", "y"] }
        },
        {
            "name": "medusa_scroll",
            "description": "Scroll the mouse wheel on the remote machine.",
            "inputSchema": { "type": "object", "properties": {
                "peer": peer_prop(),
                "dx": { "type": "integer", "description": "Horizontal scroll delta" },
                "dy": { "type": "integer", "description": "Vertical scroll delta (positive scrolls up)" }
            }, "required": ["peer"] }
        },
        {
            "name": "medusa_type",
            "description": "Type a UTF-8 text string on the remote machine (goes to the focused control).",
            "inputSchema": { "type": "object", "properties": {
                "peer": peer_prop(), "text": { "type": "string" }
            }, "required": ["peer", "text"] }
        },
        {
            "name": "medusa_key",
            "description": "Press a single key, optionally with modifiers. Key names: enter, tab, escape, backspace, delete, home, end, pageup, pagedown, up, down, left, right, space, f1-f12, or any single character.",
            "inputSchema": { "type": "object", "properties": {
                "peer": peer_prop(),
                "key": { "type": "string" },
                "ctrl": { "type": "boolean" }, "alt": { "type": "boolean" },
                "shift": { "type": "boolean" }, "command": { "type": "boolean" }
            }, "required": ["peer", "key"] }
        },
        {
            "name": "medusa_exec",
            "description": "Run a shell command on the remote machine and return its output and exit code. Uses the remote's default shell (PowerShell/cmd on Windows, sh/bash elsewhere).",
            "inputSchema": { "type": "object", "properties": {
                "peer": peer_prop(),
                "command": { "type": "string" },
                "timeout_secs": { "type": "integer", "description": "Default 60" }
            }, "required": ["peer", "command"] }
        },
        {
            "name": "medusa_upload",
            "description": "Upload a local file or directory to the remote machine.",
            "inputSchema": { "type": "object", "properties": {
                "peer": peer_prop(),
                "local_path": { "type": "string" },
                "remote_path": { "type": "string" }
            }, "required": ["peer", "local_path", "remote_path"] }
        },
        {
            "name": "medusa_download",
            "description": "Download a file or directory from the remote machine.",
            "inputSchema": { "type": "object", "properties": {
                "peer": peer_prop(),
                "remote_path": { "type": "string" },
                "local_path": { "type": "string" }
            }, "required": ["peer", "remote_path", "local_path"] }
        },
        {
            "name": "medusa_clipboard_set",
            "description": "Set the clipboard text on the remote machine.",
            "inputSchema": { "type": "object", "properties": {
                "peer": peer_prop(), "text": { "type": "string" }
            }, "required": ["peer", "text"] }
        },
        {
            "name": "medusa_clipboard_get",
            "description": "Read clipboard text (local side, mirrors the remote when clipboard sync is enabled).",
            "inputSchema": { "type": "object", "properties": {
                "peer": peer_prop()
            }, "required": ["peer"] }
        }
    ])
}

fn arg_str(args: &Value, key: &str) -> String {
    args.get(key).and_then(|v| v.as_str()).unwrap_or("").to_owned()
}

fn arg_i32(args: &Value, key: &str) -> i32 {
    args.get(key).and_then(|v| v.as_i64()).unwrap_or(0) as i32
}

fn arg_bool(args: &Value, key: &str) -> bool {
    args.get(key).and_then(|v| v.as_bool()).unwrap_or(false)
}

fn text_content(value: Value) -> Vec<Value> {
    vec![json!({ "type": "text", "text": value.to_string() })]
}

async fn call_tool(
    name: &str,
    args: Value,
    pool: &Arc<SessionPool>,
) -> Result<Vec<Value>, String> {
    let peer = arg_str(&args, "peer");
    if name != "medusa_list_peers" && peer.is_empty() {
        return Err("missing required argument: peer".to_owned());
    }
    match name {
        "medusa_list_peers" => {
            let peers = serde_json::to_value(proto::list_peers()).map_err(|e| e.to_string())?;
            Ok(text_content(peers))
        }
        "medusa_screenshot" => {
            let sess = pool
                .get(&peer, Kind::Control)
                .await
                .map_err(|e| e.to_string())?;
            let png = sess
                .screenshot(arg_i32(&args, "display"))
                .await
                .map_err(|e| e.to_string())?;
            Ok(vec![json!({
                "type": "image",
                "data": crate::encode64(&png),
                "mimeType": "image/png"
            })])
        }
        "medusa_click" => {
            let button = proto::parse_button(&arg_str(&args, "button"))?;
            let sess = pool
                .get(&peer, Kind::Control)
                .await
                .map_err(|e| e.to_string())?;
            sess.click(
                arg_i32(&args, "x"),
                arg_i32(&args, "y"),
                button,
                arg_bool(&args, "double"),
            )
            .await;
            Ok(text_content(json!({ "ok": true })))
        }
        "medusa_move_mouse" => {
            let sess = pool
                .get(&peer, Kind::Control)
                .await
                .map_err(|e| e.to_string())?;
            sess.move_mouse(arg_i32(&args, "x"), arg_i32(&args, "y"));
            Ok(text_content(json!({ "ok": true })))
        }
        "medusa_scroll" => {
            let sess = pool
                .get(&peer, Kind::Control)
                .await
                .map_err(|e| e.to_string())?;
            sess.scroll(arg_i32(&args, "dx"), arg_i32(&args, "dy"));
            Ok(text_content(json!({ "ok": true })))
        }
        "medusa_type" => {
            let sess = pool
                .get(&peer, Kind::Control)
                .await
                .map_err(|e| e.to_string())?;
            sess.type_text(&arg_str(&args, "text"));
            Ok(text_content(json!({ "ok": true })))
        }
        "medusa_key" => {
            let sess = pool
                .get(&peer, Kind::Control)
                .await
                .map_err(|e| e.to_string())?;
            let key = proto::normalize_key(&arg_str(&args, "key"));
            sess.key(
                &key,
                arg_bool(&args, "alt"),
                arg_bool(&args, "ctrl"),
                arg_bool(&args, "shift"),
                arg_bool(&args, "command"),
            );
            Ok(text_content(json!({ "ok": true })))
        }
        "medusa_exec" => {
            let sess = pool
                .get(&peer, Kind::Terminal)
                .await
                .map_err(|e| e.to_string())?;
            let secs = args
                .get("timeout_secs")
                .and_then(|v| v.as_u64())
                .unwrap_or(DEFAULT_EXEC_TIMEOUT_SECS);
            let res = sess
                .exec(&arg_str(&args, "command"), Duration::from_secs(secs))
                .await
                .map_err(|e| e.to_string())?;
            Ok(text_content(
                serde_json::to_value(&res).map_err(|e| e.to_string())?,
            ))
        }
        "medusa_upload" | "medusa_download" => {
            let upload = name == "medusa_upload";
            let sess = pool
                .get(&peer, Kind::FileTransfer)
                .await
                .map_err(|e| e.to_string())?;
            sess.transfer(
                &arg_str(&args, "local_path"),
                &arg_str(&args, "remote_path"),
                upload,
                TRANSFER_TIMEOUT,
            )
            .await
            .map_err(|e| e.to_string())?;
            Ok(text_content(json!({ "ok": true })))
        }
        "medusa_clipboard_set" => {
            let sess = pool
                .get(&peer, Kind::Control)
                .await
                .map_err(|e| e.to_string())?;
            sess.clipboard_set(&arg_str(&args, "text"));
            Ok(text_content(json!({ "ok": true })))
        }
        "medusa_clipboard_get" => {
            let sess = pool
                .get(&peer, Kind::Control)
                .await
                .map_err(|e| e.to_string())?;
            let text = sess.clipboard_get().map_err(|e| e.to_string())?;
            Ok(text_content(json!({ "text": text })))
        }
        other => Err(format!("unknown tool: {other}")),
    }
}
