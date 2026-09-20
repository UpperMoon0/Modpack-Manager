# Modpack Manager

A manifest-driven patch manager for modpacks.

This repository is a full replacement for the old GT New Horizons PyQt installer. The new system has one patch engine and two front ends:

- modpackctl: a headless Rust CLI for servers and automation.
- Tauri + React + TypeScript: a desktop patcher for players.

Both use the same patch-core crate. A patch manifest therefore has one meaning on the server and on every client.

## Why

Updating an upstream pack such as TFG should not mean manually copying custom mods, KubeJS scripts and configuration back into the server and telling every player to repeat the same fragile steps.

The intended flow is:

1. Update the upstream pack.
2. Publish the custom patch artifacts.
3. Bump one JSON manifest and its SHA-256 values.
4. Run the server patch script.
5. Players open Modpack Manager, preview the same manifest and apply it.

## Safety model

Patch manifests are data, not shell scripts. They cannot execute arbitrary commands.

Supported operations:

- removeMatching: remove obsolete files or directories by a relative glob.
- installFile: install a SHA-256 verified artifact at an exact relative path.
- extractZip: replace or merge a directory from a verified ZIP.
- writeText: write a manifest-managed text or config file.

The engine also:

- rejects absolute paths and parent traversal;
- blocks writes into .modpack-manager metadata;
- refuses modpack roots and destination components that are symlinks;
- rejects ZIP traversal and ZIP symlink entries;
- verifies all referenced artifacts before changing the modpack;
- serializes patch runs with a per-installation lock;
- backs up touched paths under .modpack-manager/backups;
- automatically rolls files back when a later operation fails;
- records client and server patch state separately.

## Manifest

See examples/tfg.patch.example.json.

Operations with no targets apply to both sides. Otherwise use client and/or server.

requiredPaths guards against selecting the wrong Minecraft directory. Relative artifact URLs are supported, so a manifest and its payloads can live together in one release/static directory.

## Server

Build the CLI:

    cargo build --release -p modpackctl

Install modpackctl on the server PATH, then run:

    ./scripts/patch-server.sh /srv/tfg https://example.com/tfg/patch.json

Environment-variable form:

    export MODPACK_ROOT=/srv/tfg
    export PATCH_MANIFEST=https://example.com/tfg/patch.json
    ./scripts/patch-server.sh

Preview only:

    DRY_RUN=1 ./scripts/patch-server.sh

Recommended server update order:

    stop server
    replace/update the upstream TFG server pack
    run patch-server.sh
    start server

If patching fails, the script exits non-zero. Keep the server stopped instead of starting a half-patched pack.

## Player app

The desktop app asks for a patch manifest URL and the modpack game directory. It shows the exact change plan before applying anything and remembers the last source/directory locally.

Development:

    npm install
    npm run tauri dev

Production installer:

    npm run tauri build

## Authoring TFG patches

For a custom mod, remove old versions and install one exact verified JAR:

    {
      "type": "removeMatching",
      "pattern": "mods/MyCustomMod-*.jar",
      "targets": ["client", "server"]
    }

followed by an installFile operation for the new JAR.

For KubeJS or another script tree, build a deterministic ZIP:

    ./scripts/make-patch-archive.sh ./my-kubejs ./dist/kubejs.zip

Then use extractZip with cleanDestination true. This matters: scripts deleted from your patch source also disappear from the installation instead of surviving as stale files.

Client-only visual mods can target client. Dedicated-server scripts or mods can target server.

## CLI

Preview:

    modpackctl plan --target server --root /srv/tfg --manifest https://example.com/tfg/patch.json

Apply:

    modpackctl apply --target server --root /srv/tfg --manifest https://example.com/tfg/patch.json

Status:

    modpackctl status --target server --root /srv/tfg

All three commands support --json.

## Layout

    crates/patch-core/   shared patch schema, planning, checksum, backup and rollback engine
    crates/modpackctl/   headless/server CLI
    src/                 React + TypeScript player UI
    src-tauri/           Tauri native shell
    scripts/             server and patch-authoring helpers
    examples/            sample manifests

## Validation

    npm test
    npm run build
    cargo test -p patch-core -p modpackctl
    cargo clippy -p patch-core -p modpackctl --all-targets -- -D warnings

CI additionally builds the real Tauri app on Windows. Tag releases create the Windows player bundle and a Linux modpackctl server archive.
