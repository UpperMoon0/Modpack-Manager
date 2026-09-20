#!/usr/bin/env bash
set -euo pipefail

ROOT="${MODPACK_ROOT:-${1:-}}"
CHANNEL="${PATCH_CHANNEL:-${2:-}}"
MANIFEST="${PATCH_MANIFEST:-}"
MODPACKCTL="${MODPACKCTL:-modpackctl}"

if [[ -z "$ROOT" ]]; then
  cat >&2 <<'USAGE'
Usage:
  patch-server.sh <server-root> <channel-url-or-file>

Preferred environment:
  MODPACK_ROOT=/srv/tfg
  PATCH_CHANNEL=https://example.com/tfg/channel.json
  MODPACKCTL=/usr/local/bin/modpackctl

PATCH_MANIFEST is also supported for pinning one exact manifest.
Set DRY_RUN=1 to preview without changing files.
USAGE
  exit 2
fi

if ! command -v "$MODPACKCTL" >/dev/null 2>&1; then
  echo "modpackctl was not found: $MODPACKCTL" >&2
  exit 127
fi

SOURCE_ARGS=()
if [[ -n "$MANIFEST" ]]; then
  SOURCE_ARGS=(--manifest "$MANIFEST")
elif [[ -n "$CHANNEL" ]]; then
  SOURCE_ARGS=(--channel "$CHANNEL")
else
  echo "Set PATCH_CHANNEL (preferred), PATCH_MANIFEST, or pass the channel as argument 2." >&2
  exit 2
fi

if [[ "${DRY_RUN:-0}" == "1" ]]; then
  exec "$MODPACKCTL" plan --target server --root "$ROOT" "${SOURCE_ARGS[@]}"
fi

"$MODPACKCTL" plan --target server --root "$ROOT" "${SOURCE_ARGS[@]}"
exec "$MODPACKCTL" apply --target server --root "$ROOT" "${SOURCE_ARGS[@]}"
