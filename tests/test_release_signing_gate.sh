#!/usr/bin/env bash
set -euo pipefail

workflow="${1:-.github/workflows/flutter-build.yml}"
windows_release_job="$(sed -n '1,/^  build-for-windows-sciter:/p' "$workflow")"

require_text() {
  if ! grep -Fq -- "$1" <<< "$windows_release_job"; then
    echo "release signing gate is missing: $1" >&2
    exit 1
  fi
}

require_text "Require Windows code signing configuration"
require_text "SIGN_BASE_URL and SIGN_SECRET_KEY repository secrets"
require_text "Verify published Windows signatures"
require_text "Get-AuthenticodeSignature -FilePath"
require_text "if (\$signature.Status -ne 'Valid')"

if grep -Fq -- "env.SIGN_BASE_URL != '-2'" <<< "$windows_release_job"; then
  echo "release signing is still optional" >&2
  exit 1
fi

verify_line="$(grep -n -F "Verify published Windows signatures" "$workflow" | head -n 1 | cut -d: -f1)"
publish_line="$(grep -n -F -- "- name: Publish Release" "$workflow" | head -n 1 | cut -d: -f1)"
if [ "$verify_line" -ge "$publish_line" ]; then
  echo "signature verification must run before publishing" >&2
  exit 1
fi

echo "release signing gate verified"
