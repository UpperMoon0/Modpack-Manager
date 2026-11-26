import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import test from "node:test";
import { verifyDesktopRelease } from "./verify-desktop-release.mjs";

const VERSION = "1.0.18";
const TAG = "v" + VERSION;
const KEY = Buffer.alloc(8, 7);
const platforms = {
  "windows-x86_64": "manager_1.0.18_x64_en-US.msi",
  "windows-x86_64-msi": "manager_1.0.18_x64_en-US.msi",
  "windows-x86_64-nsis": "manager_1.0.18_x64-setup.exe",
  "linux-x86_64": "manager_1.0.18_amd64.AppImage",
  "linux-x86_64-appimage": "manager_1.0.18_amd64.AppImage",
  "linux-x86_64-deb": "manager_1.0.18_amd64.deb"
};
function encode(text) { return Buffer.from(text).toString("base64"); }
function signature(name, version = VERSION, key = KEY) {
  const record = Buffer.concat([Buffer.from("ED"), key, Buffer.alloc(64, 1)]);
  return encode(`untrusted comment: signature\n${record.toString("base64")}\ntrusted comment: timestamp:1\tfile:${name}\tversion:${version}\n${Buffer.alloc(64, 2).toString("base64")}\n`);
}
function asset(name, bytes = "payload") {
  return {name, size: Buffer.byteLength(bytes), digest: "sha256:" + createHash("sha256").update(bytes).digest("hex")};
}
function fixture() {
  const pubkey = encode("untrusted comment: minisign public key TEST\n" +
    Buffer.concat([Buffer.from("Ed"), KEY, Buffer.alloc(32, 3)]).toString("base64") + "\n");
  const manifest = { version: VERSION, platforms: {} };
  for (const [platform, name] of Object.entries(platforms)) {
    manifest.platforms[platform] = {signature:signature(name),
      url:`https://github.com/UpperMoon0/Modpack-Manager/releases/download/${TAG}/${name}`};
  }
  const names = [...new Set(Object.values(platforms))];
  const assets = [...names.map(n => asset(n)), ...names.map(n => asset(n+".sig", signature(n))),
    ...["latest.json","SHA256SUMS-windows.txt","SHA256SUMS-linux.txt","modpackctl-linux-x86_64.tar.gz",
      "modpackctl-linux-x86_64.tar.gz.sha256"].map(n => asset(n))];
  return {manifest, assets, pubkey};
}
function verify(f) { return verifyDesktopRelease(TAG, f.manifest, f.assets, f.pubkey); }
test("accepts complete six-platform four-installer metadata", () => verify(fixture()));
for (const platform of Object.keys(platforms)) {
  test("rejects missing " + platform, () => {
    const f=fixture();delete f.manifest.platforms[platform];
    assert.throws(() => verify(f), /platform metadata/);
  });
}
for (const name of [...new Set(Object.values(platforms))].flatMap(n => [n,n+".sig"]).concat(
  ["latest.json","SHA256SUMS-linux.txt","SHA256SUMS-windows.txt","modpackctl-linux-x86_64.tar.gz",
  "modpackctl-linux-x86_64.tar.gz.sha256"])) {
  test("rejects missing " + name, () => {
    const f=fixture();f.assets=f.assets.filter(a=>a.name!==name);
    assert.throws(() => verify(f), /Missing/);
  });
}
test("rejects stale version, unsafe URL and wrong installer type", () => {
  const f=fixture();f.manifest.version="1.0.17";assert.throws(()=>verify(f), /version/);
  f.manifest.version=VERSION;
  f.manifest.platforms["linux-x86_64"].url=f.manifest.platforms["linux-x86_64"].url.replace(TAG,"v1.0.17");
  assert.throws(()=>verify(f), /URL/);
  f.manifest.platforms["linux-x86_64"].url=f.manifest.platforms["linux-x86_64-deb"].url;
  assert.throws(()=>verify(f), /filename/);
});
test("rejects wrong key, wrong signed version, and wrong signed filename", () => {
  for (const sig of [signature(platforms["linux-x86_64-deb"], VERSION, Buffer.alloc(8, 9)),
    signature(platforms["linux-x86_64-deb"],"1.0.17"),signature("different.deb")]) {
    const f=fixture();f.manifest.platforms["linux-x86_64-deb"].signature=sig;
    assert.throws(()=>verify(f), /key|filename|version/);
  }
});
test("rejects changed hosted signature metadata and duplicate or unexpected assets", () => {
  const f=fixture();const sig=f.assets.find(a=>a.name.endsWith(".sig"));sig.digest="sha256:"+"0".repeat(64);
  assert.throws(()=>verify(f), /signature digest/);
  const dupe=fixture();dupe.assets.push(dupe.assets[0]);assert.throws(()=>verify(dupe), /Duplicate/);
  const extra=fixture();extra.assets.push(asset("extra.txt"));assert.throws(()=>verify(extra), /Unexpected/);
});
test("rejects divergent default and explicit platform aliases", () => {
  const f=fixture();const name="alternate_1.0.18_x64_en-US.msi";const sig=signature(name);
  f.manifest.platforms["windows-x86_64-msi"]={signature:sig,
    url:`https://github.com/UpperMoon0/Modpack-Manager/releases/download/${TAG}/${name}`};
  f.assets.push(asset(name),asset(name+".sig",sig));
  assert.throws(()=>verify(f), /aliases/);
});
