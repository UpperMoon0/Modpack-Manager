# Modpack Manager

A manifest-driven modpack update and patch manager.

The old GT New Horizons PyQt installer has been replaced by one Rust patch engine with two front ends:

- **modpackctl** — headless CLI for servers and automation.
- **Tauri + React + TypeScript** — desktop client for players.

The server and desktop client consume the same remote patch data, so a TFG update is published once and discovered everywhere.

## Update architecture

There are deliberately two independent update layers.

### 1. Modpack patch channel

Players configure one stable channel URL once. It can be hosted as a GitHub Release asset, raw GitHub file, CDN object, or any HTTPS resource.

Example:

    https://github.com/owner/tfg-patches/releases/latest/download/channel.json

A channel is tiny metadata that points to the current patch manifest:

    {
      "schemaVersion": 1,
      "id": "tfg-nstut-stable",
      "name": "TFG NsTut Stable",
      "checkIntervalMinutes": 30,
      "latest": {
        "version": "2026.09.20.1",
        "manifest": "./tfg.patch.json",
        "notes": "Custom TFG compatibility layer."
      }
    }

The desktop client checks this channel on startup and periodically in the background. The channel URL does not change when a new TFG patch is published. Update the remote channel JSON, manifest and payloads; installed clients discover the new patch without a new Modpack Manager build.

Relative manifest and artifact URLs are supported. This makes GitHub Release assets convenient: upload `channel.json`, `tfg.patch.json`, JARs and patch ZIPs to one release.

`VITE_DEFAULT_PATCH_CHANNEL` can be set at build time to preconfigure a distribution so ordinary players never have to enter the channel URL.

See `examples/tfg.channel.example.json` and `examples/tfg.patch.example.json`.

### 2. Modpack Manager self-update

The desktop app uses Tauri's updater plugin against:

    https://github.com/UpperMoon0/Modpack-Manager/releases/latest/download/latest.json

It checks shortly after launch and every six hours. When a newer app exists, the UI offers **Install update**. The app downloads, verifies and installs the signed update itself; players do not need to download or reinstall a new frontend manually.

Updater artifacts are cryptographically signed. The public key is embedded in `src-tauri/tauri.conf.json`; the private key must exist only in release CI as `TAURI_SIGNING_PRIVATE_KEY`. `requireSignedVersion` is enabled so the signed artifact is bound to the advertised application version.

For tag releases, add repository secrets:

- `TAURI_SIGNING_PRIVATE_KEY` — contents of the private updater key.
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — blank for the current unencrypted CI key, or the password if the key is replaced with an encrypted one.

Tagging `vX.Y.Z` runs the release workflow, applies that version to the Tauri/package/Cargo metadata, enables updater artifacts for the release build, publishes the Windows installer plus signed updater artifacts and `latest.json`, then uploads the Linux server CLI.

## Patch safety model

Patch manifests are data, not shell scripts. They cannot execute arbitrary commands.

Supported operations:

- `removeMatching` — remove obsolete files or directories by a relative glob.
- `installFile` — install a SHA-256 verified artifact at an exact relative path.
- `extractZip` — replace or merge a directory from a verified ZIP.
- `writeText` — write a manifest-managed text/config file.

The engine also:

- rejects absolute paths and parent traversal;
- blocks writes into `.modpack-manager` metadata;
- refuses modpack roots and destination components that are symlinks;
- rejects ZIP traversal and ZIP symlink entries;
- downloads and verifies all referenced artifacts before changing the modpack;
- serializes patch runs with a per-installation lock;
- backs up touched paths under `.modpack-manager/backups`;
- automatically rolls files back when a later operation fails;
- records client and server patch state separately.

For overlay archives such as custom KubeJS files, use `cleanDestination: false` so unrelated upstream scripts are preserved. Use `cleanDestination: true` only when the patch owns the entire destination directory. Explicit `removeMatching` operations can remove obsolete managed paths when needed.

## Server

Build:

    cargo build --release -p modpackctl

Apply the same manifest the client channel points at:

    ./scripts/patch-server.sh /srv/tfg https://example.com/tfg/tfg.patch.json

Preview only:

    DRY_RUN=1 ./scripts/patch-server.sh /srv/tfg https://example.com/tfg/tfg.patch.json

Recommended update order:

    stop server
    update/replace upstream TFG server files
    run patch-server.sh
    start server

If patching fails, the command exits non-zero and restores the touched files from backup.

## Player app

Development:

    npm install
    npm run tauri dev

Normal CI:

    npm test
    npm run build
    cargo test -p patch-core -p modpackctl
    cargo clippy -p patch-core -p modpackctl --all-targets -- -D warnings

CI additionally builds the native Windows Tauri app.

## Repository layout

    crates/patch-core/   channel resolver + shared patch/backup/rollback engine
    crates/modpackctl/   headless/server CLI
    src/                 React + TypeScript player UI and background update checks
    src-tauri/           Tauri shell and signed self-updater
    scripts/             server helpers + release preparation
    examples/            channel and patch manifest examples
