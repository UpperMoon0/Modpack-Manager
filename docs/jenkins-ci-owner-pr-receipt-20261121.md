# KAN-62 native Jenkins CI acceptance canary

This owner-authored, same-repository pull request exercises the authenticated Jenkins `pull_request` route while the v1.0.15 release remains pinned.

The change is documentation-only. Do not merge it into `main`; Jenkins must produce its own complete Linux and Windows installer archives and original signed event/status receipts before the cutover verifier is allowed to accept them. GitHub Actions remain disabled.

## Protected Windows broker rerun

The first Jenkins owner-PR acceptance exposed an incorrectly qualified Windows worker. Central #755 restricts the Windows Tauri capability to on-prem-2, the reviewed MSI ICE host; live Jenkins labels now confirm the fix. This documentation-only update requests a fresh authenticated Jenkins PR run and must remain unmerged.
