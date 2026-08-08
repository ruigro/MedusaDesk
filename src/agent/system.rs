//! Per-platform command lines for the system operations support staff reach
//! for, so an agent can ask for "the service list" instead of composing a shell
//! string and hoping it matches the remote's OS.
//!
//! Every command that a tool argument feeds into is built here and nowhere
//! else. Arguments are either numbers or pass [`safe_name`], so a tool argument
//! can never close a quote or append a second command.
//!
//! Free of external crates so it can be exercised in isolation.

/// The remote's OS, as reported by `PeerInfo.platform`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Platform {
    Windows,
    Macos,
    Linux,
}

impl Platform {
    /// Anything that is not recognisably Windows or macOS is treated as a
    /// POSIX shell, which is also right for Android and the BSDs.
    pub fn of(peer_platform: &str) -> Self {
        match peer_platform.trim().to_ascii_lowercase().as_str() {
            "windows" => Platform::Windows,
            "mac os" | "macos" | "darwin" => Platform::Macos,
            _ => Platform::Linux,
        }
    }

    fn is_windows(self) -> bool {
        self == Platform::Windows
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ServiceAction {
    Start,
    Stop,
    Restart,
}

impl ServiceAction {
    pub fn parse(name: &str) -> Result<Self, String> {
        match name.trim().to_ascii_lowercase().as_str() {
            "start" => Ok(ServiceAction::Start),
            "stop" => Ok(ServiceAction::Stop),
            "restart" => Ok(ServiceAction::Restart),
            other => Err(format!(
                "unknown service action '{other}' (use start|stop|restart)"
            )),
        }
    }
}

/// Most processes and services are listed within this many rows.
const DEFAULT_LIMIT: usize = 50;
const MAX_LIMIT: usize = 500;
const MAX_NAME: usize = 128;

/// A command must fit on one line of the remote terminal, prompt included, or
/// the shell's echo of it wraps and `exec::scrub` returns that echo to the
/// caller as if it were output. Must stay below `exec::TERM_COLS`.
pub const MAX_COMMAND_LEN: usize = 400;

/// Accept only characters that cannot terminate an argument, a quote or a
/// command in any of the shells reached, and reject everything else rather
/// than trying to escape it.
pub fn safe_name(value: &str) -> Result<&str, String> {
    let name = value.trim();
    if name.is_empty() {
        return Err("name must not be empty".to_owned());
    }
    if name.len() > MAX_NAME {
        return Err(format!("name must be at most {MAX_NAME} characters"));
    }
    if let Some(bad) = name
        .chars()
        .find(|c| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '@' | '/' | '\\')))
    {
        return Err(format!(
            "name may only contain letters, digits and . _ - @ / \\ (found '{bad}')"
        ));
    }
    Ok(name)
}

fn clamp_limit(limit: Option<usize>) -> usize {
    limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
}

/// Run a PowerShell expression regardless of which shell the remote opened:
/// `powershell` resolves the same from cmd.exe and from PowerShell itself.
fn powershell(expression: &str) -> String {
    format!("powershell -NoProfile -NonInteractive -Command \"{expression}\"")
}

pub fn system_info(platform: Platform) -> String {
    match platform {
        Platform::Windows => powershell(
            "Get-CimInstance Win32_OperatingSystem | Select-Object -Property \
             CSName,Caption,Version,OSArchitecture,LastBootUpTime,FreePhysicalMemory,\
             TotalVisibleMemorySize | ConvertTo-Json -Compress",
        ),
        Platform::Macos => "sw_vers; uname -a; uptime; df -h /".to_owned(),
        Platform::Linux => {
            "uname -a; grep PRETTY_NAME /etc/os-release 2>/dev/null; uptime; df -h /".to_owned()
        }
    }
}

pub fn list_processes(
    platform: Platform,
    filter: Option<&str>,
    limit: Option<usize>,
) -> Result<String, String> {
    let limit = clamp_limit(limit);
    let filter = filter
        .filter(|f| !f.trim().is_empty())
        .map(safe_name)
        .transpose()?;
    Ok(match platform {
        Platform::Windows => {
            let select = match filter {
                Some(f) => format!("Get-Process -Name '*{f}*' -ErrorAction SilentlyContinue"),
                None => "Get-Process".to_owned(),
            };
            powershell(&format!(
                "{select} | Sort-Object -Property WorkingSet -Descending | \
                 Select-Object -First {limit} -Property Id,ProcessName,WorkingSet,CPU | \
                 ConvertTo-Json -Compress"
            ))
        }
        // macOS `ps` has no --sort; -m orders by memory, matching Linux's -rss.
        Platform::Macos | Platform::Linux => {
            let ps = if platform == Platform::Macos {
                "ps -Ao pid,ppid,rss,comm -m"
            } else {
                "ps -eo pid,ppid,rss,comm --sort=-rss"
            };
            match filter {
                Some(f) => format!("{ps} | grep -i -- '{f}' | head -n {limit}"),
                // One extra row so the header does not eat a result.
                None => format!("{ps} | head -n {}", limit + 1),
            }
        }
    })
}

pub fn kill_process(platform: Platform, pid: u32) -> String {
    if platform.is_windows() {
        format!("taskkill /PID {pid} /F")
    } else {
        format!("kill -9 {pid}")
    }
}

pub fn list_services(platform: Platform, limit: Option<usize>) -> String {
    let limit = clamp_limit(limit);
    match platform {
        Platform::Windows => powershell(&format!(
            "Get-Service | Select-Object -First {limit} -Property Name,DisplayName,Status | \
             ConvertTo-Json -Compress"
        )),
        Platform::Macos => format!("launchctl list | head -n {}", limit + 1),
        Platform::Linux => format!(
            "systemctl list-units --type=service --all --no-pager --no-legend --plain | head -n {limit}"
        ),
    }
}

pub fn service_control(
    platform: Platform,
    name: &str,
    action: ServiceAction,
) -> Result<String, String> {
    let name = safe_name(name)?;
    Ok(match platform {
        Platform::Windows => powershell(&match action {
            ServiceAction::Start => format!("Start-Service -Name '{name}'"),
            ServiceAction::Stop => format!("Stop-Service -Name '{name}' -Force"),
            ServiceAction::Restart => format!("Restart-Service -Name '{name}' -Force"),
        }),
        Platform::Linux => {
            let verb = match action {
                ServiceAction::Start => "start",
                ServiceAction::Stop => "stop",
                ServiceAction::Restart => "restart",
            };
            format!("systemctl {verb} {name}")
        }
        Platform::Macos => match action {
            ServiceAction::Start => format!("launchctl start {name}"),
            ServiceAction::Stop => format!("launchctl stop {name}"),
            ServiceAction::Restart => format!("launchctl stop {name}; launchctl start {name}"),
        },
    })
}

/// Name the remote-side setting behind a login refusal.
///
/// The peer answers with terse strings like "No permission of terminal", which
/// say what was refused but not what to do about it. An agent driving this
/// headlessly has no other way to find out, and a disabled `enable-terminal` is
/// the most common reason `exec` looks broken.
pub fn explain_login_error(error: &str) -> Option<String> {
    let setting = |name: &str, key: &str| {
        Some(format!(
            "Turn on '{name}' ({key}) in Settings -> Security -> Permissions on the remote machine."
        ))
    };
    if error.contains("No permission of terminal") {
        setting("Enable terminal", "enable-terminal")
    } else if error.contains("No permission of file transfer") {
        setting("Enable file transfer", "enable-file-transfer")
    } else if error.contains("No permission of viewing camera") {
        setting("Enable camera", "enable-camera")
    } else if error.contains("No permission of IP tunneling") {
        setting("Enable IP tunneling", "enable-tunnel")
    } else if error.contains("Supported only in the installed version") {
        Some(
            "An elevated terminal login needs an installed MedusaDesk on the remote machine, \
             not the portable build."
                .to_owned(),
        )
    } else {
        None
    }
}

/// Read the *remote* clipboard. There is no protocol message that asks a peer
/// for its clipboard — the peer only pushes it on change — so this goes through
/// the remote's own clipboard tool.
pub fn read_clipboard(platform: Platform) -> String {
    match platform {
        Platform::Windows => powershell("Get-Clipboard -Raw"),
        Platform::Macos => "pbpaste".to_owned(),
        Platform::Linux => "wl-paste --no-newline 2>/dev/null || \
                            xclip -selection clipboard -o 2>/dev/null || \
                            xsel --clipboard --output 2>/dev/null"
            .to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_of_known_and_unknown_names() {
        // The exact strings PeerInfo.platform carries (src/common.rs).
        assert_eq!(Platform::of("Windows"), Platform::Windows);
        assert_eq!(Platform::of("Mac OS"), Platform::Macos);
        assert_eq!(Platform::of("Linux"), Platform::Linux);
        assert_eq!(Platform::of("windows"), Platform::Windows);
        assert_eq!(Platform::of("Android"), Platform::Linux);
        assert_eq!(Platform::of(""), Platform::Linux);
    }

    /// A tool argument must never be able to add a command of its own.
    #[test]
    fn safe_name_rejects_shell_metacharacters() {
        for bad in [
            "svc; rm -rf /",
            "svc && whoami",
            "svc | whoami",
            "svc`whoami`",
            "svc$(whoami)",
            "svc'",
            "svc\"",
            "svc\nwhoami",
            "svc>out",
            "svc*",
            "",
            "   ",
        ] {
            assert!(safe_name(bad).is_err(), "{bad:?} was accepted");
        }
        assert!(safe_name(&"a".repeat(MAX_NAME + 1)).is_err());
    }

    #[test]
    fn safe_name_accepts_real_service_names() {
        for good in [
            "Spooler",
            "ssh.service",
            "com.apple.afpfs_afpLoad",
            "wu-agent",
        ] {
            assert_eq!(safe_name(good), Ok(good));
        }
        assert_eq!(safe_name("  Spooler  "), Ok("Spooler"));
    }

    #[test]
    fn injection_attempts_never_reach_a_command() {
        assert!(service_control(Platform::Linux, "ssh; rm -rf /", ServiceAction::Stop).is_err());
        assert!(service_control(Platform::Windows, "a'; calc; '", ServiceAction::Start).is_err());
        assert!(list_processes(Platform::Linux, Some("x' ; id ; '"), None).is_err());
    }

    /// A pid is parsed as a number before it gets here, so there is nothing to
    /// escape — this pins that the signature keeps it that way.
    #[test]
    fn kill_process_is_numeric_only() {
        assert_eq!(
            kill_process(Platform::Windows, 1234),
            "taskkill /PID 1234 /F"
        );
        assert_eq!(kill_process(Platform::Linux, 1234), "kill -9 1234");
    }

    #[test]
    fn limits_are_clamped_to_a_sane_range() {
        let huge = list_processes(Platform::Linux, None, Some(10_000)).unwrap();
        assert!(
            huge.ends_with(&format!("head -n {}", MAX_LIMIT + 1)),
            "{huge}"
        );
        let zero = list_processes(Platform::Linux, None, Some(0)).unwrap();
        assert!(zero.ends_with("head -n 2"), "{zero}");
        let default = list_processes(Platform::Linux, None, None).unwrap();
        assert!(default.ends_with(&format!("head -n {}", DEFAULT_LIMIT + 1)));
    }

    #[test]
    fn windows_commands_run_through_powershell() {
        for cmd in [
            system_info(Platform::Windows),
            list_processes(Platform::Windows, None, None).unwrap(),
            list_services(Platform::Windows, None),
            service_control(Platform::Windows, "Spooler", ServiceAction::Restart).unwrap(),
            read_clipboard(Platform::Windows),
        ] {
            // Resolves the same whether the remote shell is cmd.exe or PowerShell.
            assert!(
                cmd.starts_with("powershell -NoProfile -NonInteractive -Command \""),
                "{cmd}"
            );
            assert!(cmd.ends_with('"'), "{cmd}");
            // The outer quotes must be the only double quotes in the line.
            assert_eq!(cmd.matches('"').count(), 2, "{cmd}");
        }
    }

    /// Every command must fit on one terminal line; past that the shell's
    /// echo wraps, survives scrubbing, and lands in the caller's stdout —
    /// which for the JSON-emitting Windows commands means unparseable output.
    #[test]
    fn every_command_fits_on_one_terminal_line() {
        let long_name = "a".repeat(MAX_NAME);
        let long_filter = "b".repeat(MAX_NAME);
        for platform in [Platform::Windows, Platform::Macos, Platform::Linux] {
            let mut commands = vec![
                system_info(platform),
                read_clipboard(platform),
                kill_process(platform, u32::MAX),
                list_services(platform, Some(MAX_LIMIT)),
                list_processes(platform, None, Some(MAX_LIMIT)).unwrap(),
                list_processes(platform, Some(&long_filter), Some(MAX_LIMIT)).unwrap(),
            ];
            for action in [
                ServiceAction::Start,
                ServiceAction::Stop,
                ServiceAction::Restart,
            ] {
                commands.push(service_control(platform, &long_name, action).unwrap());
            }
            for cmd in commands {
                assert!(
                    cmd.len() <= MAX_COMMAND_LEN,
                    "{platform:?} command is {} chars (max {MAX_COMMAND_LEN}): {cmd}",
                    cmd.len()
                );
            }
        }
    }

    #[test]
    fn service_control_covers_every_action_per_platform() {
        for platform in [Platform::Windows, Platform::Macos, Platform::Linux] {
            for action in [
                ServiceAction::Start,
                ServiceAction::Stop,
                ServiceAction::Restart,
            ] {
                let cmd = service_control(platform, "Spooler", action).unwrap();
                assert!(cmd.contains("Spooler"), "{platform:?} {action:?}: {cmd}");
            }
        }
        assert_eq!(
            service_control(Platform::Linux, "ssh", ServiceAction::Restart),
            Ok("systemctl restart ssh".to_owned())
        );
    }

    #[test]
    fn service_action_parsing() {
        assert_eq!(ServiceAction::parse("RESTART"), Ok(ServiceAction::Restart));
        assert_eq!(ServiceAction::parse(" stop "), Ok(ServiceAction::Stop));
        assert!(ServiceAction::parse("delete").is_err());
    }

    /// The peer's refusal names the permission but not the setting; without
    /// this an agent just sees "No permission of terminal" and stops.
    #[test]
    fn login_refusals_name_the_remote_setting() {
        // The exact strings src/server/connection.rs sends.
        let hint = explain_login_error("Login Error: No permission of terminal").unwrap();
        assert!(hint.contains("enable-terminal"), "{hint}");
        assert!(explain_login_error("No permission of file transfer")
            .unwrap()
            .contains("enable-file-transfer"));
        assert!(explain_login_error("No permission of viewing camera")
            .unwrap()
            .contains("enable-camera"));
        assert!(explain_login_error("No permission of IP tunneling")
            .unwrap()
            .contains("enable-tunnel"));
        assert!(
            explain_login_error("Supported only in the installed version.")
                .unwrap()
                .contains("installed")
        );
        // Errors we have nothing useful to add to stay untouched.
        assert_eq!(explain_login_error("Wrong password"), None);
    }

    #[test]
    fn clipboard_read_uses_the_remote_side_tool() {
        assert_eq!(read_clipboard(Platform::Macos), "pbpaste");
        assert!(read_clipboard(Platform::Linux).contains("wl-paste"));
        assert!(read_clipboard(Platform::Linux).contains("xclip"));
        assert!(read_clipboard(Platform::Windows).contains("Get-Clipboard"));
    }
}
