use std::{ffi::OsString, path::PathBuf};

pub fn resolve_vcpkg_root(
    configured_root: Option<OsString>,
    home: Option<OsString>,
    target_os: &str,
) -> Option<PathBuf> {
    if let Some(root) = configured_root.filter(|root| !root.is_empty()) {
        return Some(root.into());
    }

    if target_os != "linux" {
        return None;
    }

    home.map(PathBuf::from)
        .map(|home| home.join("vcpkg"))
        .filter(|root| root.is_dir())
}
