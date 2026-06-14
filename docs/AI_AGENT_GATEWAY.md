# Medusa Desk — AI Agent Gateway

The AI Agent Gateway lets headless AI agents (Claude Code, Cursor, custom
scripts over SSH) drive remote machines through Medusa Desk's remote-desktop
protocol: take screenshots, send mouse/keyboard input, run shell commands,
transfer files and use the clipboard.

Three interfaces share one session core:

| Interface | Start with | Best for |
|---|---|---|
| CLI verbs | `medusadesk agent <verb>` | SSH sessions, shell scripts, CI |
| MCP server | `medusadesk agent mcp` | AI coding agents (Claude Code, Cursor, ...) |
| HTTP JSON API | `medusadesk agent serve` | Custom agent frameworks |

## Authentication

The gateway authenticates to peers exactly like the desktop client:

1. The saved peer password (peers you've connected to with "remember password").
2. `--password <PW>` for an explicit password.
3. If the peer has 2FA enabled, pass the base32 TOTP secret with
   `--2fa-secret <SECRET>`; codes are computed automatically.
4. If no password is available, the remote side gets the usual
   click-to-approve prompt.

There is no separate token system: an agent can only reach peers it has
credentials for.

## CLI

```sh
medusadesk agent peers                                          # saved peers as JSON
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
- `medusa_screenshot` — returns a PNG image content block
- `medusa_click`, `medusa_move_mouse`, `medusa_scroll`
- `medusa_type`, `medusa_key`
- `medusa_exec` — run a shell command, get stdout + exit code
- `medusa_upload`, `medusa_download`
- `medusa_clipboard_set`, `medusa_clipboard_get`

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
| `POST /screenshot` | `{peer, display?}` | `image/png` bytes |
| `POST /input/mouse` | `{peer, action: click\|move\|scroll, x, y, button?, double?, dx?, dy?}` | `{ok}` |
| `POST /input/key` | `{peer, text?}` or `{peer, key?, ctrl?, alt?, shift?, command?}` | `{ok}` |
| `POST /exec` | `{peer, command, timeout_secs?}` | `{stdout, exit_code, timed_out}` |
| `POST /files/upload` | `{peer, local_path, remote_path}` | `{ok}` |
| `POST /files/download` | `{peer, remote_path, local_path}` | `{ok}` |
| `POST /clipboard` | `{peer, action: get\|set, text?}` | `{text}` / `{ok}` |

Example:

```sh
curl -s -X POST http://127.0.0.1:21120/exec \
  -d '{"peer":"123456789","command":"whoami"}'
```

## Notes & limitations

- Screenshots require the remote peer to run RustDesk/MedusaDesk >= 1.4.0.
- `clipboard_get` reads the local clipboard, which mirrors the remote when
  clipboard sync is enabled (the protocol pushes clipboard changes; there is
  no direct "read remote clipboard" request).
- Peers configured for click-approval only (no password) will block until a
  human accepts on the remote side — by design.
- The gateway is compiled in by default (`agent` cargo feature); build with
  `--no-default-features` to exclude it.
