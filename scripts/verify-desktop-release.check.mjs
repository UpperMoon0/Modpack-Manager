import assert from "node:assert/strict";
import test from "node:test";
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { generateWindowsChecksums } from "./generate-windows-checksums.mjs";
import { verifyDesktopRelease, verifyChecksumFiles } from "./verify-desktop-release.mjs";

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

function checksumFixture() {
  const names = ["Modpack.Manager_1.2.3_x64-setup.exe", "Modpack.Manager_1.2.3_x64_en-US.msi",
    "manager.AppImage", "manager.deb", "modpackctl-linux-x86_64.tar.gz"];
  const assets = names.map((name) => ({ name,
    digest: `sha256:${createHash("sha256").update(`bytes:${name}`).digest("hex")}` }));
  const lines = (includes) => assets.filter(({ name }) => includes(name))
    .map(({ name, digest }) => `${digest.slice(7)}  ${name}\n`).join("");
  const checksums = {
    "SHA256SUMS-windows.txt": lines((name) => /\.(exe|msi)$/.test(name)),
    "SHA256SUMS-linux.txt": lines((name) => /\.(AppImage|deb)$/.test(name)),
    "modpackctl-linux-x86_64.tar.gz.sha256": lines((name) => name.endsWith(".tar.gz"))
  };
  return { assets, checksums };
}

test("Windows checksum generation names uploaded MSI and EXE assets and passes verification", (t) => {
  const { assets, checksums } = checksumFixture();
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "manager-checksums-"));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  const windows = assets.filter(({ name }) => /\.(exe|msi)$/.test(name));
  for (const { name } of windows) {
    const bundleDirectory = path.join(directory, path.extname(name).slice(1));
    fs.mkdirSync(bundleDirectory);
    const bundleName = name.replace("Modpack.Manager", "Modpack Manager");
    fs.writeFileSync(path.join(bundleDirectory, bundleName), `bytes:${name}`);
    fs.writeFileSync(path.join(bundleDirectory, `${bundleName}.sig`), "ignored signature");
  }
  checksums["SHA256SUMS-windows.txt"] = generateWindowsChecksums(directory);
  assert.deepEqual(checksums["SHA256SUMS-windows.txt"].trim().split("\n")
    .map((line) => line.slice(66)).sort(), windows.map(({ name }) => name).sort());
  verifyChecksumFiles(assets, checksums);
});

test("checksum verifier rejects original bundle names with spaces instead of uploaded names", () => {
  const { assets, checksums } = checksumFixture();
  checksums["SHA256SUMS-windows.txt"] = checksums["SHA256SUMS-windows.txt"]
    .replaceAll("Modpack.Manager", "Modpack Manager");
  assert.throws(() => verifyChecksumFiles(assets, checksums), /Unknown checksum asset/);
});

test("checksum verifier requires every uploaded Windows installer", () => {
  const { assets, checksums } = checksumFixture();
  checksums["SHA256SUMS-windows.txt"] = checksums["SHA256SUMS-windows.txt"]
    .split("\n").filter((line) => !line.endsWith(".exe")).join("\n");
  assert.throws(() => verifyChecksumFiles(assets, checksums), /Missing checksum for asset: .*\.exe/);
});

test("checksum verifier compares checksums with uploaded asset digests", () => {
  const { assets, checksums } = checksumFixture();
  assets[0].digest = `sha256:${"0".repeat(64)}`;
  assert.throws(() => verifyChecksumFiles(assets, checksums), /does not match uploaded asset digest/);
});
