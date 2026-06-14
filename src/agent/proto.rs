//! Shared DTOs for the CLI / MCP / HTTP interfaces.

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct PeerDto {
    pub id: String,
    pub alias: String,
    pub username: String,
    pub hostname: String,
    pub platform: String,
}

pub fn list_peers() -> Vec<PeerDto> {
    hbb_common::config::PeerConfig::peers(None)
        .into_iter()
        .map(|(id, _modified, cfg)| PeerDto {
            id,
            alias: cfg.options.get("alias").cloned().unwrap_or_default(),
            username: cfg.info.username,
            hostname: cfg.info.hostname,
            platform: cfg.info.platform,
        })
        .collect()
}

pub fn parse_button(name: &str) -> Result<i32, String> {
    match name.to_ascii_lowercase().as_str() {
        "left" | "l" | "" => Ok(super::session::MOUSE_BUTTON_LEFT),
        "right" | "r" => Ok(super::session::MOUSE_BUTTON_RIGHT),
        "middle" | "m" | "wheel" => Ok(super::session::MOUSE_BUTTON_MIDDLE),
        other => Err(format!("Unknown mouse button '{other}' (use left|right|middle)")),
    }
}

/// Map a friendly key name to the VK_* names in `crate::client::KEY_MAP`,
/// passing single characters and existing VK_* names through.
pub fn normalize_key(name: &str) -> String {
    if name.chars().count() == 1 || name.starts_with("VK_") {
        return name.to_owned();
    }
    let vk = match name.to_ascii_lowercase().as_str() {
        "enter" | "return" => "VK_ENTER",
        "tab" => "VK_TAB",
        "escape" | "esc" => "VK_ESCAPE",
        "backspace" => "VK_BACK",
        "delete" | "del" => "VK_DELETE",
        "insert" | "ins" => "VK_INSERT",
        "home" => "VK_HOME",
        "end" => "VK_END",
        "pageup" | "pgup" => "VK_PRIOR",
        "pagedown" | "pgdn" => "VK_NEXT",
        "up" => "VK_UP",
        "down" => "VK_DOWN",
        "left" => "VK_LEFT",
        "right" => "VK_RIGHT",
        "space" => "VK_SPACE",
        "f1" => "VK_F1",
        "f2" => "VK_F2",
        "f3" => "VK_F3",
        "f4" => "VK_F4",
        "f5" => "VK_F5",
        "f6" => "VK_F6",
        "f7" => "VK_F7",
        "f8" => "VK_F8",
        "f9" => "VK_F9",
        "f10" => "VK_F10",
        "f11" => "VK_F11",
        "f12" => "VK_F12",
        _ => return name.to_owned(),
    };
    vk.to_owned()
}

#[derive(Debug, Deserialize)]
pub struct MouseReq {
    pub peer: String,
    #[serde(default)]
    pub action: String, // click | move | scroll
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
    #[serde(default)]
    pub dx: i32,
    #[serde(default)]
    pub dy: i32,
    #[serde(default)]
    pub button: String,
    #[serde(default)]
    pub double: bool,
}

#[derive(Debug, Deserialize)]
pub struct KeyReq {
    pub peer: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub alt: bool,
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub command: bool,
}

#[derive(Debug, Deserialize)]
pub struct ExecReq {
    pub peer: String,
    pub command: String,
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct ScreenshotReq {
    pub peer: String,
    #[serde(default)]
    pub display: i32,
}

#[derive(Debug, Deserialize)]
pub struct TransferReq {
    pub peer: String,
    pub local_path: String,
    pub remote_path: String,
}

#[derive(Debug, Deserialize)]
pub struct ClipboardReq {
    pub peer: String,
    #[serde(default)]
    pub action: String, // get | set
    #[serde(default)]
    pub text: String,
}
