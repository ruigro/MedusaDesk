//! `medusadesk agent <verb>` — one-shot CLI commands plus the `mcp` and
//! `serve` server modes. JSON results go to stdout; errors go to stderr as
//! JSON with a non-zero exit code, so shell scripts and AI agents over SSH can
//! consume everything mechanically.

use std::time::Duration;

use clap::{Arg, ArgAction, ArgMatches, Command};
use hbb_common::{config::Config, log, rendezvous_proto::ConnType, tokio, ResultType};
use serde_json::json;

use super::proto;
use super::session::{AgentSession, Auth};
use super::SessionPool;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(45);
const DEFAULT_EXEC_TIMEOUT_SECS: u64 = 60;
const DEFAULT_TRANSFER_TIMEOUT_SECS: u64 = 3600;
pub const DEFAULT_HTTP_PORT: u16 = 21120;
const POOL_IDLE: Duration = Duration::from_secs(300);

pub fn run(args: Vec<String>) {
    #[cfg(windows)]
    attach_console();
    use hbb_common::env_logger;
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "warn"),
    );
    let matches = match build_command()
        .try_get_matches_from(std::iter::once("agent".to_owned()).chain(args.into_iter()))
    {
        Ok(m) => m,
        Err(e) => {
            let _ = e.print();
            std::process::exit(2);
        }
    };
    let rt = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("{}", json!({ "error": format!("tokio runtime: {e}") }));
            std::process::exit(1);
        }
    };
    let code = rt.block_on(dispatch(matches));
    // Allow spawned io_loop threads to wind down.
    rt.shutdown_timeout(Duration::from_secs(2));
    std::process::exit(code);
}

#[cfg(windows)]
fn attach_console() {
    use winapi::um::wincon::{AttachConsole, ATTACH_PARENT_PROCESS};
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

fn peer_args(cmd: Command) -> Command {
    cmd.arg(
        Arg::new("peer")
            .long("peer")
            .short('p')
            .required(true)
            .help("Remote peer ID"),
    )
    .arg(
        Arg::new("password")
            .long("password")
            .help("Peer password (falls back to the saved peer password)"),
    )
    .arg(
        Arg::new("2fa-secret")
            .long("2fa-secret")
            .help("Base32 TOTP secret if the peer has 2FA enabled"),
    )
}

fn auth_only_args(cmd: Command) -> Command {
    cmd.arg(Arg::new("password").long("password").help(
        "Default peer password used for all connections (falls back to saved peer passwords)",
    ))
    .arg(
        Arg::new("2fa-secret")
            .long("2fa-secret")
            .help("Base32 TOTP secret used for all connections"),
    )
}

fn build_command() -> Command {
    Command::new("agent")
        .about("Medusa Desk AI agent gateway: drive remote machines headlessly")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(Command::new("peers").about("List saved peers as JSON"))
        .subcommand(
            peer_args(Command::new("screenshot").about("Capture the remote screen as PNG"))
                .arg(Arg::new("display").long("display").default_value("0"))
                .arg(
                    Arg::new("out")
                        .long("out")
                        .short('o')
                        .default_value("screenshot.png")
                        .help("Output file, or '-' for raw PNG on stdout"),
                ),
        )
        .subcommand(
            peer_args(Command::new("click").about("Click at remote coordinates"))
                .arg(Arg::new("x").long("x").required(true))
                .arg(Arg::new("y").long("y").required(true))
                .arg(Arg::new("button").long("button").default_value("left"))
                .arg(
                    Arg::new("double")
                        .long("double")
                        .action(ArgAction::SetTrue)
                        .help("Double-click"),
                ),
        )
        .subcommand(
            peer_args(Command::new("move").about("Move the remote mouse cursor"))
                .arg(Arg::new("x").long("x").required(true))
                .arg(Arg::new("y").long("y").required(true)),
        )
        .subcommand(
            peer_args(Command::new("scroll").about("Scroll the remote mouse wheel"))
                .arg(Arg::new("dx").long("dx").default_value("0"))
                .arg(Arg::new("dy").long("dy").default_value("0")),
        )
        .subcommand(
            peer_args(Command::new("type").about("Type a UTF-8 string on the remote"))
                .arg(Arg::new("text").required(true)),
        )
        .subcommand(
            peer_args(Command::new("key").about("Press a key (enter, tab, f5, a, VK_HOME...)"))
                .arg(Arg::new("key").required(true))
                .arg(Arg::new("ctrl").long("ctrl").action(ArgAction::SetTrue))
                .arg(Arg::new("alt").long("alt").action(ArgAction::SetTrue))
                .arg(Arg::new("shift").long("shift").action(ArgAction::SetTrue))
                .arg(
                    Arg::new("command")
                        .long("command")
                        .action(ArgAction::SetTrue)
                        .help("Meta/Win/Cmd modifier"),
                ),
        )
        .subcommand(
            peer_args(Command::new("exec").about("Run a command in the remote shell"))
                .arg(
                    Arg::new("timeout")
                        .long("timeout")
                        .default_value("60")
                        .help("Seconds to wait for completion"),
                )
                .arg(
                    Arg::new("cmd")
                        .required(true)
                        .num_args(1..)
                        .trailing_var_arg(true)
                        .help("Command line (prefix with -- if it contains flags)"),
                ),
        )
        .subcommand(
            peer_args(Command::new("push").about("Upload a local file/dir to the remote"))
                .arg(Arg::new("local").required(true))
                .arg(Arg::new("remote").required(true)),
        )
        .subcommand(
            peer_args(Command::new("pull").about("Download a remote file/dir"))
                .arg(Arg::new("remote").required(true))
                .arg(Arg::new("local").required(true)),
        )
        .subcommand(
            peer_args(Command::new("clipboard-set").about("Set the remote clipboard text"))
                .arg(Arg::new("text").required(true)),
        )
        .subcommand(peer_args(Command::new("clipboard-get").about(
            "Read clipboard text (local side, synced from the remote when clipboard sync is on)",
        )))
        .subcommand(auth_only_args(Command::new("mcp").about(
            "Run a Model Context Protocol server on stdio (for Claude Code, Cursor, ...)",
        )))
        .subcommand(
            auth_only_args(Command::new("serve").about("Run the localhost HTTP JSON API")).arg(
                Arg::new("port")
                    .long("port")
                    .help("Port to bind on 127.0.0.1 (default: option agent-http-port or 21120)"),
            ),
        )
}

fn auth_from(matches: &ArgMatches) -> Auth {
    Auth {
        password: matches
            .get_one::<String>("password")
            .cloned()
            .unwrap_or_default(),
        totp_secret: matches.get_one::<String>("2fa-secret").cloned(),
    }
}

fn parse_i32(matches: &ArgMatches, name: &str) -> i32 {
    matches
        .get_one::<String>(name)
        .and_then(|v| v.parse().ok())
        .unwrap_or_default()
}

fn fail(e: impl std::fmt::Display) -> i32 {
    eprintln!("{}", json!({ "error": e.to_string() }));
    1
}

fn ok(value: serde_json::Value) -> i32 {
    println!("{}", value);
    0
}

async fn connect(matches: &ArgMatches, conn_type: ConnType) -> ResultType<AgentSession> {
    let peer = matches
        .get_one::<String>("peer")
        .cloned()
        .unwrap_or_default();
    crate::common::test_rendezvous_server();
    crate::common::test_nat_type();
    AgentSession::connect(&peer, auth_from(matches), conn_type, CONNECT_TIMEOUT).await
}

async fn dispatch(matches: ArgMatches) -> i32 {
    match matches.subcommand() {
        Some(("peers", _)) => match serde_json::to_value(proto::list_peers()) {
            Ok(v) => ok(v),
            Err(e) => fail(e),
        },
        Some(("screenshot", m)) => {
            let sess = match connect(m, ConnType::DEFAULT_CONN).await {
                Ok(s) => s,
                Err(e) => return fail(e),
            };
            let display = parse_i32(m, "display");
            let res = sess.screenshot(display).await;
            sess.close().await;
            match res {
                Ok(png) => {
                    let out = m.get_one::<String>("out").cloned().unwrap_or_default();
                    if out == "-" {
                        use std::io::Write;
                        if let Err(e) = std::io::stdout().write_all(&png) {
                            return fail(e);
                        }
                        0
                    } else {
                        match std::fs::write(&out, &png) {
                            Ok(()) => ok(json!({ "ok": true, "path": out, "bytes": png.len() })),
                            Err(e) => fail(e),
                        }
                    }
                }
                Err(e) => fail(e),
            }
        }
        Some(("click", m)) => {
            let button = match proto::parse_button(
                m.get_one::<String>("button").map(|s| s.as_str()).unwrap_or(""),
            ) {
                Ok(b) => b,
                Err(e) => return fail(e),
            };
            let sess = match connect(m, ConnType::DEFAULT_CONN).await {
                Ok(s) => s,
                Err(e) => return fail(e),
            };
            sess.click(
                parse_i32(m, "x"),
                parse_i32(m, "y"),
                button,
                m.get_flag("double"),
            )
            .await;
            sess.close().await;
            ok(json!({ "ok": true }))
        }
        Some(("move", m)) => {
            let sess = match connect(m, ConnType::DEFAULT_CONN).await {
                Ok(s) => s,
                Err(e) => return fail(e),
            };
            sess.move_mouse(parse_i32(m, "x"), parse_i32(m, "y"));
            tokio::time::sleep(Duration::from_millis(200)).await;
            sess.close().await;
            ok(json!({ "ok": true }))
        }
        Some(("scroll", m)) => {
            let sess = match connect(m, ConnType::DEFAULT_CONN).await {
                Ok(s) => s,
                Err(e) => return fail(e),
            };
            sess.scroll(parse_i32(m, "dx"), parse_i32(m, "dy"));
            tokio::time::sleep(Duration::from_millis(200)).await;
            sess.close().await;
            ok(json!({ "ok": true }))
        }
        Some(("type", m)) => {
            let sess = match connect(m, ConnType::DEFAULT_CONN).await {
                Ok(s) => s,
                Err(e) => return fail(e),
            };
            sess.type_text(m.get_one::<String>("text").map(|s| s.as_str()).unwrap_or(""));
            tokio::time::sleep(Duration::from_millis(300)).await;
            sess.close().await;
            ok(json!({ "ok": true }))
        }
        Some(("key", m)) => {
            let sess = match connect(m, ConnType::DEFAULT_CONN).await {
                Ok(s) => s,
                Err(e) => return fail(e),
            };
            let key = proto::normalize_key(
                m.get_one::<String>("key").map(|s| s.as_str()).unwrap_or(""),
            );
            sess.key(
                &key,
                m.get_flag("alt"),
                m.get_flag("ctrl"),
                m.get_flag("shift"),
                m.get_flag("command"),
            );
            tokio::time::sleep(Duration::from_millis(300)).await;
            sess.close().await;
            ok(json!({ "ok": true }))
        }
        Some(("exec", m)) => {
            let cmd = m
                .get_many::<String>("cmd")
                .map(|v| v.cloned().collect::<Vec<_>>().join(" "))
                .unwrap_or_default();
            let secs = m
                .get_one::<String>("timeout")
                .and_then(|v| v.parse().ok())
                .unwrap_or(DEFAULT_EXEC_TIMEOUT_SECS);
            let sess = match connect(m, ConnType::TERMINAL).await {
                Ok(s) => s,
                Err(e) => return fail(e),
            };
            let res = sess.exec(&cmd, Duration::from_secs(secs)).await;
            sess.close().await;
            match res {
                Ok(r) => match serde_json::to_value(&r) {
                    Ok(v) => ok(v),
                    Err(e) => fail(e),
                },
                Err(e) => fail(e),
            }
        }
        Some(("push", m)) | Some(("pull", m)) => {
            let upload = matches.subcommand().map(|(n, _)| n == "push").unwrap_or(false);
            let local = m.get_one::<String>("local").cloned().unwrap_or_default();
            let remote = m.get_one::<String>("remote").cloned().unwrap_or_default();
            let sess = match connect(m, ConnType::FILE_TRANSFER).await {
                Ok(s) => s,
                Err(e) => return fail(e),
            };
            let res = sess
                .transfer(
                    &local,
                    &remote,
                    upload,
                    Duration::from_secs(DEFAULT_TRANSFER_TIMEOUT_SECS),
                )
                .await;
            sess.close().await;
            match res {
                Ok(()) => ok(json!({ "ok": true, "local": local, "remote": remote })),
                Err(e) => fail(e),
            }
        }
        Some(("clipboard-set", m)) => {
            let sess = match connect(m, ConnType::DEFAULT_CONN).await {
                Ok(s) => s,
                Err(e) => return fail(e),
            };
            sess.clipboard_set(m.get_one::<String>("text").map(|s| s.as_str()).unwrap_or(""));
            tokio::time::sleep(Duration::from_millis(300)).await;
            sess.close().await;
            ok(json!({ "ok": true }))
        }
        Some(("clipboard-get", m)) => {
            let sess = match connect(m, ConnType::DEFAULT_CONN).await {
                Ok(s) => s,
                Err(e) => return fail(e),
            };
            // Give clipboard sync a moment after connecting.
            tokio::time::sleep(Duration::from_millis(500)).await;
            let res = sess.clipboard_get();
            sess.close().await;
            match res {
                Ok(text) => ok(json!({ "text": text })),
                Err(e) => fail(e),
            }
        }
        Some(("mcp", m)) => {
            let pool = SessionPool::new(auth_from(m), POOL_IDLE);
            crate::common::test_rendezvous_server();
            crate::common::test_nat_type();
            super::mcp::serve(pool.clone()).await;
            pool.close_all().await;
            0
        }
        Some(("serve", m)) => {
            let port = m
                .get_one::<String>("port")
                .and_then(|v| v.parse::<u16>().ok())
                .or_else(|| Config::get_option("agent-http-port").parse::<u16>().ok())
                .unwrap_or(DEFAULT_HTTP_PORT);
            let pool = SessionPool::new(auth_from(m), POOL_IDLE);
            crate::common::test_rendezvous_server();
            crate::common::test_nat_type();
            match super::http::serve(pool.clone(), port).await {
                Ok(()) => 0,
                Err(e) => {
                    pool.close_all().await;
                    fail(e)
                }
            }
        }
        _ => {
            log::error!("Unknown agent subcommand");
            2
        }
    }
}

