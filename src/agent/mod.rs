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
mod request;
mod session;
mod system;
mod ui;

pub use exec::ExecResult;
pub use pool::{Kind, SessionPool};
pub use proto::DisplayDto;
pub use session::{AgentSession, Auth};
pub use system::Platform;

/// Per-user secret every HTTP gateway caller must present. Stored as an option
/// so Settings -> AI Agents can show it; never shipped with the build.
pub const OPTION_HTTP_TOKEN: &str = "agent-http-token";

/// The gateway token, generated on first use.
///
/// The gateway binds to loopback, but that only checks *where* a caller
/// connects from, and a web page in the operator's browser also connects from
/// loopback. This token is what actually separates a local agent from a web
/// page or another user's process on the same machine.
pub fn http_token() -> String {
    use hbb_common::config::Config;
    let existing = Config::get_option(OPTION_HTTP_TOKEN);
    if !existing.trim().is_empty() {
        return existing;
    }
    let token = uuid::Uuid::new_v4().simple().to_string();
    Config::set_option(OPTION_HTTP_TOKEN.to_owned(), token.clone());
    token
}

/// Auto-start the localhost HTTP gateway alongside the main UI when the
/// `agent-http-enabled` option is set (Settings -> AI Agents).
pub fn start_http_gateway_if_enabled() {
    use hbb_common::config::Config;
    // Generated up front so Settings -> AI Agents can show a token before the
    // gateway has ever run; the auto-started gateway has no console to print to.
    let token = http_token();
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
            if let Err(e) = http::serve(pool, port, token).await {
                hbb_common::log::error!("[agent] http gateway exited: {e}");
            }
        });
    });
}
