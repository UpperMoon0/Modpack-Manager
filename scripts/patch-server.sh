#!/usr/bin/env bash
set -euo pipefail

ROOT="${MODPACK_ROOT:-${1:-}}"
MANIFEST="${PATCH_MANIFEST:-${2:-}}"
MODPACKCTL="${MODPACKCTL:-modpackctl}"

if [[ -z "$ROOT" || -z "$MANIFEST" ]]; then
  cat >&2 <<'USAGE'
Usage:
  patch-server.sh <server-root> <manifest-url-or-file>

Or set:
  MODPACK_ROOT=/srv/tfg
  PATCH_MANIFEST=https://example.com/tfg/patch.json
  MODPACKCTL=/usr/local/bin/modpackctl

Set DRY_RUN=1 to preview without changing files.
USAGE
  exit 2
fi

if ! command -v "$MODPACKCTL" >/dev/null 2>&1; then
  echo "modpackctl was not found: $MODPACKCTL" >&2
  exit 127
fi

if [[ "${DRY_RUN:-0}" == "1" ]]; then
  exec "$MODPACKCTL" plan --target server --root "$ROOT" --manifest "$MANIFEST"
fi

"$MODPACKCTL" plan --target server --root "$ROOT" --manifest "$MANIFEST"
exec "$MODPACKCTL" apply --target server --root "$ROOT" --manifest "$MANIFEST"
