import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import fs from "node:fs";
import { pathToFileURL } from "node:url";

const PLATFORMS = {
  "windows-x86_64": ".msi", "windows-x86_64-msi": ".msi",
  "windows-x86_64-nsis": "-setup.exe", "linux-x86_64": ".AppImage",
  "linux-x86_64-appimage": ".AppImage", "linux-x86_64-deb": ".deb"
};
const FIXED_ASSETS = [
  "latest.json", "SHA256SUMS-windows.txt", "SHA256SUMS-linux.txt",
  "modpackctl-linux-x86_64.tar.gz", "modpackctl-linux-x86_64.tar.gz.sha256"
];

function decode(encoded, label) {
  assert.equal(typeof encoded, "string", `${label} is missing`);
  assert.ok(encoded.length > 0 && encoded.length <= 8192, `${label} size is invalid`);
  const bytes = Buffer.from(encoded, "base64");
  assert.equal(bytes.toString("base64"), encoded, `${label} is not canonical base64`);
  return bytes;
}

function publicKeyId(encoded) {
  const lines = decode(encoded, "Embedded updater public key").toString("utf8").trimEnd().split(/\r?\n/);
  assert.equal(lines.length, 2, "Invalid embedded updater key");
  assert.ok(lines[0].startsWith("untrusted comment: minisign public key"), "Invalid embedded updater key header");
  const record = decode(lines[1], "Embedded updater key record");
  assert.equal(record.length, 42, "Invalid embedded updater key length");
  assert.equal(record.subarray(0, 2).toString("ascii"), "Ed", "Invalid embedded updater key algorithm");
  return record.subarray(2, 10);
}

function verifySignature(encoded, key, filename, version) {
  const lines = decode(encoded, "Updater signature").toString("utf8").trimEnd().split(/\r?\n/);
  assert.equal(lines.length, 4, "Incomplete minisign signature");
  assert.ok(lines[0].startsWith("untrusted comment:"), "Invalid minisign signature header");
  const record = decode(lines[1], "Updater signature record");
  assert.equal(record.length, 74, "Invalid minisign signature record length");
  assert.equal(record.subarray(0, 2).toString("ascii"), "ED", "Invalid updater signature algorithm");
  assert.ok(record.subarray(2, 10).equals(key), "Signature does not match embedded updater key");
  assert.equal(decode(lines[3], "Minisign trusted comment signature").length, 64,
    "Invalid minisign trusted comment signature length");
  const fields = lines[2].split("\t");
  assert.match(fields[0], /^trusted comment: timestamp:[0-9]+$/, "Invalid signed timestamp");
  assert.deepEqual(fields.slice(1), [`file:${filename}`, `version:${version}`],
    "Signed filename or version does not match release");
}

// Structural and hosted metadata checks, not cryptographic proof of bytes.
// The root publisher independently verifies minisign against the real packages.
export function verifyDesktopRelease(tag, manifest, assets, pubkey) {
  assert.match(tag, /^v(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)$/, "Expected stable release tag");
  const version = tag.slice(1);
  assert.equal(manifest.version, version, "Updater version does not match release tag");
  assert.ok(Array.isArray(assets), "Missing hosted asset inventory");
  const byName = new Map();
  for (const asset of assets) {
    assert.ok(asset && typeof asset.name === "string" && /^[A-Za-z0-9_.-]{1,180}$/.test(asset.name)
      && !asset.name.includes(".."), "Invalid hosted asset name");
    assert.ok(!byName.has(asset.name), "Duplicate hosted asset name");
    assert.ok(Number.isSafeInteger(asset.size) && asset.size > 0 && asset.size <= 260_000_000,
      "Invalid hosted asset size");
    assert.match(asset.digest ?? "", /^sha256:[0-9a-f]{64}$/, "Missing hosted asset SHA256 digest");
    byName.set(asset.name, asset);
  }
  const key = publicKeyId(pubkey);
  assert.ok(manifest.platforms && typeof manifest.platforms === "object", "Missing platform map");
  assert.deepEqual(Object.keys(manifest.platforms).sort(), Object.keys(PLATFORMS).sort(),
    "Missing or unexpected updater platform metadata");
  const prefix = `https://github.com/UpperMoon0/Modpack-Manager/releases/download/${tag}/`;
  const installers = new Map();
  for (const [platform, suffix] of Object.entries(PLATFORMS)) {
    const entry = manifest.platforms[platform];
    assert.ok(entry && typeof entry.url === "string" && entry.url.startsWith(prefix),
      `${platform} updater URL does not reference this release`);
    const name = entry.url.slice(prefix.length);
    assert.ok(/^[A-Za-z0-9_.-]{1,180}$/.test(name) && !name.includes("..") &&
      name.includes(`_${version}_`) && name.endsWith(suffix), "Invalid updater installer filename or URL");
    assert.ok(byName.has(name), `Missing updater asset: ${name}`);
    const sig = byName.get(name + ".sig");
    assert.ok(sig, `Missing updater signature asset: ${name}.sig`);
    verifySignature(entry.signature, key, name, version);
    assert.equal(sig.size, Buffer.byteLength(entry.signature), "Hosted signature size mismatch");
    assert.equal(sig.digest, "sha256:" + createHash("sha256").update(entry.signature).digest("hex"),
      "Hosted signature digest mismatch");
    if (installers.has(suffix)) {
      assert.deepEqual(installers.get(suffix), { name, signature: entry.signature },
        "Platform aliases disagree");
    } else {
      installers.set(suffix, { name, signature: entry.signature });
    }
  }
  assert.equal(installers.size, 4, "Missing four independent signed installers");
  const expected = new Set(FIXED_ASSETS);
  for (const {name} of installers.values()) {
    expected.add(name);
    expected.add(name + ".sig");
  }
  for (const name of expected) assert.ok(byName.has(name), `Missing release asset: ${name}`);
  assert.equal(byName.size, expected.size, "Unexpected hosted release assets");
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [tag, manifestPath, assetsPath] = process.argv.slice(2);
  assert.ok(tag && manifestPath && assetsPath, "Usage: verify-desktop-release.mjs TAG LATEST_JSON ASSETS_JSON");
  const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
  const { assets } = JSON.parse(fs.readFileSync(assetsPath, "utf8"));
  const config = JSON.parse(fs.readFileSync("src-tauri/tauri.conf.json", "utf8"));
  verifyDesktopRelease(tag, manifest, assets, config.plugins.updater.pubkey);
  console.log("All six updater entries, four signature identities and hosted release metadata verified");
}
