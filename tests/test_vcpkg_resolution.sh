#!/usr/bin/env bash
set -euo pipefail

test_binary="$(mktemp)"
trap 'rm -f "$test_binary"' EXIT

rustc --edition 2021 --test tests/test_vcpkg_resolution.rs -o "$test_binary"
"$test_binary"

grep -Fq 'magnum-opus = { path = "libs/magnum-opus" }' Cargo.toml
grep -Fq '#[path = "../vcpkg_root.rs"]' libs/magnum-opus/build.rs
grep -Fq '#[path = "../vcpkg_root.rs"]' libs/scrap/build.rs

for bindings in \
  libs/magnum-opus/generated/opus_ffi.rs \
  libs/scrap/generated/aom_ffi.rs \
  libs/scrap/generated/vpx_ffi.rs \
  libs/scrap/generated/yuv_ffi.rs; do
  test -s "$bindings"
done

grep -Fq 'using checked-in bindings' libs/magnum-opus/build.rs
grep -Fq 'using checked-in bindings' libs/scrap/build.rs
