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
const devPackages = lock.packages ?? {};
const vulnerabilities = Object.entries(report.vulnerabilities ?? {});
const allowed = [];
const rejected = [];

for (const [name, item] of vulnerabilities) {
  if (!["high", "critical"].includes(item.severity)) continue;

  const packageEntry = devPackages["node_modules/" + name];
  const isDevOnly = packageEntry?.dev === true;

  const directAdvisories = (item.via ?? []).filter(
    (entry) => typeof entry === "object" && entry !== null,
  );
  const isPicomatchAdvisory =
    name === "picomatch" &&
    packageEntry?.version === "4.0.7" &&
    item.severity === "high" &&
    isDevOnly &&
    item.isDirect === false &&
    item.via?.length === 1 &&
    directAdvisories.length === 1 &&
    directAdvisories[0].url === allowedAdvisory;

  const isFdirPropagation =
    name === "fdir" &&
    packageEntry?.version === "6.5.0" &&
    item.severity === "high" &&
    isDevOnly &&
    item.isDirect === false &&
    item.via?.length === 1 &&
    item.via[0] === "picomatch";

  if (isPicomatchAdvisory || isFdirPropagation) {
    allowed.push({ name, version: packageEntry.version, advisory: allowedAdvisory });
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
