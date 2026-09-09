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
