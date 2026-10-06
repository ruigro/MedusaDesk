use std::{ffi::OsString, path::PathBuf};

pub fn resolve_vcpkg_root(
    configured_root: Option<OsString>,
    target_os: &str,
    search_anchors: &[Option<OsString>],
) -> Option<PathBuf> {
    if let Some(root) = configured_root.filter(|root| !root.is_empty()) {
        return Some(root.into());
    }

    if target_os != "linux" {
        return None;
    }

    search_anchors
        .iter()
        .flatten()
        .flat_map(|anchor| {
            PathBuf::from(anchor)
                .ancestors()
                .map(PathBuf::from)
                .collect::<Vec<_>>()
        })
        .map(|base| base.join("vcpkg"))
        .find(|root| root.is_dir())
}

pub fn has_target_headers(root: &std::path::Path, target_os: &str, target_arch: &str) -> bool {
    if target_os != "linux" {
        return true;
    }

    let triplet = match target_arch {
        "x86_64" => "x64-linux",
        "x86" => "x86-linux",
        "aarch64" => "arm64-linux",
        "loongarch64" => "loongarch64-linux",
        _ => "arm-linux",
    };
    root.join("installed")
        .join(triplet)
        .join("include")
        .is_dir()
}

/// Static-CRT vcpkg triplet for a Windows target, matching the `+crt-static`
/// rustflags in `.cargo/config.toml`. Accepts `CARGO_CFG_TARGET_ARCH` values
/// as well as the vcpkg architecture names the build scripts normalize to.
pub fn windows_vcpkg_triplet(target_arch: &str) -> String {
    let arch = match target_arch {
        "x86_64" | "x64" => "x64",
        "aarch64" | "arm64" => "arm64",
        other => other,
    };
    format!("{arch}-windows-static")
}
