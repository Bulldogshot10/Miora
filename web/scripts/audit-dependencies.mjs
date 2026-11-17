import { spawnSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const webDir = resolve(scriptDir, "..");
const lock = JSON.parse(await readFile(resolve(webDir, "package-lock.json"), "utf8"));

const result = spawnSync("npm", ["audit", "--json", "--audit-level=high"], {
  cwd: webDir,
  encoding: "utf8",
  maxBuffer: 16 * 1024 * 1024,
});

if (result.error) throw result.error;

let report;
try {
  report = JSON.parse(result.stdout);
} catch {
  throw new Error(
    "npm audit did not return valid JSON.\n" + (result.stderr || result.stdout || ""),
  );
}

const allowedAdvisory = "https://github.com/advisories/GHSA-fc5q-39xc-vjpf";
const allowedVersions = new Map([
  ["picomatch", new Set(["4.0.7"])],
  ["fdir", new Set(["6.5.0"])],
  ["tinyglobby", new Set(["0.2.17"])],
  ["svelte-check", new Set(["4.7.8"])],
  ["vite", new Set(["6.4.6"])],
]);
const devPackages = lock.packages ?? {};
const vulnerabilityMap = report.vulnerabilities ?? {};
const vulnerabilities = Object.entries(vulnerabilityMap);
const allowed = [];
const rejected = [];

function advisoryUrls(name, seen = new Set()) {
  if (seen.has(name)) return new Set();
  seen.add(name);
  const urls = new Set();
  for (const entry of vulnerabilityMap[name]?.via ?? []) {
    if (entry && typeof entry === "object" && entry.url) {
      urls.add(entry.url);
    } else if (typeof entry === "string" && vulnerabilityMap[entry]) {
      for (const url of advisoryUrls(entry, seen)) urls.add(url);
    }
  }
  return urls;
}

function affectedByPicomatch(name, seen = new Set()) {
  if (name === "picomatch") return true;
  if (seen.has(name)) return false;
  seen.add(name);
  return (vulnerabilityMap[name]?.via ?? []).some((entry) => {
    if (
      entry && typeof entry === "object" &&
      entry.name === "picomatch" && entry.url === allowedAdvisory
    ) return true;
    return typeof entry === "string" && affectedByPicomatch(entry, seen);
  });
}

for (const [name, item] of vulnerabilities) {
  if (!["high", "critical"].includes(item.severity)) continue;

  const nodes = item.nodes ?? [];
  const isDevOnly = nodes.length > 0 && nodes.every(
    (node) => devPackages[node]?.dev === true,
  );
  const expectedVersions = allowedVersions.get(name);
  const exactAffectedVersions = expectedVersions !== undefined && nodes.length > 0 &&
    nodes.every((node) => expectedVersions.has(devPackages[node]?.version));
  const urls = advisoryUrls(name);

  const approved =
    item.severity === "high" &&
    isDevOnly &&
    exactAffectedVersions &&
    urls.size === 1 &&
    urls.has(allowedAdvisory) &&
    affectedByPicomatch(name);

  if (approved) {
    allowed.push({ name, version: [...expectedVersions][0], advisory: allowedAdvisory });
  } else {
    rejected.push({
      name,
      severity: item.severity,
      via: item.via,
      fixAvailable: item.fixAvailable,
      isDevOnly,
    });
  }
}

if (rejected.length > 0) {
  console.error("High/critical npm advisories outside the documented exception:");
  console.error(JSON.stringify(rejected, null, 2));
  process.exit(1);
}

for (const item of allowed) {
  console.warn(
    `ALLOWED DEV-TOOL ADVISORY: ${item.name}@${item.version} — ${item.advisory}. ` +
    "No patched release is published; Vite is kept on loopback by default. " +
    "This exception applies only to this exact transitive dev dependency and advisory.",
  );
}

if (result.status !== 0 && allowed.length === 0) {
  console.error(result.stderr || "npm audit failed.");
  process.exit(result.status ?? 1);
}

console.log(
  `Dependency audit passed: no unapproved high/critical advisories (${allowed.length} documented dev-only exception(s)).`,
);
