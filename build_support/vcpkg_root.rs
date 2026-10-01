use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

pub fn resolve_vcpkg_root(
    target_os: &str,
    explicit_root: Option<&OsStr>,
    home: Option<&OsStr>,
    cargo_home: Option<&OsStr>,
    rustup_home: Option<&OsStr>,
    rustc: Option<&OsStr>,
    is_dir: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    if let Some(root) = explicit_root.filter(|root| !root.is_empty()) {
        return Some(root.into());
    }

    if target_os != "linux" {
        return None;
    }

    let home_root = home.map(PathBuf::from).map(|home| home.join("vcpkg"));
    let cargo_root = cargo_home.and_then(vcpkg_sibling_of_tool_home);
    let rustup_root = rustup_home.and_then(vcpkg_sibling_of_tool_home);
    let rustc_root = rustc.and_then(vcpkg_sibling_of_tool_path);

    vec![home_root, cargo_root, rustup_root, rustc_root]
        .into_iter()
        .flatten()
        .find(|root| is_dir(root))
}

fn vcpkg_sibling_of_tool_home(tool_home: &OsStr) -> Option<PathBuf> {
    let tool_home = Path::new(tool_home);
    matches!(tool_home.file_name(), Some(name) if name == ".cargo" || name == ".rustup")
        .then(|| tool_home.parent().map(|home| home.join("vcpkg")))
        .flatten()
}

fn vcpkg_sibling_of_tool_path(tool_path: &OsStr) -> Option<PathBuf> {
    Path::new(tool_path)
        .ancestors()
        .find(
            |path| matches!(path.file_name(), Some(name) if name == ".cargo" || name == ".rustup"),
        )
        .and_then(|tool_home| tool_home.parent())
        .map(|home| home.join("vcpkg"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_root_takes_precedence() {
        let root = resolve_vcpkg_root(
            "linux",
            Some(OsStr::new("/custom/vcpkg")),
            Some(OsStr::new("/home/user")),
            None,
            None,
            None,
            |_| true,
        );

        assert_eq!(root, Some(PathBuf::from("/custom/vcpkg")));
    }

    #[test]
    fn finds_default_linux_installation() {
        let root = resolve_vcpkg_root(
            "linux",
            None,
            Some(OsStr::new("/home/user")),
            None,
            None,
            None,
            |path| path == Path::new("/home/user/vcpkg"),
        );

        assert_eq!(root, Some(PathBuf::from("/home/user/vcpkg")));
    }

    #[test]
    fn finds_installation_when_home_is_overridden() {
        let root = resolve_vcpkg_root(
            "linux",
            None,
            Some(OsStr::new("/tmp/isolated-home")),
            Some(OsStr::new("/home/user/.cargo")),
            None,
            None,
            |path| path == Path::new("/home/user/vcpkg"),
        );

        assert_eq!(root, Some(PathBuf::from("/home/user/vcpkg")));
    }

    #[test]
    fn finds_installation_from_rustc_path() {
        let root = resolve_vcpkg_root(
            "linux",
            None,
            None,
            None,
            None,
            Some(OsStr::new("/home/user/.rustup/toolchains/stable/bin/rustc")),
            |path| path == Path::new("/home/user/vcpkg"),
        );

        assert_eq!(root, Some(PathBuf::from("/home/user/vcpkg")));
    }

    #[test]
    fn ignores_missing_default_installation() {
        let root = resolve_vcpkg_root(
            "linux",
            None,
            Some(OsStr::new("/home/user")),
            None,
            None,
            None,
            |_| false,
        );

        assert_eq!(root, None);
    }

    #[test]
    fn preserves_macos_homebrew_fallback() {
        let root = resolve_vcpkg_root(
            "macos",
            None,
            Some(OsStr::new("/Users/user")),
            Some(OsStr::new("/Users/user/.cargo")),
            Some(OsStr::new("/Users/user/.rustup")),
            Some(OsStr::new(
                "/Users/user/.rustup/toolchains/stable/bin/rustc",
            )),
            |_| true,
        );

        assert_eq!(root, None);
    }
}
