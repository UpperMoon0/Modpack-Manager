import assert from "node:assert/strict";
import test from "node:test";
import { verifyDesktopRelease } from "./verify-desktop-release.mjs";

function fixture() {
  const payload = Buffer.alloc(74, 1).toString("base64");
  const encoded = Buffer.from(`untrusted comment: test\n${payload}\n`).toString("base64");
  const manifest = { version: "1.2.3", platforms: {} };
  const filenames = ["manager.msi", "manager.AppImage"];
  for (const [i, platform] of ["windows-x86_64", "linux-x86_64"].entries()) {
    manifest.platforms[platform] = {
      signature: encoded,
      url: `https://github.com/UpperMoon0/Modpack-Manager/releases/download/v1.2.3/${filenames[i]}`
    };
  }
  const assets = [...filenames, ...filenames.map((name) => `${name}.sig`), "manager.deb",
    "SHA256SUMS-windows.txt", "SHA256SUMS-linux.txt", "modpackctl-linux-x86_64.tar.gz",
    "modpackctl-linux-x86_64.tar.gz.sha256"].map((name) => ({ name }));
  return { manifest, assets, pubkey: encoded };
}

test("accepts a complete two-platform release", () => {
  const { manifest, assets, pubkey } = fixture();
  verifyDesktopRelease("v1.2.3", manifest, assets, pubkey);
});

for (const platform of ["windows-x86_64", "linux-x86_64"]) {
  test(`rejects missing ${platform} metadata`, () => {
    const { manifest, assets, pubkey } = fixture();
    delete manifest.platforms[platform];
    assert.throws(() => verifyDesktopRelease("v1.2.3", manifest, assets, pubkey), /Missing .* updater entry/);
  });
}

test("rejects stale updater versions and URLs", () => {
  const { manifest, assets, pubkey } = fixture();
  assert.throws(() => verifyDesktopRelease("v1.2.4", manifest, assets, pubkey), /version/);
  manifest.platforms["linux-x86_64"].url = manifest.platforms["linux-x86_64"].url.replace("v1.2.3", "v1.2.2");
  assert.throws(() => verifyDesktopRelease("v1.2.3", manifest, assets, pubkey), /URL/);
});

test("rejects signatures from a different updater key", () => {
  const { manifest, assets, pubkey } = fixture();
  manifest.platforms["linux-x86_64"].signature = Buffer.from(
    `untrusted comment: wrong key\n${Buffer.alloc(74, 2).toString("base64")}\n`).toString("base64");
  assert.throws(() => verifyDesktopRelease("v1.2.3", manifest, assets, pubkey), /embedded updater key/);
});

for (const name of ["manager.AppImage", "manager.AppImage.sig", "manager.deb",
  "SHA256SUMS-linux.txt", "modpackctl-linux-x86_64.tar.gz"]) {
  test(`rejects a release missing ${name}`, () => {
    const { manifest, assets, pubkey } = fixture();
    assert.throws(() => verifyDesktopRelease("v1.2.3", manifest,
      assets.filter((asset) => asset.name !== name), pubkey), /Missing/);
  });
}
