# Modpack Manager v1.0.15 — manual trust migration

The updater signing key in v1.0.1–v1.0.14 cannot authenticate new-key installers. Download **v1.0.15** manually from the official [GitHub releases](https://github.com/UpperMoon0/Modpack-Manager/releases), selecting the signed migration installer for your operating system. The new release is intentionally marked as a GitHub prerelease to keep the legacy `releases/latest` updater manifest on v1.0.14.

## Protect your installed modpacks

Do not uninstall the old app or delete its data directory before upgrading. Close Minecraft and the Manager, back up your Manager preferences and pack directories, then install the new version **over** the existing installation. The app identifier remains `dev.nstut.modpack-manager`, and the new updater feed is separate from the legacy feed. Your Minecraft worlds and Prism instances must remain in their original directories. If the installer unexpectedly prompts to remove user data, cancel it.

After installing, verify **Settings → Manager version** reports 1.0.15, your saved pack locations still work, and check for application updates. Future versions signed with the new key can update automatically through `updater-v2/latest.json`.

Only use this procedure **after the actual signed migration installers and independent over-install test are published and verified**. The source change alone is not a released installer.
