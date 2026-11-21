# KAN-62 native Jenkins CI acceptance canary

This owner-authored, same-repository pull request exercises the authenticated Jenkins `pull_request` route while the v1.0.15 release remains pinned.

The change is documentation-only. Do not merge it into `main`; Jenkins must produce its own complete Linux and Windows installer archives and original signed event/status receipts before the cutover verifier is allowed to accept them. GitHub Actions remain disabled.
