# Modpack Manager v1.0.17 — TFG 0.13.19 client compatibility

- Admit only reviewed TFG `0.13.19-nstut.*` for **fresh Prism installations**, alongside previously approved 0.13.12, 0.13.15, 0.13.17, and 0.13.18 bases.
- Keep the loader pinned to Minecraft 1.20.1 / Forge 47.4.13. Reject unknown 0.13.20+, malformed version strings, unsupported loaders, and missing complete-base migrations.
- Extend real installation/export/import roundtrip fixtures and explicit marker/loader tests. A 0.13.19 real manifest dry run is required by the protected TFG policy before promotion.
- Preserve the existing update signing key, updater-v2 feed and package identity unchanged. The legacy latest release **must remain v1.0.14**; new-key releases remain prereleases. Do not advance either updater feed without protected signer and existing-client acceptance.
- Never treat a source-only PR as a published signed desktop application. CLI-only promotion, if independently approved, does not claim a desktop/updater release.
