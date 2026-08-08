# Medusa Desk — AI Agent Gateway

The AI Agent Gateway lets headless AI agents (Claude Code, Cursor, custom
scripts over SSH) drive remote machines through Medusa Desk's remote-desktop
protocol: take screenshots, send mouse/keyboard input, run shell commands,
inspect processes and services, transfer files and use the clipboard.

The agent runs next to Medusa Desk on *your* machine and drives *other*
machines:

```
you + your AI agent  ->  Medusa Desk (your PC)  ->  remote PC
```

Three interfaces share one session core:

| Interface | Start with | Best for |
|---|---|---|
| CLI verbs | `medusadesk agent <verb>` | SSH sessions, shell scripts, CI |
| MCP server | `medusadesk agent mcp` | AI coding agents (Claude Code, Cursor, ...) |
| HTTP JSON API | `medusadesk agent serve` | Custom agent frameworks |

## Authenticating to peers

The gateway authenticates to peers exactly like the desktop client:

1. The saved peer password (peers you've connected to with "remember password").
2. `--password <PW>` for an explicit password.
3. If the peer has 2FA enabled, pass the base32 TOTP secret with
   `--2fa-secret <SECRET>`; codes are computed automatically.
4. If no password is available, the remote side gets the usual
   click-to-approve prompt.

## Authenticating *to* the gateway

The CLI and the MCP server run as you, in your own process, and need nothing
extra. The HTTP gateway is a socket, so it needs a caller token.

Anything that can reach the gateway inherits every saved peer credential,
including `exec`. Binding to `127.0.0.1` is not enough on its own: a web page in
your browser also connects from loopback, so without a token any site you visit
could `POST /exec` to your machines. Every HTTP request therefore needs:

- `Authorization: Bearer <token>`, and
- a `Host` header naming loopback on the gateway's port.

Requests carrying an `Origin` header are refused outright — no browser is a
legitimate caller.

The token is generated per user on first use and stored in the local config as
`agent-http-token`. `medusadesk agent serve` prints it on startup, and
Settings → AI Agents shows it. It is never part of a build or a release
artifact. Delete the option to have a fresh one generated.

```sh
medusadesk agent serve
# {"listening":"http://127.0.0.1:21120","token":"5f1c..."}
```

## CLI

```sh
medusadesk agent peers                                          # saved peers as JSON
medusadesk agent displays   --peer 123456789                    # remote displays
medusadesk agent screenshot --peer 123456789 --out shot.png     # --out - for stdout
medusadesk agent click      --peer 123456789 --x 500 --y 300 [--button right] [--double]
medusadesk agent move       --peer 123456789 --x 500 --y 300
medusadesk agent scroll     --peer 123456789 --dy -3
medusadesk agent type       --peer 123456789 "hello world"
medusadesk agent key        --peer 123456789 enter [--ctrl] [--alt] [--shift]
medusadesk agent exec       --peer 123456789 [--timeout 60] -- systeminfo
medusadesk agent push       --peer 123456789 .\local.txt  C:\Temp\remote.txt
medusadesk agent pull       --peer 123456789 C:\Temp\remote.txt .\local.txt
medusadesk agent clipboard-set --peer 123456789 "text"
medusadesk agent clipboard-get --peer 123456789
medusadesk agent sysinfo    --peer 123456789
medusadesk agent ps         --peer 123456789 [--filter chrome] [--limit 50]
medusadesk agent kill       --peer 123456789 4321
medusadesk agent services   --peer 123456789 [--limit 50]
medusadesk agent service    --peer 123456789 Spooler restart
```

All commands print JSON on stdout (`{"error": ...}` on stderr + non-zero exit
on failure), so output is machine-parseable. `exec` returns
`{"stdout": "...", "exit_code": 0, "timed_out": false}`.

`exec` runs the command in the remote's default shell (PowerShell/cmd on
Windows, sh/bash elsewhere). Exit codes on Windows PowerShell are best-effort
for cmdlet (non-native) commands.

## MCP server

Register with Claude Code:

```sh
claude mcp add medusadesk -- medusadesk agent mcp
```

Tools exposed (each takes a `peer` argument):

- `medusa_list_peers` — discover saved machines
- `medusa_list_displays` — display indices, sizes and which one is captured
- `medusa_screenshot` — returns a PNG image content block
- `medusa_click`, `medusa_move_mouse`, `medusa_scroll`
- `medusa_type`, `medusa_key`
- `medusa_exec` — run a shell command, get stdout + exit code
- `medusa_upload`, `medusa_download`
- `medusa_clipboard_set`, `medusa_clipboard_get`
- `medusa_system_info` — OS, architecture, uptime, free memory and disk
- `medusa_list_processes`, `medusa_kill_process`
- `medusa_list_services`, `medusa_service_control`

The typed system tools pick the right command for the remote's OS (PowerShell
cmdlets, `systemctl` or `launchctl`), so an agent does not have to compose a
shell string and guess the platform. Their arguments are numbers or names
restricted to `A-Z a-z 0-9 . _ - @ / \`, so a tool argument can never inject a
second command; use `medusa_exec` when you need something they don't cover.

Sessions are pooled per (peer, channel) and reaped after 5 minutes idle, so a
sequence of screenshot → click → type reuses one connection.

## HTTP JSON API

```sh
medusadesk agent serve [--port 21120]
```

Binds to `127.0.0.1` only. The port can also be set in
Settings → AI Agents (option `agent-http-port`).

| Route | Body | Result |
|---|---|---|
| `GET /status` | — | `{ status, version, sessions }` |
| `GET /peers` | — | saved peers |
| `POST /displays` | `{peer}` | `[{index, x, y, width, height, name, online, scale, is_current}]` |
| `POST /screenshot` | `{peer, display?}` | `image/png` bytes |
| `POST /input/mouse` | `{peer, action: click\|move\|scroll, x, y, button?, double?, dx?, dy?}` | `{ok}` |
| `POST /input/key` | `{peer, text?}` or `{peer, key?, ctrl?, alt?, shift?, command?}` | `{ok}` |
| `POST /exec` | `{peer, command, timeout_secs?}` | `{stdout, exit_code, timed_out}` |
| `POST /files/upload` | `{peer, local_path, remote_path}` | `{ok}` |
| `POST /files/download` | `{peer, remote_path, local_path}` | `{ok}` |
| `POST /clipboard` | `{peer, action: get\|set, text?}` | `{text}` / `{ok}` |
| `POST /system/info` | `{peer}` | `{stdout, exit_code, timed_out}` |
| `POST /system/processes` | `{peer, filter?, limit?}` | `{stdout, exit_code, timed_out}` |
| `POST /system/kill` | `{peer, pid}` | `{stdout, exit_code, timed_out}` |
| `POST /system/services` | `{peer, limit?}` | `{stdout, exit_code, timed_out}` |
| `POST /system/service` | `{peer, name, action: start\|stop\|restart}` | `{stdout, exit_code, timed_out}` |

Errors: `400` malformed request, `401` missing/invalid token, `403` untrusted
`Host` or a browser `Origin`, `500` the operation failed.

Example:

```sh
curl -s -X POST http://127.0.0.1:21120/exec \
  -H "Authorization: Bearer $MEDUSA_AGENT_TOKEN" \
  -d '{"peer":"123456789","command":"whoami"}'
```

## Notes & limitations

- `exec`, `clipboard-get` and the `system/*` tools use the remote's terminal
  channel, so the remote must have **Enable terminal** (`enable-terminal`) on
  in Settings → Security → Permissions. If it is off, the connection is refused
  at login and the gateway says which setting to turn on.
- `clipboard_get` reads the remote's clipboard through the remote's own
  clipboard tool (`Get-Clipboard`, `pbpaste`, `wl-paste`/`xclip`/`xsel`),
  because the protocol only pushes clipboard changes and has no "read remote
  clipboard" request. On Linux the remote needs one of those tools installed.
- Screenshots require the remote peer to run RustDesk/MedusaDesk >= 1.4.0.
- `service_control` and `kill` act with the privileges of the remote terminal;
  an elevated action needs an elevated terminal login.
- Peers configured for click-approval only (no password) will block until a
  human accepts on the remote side — by design.
- The gateway is compiled in by default (`agent` cargo feature); build with
  `--no-default-features` to exclude it.
