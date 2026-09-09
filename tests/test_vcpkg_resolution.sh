#!/usr/bin/env bash
set -euo pipefail

test_binary="$(mktemp)"
trap 'rm -f "$test_binary"' EXIT

rustc --edition 2021 --test tests/test_vcpkg_resolution.rs -o "$test_binary"
"$test_binary"
