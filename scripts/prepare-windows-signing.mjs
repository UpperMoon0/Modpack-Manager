import fs from "node:fs";

const thumbprint = (process.argv[2] ?? "").replace(/\s+/g, "").toUpperCase();
if (!/^[0-9A-F]{40,64}$/.test(thumbprint)) {
  throw new Error("Expected a certificate thumbprint; got an invalid value");
}

const tauriPath = "src-tauri/tauri.conf.json";
const tauri = JSON.parse(fs.readFileSync(tauriPath, "utf8"));
tauri.bundle ??= {};
tauri.bundle.windows ??= {};
tauri.bundle.windows.certificateThumbprint = thumbprint;
tauri.bundle.windows.digestAlgorithm = "sha256";
tauri.bundle.windows.timestampUrl = "http://timestamp.digicert.com";

fs.writeFileSync(tauriPath, JSON.stringify(tauri, null, 2) + "\n");
console.log("Configured trusted Windows Authenticode signing for the release build");
