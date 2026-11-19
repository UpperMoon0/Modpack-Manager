import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

export function generateWindowsChecksums(bundleDirectory) {
  const bundles = [];
  function visit(directory) {
    for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
      const filename = path.join(directory, entry.name);
      if (entry.isDirectory()) visit(filename);
      else if (entry.isFile() && /\.(exe|msi)$/i.test(entry.name)) bundles.push(filename);
    }
  }
  visit(bundleDirectory);
  assert.ok(bundles.length, "No Windows installers found");
  const names = new Set();
  return bundles.sort().map((filename) => {
    // tauri-action replaces spaces with dots in uploaded asset names.
    const name = path.basename(filename).replaceAll(" ", ".");
    assert.ok(!names.has(name), `Duplicate uploaded installer name: ${name}`);
    names.add(name);
    const hash = createHash("sha256").update(fs.readFileSync(filename)).digest("hex");
    return `${hash}  ${name}\n`;
  }).join("");
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [bundleDirectory, outputPath] = process.argv.slice(2);
  assert.ok(bundleDirectory && outputPath,
    "Usage: generate-windows-checksums.mjs BUNDLE_DIRECTORY OUTPUT_PATH");
  fs.writeFileSync(outputPath, generateWindowsChecksums(bundleDirectory));
}
