//! Localhost-only HTTP/1.1 JSON API (hand-rolled over tokio TcpListener; no
//! new dependencies). One request per connection, `Connection: close`.
//!
//! Routes:
//!   GET  /status                       -> { status, version, sessions }
//!   GET  /peers                        -> [ { id, alias, hostname, ... } ]
//!   POST /screenshot  { peer, display? }            -> image/png bytes
//!   POST /input/mouse { peer, action, x, y, ... }   -> { ok }
//!   POST /input/key   { peer, text? | key?, ... }   -> { ok }
//!   POST /exec        { peer, command, timeout_secs? } -> { stdout, exit_code, timed_out }
//!   POST /files/upload   { peer, local_path, remote_path } -> { ok }
//!   POST /files/download { peer, remote_path, local_path } -> { ok }
//!   POST /clipboard   { peer, action: "get"|"set", text? } -> { ok | text }

use std::sync::Arc;
use std::time::Duration;

use hbb_common::{
    bail, log,
    tokio::{
        self,
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
    },
    ResultType,
};
use serde_json::{json, Value};

use super::proto::{self, ClipboardReq, ExecReq, KeyReq, MouseReq, ScreenshotReq, TransferReq};
use super::{Kind, SessionPool};

const MAX_HEADER: usize = 64 * 1024;
const MAX_BODY: usize = 32 * 1024 * 1024;
const DEFAULT_EXEC_TIMEOUT_SECS: u64 = 60;
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(3600);

pub async fn serve(pool: Arc<SessionPool>, port: u16) -> ResultType<()> {
    let listener = TcpListener::bind(("127.0.0.1", port)).await?;
    log::info!("[agent] HTTP gateway listening on http://127.0.0.1:{port}");
    println!("Medusa Desk agent gateway listening on http://127.0.0.1:{port}");
    loop {
        let (stream, addr) = listener.accept().await?;
        if !addr.ip().is_loopback() {
            continue;
        }
        let pool = pool.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_conn(stream, pool).await {
                log::debug!("[agent] http conn error: {e}");
            }
        });
    }
}

async fn handle_conn(mut stream: TcpStream, pool: Arc<SessionPool>) -> ResultType<()> {
    let (method, path, body) = match read_request(&mut stream).await {
        Ok(r) => r,
        Err(e) => {
            write_json(&mut stream, 400, &json!({ "error": e.to_string() })).await?;
            return Ok(());
        }
    };

    match route(&method, &path, body, &pool).await {
        Ok(Reply::Json(v)) => write_json(&mut stream, 200, &v).await?,
        Ok(Reply::Png(bytes)) => write_raw(&mut stream, 200, "image/png", &bytes).await?,
        Err(e) => write_json(&mut stream, 500, &json!({ "error": e.to_string() })).await?,
    }
    Ok(())
}

enum Reply {
    Json(Value),
    Png(bytes::Bytes),
}

async fn read_request(stream: &mut TcpStream) -> ResultType<(String, String, Vec<u8>)> {
    let mut buf = Vec::with_capacity(4096);
    let header_end;
    loop {
        let mut chunk = [0u8; 4096];
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            bail!("connection closed mid-request");
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(pos) = find_header_end(&buf) {
            header_end = pos;
            break;
        }
        if buf.len() > MAX_HEADER {
            bail!("request header too large");
        }
    }

    let header_text = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let mut lines = header_text.split("\r\n");
    let request_line = lines.next().unwrap_or_default();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_owned();
    let path = parts
        .next()
        .unwrap_or_default()
        .split('?')
        .next()
        .unwrap_or_default()
        .to_owned();

    let mut content_length = 0usize;
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse().unwrap_or(0);
            }
        }
    }
    if content_length > MAX_BODY {
        bail!("request body too large");
    }

    let mut body = buf[header_end + 4..].to_vec();
    while body.len() < content_length {
        let mut chunk = vec![0u8; (content_length - body.len()).min(64 * 1024)];
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            bail!("connection closed mid-body");
        }
        body.extend_from_slice(&chunk[..n]);
    }
    body.truncate(content_length);
    Ok((method, path, body))
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

fn parse_body<T: serde::de::DeserializeOwned>(body: &[u8]) -> ResultType<T> {
    Ok(serde_json::from_slice(body)?)
}

async fn route(
    method: &str,
    path: &str,
    body: Vec<u8>,
    pool: &Arc<SessionPool>,
) -> ResultType<Reply> {
    match (method, path) {
        ("GET", "/status") => {
            let sessions: Vec<Value> = pool
                .status()
                .await
                .into_iter()
                .map(|(peer, kind, alive)| json!({ "peer": peer, "kind": kind, "alive": alive }))
                .collect();
            Ok(Reply::Json(json!({
                "status": "ok",
                "version": crate::VERSION,
                "sessions": sessions,
            })))
        }
        ("GET", "/peers") => Ok(Reply::Json(serde_json::to_value(proto::list_peers())?)),
        ("POST", "/screenshot") => {
            let req: ScreenshotReq = parse_body(&body)?;
            let sess = pool.get(&req.peer, Kind::Control).await?;
            let png = sess.screenshot(req.display).await?;
            Ok(Reply::Png(png))
        }
        ("POST", "/input/mouse") => {
            let req: MouseReq = parse_body(&body)?;
            let sess = pool.get(&req.peer, Kind::Control).await?;
            match req.action.as_str() {
                "move" => sess.move_mouse(req.x, req.y),
                "scroll" => sess.scroll(req.dx, req.dy),
                _ => {
                    let button = proto::parse_button(&req.button)
                        .map_err(|e| hbb_common::anyhow::anyhow!(e))?;
                    sess.click(req.x, req.y, button, req.double).await;
                }
            }
            Ok(Reply::Json(json!({ "ok": true })))
        }
        ("POST", "/input/key") => {
            let req: KeyReq = parse_body(&body)?;
            let sess = pool.get(&req.peer, Kind::Control).await?;
            if let Some(text) = req.text.as_deref() {
                sess.type_text(text);
            } else if let Some(key) = req.key.as_deref() {
                let key = proto::normalize_key(key);
                sess.key(&key, req.alt, req.ctrl, req.shift, req.command);
            } else {
                bail!("provide either 'text' or 'key'");
            }
            Ok(Reply::Json(json!({ "ok": true })))
        }
        ("POST", "/exec") => {
            let req: ExecReq = parse_body(&body)?;
            let sess = pool.get(&req.peer, Kind::Terminal).await?;
            let res = sess
                .exec(
                    &req.command,
                    Duration::from_secs(req.timeout_secs.unwrap_or(DEFAULT_EXEC_TIMEOUT_SECS)),
                )
                .await?;
            Ok(Reply::Json(serde_json::to_value(&res)?))
        }
        ("POST", "/files/upload") | ("POST", "/files/download") => {
            let upload = path == "/files/upload";
            let req: TransferReq = parse_body(&body)?;
            let sess = pool.get(&req.peer, Kind::FileTransfer).await?;
            sess.transfer(&req.local_path, &req.remote_path, upload, TRANSFER_TIMEOUT)
                .await?;
            Ok(Reply::Json(json!({ "ok": true })))
        }
        ("POST", "/clipboard") => {
            let req: ClipboardReq = parse_body(&body)?;
            let sess = pool.get(&req.peer, Kind::Control).await?;
            if req.action == "get" {
                let text = sess.clipboard_get()?;
                Ok(Reply::Json(json!({ "text": text })))
            } else {
                sess.clipboard_set(&req.text);
                Ok(Reply::Json(json!({ "ok": true })))
            }
        }
        _ => bail!("no such endpoint: {method} {path}"),
    }
}

async fn write_json(stream: &mut TcpStream, status: u16, value: &Value) -> ResultType<()> {
    write_raw(stream, status, "application/json", value.to_string().as_bytes()).await
}

async fn write_raw(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> ResultType<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        _ => "Internal Server Error",
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(body).await?;
    stream.flush().await?;
    Ok(())
}
