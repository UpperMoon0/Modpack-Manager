#!/usr/bin/env bash
set -euo pipefail

RELEASE_URL="${MODPACK_MANAGER_TFG_RELEASE_URL:-https://raw.githubusercontent.com/UpperMoon0/TFG-Modern-Fork/refs/heads/nstut/stable/nstut/release.json}"
ROOT="${1:-${RUNNER_TEMP:-/tmp}/tfg-promotion-smoke}"

if command -v python3 >/dev/null 2>&1; then
  PYTHON=python3
elif command -v python >/dev/null 2>&1; then
  PYTHON=python
else
  echo "Python is required for TFG promotion validation" >&2
  exit 127
fi

release_json="$(curl --fail --silent --show-error --location "$RELEASE_URL")"
source_ref="$($PYTHON -c 'import json,sys; data=json.load(sys.stdin); ref=data.get("sourceRef", ""); assert ref.startswith("nstut-") and all(c.isalnum() or c in "._-" for c in ref), ref; print(ref)' <<<"$release_json")"
release_snapshot=".tfg-release-snapshot-$$.json"
printf '%s\n' "$release_json" > "$release_snapshot"
trap 'rm -f "$release_snapshot"' EXIT

rm -rf "$ROOT"
git clone --quiet --depth 1 --branch "$source_ref" https://github.com/UpperMoon0/TFG-Modern-Fork.git "$ROOT"
mkdir -p "$ROOT/mods"

MODPACK_MANAGER_TFG_RELEASE="$release_snapshot" cargo run --quiet -p modpackctl -- plan --tfg --target client --root "$ROOT" --json > /dev/null
MODPACK_MANAGER_TFG_RELEASE="$release_snapshot" cargo run --quiet -p modpackctl -- plan --tfg --target server --root "$ROOT" --json > /dev/null

echo "TFG promotion smoke passed for $source_ref"
