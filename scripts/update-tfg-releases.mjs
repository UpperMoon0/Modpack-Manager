import { createHash } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";

const githubToken = process.env.GITHUB_TOKEN?.trim();
if (!githubToken) {
  throw new Error("GITHUB_TOKEN is required");
}

const githubSpecs = [
  ["economy", "Economy", "UpperMoon0/Economy", "economy-forge-1.20.1-"],
  ["openui", "OpenUI MC", "UpperMoon0/OpenUI-MC", "openui-mc-forge-1.20.1-"],
  ["create-precise-controls", "Create: Precise Controls", "UpperMoon0/Create-Precise-Controls", "create-precise-controls-forge-1.20.1-"],
  ["simply-screens", "Simply Screens", "UpperMoon0/Simply-Screens", "simply_screens-forge-1.20.1-"],
  ["simply-speakers", "Simply Speakers", "UpperMoon0/Simply-Speakers", "simplyspeakers-forge-1.20.1-"],
  ["create-horse-power-ce", "Create Horse Power - CE", "UpperMoon0/CreateHorsePower-CE", "createhorsepower-ce-1.20.1-"],
  ["building-gadgets-extra", "Building Gadgets Extra", "UpperMoon0/Building-Gadgets-Extra", "building-gadgets-extra-forge-1.20.1-"],
];

const outPath = new URL("../data/tfg-releases.json", import.meta.url);

function isReleaseJar(name, prefix) {
  return name.startsWith(prefix)
    && name.endsWith(".jar")
    && !name.endsWith("-sources.jar")
    && !name.endsWith("-dev-shadow.jar")
    && !name.endsWith("-javadoc.jar");
}

async function fetchJson(url, headers = {}) {
  const response = await fetch(url, { headers });
  if (!response.ok) {
    throw new Error(`${url} returned ${response.status} ${response.statusText}`);
  }
  return response.json();
}

async function sha256Url(url) {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`failed to download ${url}: ${response.status} ${response.statusText}`);
  }
  const bytes = Buffer.from(await response.arrayBuffer());
  return createHash("sha256").update(bytes).digest("hex");
}

function validSha256(value) {
  return typeof value === "string" && /^[0-9a-fA-F]{64}$/.test(value);
}

let previous = { mods: [] };
try {
  previous = JSON.parse(await readFile(outPath, "utf8"));
} catch {}
const previousByUrl = new Map((previous.mods ?? []).map((mod) => [mod.url, mod.sha256]));

async function resolveGithub([id, name, repository, assetPrefix]) {
  const releases = await fetchJson(
    `https://api.github.com/repos/${repository}/releases?per_page=30`,
    {
      Accept: "application/vnd.github+json",
      Authorization: `Bearer ${githubToken}`,
      "X-GitHub-Api-Version": "2022-11-28",
      "User-Agent": "Modpack-Manager-release-index",
    },
  );

  const stable = releases
    .filter((release) => !release.draft && !release.prerelease)
    .sort((a, b) => String(b.published_at ?? "").localeCompare(String(a.published_at ?? "")));

  for (const release of stable) {
    const asset = release.assets?.find((candidate) => isReleaseJar(candidate.name, assetPrefix));
    if (!asset) continue;
    const digest = asset.digest?.startsWith("sha256:") ? asset.digest.slice(7) : null;
    let sha256 = validSha256(digest) ? digest.toLowerCase() : previousByUrl.get(asset.browser_download_url);
    if (!validSha256(sha256)) sha256 = await sha256Url(asset.browser_download_url);
    return {
      id, name,
      version: String(release.tag_name).replace(/^v/, ""),
      fileName: asset.name,
      source: `GitHub · ${repository}`,
      url: asset.browser_download_url,
      sha256: sha256.toLowerCase(),
    };
  }
  throw new Error(`no stable Forge 1.20.1 release asset matching ${assetPrefix} in ${repository}`);
}

async function resolveExtraGauges() {
  const versions = await fetchJson(
    'https://api.modrinth.com/v2/project/extra-gauges/version?game_versions=%5B%221.20.1%22%5D&loaders=%5B%22forge%22%5D',
    { "User-Agent": "Modpack-Manager-release-index" },
  );
  const stable = versions
    .filter((version) => version.version_type === "release")
    .sort((a, b) => String(b.date_published ?? "").localeCompare(String(a.date_published ?? "")));

  for (const version of stable) {
    const file = version.files?.find((candidate) => candidate.primary) ?? version.files?.[0];
    if (!file?.filename?.endsWith(".jar")) continue;
    let sha256 = file.hashes?.sha256 ?? previousByUrl.get(file.url);
    if (!validSha256(sha256)) sha256 = await sha256Url(file.url);
    return {
      id: "extra-gauges",
      name: "Create: Extra Gauges",
      version: version.version_number,
      fileName: file.filename,
      source: "Modrinth · extra-gauges",
      url: file.url,
      sha256: sha256.toLowerCase(),
    };
  }
  throw new Error("Modrinth has no stable Forge 1.20.1 release for Create: Extra Gauges");
}

const mods = [];
for (const spec of githubSpecs) mods.push(await resolveGithub(spec));
mods.push(await resolveExtraGauges());

await writeFile(outPath, `${JSON.stringify({ schemaVersion: 1, mods }, null, 2)}\n`, "utf8");
console.log(`wrote ${mods.length} managed releases to ${outPath.pathname}`);
