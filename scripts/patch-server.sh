#!/usr/bin/env bash
set -euo pipefail

ROOT="${MODPACK_ROOT:-${1:-}}"
CHANNEL="${PATCH_CHANNEL:-${2:-}}"
MANIFEST="${PATCH_MANIFEST:-}"
MODPACKCTL="${MODPACKCTL:-modpackctl}"

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

if [[ "${DRY_RUN:-0}" == "1" ]]; then
  exec "$MODPACKCTL" plan --target server --root "$ROOT" "${SOURCE_ARGS[@]}"
fi

"$MODPACKCTL" plan --target server --root "$ROOT" "${SOURCE_ARGS[@]}"
exec "$MODPACKCTL" apply --target server --root "$ROOT" "${SOURCE_ARGS[@]}"
