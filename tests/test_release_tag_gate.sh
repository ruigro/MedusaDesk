#!/usr/bin/env bash
set -euo pipefail

# A package built without MEDUSA_RELEASE_TAG reports v0.1.0-test, so its update
# check offers every published release, older ones included, as an update.
workflows_dir="${1:-.github/workflows}"
checked=0

for workflow in "$workflows_dir"/medusa-*.yml "$workflows_dir"/flutter-build.yml; do
  if ! grep -Fq "build.py" "$workflow"; then
    continue
  fi
  checked=$((checked + 1))
  if ! grep -Eq 'MEDUSA_RELEASE_TAG: "?\$\{\{ inputs\.' "$workflow"; then
    echo "$workflow builds packages without setting MEDUSA_RELEASE_TAG from its inputs" >&2
    exit 1
  fi
  case "$workflow" in
    */medusa-*.yml)
      if ! grep -Fq "release_tag must look like" "$workflow"; then
        echo "$workflow does not reject a missing or malformed release tag" >&2
        exit 1
      fi
      ;;
  esac
done

if [ "$checked" -eq 0 ]; then
  echo "no packaging workflows found in $workflows_dir" >&2
  exit 1
fi

echo "release tag gate verified ($checked workflows)"
