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

# Exercise Install new against the same immutable release used by the update
# checks. Version/loader incompatibility must block Manager CI and publication.
install_plan="$ROOT/fresh-install-plan.json"
MODPACK_MANAGER_TFG_RELEASE="$release_snapshot" cargo run --quiet -p modpackctl -- install --tfg --root "$ROOT/fresh-instance" --dry-run --json > "$install_plan"
"$PYTHON" - "$ROOT" <<'PY'
import json, pathlib, sys
root = pathlib.Path(sys.argv[1])
plan = json.loads((root / 'fresh-install-plan.json').read_text(encoding='utf-8'))
release = json.loads((root / 'nstut/release.json').read_text(encoding='utf-8'))
lock = json.loads((root / 'pakku-lock.json').read_text(encoding='utf-8'))
assert plan['version'] == release['overlayVersion'], 'fresh install resolved the wrong release'
assert lock['mc_versions'] == [plan['minecraftVersion']], 'fresh install Minecraft version differs from the pack'
assert lock['loaders'] == {'forge': plan['forgeVersion']}, 'fresh install Forge version differs from the pack'
assert not (root / 'fresh-instance').exists(), 'dry run must leave the destination untouched'
PY

# This is a profile-resolution fixture, not an installed game. Seed the exact
# target-core/marker guard for cross-base releases; migration behaviour is tested
# separately against installed-root fixtures.
if [[ -f "$ROOT/nstut/base-migration.json" ]]; then
  "$PYTHON" - "$ROOT" <<'PY'
import hashlib, json, pathlib, sys, urllib.request
root = pathlib.Path(sys.argv[1])
metadata = json.loads((root / 'nstut/base-migration.json').read_text(encoding='utf-8'))
for target in metadata['targets'].values():
    download = next(x for x in target['downloads'] if x['path'] == target['targetFile'])
    path = root / target['targetFile']
    if not path.exists():
        data = urllib.request.urlopen(download['url'], timeout=120).read()
        assert hashlib.sha256(data).hexdigest() == target['targetSha256']
        path.write_bytes(data)
    (root / target['marker']).write_bytes(target['markerContent'].encode('utf-8'))
PY
fi

MODPACK_MANAGER_TFG_RELEASE="$release_snapshot" cargo run --quiet -p modpackctl -- plan --tfg --target client --root "$ROOT" --json > /dev/null
MODPACK_MANAGER_TFG_RELEASE="$release_snapshot" cargo run --quiet -p modpackctl -- plan --tfg --target server --root "$ROOT" --json > /dev/null

echo "TFG promotion smoke passed for $source_ref (fresh install, client update, server update)"
