#[path = "../libs/vcpkg_root.rs"]
mod vcpkg_root;

use std::{ffi::OsString, fs, path::PathBuf};

use vcpkg_root::{has_target_headers, resolve_vcpkg_root};

#[test]
fn resolves_configured_root() {
    let configured = PathBuf::from("/configured/vcpkg");
    assert_eq!(
        resolve_vcpkg_root(
            Some(configured.clone().into_os_string()),
            "linux",
            &[Some(OsString::from("/unused/home"))],
        ),
        Some(configured),
        "an explicit VCPKG_ROOT must remain authoritative",
    );
}

#[test]
fn resolves_vcpkg_from_home() {
    let test_home = std::env::temp_dir().join(format!(
        "medusadesk-vcpkg-home-resolution-{}",
        std::process::id()
    ));
    let conventional_root = test_home.join("vcpkg");
    fs::create_dir_all(&conventional_root).expect("create conventional vcpkg directory");

    assert_eq!(
        resolve_vcpkg_root(None, "linux", &[Some(test_home.clone().into_os_string())]),
        Some(conventional_root),
        "Linux builds must discover the documented $HOME/vcpkg installation",
    );

    fs::remove_dir_all(test_home).expect("remove vcpkg resolver test directory");
}

#[test]
fn resolves_vcpkg_above_a_sanitized_home_workspace() {
    let test_root = std::env::temp_dir().join(format!(
        "medusadesk-vcpkg-workspace-resolution-{}",
        std::process::id()
    ));
    let conventional_root = test_root.join("vcpkg");
    let manifest_dir = test_root.join(".owllm/fleet/MedusaDesk/libs/scrap");
    fs::create_dir_all(&conventional_root).expect("create conventional vcpkg directory");
    fs::create_dir_all(&manifest_dir).expect("create nested manifest directory");

    assert_eq!(
        resolve_vcpkg_root(
            None,
            "linux",
            &[
                Some(OsString::from("/sanitized/home")),
                Some(manifest_dir.into_os_string()),
            ],
        ),
        Some(conventional_root),
        "Linux builds must find vcpkg from a workspace anchor when HOME is sanitized",
    );

    fs::remove_dir_all(test_root).expect("remove workspace resolver test directory");
}

#[test]
fn resolves_vcpkg_from_a_toolchain_anchor() {
    let test_root = std::env::temp_dir().join(format!(
        "medusadesk-vcpkg-toolchain-resolution-{}",
        std::process::id()
    ));
    let conventional_root = test_root.join("vcpkg");
    let cargo = test_root.join(".cargo/bin/cargo");
    fs::create_dir_all(&conventional_root).expect("create conventional vcpkg directory");

    assert_eq!(
        resolve_vcpkg_root(None, "linux", &[Some(cargo.into_os_string())]),
        Some(conventional_root),
        "Linux builds must find vcpkg from Cargo or Rust toolchain paths",
    );

    fs::remove_dir_all(test_root).expect("remove toolchain resolver test directory");
}

#[test]
fn does_not_autodiscover_vcpkg_on_other_platforms() {
    let test_home = std::env::temp_dir().join(format!(
        "medusadesk-vcpkg-platform-resolution-{}",
        std::process::id()
    ));
    fs::create_dir_all(test_home.join("vcpkg")).expect("create conventional vcpkg directory");

    assert_eq!(
        resolve_vcpkg_root(None, "macos", &[Some(test_home.clone().into_os_string())]),
        None,
        "automatic discovery must not replace the macOS Homebrew fallback",
    );

    fs::remove_dir_all(test_home).expect("remove vcpkg resolver test directory");
}

#[test]
fn returns_none_when_no_candidate_exists() {
    assert_eq!(
        resolve_vcpkg_root(
            None,
            "linux",
            &[Some(OsString::from("/definitely/not/a/vcpkg/root"))],
        ),
        None,
    );
}

#[test]
fn rejects_an_empty_linux_vcpkg_root() {
    let root = std::env::temp_dir().join(format!(
        "medusadesk-empty-vcpkg-root-{}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create empty vcpkg root");

    assert!(
        !has_target_headers(&root, "linux", "x86_64"),
        "an empty directory must not be treated as a usable vcpkg installation",
    );

    fs::remove_dir_all(root).expect("remove empty vcpkg root");
}

#[test]
fn accepts_linux_vcpkg_target_headers() {
    let root = std::env::temp_dir().join(format!(
        "medusadesk-vcpkg-target-headers-{}",
        std::process::id()
    ));
    fs::create_dir_all(root.join("installed/x64-linux/include"))
        .expect("create vcpkg target include directory");

    assert!(has_target_headers(&root, "linux", "x86_64"));

    fs::remove_dir_all(root).expect("remove vcpkg target headers test directory");
}
