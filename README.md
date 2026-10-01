# Modpack Manager

A Tauri + React + Rust updater for maintaining the NsTut TFG Forge 1.20.1 client/server patch set.

## TFG managed profile

The desktop client asks only for the TFG modpack folder. TFG policy is authored in the UpperMoon0/TFG-Modern-Fork fork. Modpack Manager reads the nstut/stable release pointer, cross-checks it against release.json stored in the immutable tag, then consumes that tag's generated manifest and managed-mod metadata.

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

## Linux desktop

Linux desktop builds target **x86_64**, using Ubuntu 22.04 as the build baseline. Releases provide an **AppImage** and a **Debian package** alongside the Windows installers and Linux server CLI.

Download the Linux installer from [GitHub Releases](https://github.com/UpperMoon0/Modpack-Manager/releases). Check it against `SHA256SUMS-linux.txt` in the same release.

For the AppImage, make the downloaded file executable and launch it:

    chmod +x ./Modpack.Manager_*.AppImage
    ./Modpack.Manager_*.AppImage

If your system cannot mount AppImages through FUSE, launch with `--appimage-extract-and-run` instead. Keep the AppImage in a folder you can write to; signed in-app updates replace that file.

For Debian/Ubuntu, install the downloaded `.deb` with `sudo apt install ./<downloaded-file>.deb` and open **Modpack Manager** from your application menu. The package manager installs its WebKitGTK/GTK runtime dependencies. Update Debian installations by installing a newer `.deb`; Tauri's Linux in-app updater is supported for AppImage installations.

Use **Browse** to select the TFG instance's `.minecraft` directory, including for Prism Launcher installations. The desktop patch workflow is the same on both operating systems.

## Desktop flow

1. Enter or browse to the TFG game directory.
2. Modpack Manager loads the pinned generated overlay and managed-mod metadata from the NsTut TFG fork.
3. Review the exact managed mod versions and the live filesystem diff against the desired patch state. Already-correct managed files are omitted from the diff.
4. Click **Apply TFG managed changes**.
5. All referenced artifacts are verified before any filesystem mutation.
6. Old managed JARs are backed up and removed.
7. Current managed JARs are installed.
8. If a later operation fails, touched paths are rolled back.

The client rechecks the fork release pointer while open. Publishing a compatible TFG overlay only requires creating a new immutable fork tag and promoting nstut/stable; it does not require a Modpack Manager rebuild. CI and release validation run scripts/check-tfg-promotion.sh against the promoted tag so Manager does not duplicate fork policy in hard-coded mod counts or config assertions.

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
- `patchu◊~≠¢Gß≤⁄Óù∆≠y—.0.1",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc -b && vite build",
    "test": "vitest run",
    "tauri": "tauri",
    "tauri:linux": "tauri build --config src-tauri/tauri.linux.conf.json",
    "test:release": "node --test scripts/verify-desktop-release.check.mjs"
  },
  "dependencies": {
    "@tauri-apps/api": "^2.11.0",
    "@tauri-apps/plugin-dialog": "^2.7.0",
    "@tauri-apps/plugin-process": "^2.3.0",
    "@tauri-apps/plugin-updater": "^2.11.0",
    "react": "^19.1.1",
    "react-dom": "^19.1.1"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2.11.5",
    "@types/node": "^24.7.0",
    "@types/react": "^19.1.10",
    "@types/react-dom": "^19.1.7",
    "@vitejs/plugin-react": "^5.0.4",
    "typescript": "~5.9.2",
    "vite": "^7.1.7",
    "vitest": "^3.2.4"
  }
}
