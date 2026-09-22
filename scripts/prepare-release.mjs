import fs from "node:fs";

const raw = process.argv[2] ?? "";
const version = raw.startsWith("v") ? raw.slice(1) : raw;

if (!/^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(version)) {
  throw new Error("Expected a SemVer tag such as v1.2.3; got " + raw);
}

const packagePath = "package.json";
const packageJson = JSON.parse(fs.readFileSync(packagePath, "utf8"));
packageJson.version = version;
fs.writeFileSync(packagePath, JSON.stringify(packageJson, null, 2) + "\n");

const tauriPath = "src-tauri/tauri.conf.json";
const tauri = JSON.parse(fs.readFileSync(tauriPath, "utf8"));
tauri.version = version;
tauri.bundle.createUpdaterArtifacts = true;
fs.writeFileSync(tauriPath, JSON.stringify(tauri, null, 2) + "\n");

const cargoPath = "Cargo.toml";
const cargo = fs.readFileSync(cargoPath, "utf8");
const workspaceVersionPattern =
  /(\[workspace\.package\][\s\S]*?\nversion = ")[^"]+(")/;

if (!workspaceVersionPattern.test(cargo)) {
  throw new Error("Could not find workspace package version in Cargo.toml");
}

const updatedCargo = cargo.replace(
  workspaceVersionPattern,
  "$1" + version + "$2"
);
fs.writeFileSync(cargoPath, updatedCargo);

console.log("Prepared Modpack Manager release " + version);
