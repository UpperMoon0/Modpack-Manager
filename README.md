# Modpack Manager

A Tauri + React + Rust updater for maintaining the NsTut TFG Forge 1.20.1 client/server patch set.

## TFG managed profile

The desktop client asks only for the TFG modpack folder. TFG policy is authored in the UpperMoon0/Modpack-Modern fork. Modpack Manager reads the nstut/stable release pointer, then consumes the immutable tag named by that pointer.

Managed mods:

| Mod | Source | Client | Server |
| --- | --- | :---: | :---: |
| Economy | GitHub ? UpperMoon0/Economy | yes | yes |
| OpenUI MC | GitHub ? UpperMoon0/OpenUI-MC | yes | no |
| Create: Precise Controls | GitHub ? UpperMoon0/Create-Precise-Controls | yes | no |
| Simply Screens | GitHub ? UpperMoon0/Simply-Screens | yes | yes |
| Simply Speakers | GitHub ? UpperMoon0/Simply-Speakers | yes | yes |
| Create Horse Power - CE | GitHub ? UpperMoon0/CreateHorsePower-CE | yes | yes |
| Building Gadgets Extra | GitHub ? UpperMoon0/Building-Gadgets-Extra | yes | yes |
| Create: Extra Gauges | Modrinth ? extra-gauges | yes | yes |

Managed mod versions and exact Forge 1.20.1 artifact hashes are pinned in the TFG fork. Updating a managed mod is an explicit fork commit, so upstream-pack merges and mod-version changes are reviewed together instead of being resolved implicitly at runtime.

All managed artifacts are selected and hashed in the fork. Modpack Manager verifies those pinned SHA-256 values before modifying an installation.

### Replacement behavior

Before installing a managed release, the patch removes matching older JARs.

Important special cases:

- `mods/createhorsepower-*.jar` is removed before Create Horse Power - CE is installed. This removes both the original Create Horse Power mod and previous CE builds.
- TFG Core intentionally redirects Forge `SERVER` configs to the game-level `defaultconfigs/` directory. For Create Horse Power - CE, `defaultconfigs/createhorsepower-server.toml` is therefore the authoritative live TFG config; the managed profile does not patch `world/serverconfig/` copies.
- OpenUI and Create: Precise Controls are installed on clients only. On the server, stale `openui-mc-*.jar` and `create-precise-controls-*.jar` copies are removed and no replacement is installed.
- historical filename variants for Simply Screens, Simply Speakers and Building Gadgets Extra are also cleaned up.

The TFG folder must contain:

    mods/
    config/
    kubejs/

This prevents selecting an unrelated Minecraft instance accidentally.

## Desktop flow

1. Enter or browse to the TFG game directory.
2. Modpack Manager loads the pinned generated overlay and managed-mod metadata from the NsTut TFG fork.
3. Review the exact managed mod versions and the live filesystem diff against the desired patch state. Already-correct managed files are omitted from the diff.
4. Click **Apply TFG managed changes**.
5. All referenced artifacts are verified before any filesystem mutation.
6. Old managed JARs are backed up and removed.
7. Current managed JARs are installed.
8. If a later operation fails, touched paths are rolled back.

The client rechecks the fork release pointer while open. Publishing a compatible TFG overlay only requires creating a new immutable fork tag and promoting nstut/stable; it does not require a Modpack Manager rebuild.

## Server flow

The server CLI uses the same fork-generated TFG profile:

    modpackctl plan --tfg --target server --root /srv/tfg
    modpackctl apply --tfg --target server --root /srv/tfg

The wrapper defaults to the TFG profile:

    ./scripts/patch-server.sh /srv/tfg

Preview the live server-state diff without applying anything:

    DRY_RUN=1 ./scripts/patch-server.sh /srv/tfg

The normal server wrapper prints the same live diff immediately before applying it.

`PATCH_CHANNEL` and `PATCH_MANIFEST` remain available as generic overrides for other patch sets.

## Generic patch engine

The shared Rust `patch-core` still supports declarative manifests and stable remote channels.

Supported operations:

- `removeMatching`
- `installFile`
- `extractZip`
- `writeText`
- `patchToml`
- `patchYaml`
- `patchSnbt`

Safety guarantees include:

- SHA-256 verification before modification;
- absolute-path and parent-traversal rejection;
- ZIP traversal and ZIP symlink rejection;
- symlink-safe destination handling;
- per-installation locking;
- backups under `.modpack-manager/backups`;
- automatic rollback after a failed later operation;
- live pre-patch diffing for managed files, text, TOML, YAML and SNBT scalar overlays;
- no-op skipping so already-correct managed files are not rewritten;
- separate client and server patch state.

## Modpack Manager self-update

The app uses the Tauri updater against:

    https://github.com/UpperMoon0/Modpack-Manager/releases/latest/download/latest.json

It checks on launch and every six hours. Application updates are presented in a global app-update area, separate from the selected TFG installation. When a newer signed release is detected, an in-app notification offers a one-click install-and-restart action. Silent automatic checks degrade quietly on transient network failures; manual checks report errors in the application panel.

Updater releases are signed. Release CI requires:

- `TAURI_SIGNING_PRIVATE_KEY`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`

The public verification key is embedded in `src-tauri/tauri.conf.json`; the private key is never committed.

Windows Authenticode signing is optional and independent from updater signing. When both `WINDOWS_CERTIFICATE` and `WINDOWS_CERTIFICATE_PASSWORD` are configured, release CI validates the trusted Code Signing certificate and verifies the generated EXE/MSI signatures before publication. Without those secrets, Windows installers are published without Authenticode while Tauri updater signing remains mandatory. SHA-256 checksums are published in either case. See [CODE_SIGNING.md](CODE_SIGNING.md).

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

    crates/patch-core/   pinned TFG fork-manifest resolver + generic patch/channel/backup/rollback engine
    crates/modpackctl/   server/headless CLI
    src/                 React + TypeScript TFG UI and app updater
    src-tauri/           Tauri commands and updater plugins
    scripts/             server wrapper + release helpers
    examples/            generic channel/manifest examples
