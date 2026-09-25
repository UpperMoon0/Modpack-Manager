# Code signing policy

Modpack Manager treats Windows Authenticode signing and Tauri updater signing as two separate trust boundaries.

## Windows Authenticode signing

Windows Authenticode signing is optional and independent from Tauri updater signing.

When both GitHub Actions secrets are configured:

- `WINDOWS_CERTIFICATE`: base64-encoded PFX bytes;
- `WINDOWS_CERTIFICATE_PASSWORD`: PFX import password;

release CI imports the certificate, requires a private Code Signing certificate with a valid Windows trust chain and Code Signing EKU, configures SHA-256 signing with a timestamp service, and verifies the generated EXE and MSI with `Get-AuthenticodeSignature` before publishing the draft release.

If neither Windows certificate secret is configured, release CI skips Authenticode and publishes the normal Windows installers unsigned at the Windows publisher layer. Supplying only one of the two secrets is a configuration error. Self-signed certificates are not treated as trusted Authenticode identities.

## Tauri updater signing

Updater signatures protect the integrity and authenticity of application updates independently of Authenticode.

The release workflow requires:

- `TAURI_SIGNING_PRIVATE_KEY`;
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.

The corresponding public key is embedded in `src-tauri/tauri.conf.json`. Release CI validates that the private key matches the embedded public key before building updater artifacts.

Updater private keys and passwords must never be committed to the repository or printed in CI logs.

## Release rule

A missing updater signing key is a release blocker. A missing Windows Authenticode certificate is not. Stable releases must publish Tauri-signed updater artifacts; Windows Authenticode is applied only when a trusted certificate is configured.
