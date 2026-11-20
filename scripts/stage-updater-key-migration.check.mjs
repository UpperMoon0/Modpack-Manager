import assert from "node:assert/strict";
import test from "node:test";
import fs from "node:fs";
import { createHash } from "node:crypto";
import { planMigration, LEGACY_FEED, LEGACY_SHA256, NEW_FEED } from "./stage-updater-key-migration.mjs";

const baseline = JSON.parse(fs.readFileSync("src-tauri/tauri.conf.json", "utf8"));
function candidate(replacement = 1) {
  const bytes = Buffer.alloc(42, replacement);
  bytes.write("Ed", 0, "ascii");
  return "untrusted comment: minisign public key 12345678\n" +
    bytes.toString("base64") + "\n";
}

test("stages a distinct new trust root and an isolated feed", () => {
  const { config, digest } = planMigration(baseline, candidate());
  assert.equal(config.identifier, "dev.nstut.modpack-manager");
  assert.equal(config.version, "1.0.15");
  assert.equal(config.plugins.updater.requireSignedVersion, true);
  assert.deepEqual(config.plugins.updater.endpoints, [NEW_FEED]);
  assert.notEqual(digest, LEGACY_SHA256);
  assert.notEqual(config.plugins.updater.pubkey, baseline.plugins.updater.pubkey);
  assert.deepEqual(baseline.plugins.updater.endpoints, [LEGACY_FEED]);
});

test("rejects unapproved historical trust root", () => {
  assert.throws(() => planMigration(baseline,
    Buffer.from(baseline.plugins.updater.pubkey, "base64").toString("utf8")),
    /must differ/);
});

test("rejects tampered legacy key and feed", () => {
  const alteredKey = structuredClone(baseline);
  alteredKey.plugins.updater.pubkey = Buffer.from(candidate()).toString("base64");
  assert.throws(() => planMigration(alteredKey, candidate(2)), /Legacy public key drift/);
  const alteredFeed = structuredClone(baseline);
  alteredFeed.plugins.updater.endpoints = [NEW_FEED];
  assert.throws(() => planMigration(alteredFeed, candidate()), /Legacy updater endpoint drift/);
});

test("rejects disabled signature checks", () => {
  const unsafe = structuredClone(baseline);
  unsafe.plugins.updater.requireSignedVersion = false;
  assert.throws(() => planMigration(unsafe, candidate()), /Signing verification/);
});

test("rejects malformed public keys and changed application identity", () => {
  assert.throws(() => planMigration(baseline, "not a minisign key"), /public key/);
  const bad = structuredClone(baseline);
  bad.identifier = "dev.other.app";
  assert.throws(() => planMigration(bad, candidate()), /Application identity/);
});

test("fixture new public digest is based on the exact minisign record", () => {
  const candidateText = candidate(2);
  const record = Buffer.from(candidateText.split("\n")[1], "base64");
  assert.equal(planMigration(baseline, candidateText).digest,
    createHash("sha256").update(record).digest("hex"));
});
