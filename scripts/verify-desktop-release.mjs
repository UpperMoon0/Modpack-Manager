import assert from "node:assert/strict";
import fs from "node:fs";
import { pathToFileURL } from "node:url";

function keyId(encoded, label) {
  assert.equal(typeof encoded, "string", `${label} is missing`);
  const lines = Buffer.from(encoded.trim(), "base64").toString("utf8").trim().split(/\r?\n/);
  assert.ok(lines[0]?.startsWith("untrusted comment:"), `${label} is invalid`);
  const payload = Buffer.from(lines[1] ?? "", "base64");
  assert.ok(payload.length >= 10, `${label} payload is too short`);
  return payload.subarray(2, 10).toString("hex");
}

export function verifyDesktopRelease(tag, manifest, assets, pubkey) {
  assert.equal(manifest.version, tag.replace(/^v/, ""), "Updater version does not match release tag");
  const names = new Set(assets.map((asset) => asset.name));
  const publicKeyId = keyId(pubkey, "Embedded updater public key");
  for (const platform of ["windows-x86_64", "linux-x86_64"]) {
    const entry = manifest.platforms?.[platform];
    assert.ok(entry, `Missing ${platform} updater entry`);
    assert.equal(keyId(entry.signature, `${platform} signature`), publicKeyId,
      `${platform} signature does not match embedded updater key`);
    const prefix = `https://github.com/UpperMoon0/Modpack-Manager/releases/download/${encodeURIComponent(tag)}/`;
    assert.ok(typeof entry.url === "string" && entry.url.startsWith(prefix),
      `${platform} updater URL does not reference this release`);
    const filename = decodeURIComponent(entry.url.slice(prefix.length));
    assert.ok(names.has(filename), `Missing updater asset: ${filename}`);
    assert.ok(names.has(`${filename}.sig`), `Missing updater signature asset: ${filename}.sig`);
    if (platform === "linux-x86_64") {
      assert.ok(filename.endsWith(".AppImage"), "Linux updater must use an AppImage");
    }
  }
  for (const name of ["SHA256SUMS-windows.txt", "SHA256SUMS-linux.txt",
    "modpackctl-linux-x86_64.tar.gz", "modpackctl-linux-x86_64.tar.gz.sha256"]) {
    assert.ok(names.has(name), `Missing release asset: ${name}`);
  }
  assert.ok([...names].some((name) => name.endsWith(".deb")), "Missing Linux Debian installer");
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [tag, manifestPath, assetsPath] = process.argv.slice(2);
  assert.ok(tag && manifestPath && assetsPath, "Usage: verify-desktop-release.mjs TAG LATEST_JSON ASSETS_JSON");
  const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
  const { assets } = JSON.parse(fs.readFileSync(assetsPath, "utf8"));
  const config = JSON.parse(fs.readFileSync("src-tauri/tauri.conf.json", "utf8"));
  verifyDesktopRelease(tag, manifest, assets, config.plugins.updater.pubkey);
  console.log("Windows and Linux updater entries and release assets verified");
}
