#[path = "../libs/scrap/build_vcpkg.rs"]
mod build_vcpkg;

use std::{ffi::OsString, fs, path::PathBuf};

use build_vcpkg::resolve_vcpkg_root;

#[test]
fn resolves_configured_and_documented_linux_vcpkg_roots() {
    let configured = PathBuf::from("/configured/vcpkg");
    assert_eq!(
        resolve_vcpkg_root(
            Some(configured.clone().into_os_string()),
            Some(OsString::from("/unused/home")),
            "linux",
        ),
        Some(configured),
        "an explicit VCPKG_ROOT must remain authoritative",
    );

    let test_home = std::env::temp_dir().join(format!(
        "medusadesk-vcpkg-resolution-{}",
        std::process::id()
    ));
    let conventional_root = test_home.join("vcpkg");
    fs::create_dir_all(&conventional_root).expect("create conventional vcpkg directory");

    assert_eq!(
        resolve_vcpkg_root(None, Some(test_home.clone().into_os_string()), "linux"),
        Some(conventional_root),
        "Linux builds must discover the documented $HOME/vcpkg installation",
    );
    assert_eq!(
        resolve_vcpkg_root(None, Some(test_home.clone().into_os_string()), "macos"),
        None,
        "automatic discovery must not replace the macOS Homebrew fallback",
    );

    fs::remove_dir_all(test_home).expect("remove vcpkg resolver test directory");
}
