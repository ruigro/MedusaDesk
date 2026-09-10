use std::{
    env,
    path::{Path, PathBuf},
};

#[cfg(not(all(target_os = "linux", feature = "linux-pkg-config")))]
#[path = "../vcpkg_root.rs"]
mod vcpkg_root;

#[cfg(not(all(target_os = "linux", feature = "linux-pkg-config")))]
use vcpkg_root::{has_target_headers, resolve_vcpkg_root};

#[cfg(all(target_os = "linux", feature = "linux-pkg-config"))]
fn link_pkg_config(name: &str) -> Vec<PathBuf> {
    let lib = pkg_config::probe_library(name)
        .expect(format!(
            "unable to find '{name}' development headers with pkg-config (feature linux-pkg-config is enabled).
            try installing '{name}-dev' from your system package manager.").as_str());

    lib.include_paths
}

#[cfg(not(all(target_os = "linux", feature = "linux-pkg-config")))]
fn link_vcpkg(mut path: PathBuf, name: &str) -> PathBuf {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    let mut target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    if target_arch == "x86_64" {
        target_arch = "x64".to_owned();
    } else if target_arch == "aarch64" {
        target_arch = "arm64".to_owned();
    }
    let mut target = if target_os == "macos" && target_arch == "x64" {
        "x64-osx".to_owned()
    } else if target_os == "macos" && target_arch == "arm64" {
        "arm64-osx".to_owned()
    } else if target_os == "windows" {
        "x64-windows-static".to_owned()
    } else {
        format!("{}-{}", target_arch, target_os)
    };
    if target_arch == "x86" {
        target = target.replace("x64", "x86");
    }
    println!("cargo:info={}", target);
    path.push("installed");
    path.push(target);
    println!(
        "{}",
        format!(
            "cargo:rustc-link-lib=static={}",
            name.trim_start_matches("lib")
        )
    );
    println!(
        "{}",
        format!(
            "cargo:rustc-link-search={}",
            path.join("lib").to_str().unwrap()
        )
    );
    let include = path.join("include");
    println!("{}", format!("cargo:include={}", include.to_str().unwrap()));
    include
}

#[cfg(not(all(target_os = "linux", feature = "linux-pkg-config")))]
fn link_homebrew_m1(name: &str) -> PathBuf {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    if target_os != "macos" || target_arch != "aarch64" {
        panic!("Couldn't find VCPKG_ROOT, also can't fallback to homebrew because it's only for macos aarch64.");
    }
    let mut path = PathBuf::from("/opt/homebrew/Cellar");
    path.push(name);
    let entries = if let Ok(dir) = std::fs::read_dir(&path) {
        dir
    } else {
        panic!("Could not find package in {}. Make sure your homebrew and package {} are all installed.", path.to_str().unwrap(),&name);
    };
    let mut directories = entries
        .into_iter()
        .filter(|x| x.is_ok())
        .map(|x| x.unwrap().path())
        .filter(|x| x.is_dir())
        .collect::<Vec<_>>();
    // Find the newest version.
    directories.sort_unstable();
    if directories.is_empty() {
        panic!(
            "There's no installed version of {} in /opt/homebrew/Cellar",
            name
        );
    }
    path.push(directories.pop().unwrap());
    // Link the library.
    println!(
        "{}",
        format!(
            "cargo:rustc-link-lib=static={}",
            name.trim_start_matches("lib")
        )
    );
    // Add the library path.
    println!(
        "{}",
        format!(
            "cargo:rustc-link-search={}",
            path.join("lib").to_str().unwrap()
        )
    );
    // Add the include path.
    let include = path.join("include");
    println!("{}", format!("cargo:include={}", include.to_str().unwrap()));
    include
}

#[cfg(all(target_os = "linux", feature = "linux-pkg-config"))]
fn find_package(name: &str) -> Vec<PathBuf> {
    return link_pkg_config(name);
}

#[cfg(not(all(target_os = "linux", feature = "linux-pkg-config")))]
fn find_package(name: &str) -> Vec<PathBuf> {
    const VCPKG_ENV_VARS: [&str; 6] = [
        "VCPKG_ROOT",
        "HOME",
        "CARGO_HOME",
        "RUSTUP_HOME",
        "CARGO",
        "RUSTC",
    ];
    for variable in VCPKG_ENV_VARS {
        println!("cargo:rerun-if-env-changed={variable}");
    }

    if let Some(vcpkg_root) = resolve_vcpkg_root(
        std::env::var_os("VCPKG_ROOT"),
        std::env::var("CARGO_CFG_TARGET_OS")
            .as_deref()
            .unwrap_or_default(),
        &[
            std::env::var_os("HOME"),
            std::env::var_os("CARGO_HOME"),
            std::env::var_os("RUSTUP_HOME"),
            std::env::var_os("CARGO"),
            std::env::var_os("RUSTC"),
            std::env::var_os("CARGO_MANIFEST_DIR"),
        ],
    ) {
        let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
        let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
        if !has_target_headers(&vcpkg_root, &target_os, &target_arch) {
            println!(
                "cargo:warning=Native {name} headers were not found; using checked-in bindings (the native library remains required when linking)"
            );
            Vec::new()
        } else {
            vec![link_vcpkg(vcpkg_root, name)]
        }
    } else if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        println!(
            "cargo:warning=Native {name} headers were not found; using checked-in bindings (the native library remains required when linking)"
        );
        Vec::new()
    } else {
        // Try using homebrew
        vec![link_homebrew_m1(name)]
    }
}

fn generate_bindings(
    ffi_header: &Path,
    include_paths: &[PathBuf],
    ffi_rs: &Path,
    checked_in_bindings: &Path,
) {
    if include_paths.is_empty() {
        std::fs::copy(checked_in_bindings, ffi_rs).unwrap_or_else(|error| {
            panic!(
                "failed to copy checked-in bindings from {} to {}: {error}",
                checked_in_bindings.display(),
                ffi_rs.display()
            )
        });
        return;
    }

    #[derive(Debug)]
    struct ParseCallbacks;
    impl bindgen::callbacks::ParseCallbacks for ParseCallbacks {
        fn int_macro(&self, name: &str, _value: i64) -> Option<bindgen::callbacks::IntKind> {
            if name.starts_with("OPUS") {
                Some(bindgen::callbacks::IntKind::Int)
            } else {
                None
            }
        }
    }
    let mut b = bindgen::Builder::default()
        .header(ffi_header.to_str().unwrap())
        .parse_callbacks(Box::new(ParseCallbacks))
        .generate_comments(false);

    for dir in include_paths {
        b = b.clang_arg(format!("-I{}", dir.display()));
    }

    b.generate().unwrap().write_to_file(ffi_rs).unwrap();
    std::fs::copy(ffi_rs, checked_in_bindings).unwrap_or_else(|error| {
        panic!(
            "failed to update checked-in bindings at {}: {error}",
            checked_in_bindings.display()
        )
    });
}

fn gen_opus() {
    let includes = find_package("opus");
    let src_dir = env::var_os("CARGO_MANIFEST_DIR").unwrap();
    let src_dir = Path::new(&src_dir);
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir);

    let ffi_header = src_dir.join("opus_ffi.h");
    println!("rerun-if-changed={}", ffi_header.display());
    for dir in &includes {
        println!("rerun-if-changed={}", dir.display());
    }

    let ffi_rs = out_dir.join("opus_ffi.rs");
    let checked_in_bindings = src_dir.join("generated").join("opus_ffi.rs");
    generate_bindings(&ffi_header, &includes, &ffi_rs, &checked_in_bindings);
}

fn main() {
    gen_opus()
}
