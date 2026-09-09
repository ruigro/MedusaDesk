#!/usr/bin/env bash
set -euo pipefail

test_binary="$(mktemp)"
trap 'rm -f "$test_binary"' EXIT

rustc --edition 2021 --test tests/test_vcpkg_resolution.rs -o "$test_binary"
"$test_binary"

grep -Fq 'magnum-opus = { path = "libs/magnum-opus" }' Cargo.toml
grep -Fq '#[path = "../vcpkg_root.rs"]' libs/magnum-opus/build.rs
grep -Fq '#[path = "../vcpkg_root.rs"]' libs/scrap/build.rs
