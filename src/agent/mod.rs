//! AI Agent Gateway.
//!
//! Lets headless AI agents (Claude Code, Cursor, scripts over SSH) drive remote
//! MedusaDesk peers: screenshots, mouse/keyboard input, command execution and
//! file transfer. Three interfaces share one session core:
//!   - `medusadesk agent <verb>`  : one-shot CLI commands (JSON on stdout)
//!   - `medusadesk agent mcp`     : Model Context Protocol server on stdio
//!   - `medusadesk agent serve`   : localhost HTTP JSON API
//!
//! Authentication reuses the normal peer auth flow: saved peer passwords,
//! `--password`, and TOTP 2FA via `--2fa-secret`.

pub mod cli;
mod exec;
mod http;
mod mcp;
mod pool;
mod proto;
mod session;
mod ui;

pub use exec::ExecResult;
pub use pool::{Kind, SessionPool};
pub use session::{AgentSession, Auth};

/// Auto-start the localhost HTTP gateway alongside the main UI when the
/// `agent-http-enabled` option is set (Settings -> AI Agents).
pub fn start_http_gateway_if_enabled() {
    use hbb_common::config::Config;
    if Config::get_option("agent-http-enabled") != "Y" {
        return;
    }
    let port = Config::get_option("agent-http-port")
        .parse::<u16>()
        .unwrap_or(cli::DEFAULT_HTTP_PORT);
    std::thread::spawn(move || {
        let rt = match hbb_common::tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                hbb_common::log::error!("[agent] failed to build gateway runtime: {e}");
                return;
            }
        };
        rt.block_on(async move {
            let pool = SessionPool::new(Auth::default(), std::time::Duration::from_secs(300));
            if let Err(e) = http::serve(pool, port).await {
                hbb_common::log::error!("[agent] http gateway exited: {e}");
            }
        });
    });
}
