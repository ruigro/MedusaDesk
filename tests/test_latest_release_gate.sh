#!/usr/bin/env bash
set -euo pipefail

workflow=".github/workflows/flutter-build.yml"
tag_workflow=".github/workflows/flutter-tag.yml"
release_page="docs/assets/releases.js"

if grep -E '^[[:space:]]+prerelease:[[:space:]]+true[[:space:]]*$' "$workflow"; then
  echo "versioned builds must not be published as prereleases" >&2
  exit 1
fi

expected="prerelease: \${{ inputs.upload-tag == 'nightly' }}"
if ! grep -Fq "$expected" "$workflow"; then
  echo "release workflow must keep nightly builds as prereleases" >&2
  exit 1
fi

if ! grep -Fq 'releases/latest' "$release_page"; then
  echo "download page must resolve GitHub's latest release dynamically" >&2
  exit 1
fi

if ! grep -Fq 'contents: write' "$tag_workflow"; then
  echo "tag workflow must grant release publishing permission" >&2
  exit 1
fi

echo "latest release gate verified"
