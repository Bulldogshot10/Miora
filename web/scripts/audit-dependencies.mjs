import { spawnSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const webDir = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const result = spawnSync(
  "npm",
  ["audit", "--json", "--audit-level=high"],
  {
    cwd: webDir,
    encoding: "utf8",
    maxBuffer: 16 * 1024 * 1024,
  },
);

if (result.error) throw result.error;

let report;
try {
  report = JSON.parse(result.stdout);
} catch {
  throw new Error(
    "npm audit did not return valid JSON.\n" +
      (result.stderr || result.stdout || ""),
  );
}

if (report.error) {
  throw new Error(
    "npm audit returned an error response:\n" +
      JSON.stringify(report.error, null, 2),
  );
}

const vulnerabilities = Object.entries(report.vulnerabilities ?? {});
const rejected = vulnerabilities
  .filter(([, item]) => ["high", "critical"].includes(item.severity))
  .map(([name, item]) => ({
    name,
    severity: item.severity,
    via: item.via,
    fixAvailable: item.fixAvailable,
  }));

if (rejected.length > 0) {
  console.error("Active high/critical npm advisories found:");
  console.error(JSON.stringify(rejected, null, 2));
  process.exit(1);
}

if (result.status !== 0 && result.status !== 1) {
  throw new Error(
    result.stderr || `npm audit exited unexpectedly with ${result.status}.`,
  );
}

console.log("No active high/critical npm advisories found.");
