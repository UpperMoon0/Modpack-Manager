# Code signing policy

Modpack Manager treats Windows Authenticode signing and Tauri updater signing as two separate trust boundaries.

## Public Windows releases

Stable Windows releases must:

- be built by the repository's GitHub Actions release workflow;
- be Authenticode-signed with a Code Signing certificate whose chain is trusted by Windows;
- use a certificate with the Code Signing EKU and an available private key;
- use SHA-256 and a trusted timestamp service;
- keep the GitHub release in draft state until the generated EXE and MSI both pass `Get-AuthenticodeSignature` with status `Valid`;
- publish SHA-256 checksums for the final signed installers;
- use the same trusted publisher identity across releases whenever possible so Windows reputation can accumulate.

Self-signed certificates are not accepted for public releases. Unsigned debug/development builds are for development only and must not be published as stable releases.

The release workflow currently accepts a PFX certificate through these GitHub Actions secrets:

- `WINDOWS_CERTIFICATE`: base64-encoded PFX bytes;
- `WINDOWS_CERTIFICATE_PASSWORD`: PFX import password.

A future HSM/cloud signing integration may replace the PFX transport without weakening the verification requirements above.

## Tauri updater signing

Updater signatures protect the integrity and authenticity of application updates independently of Authenticode.

The release workflow requires:

- `TAURI_SIGNING_PRIVATE_KEY`;
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.

The corresponding public key is embedded in `src-tauri/tauri.conf.json`. Release CI validates that the private key matches the embedded public key before building updater artifacts.

Updater private keys and passwords must never be committed to the repository or printed in CI logs.

## Release rule

A missing trusted Windows signing identity or missing updater signing key is a release blocker. The workflow must fail rather than fall back to a publicly downloadable unsigned installer.
