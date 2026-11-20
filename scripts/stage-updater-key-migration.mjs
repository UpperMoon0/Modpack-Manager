#!/usr/bin/env node
/**
 * KAN-68: prepare a one-time MANUAL installer migration to a new updater key.
 * This script never reads a private key, signs, publishes or edits GitHub.
 * Run only after a protected challenge proves the reviewed .pub belongs to
 * the approved new private key.
 */
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";

export const LEGACY_SHA256 = "213b2487163f83490decb54ebfbc457cfae7f36ffd48ef46a4719c313f207e9b";
export const LEGACY_FEED =
  "https://github.com/UpperMoon0/Modpack-Manager/releases/latest/download/latest.json";
export const NEW_FEED =
  "https://raw.githubusercontent.com/UpperMoon0/Modpack-Manager/refs/heads/main/updater-v2/latest.json";

function decodeRecord(encoded) {
  assert.equal(typeof encoded, "string", "Missing public key");
  assert.match(encoded, /^[A-Za-z0-9+/]+={0,2}$/, "Invalid public key encoding");
  const bytes = Buffer.from(encoded, "base64");
  assert.equal(bytes.toString("base64"), encoded, "Noncanonical base64 public key");
  const lines = bytes.toString("utf8").trim().split(/\r?\n/);
  assert.ok(lines[0].startsWith("untrusted comment: minisign public key"), "Not a minisign public key");
  assert.match(lines[1] || "", /^[A-Za-z0-9+/]+={0,2}$/, "Invalid minisign record");
  const record = Buffer.from(lines[1], "base64");
  assert.equal(record.length, 42, "Minisign record must contain 42 bytes");
  assert.equal(record.subarray(0, 2).toString("ascii"), "Ed", "Expected an Ed25519 public record");
  return { record, digest: createHash("sha256").update(record).digest("hex") };
}

export function planMigration(current, publicKeyFile) {
  assert.equal(current.identifier, "dev.nstut.modpack-manager", "Application identity drift");
  const updater = current.plugins?.updater;
  assert.equal(updater?.requireSignedVersion, true, "Signing verification must stay enabled");
  assert.deepEqual(updater?.endpoints, [LEGACY_FEED], "Legacy updater endpoint drift");
  assert.equal(decodeRecord(updater.pubkey).digest, LEGACY_SHA256, "Legacy public key drift");
  const replacement = Buffer.from(publicKeyFile, "utf8").toString("base64");
  const digest = decodeRecord(replacement).digest;
  assert.notEqual(digest, LEGACY_SHA256, "Replacement key must differ from the lost legacy key");
  const migrated = structuredClone(current);
  migrated.version = "1.0.15";
  migrated.plugins.updater.pubkey = replacement;
  migrated.plugins.updater.endpoints = [NEW_FEED];
  return { config: migrated, digest };
}

export function stage(root, publicKeyPath) {
  const versionPath = path.join(root, "VERSION");
  const configPath = path.join(root, "src-tauri/tauri.conf.json");
  const packagePath = path.join(root, "package.json");
  const cargoPath = path.join(root, "Cargo.toml");
  assert.equal(fs.readFileSync(versionPath, "utf8").trim(), "1.0.14",
    "Migration requires original published version 1.0.14");
  assert.ok(!fs.existsSync(path.join(root, ".github/workflows/release.yml")),
    "Never restore the retired GitHub Actions publisher");
  const config = JSON.parse(fs.readFileSync(configPath, "utf8"));
  const result = planMigration(config, fs.readFileSync(publicKeyPath, "utf8"));
  const pkg = JSON.parse(fs.readFileSync(packagePath, "utf8"));
  assert.ok(/^\d+\.\d+\.\d+$/.test(pkg.version), "Invalid package version");
  pkg.version = "1.0.15";
  const cargo = fs.readFileSync(cargoPath, "utf8");
  assert.match(cargo, /\[workspace\.package\][\s\S]*?\nversion = "[^"]+"/,
    "Missing Cargo workspace version");
  const updatedCargo = cargo.replace(/(\[workspace\.package\][\s\S]*?\nversion = ")[^"]+(")/,
    "$11.0.15$2");
  // Validate everything before modifying any source. Never write private material.
  const changes = [
    [versionPath, "1.0.15\n"],
    [configPath, JSON.stringify(result.config, null, 2) + "\n"],
    [packagePath, JSON.stringify(pkg, null, 2) + "\n"],
    [cargoPath, updatedCargo],
  ];
  for (const [file, value] of changes) {
    const staging = file + ".kan68-staging";
    fs.writeFileSync(staging, value, { flag: "wx", mode: 0o600 });
    fs.renameSync(staging, file);
  }
  console.log("MIGRATION_STAGED version=1.0.15 public_sha256=" + result.digest);
  return result.digest;
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  if (process.argv.length !== 4 || process.argv[2] !== "--public-key") {
    console.error("Usage: node scripts/stage-updater-key-migration.mjs --public-key /path/to/approved-new-key.pub");
    process.exitCode = 2;
  } else {
    try {
      stage(process.cwd(), process.argv[3]);
    } catch (e) {
      console.error("MIGRATION_DENIED: " + e.message);
      process.exitCode = 2;
    }
  }
}
