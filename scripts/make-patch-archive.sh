#!/usr/bin/env bash
set -euo pipefail

SOURCE="${1:?Usage: make-patch-archive.sh <directory> <output.zip>}"
OUTPUT="${2:?Usage: make-patch-archive.sh <directory> <output.zip>}"

SOURCE="$(cd "$SOURCE" && pwd)"
mkdir -p "$(dirname "$OUTPUT")"
OUTPUT="$(cd "$(dirname "$OUTPUT")" && pwd)/$(basename "$OUTPUT")"

(
  cd "$SOURCE"
  zip -qr "$OUTPUT" .
)

if command -v sha256sum >/dev/null 2>&1; then
  sha256sum "$OUTPUT"
else
  shasum -a 256 "$OUTPUT"
fi
