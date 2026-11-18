import { spawnSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

export function getTauriAndroidProjectDir(webDir) {
  const manifestPath = resolve(webDir, "src-tauri/Cargo.toml");
  const result = spawnSync(
    "cargo",
    ["metadata", "--locked", "--format-version", "1", "--manifest-path", manifestPath],
    {
      cwd: webDir,
      encoding: "utf8",
      maxBuffer: 16 * 1024 * 1024,
      stdio: ["ignore", "pipe", "inherit"],
    },
  );

  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error("cargo metadata failed while locating Tauri's Android Gradle project.");
  }

  let metadata;
  try {
    metadata = JSON.parse(result.stdout);
  } catch (error) {
    throw new Error("cargo metadata returned invalid JSON: " + String(error));
  }

  const tauriPackages = metadata.packages.filter(
    (pkg) => pkg.name === "tauri" && pkg.source?.startsWith("registry+"),
  );
  if (tauriPackages.length !== 1) {
    throw new Error(
      "Expected exactly one registry Tauri package in Cargo metadata, found " +
        tauriPackages.length +
        ".",
    );
  }

  const projectDir = resolve(dirname(tauriPackages[0].manifest_path), "mobile/android");
  return projectDir;
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(resolve(process.argv[1])).href
) {
  const scriptDir = dirname(fileURLToPath(import.meta.url));
  process.stdout.write(getTauriAndroidProjectDir(resolve(scriptDir, "..")) + "\n");
}
