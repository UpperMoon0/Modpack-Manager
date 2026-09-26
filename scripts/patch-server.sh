#!/usr/bin/env bash
set -euo pipefail

ROOT="${MODPACK_ROOT:-${1:-}}"
CHANNEL="${PATCH_CHANNEL:-${2:-}}"
MANIFEST="${PATCH_MANIFEST:-}"
MODPACKCTL="${MODPACKCTL:-modpackctl}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FTB_LIMIT_PATCH="${FTB_LIMIT_PATCH:-$SCRIPT_DIR/patch-ftb-chunk-limits.py}"
FTB_MAX_CLAIMED_CHUNKS="${FTB_MAX_CLAIMED_CHUNKS:-1000000}"
FTB_MAX_FORCE_LOADED_CHUNKS="${FTB_MAX_FORCE_LOADED_CHUNKS:-1000000}"

if [[ -z "$ROOT" ]]; then
  cat >&2 <<'USAGE'
Usage:
  patch-server.sh <tfg-server-root> [channel-url-or-file]

By default this applies the built-in TFG Forge 1.20.1 profile:
  Economy
  Simply Screens
  Simply Speakers
  Create Horse Power - CE
  Building Gadgets Extra
  Create: Extra Gauges

OpenUI is removed from servers and installed only on clients.

Optional overrides:
  PATCH_CHANNEL=https://example.com/tfg/channel.json
  PATCH_MANIFEST=https://example.com/tfg/patch.json
  MODPACKCTL=/usr/local/bin/modpackctl
  FTB_MAX_CLAIMED_CHUNKS=1000000
  FTB_MAX_FORCE_LOADED_CHUNKS=1000000

The server wrapper also enforces those FTB Chunks limits in the active world
and existing FTB Ranks overrides, preserving unrelated SNBT settings.

Set DRY_RUN=1 to preview without changing files.
USAGE
  exit 2
fi

if ! command -v "$MODPACKCTL" >/dev/null 2>&1; then
  echo "modpackctl was not found: $MODPACKCTL" >&2
  exit 127
fi

SOURCE_ARGS=(--tfg)
if [[ -n "$MANIFEST" ]]; then
  SOURCE_ARGS=(--manifest "$MANIFEST")
elif [[ -n "$CHANNEL" ]]; then
  SOURCE_ARGS=(--channel "$CHANNEL")
fi

if [[ ! -x "$FTB_LIMIT_PATCH" ]]; then
  echo "FTB chunk-limit patch helper was not found or is not executable: $FTB_LIMIT_PATCH" >&2
  exit 127
fi

FTB_ARGS=(
  "$ROOT"
  --claimed "$FTB_MAX_CLAIMED_CHUNKS"
  --force-loaded "$FTB_MAX_FORCE_LOADED_CHUNKS"
)

if [[ "${DRY_RUN:-0}" == "1" ]]; then
  "$MODPACKCTL" plan --target server --root "$ROOT" "${SOURCE_ARGS[@]}"
  exec "$FTB_LIMIT_PATCH" "${FTB_ARGS[@]}" --dry-run
fi

"$MODPACKCTL" plan --target server --root "$ROOT" "${SOURCE_ARGS[@]}"
"$FTB_LIMIT_PATCH" "${FTB_ARGS[@]}" --dry-run
"$MODPACKCTL" apply --target server --root "$ROOT" "${SOURCE_ARGS[@]}"
exec "$FTB_LIMIT_PATCH" "${FTB_ARGS[@]}"
