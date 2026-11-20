# KAN-68: lost Tauri updater signing key — manual trust migration

Status: **staging only, not a published release**. The old deployed clients
(v1.0.1–v1.0.14) all embed the legacy minisign verifier. They cannot accept a
new signing key through Tauri's existing signed update mechanism. Never weaken
signature validation or claim that an unsigned bridge exists.

## Two independent update channels

- **Legacy:** \`https://github.com/UpperMoon0/Modpack-Manager/releases/latest/download/latest.json\`.
  Preserve v1.0.14 and the 14 historical tags/assets. All new-key releases
  must be created as GitHub **prereleases** (even when operationally stable),
  so GitHub's \`releases/latest\` continues to resolve to v1.0.14.
  Do not redirect or overwrite the old \`latest.json\`.
- **New:** \`https://raw.githubusercontent.com/UpperMoon0/Modpack-Manager/refs/heads/main/updater-v2/latest.json\`.
  This public, reviewed manifest is updated only after a new-key signed
  release is fully verified. Each platform points to immutable tag assets.
  Treat the manifest update as a separate final, reversible publication step.

## Provisioning the new key

The Azure Key Vault secrets \`nstutcommonkv/tauri-private-key\` and
\`nstutcommonkv/tauri-key-password\` sign successfully, but their public
identity does not match the shipped legacy verifier. They are valid *only*
for the new channel, after protected-host RBAC and private-key challenge
verification. The **public** minisign .pub file is safe to commit; the private
key and password must never appear in GitHub, a Jenkins worker, logs, PRs or
artifacts.

The protected cloud-1 Azure identity currently cannot read those secrets.
Grant only \`Key Vault Secrets User\` on both exact secret scopes to the
existing protected host identity. No vault-wide role, new worker permission,
or GitHub Actions fallback is needed.

Once access is approved, derive the public key in a root-only 0700 temporary
directory using minisign \`-R\` or the matching protected Tauri signer.
Prove it matches the Key Vault private key with an independently verified
fresh signed challenge. Record only the **public** record digest and public
key. Then, in an isolated source branch, run:

\`\`\`sh
node scripts/stage-updater-key-migration.mjs --public-key /secure/public-key.pub
npm run test:release
node --test scripts/stage-updater-key-migration.check.mjs
\`\`\`

The staging command fails closed on the original public key, a modified
legacy endpoint, a disabled signer or an altered app identifier. It stages
v1.0.15 source metadata with the new public verifier and feed. Have the
protected publisher re-check that exact approved digest against the private
key before creating any draft.

## Production acceptance sequence

1. Build v1.0.15 from the reviewed source SHA through Jenkins Linux and
   Windows (including the protected full-ICE MSI broker), then sign the
   archived bytes on the protected host with the new private key.
2. Publish \`v1.0.15\` as a **prerelease** with downloadable manual
   installers and checksum/signature assets. Never publish it through
   GitHub's legacy \`releases/latest\` route.
3. On an isolated clean machine and a snapshot of v1.0.14, install the
   downloaded v1.0.15 MSI/NSIS/DEB/AppImage over the prior application.
   Verify the same application identifier, preserved user settings,
   Minecraft worlds and modpack paths, with no uninstall/reset side effects.
4. Sign a new v1.0.16 test candidate with the same new key. Serve its
   signed updater manifest at the new feed and perform an actual updater
   download/install from v1.0.15. Reject a tampered asset, old-key signature,
   replay and mismatched version/platform. Only then enable production feed
   advancement.
5. Recheck that \`releases/latest\` is still v1.0.14, new installers are
   independently downloadable, rollback is rehearsed, and the protected
   Jenkins publisher has an immutable receipt. Leave KAN-68 open until done.

Existing clients must manually install one migration version. Updater-only
signature compatibility cannot be cryptographically restored without the lost
old private key. A new key must not be silently substituted into legacy
\`latest.json\`.
