//! Remote command execution over the terminal (PTY) channel.
//!
//! The remote terminal is a real interactive shell (PowerShell/cmd on Windows,
//! sh/bash/zsh elsewhere), so completion is detected with a nonce sentinel that
//! echoes the command's exit code. Two sentinel dialects are sent on Windows
//! (cmd first, then PowerShell) because the remote shell flavor is unknown;
//! whichever expands to digits first wins. Echoed sentinel lines never match
//! because the un-expanded variable (`%errorlevel%`, `$?`...) is not numeric.

use std::time::Duration;

use hbb_common::{
    bail,
    tokio::time::{timeout, Instant},
    ResultType,
};
use serde::Serialize;

use super::session::AgentSession;
use super::ui::TermEvent;

const OPEN_TIMEOUT: Duration = Duration::from_secs(20);
/// Wait for the shell prompt/banner to go quiet before sending the command.
const QUIET_WINDOW: Duration = Duration::from_millis(500);
const MAX_BANNER_WAIT: Duration = Duration::from_secs(4);

#[derive(Debug, Serialize)]
pub struct ExecResult {
    pub stdout: String,
    pub exit_code: Option<i64>,
    pub timed_out: bool,
}

pub async fn exec(
    sess: &AgentSession,
    cmd: &str,
    run_timeout: Duration,
) -> ResultType<ExecResult> {
    let (tid, mut rx) = sess.register_terminal();
    let result = exec_inner(sess, tid, &mut rx, cmd, run_timeout).await;
    sess.close_terminal(tid);
    sess.unregister_terminal(tid);
    result
}

async fn exec_inner(
    sess: &AgentSession,
    tid: i32,
    rx: &mut hbb_common::tokio::sync::mpsc::UnboundedReceiver<TermEvent>,
    cmd: &str,
    run_timeout: Duration,
) -> ResultType<ExecResult> {
    sess.open_terminal(tid, 40, 200);

    // Wait for the open ack.
    let open_deadline = Instant::now() + OPEN_TIMEOUT;
    loop {
        let remaining = open_deadline.saturating_duration_since(Instant::now());
        match timeout(remaining, rx.recv()).await {
            Ok(Some(TermEvent::Opened {
                success, message, ..
            })) => {
                if !success {
                    bail!("Failed to open remote terminal: {message}");
                }
                break;
            }
            Ok(Some(TermEvent::Error { message, .. })) => {
                bail!("Remote terminal error: {message}")
            }
            Ok(Some(_)) => {}
            Ok(None) => bail!("Terminal channel closed"),
            Err(_) => bail!("Timed out opening remote terminal"),
        }
    }

    // Drain the shell banner/prompt until it goes quiet.
    let banner_deadline = Instant::now() + MAX_BANNER_WAIT;
    while Instant::now() < banner_deadline {
        match timeout(QUIET_WINDOW, rx.recv()).await {
            Ok(Some(TermEvent::Data(_))) => {}
            Ok(Some(TermEvent::Error { message, .. })) => bail!("Remote terminal error: {message}"),
            Ok(Some(_)) => {}
            Ok(None) => bail!("Terminal channel closed"),
            Err(_) => break, // quiet — shell is ready
        }
    }

    let nonce = uuid::Uuid::new_v4().simple().to_string();
    let is_windows = sess.platform() == "Windows";
    let nl = if is_windows { "\r\n" } else { "\n" };

    sess.send_terminal_input(tid, format!("{cmd}{nl}"));
    if is_windows {
        // cmd.exe dialect (PowerShell prints the %var% literally -> no match).
        sess.send_terminal_input(tid, format!("echo __MD1_{nonce}_%errorlevel%__{nl}"));
        // PowerShell dialect (cmd.exe errors out, but only after MD1 matched).
        sess.send_terminal_input(
            tid,
            format!(
                "Write-Output (\"__MD2_{nonce}_\" + $(if ($null -ne $LASTEXITCODE) {{ $LASTEXITCODE }} elseif ($?) {{ 0 }} else {{ 1 }}) + \"__\"){nl}"
            ),
        );
    } else {
        sess.send_terminal_input(tid, format!("printf '__MD1_{nonce}_%s__\\n' \"$?\"{nl}"));
    }

    let mut raw = Vec::<u8>::new();
    let deadline = Instant::now() + run_timeout;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            let text = strip_ansi(&String::from_utf8_lossy(&raw));
            return Ok(ExecResult {
                stdout: scrub(&text, cmd),
                exit_code: None,
                timed_out: true,
            });
        }
        match timeout(remaining, rx.recv()).await {
            Ok(Some(TermEvent::Data(d))) => {
                raw.extend_from_slice(&d);
                let text = strip_ansi(&String::from_utf8_lossy(&raw));
                if let Some((before, code)) = find_marker(&text, &nonce) {
                    return Ok(ExecResult {
                        stdout: scrub(before, cmd),
                        exit_code: Some(code),
                        timed_out: false,
                    });
                }
            }
            Ok(Some(TermEvent::Closed { exit_code, .. })) => {
                let text = strip_ansi(&String::from_utf8_lossy(&raw));
                return Ok(ExecResult {
                    stdout: scrub(&text, cmd),
                    exit_code: Some(exit_code as i64),
                    timed_out: false,
                });
            }
            Ok(Some(TermEvent::Error { message, .. })) => bail!("Remote terminal error: {message}"),
            Ok(Some(_)) => {}
            Ok(None) => bail!("Terminal channel closed"),
            Err(_) => {} // loop re-checks the deadline
        }
    }
}

/// Strip ANSI escape sequences (CSI, OSC) and carriage returns.
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            match chars.peek() {
                Some('[') => {
                    chars.next();
                    // CSI: consume until a final byte in '@'..='~'.
                    while let Some(&n) = chars.peek() {
                        chars.next();
                        if ('@'..='~').contains(&n) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    chars.next();
                    // OSC: consume until BEL or ESC \.
                    while let Some(n) = chars.next() {
                        if n == '\x07' {
                            break;
                        }
                        if n == '\x1b' {
                            if chars.peek() == Some(&'\\') {
                                chars.next();
                            }
                            break;
                        }
                    }
                }
                _ => {
                    chars.next();
                }
            }
        } else if c != '\r' {
            out.push(c);
        }
    }
    out
}

/// Find the first *expanded* sentinel (`__MD1_<nonce>_<digits>__` or MD2)
/// and return the text before it plus the parsed exit code. Echoed sentinel
/// commands contain a non-numeric variable there and are skipped.
fn find_marker<'a>(text: &'a str, nonce: &str) -> Option<(&'a str, i64)> {
    let mut best: Option<(usize, i64)> = None;
    for tag in ["__MD1_", "__MD2_"] {
        let prefix = format!("{tag}{nonce}_");
        let mut start = 0;
        while let Some(pos) = text[start..].find(&prefix) {
            let abs = start + pos;
            let rest = &text[abs + prefix.len()..];
            if let Some(end) = rest.find("__") {
                let code_str = &rest[..end];
                if !code_str.is_empty()
                    && code_str
                        .chars()
                        .enumerate()
                        .all(|(i, c)| c.is_ascii_digit() || (i == 0 && c == '-'))
                {
                    if let Ok(code) = code_str.parse::<i64>() {
                        if best.map(|(b, _)| abs < b).unwrap_or(true) {
                            best = Some((abs, code));
                        }
                        break;
                    }
                }
            }
            start = abs + prefix.len();
        }
    }
    best.map(|(pos, code)| (&text[..pos], code))
}

/// Remove sentinel command echoes and the echoed command line itself.
fn scrub(text: &str, cmd: &str) -> String {
    let first_cmd_line = cmd.lines().next().unwrap_or(cmd).trim();
    let mut cmd_echo_dropped = false;
    let mut lines: Vec<&str> = Vec::new();
    for line in text.lines() {
        if line.contains("__MD1_") || line.contains("__MD2_") {
            continue;
        }
        if !cmd_echo_dropped && !first_cmd_line.is_empty() && line.contains(first_cmd_line) {
            cmd_echo_dropped = true;
            continue;
        }
        lines.push(line);
    }
    let mut out = lines.join("\n");
    while out.starts_with('\n') {
        out.remove(0);
    }
    out.trim_end().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_ansi() {
        assert_eq!(strip_ansi("a\x1b[31mred\x1b[0mb"), "aredb");
        assert_eq!(strip_ansi("x\r\ny"), "x\ny");
        assert_eq!(strip_ansi("\x1b]0;title\x07text"), "text");
        assert_eq!(strip_ansi("\x1b[?25lhi\x1b[?25h"), "hi");
    }

    #[test]
    fn test_find_marker_skips_echo() {
        let nonce = "abc";
        // Echo of the cmd.exe sentinel (unexpanded) followed by the real output.
        let text = "echo __MD1_abc_%errorlevel%__\nhello\n__MD1_abc_0__\n";
        let (before, code) = find_marker(text, nonce).unwrap();
        assert_eq!(code, 0);
        assert!(before.contains("hello"));
    }

    #[test]
    fn test_find_marker_negative_code() {
        let (_, code) = find_marker("__MD1_n_-1__", "n").unwrap();
        assert_eq!(code, -1);
    }

    #[test]
    fn test_find_marker_powershell() {
        let nonce = "xyz";
        // PS echoes the Write-Output line (no digits), prints cmd literal for
        // the cmd dialect, then the expanded MD2 marker.
        let text = "Write-Output (\"__MD2_xyz_\" + ...)\n__MD1_xyz_%errorlevel%__\nout\n__MD2_xyz_42__\n";
        let (before, code) = find_marker(text, nonce).unwrap();
        assert_eq!(code, 42);
        assert!(before.contains("out"));
    }

    #[test]
    fn test_scrub() {
        let text = "C:\\> dir /b\nfile.txt\necho __MD1_n_%errorlevel%__\n";
        let out = scrub(text, "dir /b");
        assert_eq!(out, "file.txt");
    }
}
