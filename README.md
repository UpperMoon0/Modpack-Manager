# Modpack Manager

A Tauri + React + Rust updater for maintaining the NsTut TFG Forge 1.20.1 client/server patch set.

## TFG managed profile

The desktop client asks only for the TFG modpack folder. It resolves compatible releases at runtime and builds a patch plan automatically.

Managed mods:

| Mod | Source | Client | Server |
| --- | --- | :---: | :---: |
| Economy | GitHub Releases · UpperMoon0/Economy | yes | yes |
| OpenUI MC | GitHub Releases · UpperMoon0/OpenUI-MC | yes | no |
| Simply Screens | GitHub Releases · UpperMoon0/Simply-Screens | yes | yes |
| Simply Speakers | GitHub Releases · UpperMoon0/Simply-Speakers | yes | yes |
| Create Horse Power - CE | GitHub Releases · UpperMoon0/CreateHorsePower-CE | yes | yes |
| Building Gadgets Extra | GitHub Releases · UpperMoon0/Building-Gadgets-Extra | yes | yes |
| Create: Extra Gauges | Modrinth · extra-gauges | yes | yes |

For GitHub-hosted mods, the resolver scans stable releases and selects the newest asset whose filename is the Forge 1.20.1 production JAR. A newer 1.21-only release does not displace the newest compatible 1.20.1 release.

Create: Extra Gauges is resolved from the Modrinth API filtered to Minecraft 1.20.1 + Forge and selects the newest stable release.

The resulting artifact checksums are taken from GitHub release SHA-256 digests or Modrinth hashes. If a source does not provide a digest, Modpack Manager downloads the artifact once to derive its SHA-256 before constructing the patch plan.

### Replacement behavior

Before installing a managed release, the patch removes matching older JARs.

Important special cases:

- `mods/createhorsepower-*.jar` is removed before Create Horse Power - CE is installed. This removes both the original Create Horse Power mod and previous CE builds.
- TFG Core intentionally redirects Forge `SERVER` configs to the game-level `defaultconfigs/` directory. For Create Horse Power - CE, `defaultconfigs/createhorsepower-server.toml` is therefore the authoritative live TFG config; the managed profile does not patch `world/serverconfig/` copies.
- OpenUI is installed on clients only. On the server, any old `openui-mc-*.jar` is removed and no replacement is installed.
- historical filename variants for Simply Screens, Simply Speakers and Building Gadgets Extra are also cleaned up.

The TFG folder must contain:

    mods/
    config/
    kubejs/

This prevents selecting an unrelated Minecraft instance accidentally.

## Desktop flow

1. Enter or browse to the TFG game directory.
2. Modpack Manager resolves current releases from GitHub and Modrinth.
3. Review the exact managed mod versions and filesystem plan.
4. Click **Update TFG managed mods**.
5. All referenced artifacts are verified before any filesystem mutation.
6. Old managed JARs are backed up and removed.
7. Current managed JARs are installed.
8. If a later operation fails, touched paths are rolled back.

The client checks TFG releases automatically every 30 minutes while open.

## Server flow

The server CLI uses the same built-in TFG profile:

    modpackctl plan --tfg --target server --root /srv/tfg
    modpackctl apply --tfg --target server --root /srv/tfg

The wrapper defaults to the TFG profile:

    ./scripts/patch-server.sh /srv/tfg

Preview only:

    DRY_RUN=1 ./scripts/patch-server.sh /srv/tfg

`PATCH_CHANNEL` and `PATCH_MANIFEST` remain available as generic overrides for other patch sets.

## Generic patch engine

The shared Rust `patch-core` still supports declarative manifests and stable remote channels.

Supported operations:

- `removeMatching`
- `installFile`
- `extractZip`
- `writeText`

Safety guarantees include:

- SHA-256 verification before modification;
- absolute-path and parent-traversal rejection;
- ZIP traversal and ZIP symlink rejection;
- symlink-safe destination handling;
- per-installation locking;
- backups under `.modpack-manager/backups`;
- automatic rollback after a failed later operation;
- separate client and server patch state.

## Modpack Manager self-update

The app uses the Tauri updater against:

    https://github.com/UpperMoon0/Modpack-Manager/releases/latest/download/latest.json

It checks shortly after launch and every six hours. When a newer frontend release exists, the app can download, verify, install and relaunch itself without requiring the player to manually reinstall it.

Updater releases are signed. Release CI requires:

- `TAURI_SIGNING_PRIVATE_KEY`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`

The public verification key is embedded in `src-tauri/tauri.conf.json`; the private key is never committed.

## Development

Frontend:

    npm install
    npm test
    npm run build

Rust:

    cargo test -p patch-core -p modpackctl
    cargo clippy -p patch-core -p modpackctl --all-targets -- -D warnings

Native Windows app:

    npm run tauri build -- --debug

CI runs all of these, including the real Windows Tauri installer bundle.

## Repository layout

    crates/patch-core/   TFG resolver + generic patch/channel/backup/rollback engine
    crates/modpackctl/   server/headless CLI
    src/                 React + TypeScript TFG UI and app updater
    src-tauri/           Tauri commands and updater plugins
    scripts/             server wrapper + release helpers
    examples/            generic channel/manifest examples
