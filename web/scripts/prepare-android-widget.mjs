import { access, copyFile, mkdir, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { getTauriAndroidProjectDir } from "./tauri-android-project-dir.mjs";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const webDir = resolve(scriptDir, "..");
const configPath = resolve(webDir, "src-tauri/tauri.conf.json");
const generatedAndroidDir = resolve(webDir, "src-tauri/gen/android/app/src/main");
const manifestPath = resolve(generatedAndroidDir, "AndroidManifest.xml");
const config = JSON.parse(await readFile(configPath, "utf8"));
const packageName = config.identifier;
const releaseBuild = process.argv.includes("--release");

if (!packageName || !/^[A-Za-z][A-Za-z0-9_]*(\.[A-Za-z][A-Za-z0-9_]*)+$/.test(packageName)) {
  throw new Error("tauri.conf.json must contain a valid Android package identifier.");
}

let manifest = await readFile(manifestPath, "utf8");
const receiverName = packageName + ".MioraWidgetProvider";
const actionReceiverName = packageName + ".MioraWidgetActionReceiver";

// Cycle and symptom records are sensitive. Disable Android cloud backup and
// device-to-device transfer at the manifest and resource-rule levels.
const applicationStart = manifest.indexOf("<application");
const applicationTagEnd = manifest.indexOf(">", applicationStart);
if (applicationStart < 0 || applicationTagEnd < 0) {
  throw new Error("Could not find the application element in the generated Android manifest.");
}
let applicationTag = manifest.slice(applicationStart, applicationTagEnd + 1);
for (const [attribute, value] of [
  ["android:allowBackup", "false"],
  ["android:fullBackupContent", "@xml/miora_backup_rules"],
  ["android:dataExtractionRules", "@xml/miora_data_extraction_rules"],
  ["android:networkSecurityConfig", "@xml/miora_network_security_config"],
]) {
  const attributeStart = applicationTag.indexOf(attribute + '="');
  if (attributeStart >= 0) {
    const valueStart = attributeStart + attribute.length + 2;
    const valueEnd = applicationTag.indexOf('"', valueStart);
    if (valueEnd < 0) throw new Error("Malformed Android application attribute: " + attribute);
    applicationTag = applicationTag.slice(0, valueStart) + value + applicationTag.slice(valueEnd);
  } else {
    applicationTag = applicationTag.replace("<application", '<application ' + attribute + '="' + value + '"');
  }
}
manifest = manifest.slice(0, applicationStart) + applicationTag + manifest.slice(applicationTagEnd + 1);

// A release build has no Android INTERNET permission at all. The tools marker
// removes it from the merged manifest, including transitive library manifests.
if (releaseBuild) {
  // Remove Tauri's direct INTERNET permission declaration. Keep a manifest
  // merger removal marker so no transitive Android library can re-add it.
  manifest = manifest.replace(
    /^\s*<uses-permission android:name="android\.permission\.INTERNET"\s*\/>\s*$/gm,
    "",
  );
  if (!manifest.includes('android:name="android.permission.INTERNET" tools:node="remove"')) {
    if (!manifest.includes('xmlns:tools=')) {
      manifest = manifest.replace(
        "<manifest",
        '<manifest xmlns:tools="http://schemas.android.com/tools"',
      );
    }
    manifest = manifest.replace(
      "</manifest>",
      '    <uses-permission android:name="android.permission.INTERNET" tools:node="remove" />\n</manifest>',
    );
  }
  const unmarkedInternetPermission = manifest
    .split("\n")
    .some((line) =>
      line.includes('android:name="android.permission.INTERNET"') &&
      !line.includes('tools:node="remove"'),
    );
  if (unmarkedInternetPermission) {
    throw new Error("Release manifest still declares an active INTERNET permission.");
  }
}
const receiverDeclarations = [];
if (!manifest.includes('android:name="' + receiverName + '"')) {
  receiverDeclarations.push([
    '        <receiver android:name="' + receiverName + '" android:exported="true" android:label="Miora Daily Check-in">',
    '            <intent-filter>',
    '                <action android:name="android.appwidget.action.APPWIDGET_UPDATE" />',
    '            </intent-filter>',
    '            <meta-data android:name="android.appwidget.provider" android:resource="@xml/miora_daily_checkin_widget_info" />',
    '        </receiver>',
  ].join("\n"));
}
if (!manifest.includes('android:name="' + actionReceiverName + '"')) {
  receiverDeclarations.push(
    '        <receiver android:name="' + actionReceiverName + '" android:exported="false" />',
  );
}
if (receiverDeclarations.length > 0) {
  if (!manifest.includes("</application>")) {
    throw new Error("Could not find the application element in the generated Android manifest.");
  }
  manifest = manifest.replace(
    "</application>",
    receiverDeclarations.join("\n") + "\n    </application>",
  );
}
await writeFile(manifestPath, manifest);

const packagePath = packageName.split(".").join("/");
const javaDir = resolve(generatedAndroidDir, "java", packagePath);
const layoutDir = resolve(generatedAndroidDir, "res/layout");
const xmlDir = resolve(generatedAndroidDir, "res/xml");
const drawableDir = resolve(generatedAndroidDir, "res/drawable");
const valuesDir = resolve(generatedAndroidDir, "res/values");
await Promise.all([
  mkdir(javaDir, { recursive: true }),
  mkdir(layoutDir, { recursive: true }),
  mkdir(xmlDir, { recursive: true }),
  mkdir(drawableDir, { recursive: true }),
  mkdir(valuesDir, { recursive: true }),
]);


const javaRoot = resolve(generatedAndroidDir, "java");
const gradlePath = resolve(webDir, "src-tauri/gen/android/app/build.gradle.kts");
const rootGradlePath = resolve(webDir, "src-tauri/gen/android/build.gradle.kts");
const buildSrcGradlePath = resolve(
  webDir,
  "src-tauri/gen/android/buildSrc/build.gradle.kts",
);

function enableGradleLocking(
  source,
  label,
  { lockBuildscriptClasspath = false, allProjects = false, strict = false } = {},
) {
  if (lockBuildscriptClasspath) {

    if (!source.includes("resolutionStrategy.activateDependencyLocking()")) {
      const classpathConfiguration =
        "configurations.classpath {\n" +
        "    resolutionStrategy.activateDependencyLocking()\n" +
        "}";
      if (source.includes("buildscript {")) {
        source = source.replace(
          "buildscript {",
          "buildscript {\n    " + classpathConfiguration.replaceAll("\n", "\n    "),
        );
      } else {
        // buildSrc uses the plugins DSL and has no explicit buildscript block.
        source = "buildscript {\n    " +
          classpathConfiguration.replaceAll("\n", "\n    ") +
          "\n}\n\n" + source;
      }
    }
  }

  const beginMarker = "// Miora dependency locking BEGIN";
  const endMarker = "// Miora dependency locking END";
  const lockMode = strict ? "STRICT" : "DEFAULT";
  const blockLines = [
    beginMarker,
    allProjects ? "allprojects {" : "dependencyLocking {",
    ...(allProjects ? ["    dependencyLocking {"] : []),
    allProjects ? "        lockAllConfigurations()" : "    lockAllConfigurations()",
    allProjects
      ? "        lockMode.set(org.gradle.api.artifacts.dsl.LockMode." + lockMode + ")"
      : "    lockMode.set(org.gradle.api.artifacts.dsl.LockMode." + lockMode + ")",
    ...(allProjects ? ["    }"] : []),
    "}",
    endMarker,
  ];
  const block = blockLines.join("\n");
  const begin = source.indexOf(beginMarker);
  if (begin >= 0) {
    const end = source.indexOf(endMarker, begin);
    if (end < 0) throw new Error("Unclosed Miora Gradle dependency-locking marker.");
    source = source.slice(0, begin) + block + source.slice(end + endMarker.length);
  } else {
    source = source.trimEnd() + "\n\n" + block + "\n";
  }

  return source;
}

async function enableGeneratedGradleDependencyLocking(strict) {
  let rootBuild = await readFile(rootGradlePath, "utf8");
  let buildSrcBuild = await readFile(buildSrcGradlePath, "utf8");

  // allprojects applies to the root, :app, and Tauri's :tauri-android subproject.
  rootBuild = enableGradleLocking(rootBuild, "root", {
    lockBuildscriptClasspath: true,
    allProjects: true,
    strict,
  });
  buildSrcBuild = enableGradleLocking(buildSrcBuild, "buildSrc", {
    lockBuildscriptClasspath: true,
    strict,
  });

  await writeFile(rootGradlePath, rootBuild);
  await writeFile(buildSrcGradlePath, buildSrcBuild);
}
const gradleWrapperPropertiesPath = resolve(
  webDir,
  "src-tauri/gen/android/gradle/wrapper/gradle-wrapper.properties",
);
const GRADLE_DISTRIBUTION = "gradle-9.6.1-bin.zip";
const GRADLE_DISTRIBUTION_SHA256 =
  "9c0f7faeeb306cb14e4279a3e084ca6b596894089a0638e68a07c945a32c9e14";

async function verifyGradleWrapper() {
  let properties = await readFile(gradleWrapperPropertiesPath, "utf8");
  const distributionLine = properties
    .split("\n")
    .find((line) => line.startsWith("distributionUrl="));
  if (!distributionLine || !distributionLine.includes(GRADLE_DISTRIBUTION)) {
    throw new Error(
      "Unexpected Tauri Gradle wrapper distribution; expected " + GRADLE_DISTRIBUTION,
    );
  }

  const checksumLine = "distributionSha256Sum=" + GRADLE_DISTRIBUTION_SHA256;
  if (/^distributionSha256Sum=/m.test(properties)) {
    properties = properties.replace(/^distributionSha256Sum=.*$/m, checksumLine);
  } else {
    properties = properties.trimEnd() + "\n" + checksumLine + "\n";
  }
  await writeFile(gradleWrapperPropertiesPath, properties);

  // Tauri invokes Gradle through cargo-mobile2, so it does not expose a direct
  // --stacktrace argument. Gradle's equivalent project property keeps the
  // actual Tauri release assemble task's failures fully diagnosable.
  const gradlePropertiesPath = resolve(webDir, "src-tauri/gen/android/gradle.properties");
  let gradleProperties = await readFile(gradlePropertiesPath, "utf8");
  const stacktraceSetting = "org.gradle.logging.stacktrace=all";
  const stacktracePrefix = "org.gradle.logging.stacktrace=";
  const gradlePropertyLines = gradleProperties
    .split(/\r?\n/)
    .filter((line) => !line.startsWith(stacktracePrefix));
  gradlePropertyLines.push(stacktraceSetting);
  gradleProperties = gradlePropertyLines.join("\n").trimEnd() + "\n";
  await writeFile(gradlePropertiesPath, gradleProperties);
}

function run(command, args, cwd) {
  const result = spawnSync(command, args, { cwd, stdio: "inherit" });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(command + " failed with exit code " + result.status);
  }
}

async function copyGeneratedKotlin(source, destination) {
  let copied = 0;
  for (const entry of await readdir(source, { withFileTypes: true })) {
    const from = resolve(source, entry.name);
    if (entry.isDirectory()) {
      copied += await copyGeneratedKotlin(from, destination);
    } else if (entry.isFile() && entry.name.endsWith(".kt")) {
      await copyFile(from, resolve(destination, entry.name));
      copied += 1;
    }
  }
  return copied;
}

const sourceDir = resolve(webDir, "android-widget");
await copyFile(
  resolve(sourceDir, "MioraWidgetProvider.kt"),
  resolve(javaDir, "MioraWidgetProvider.kt"),
);
const keyStoreSource = await readFile(
  resolve(sourceDir, "MioraDatabaseKeyStore.kt"),
  "utf8",
);
await writeFile(
  resolve(javaDir, "MioraDatabaseKeyStore.kt"),
  keyStoreSource.replace("package __PACKAGE__", "package " + packageName),
);
await copyFile(
  resolve(sourceDir, "miora_daily_checkin_widget.xml"),
  resolve(layoutDir, "miora_daily_checkin_widget.xml"),
);
await copyFile(
  resolve(sourceDir, "miora_daily_checkin_widget_info.xml"),
  resolve(xmlDir, "miora_daily_checkin_widget_info.xml"),
);
await copyFile(
  resolve(sourceDir, "miora_backup_rules.xml"),
  resolve(xmlDir, "miora_backup_rules.xml"),
);
await copyFile(
  resolve(sourceDir, "miora_data_extraction_rules.xml"),
  resolve(xmlDir, "miora_data_extraction_rules.xml"),
);
await copyFile(
  resolve(sourceDir, "miora_network_security_config.xml"),
  resolve(xmlDir, "miora_network_security_config.xml"),
);
await copyFile(resolve(sourceDir, "miora_widget_panel.xml"), resolve(drawableDir, "miora_widget_panel.xml"));
await copyFile(resolve(sourceDir, "miora_widget_chip.xml"), resolve(drawableDir, "miora_widget_chip.xml"));
await copyFile(
  resolve(sourceDir, "miora_widget_chip_selected.xml"),
  resolve(drawableDir, "miora_widget_chip_selected.xml"),
);

const stylesPath = resolve(valuesDir, "miora_widget_styles.xml");
await copyFile(resolve(sourceDir, "miora_widget_styles.xml"), stylesPath);

const providerPath = resolve(javaDir, "MioraWidgetProvider.kt");
const providerSource = await readFile(providerPath, "utf8");
await writeFile(providerPath, providerSource.replace("package __PACKAGE__", "package " + packageName));


const bindingsOutput = resolve(webDir, ".uniffi-generated");
const cargoManifest = resolve(webDir, "src-tauri/Cargo.toml");
const cargoWorkingDir = resolve(webDir, "src-tauri");
const nativeLibraryName = process.platform === "win32"
  ? "miora_lib.dll"
  : process.platform === "darwin"
    ? "libmiora_lib.dylib"
    : "libmiora_lib.so";
const nativeLibrary = resolve(cargoWorkingDir, "target/debug", nativeLibraryName);

await rm(bindingsOutput, { recursive: true, force: true });
await mkdir(bindingsOutput, { recursive: true });

// Proc-macro interfaces use UniFFI library mode. The embedded metadata is
// architecture-independent, so a host build can generate the Kotlin bindings
// later used by the Android build.
run("cargo", ["build", "--locked", "--manifest-path", cargoManifest, "--lib"], cargoWorkingDir);
run(
  "cargo",
  [
    "run",
    "--locked",
    "--manifest-path",
    cargoManifest,
    "--features",
    "uniffi-cli",
    "--bin",
    "uniffi-bindgen",
    "--",
    "generate",
    "--library",
    nativeLibrary,
    "--language",
    "kotlin",
    "--out-dir",
    bindingsOutput,
  ],
  cargoWorkingDir,
);
const generatedBindings = await copyGeneratedKotlin(bindingsOutput, javaRoot);
await rm(bindingsOutput, { recursive: true, force: true });
if (generatedBindings === 0) {
  throw new Error("UniFFI did not generate any Kotlin source files.");
}

const mainActivityPath = resolve(javaDir, "MainActivity.kt");
let mainActivity = await readFile(mainActivityPath, "utf8");
if (!mainActivity.includes("MioraDatabaseKeyStore.getOrCreateDatabaseKey")) {
  const importMarker = "import androidx.activity.enableEdgeToEdge";
  if (!mainActivity.includes(importMarker)) {
    throw new Error("Could not locate the Tauri MainActivity import section.");
  }
  mainActivity = mainActivity.replace(
    importMarker,
    importMarker + "\nimport " + packageName + ".uniffi.configureDatabaseKey as configureRustDatabaseKey",
  );

  const onCreate = `  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
  }`;
  const secureOnCreate = `  override fun onCreate(savedInstanceState: Bundle?) {
    val databaseKey = MioraDatabaseKeyStore.getOrCreateDatabaseKey(applicationContext)
    try {
      configureRustDatabaseKey(databaseKey)
    } finally {
      databaseKey.fill(0)
    }
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
  }`;
  if (!mainActivity.includes(onCreate)) {
    throw new Error("Could not locate Tauri MainActivity.onCreate to initialize the database key.");
  }
  mainActivity = mainActivity.replace(onCreate, secureOnCreate);
  await writeFile(mainActivityPath, mainActivity);
}

await verifyGradleWrapper();

const bootstrapGradleLocks = process.argv.includes("--bootstrap-gradle-locks");
const committedGradleLockDirectory = resolve(webDir, "android-gradle-locks");

async function hasFile(path) {
  try {
    await access(path);
    return true;
  } catch {
    return false;
  }
}

async function listGradleLockFiles(directory, prefix = "") {
  const files = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const relativePath = prefix ? prefix + "/" + entry.name : entry.name;
    const fullPath = resolve(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...await listGradleLockFiles(fullPath, relativePath));
    } else if (
      entry.isFile() &&
      (entry.name === "gradle.lockfile" ||
        entry.name === "buildscript-gradle.lockfile" ||
        entry.name === "verification-metadata.xml")
    ) {
      files.push(relativePath);
    }
  }
  return files.sort();
}

const committedGradleLockFiles = await hasFile(committedGradleLockDirectory)
  ? await listGradleLockFiles(committedGradleLockDirectory)
  : [];
const bootstrapGradleLockDirectory = process.env.MIORA_GRADLE_LOCKS_DIR
  ? resolve(process.env.MIORA_GRADLE_LOCKS_DIR)
  : null;
const hasCommittedGradleLocks = committedGradleLockFiles.length > 0;
const gradleLockSourceDirectory = hasCommittedGradleLocks
  ? committedGradleLockDirectory
  : bootstrapGradleLockDirectory;
const gradleLockFiles = hasCommittedGradleLocks
  ? committedGradleLockFiles
  : gradleLockSourceDirectory && await hasFile(gradleLockSourceDirectory)
    ? await listGradleLockFiles(gradleLockSourceDirectory)
    : [];
const regularGradleLockCount = gradleLockFiles.filter((entry) =>
  entry.endsWith("/gradle.lockfile") || entry === "gradle.lockfile"
).length;
const buildscriptGradleLockCount = gradleLockFiles.filter((entry) =>
  entry.endsWith("/buildscript-gradle.lockfile") || entry === "buildscript-gradle.lockfile"
).length;
const hasGradleLocks = gradleLockFiles.length > 0;
const tauriAndroidLockRelativePath = "external/tauri-android/gradle.lockfile";
const tauriAndroidProjectDir =
  hasGradleLocks && gradleLockFiles.includes(tauriAndroidLockRelativePath)
    ? getTauriAndroidProjectDir(webDir)
    : null;

if (hasGradleLocks) {
  if (regularGradleLockCount < 3 || buildscriptGradleLockCount < 2) {
    throw new Error(
      "The Android Gradle lock set is incomplete: expected at least three project lockfiles and two buildscript lockfiles.",
    );
  }
  if (!gradleLockFiles.includes("gradle/verification-metadata.xml")) {
    throw new Error("The Android Gradle dependency verification metadata is missing.");
  }
  console.log(
    (hasCommittedGradleLocks ? "Committed" : "CI-bootstrap") +
    " Android Gradle lock set found; strict dependency locking will be enabled."
  );
}

if (bootstrapGradleLocks || hasGradleLocks) {
  await enableGeneratedGradleDependencyLocking(hasGradleLocks && !bootstrapGradleLocks);
}
if (hasGradleLocks) {
  if (!gradleLockSourceDirectory) {
    throw new Error("Android Gradle lock files were found without a source directory.");
  }
  for (const relativePath of gradleLockFiles) {
    const source = resolve(gradleLockSourceDirectory, relativePath);
    if (relativePath === tauriAndroidLockRelativePath) {
      if (!tauriAndroidProjectDir) {
        throw new Error("Could not locate the Tauri Android Gradle project for its lockfile.");
      }
      await copyFile(source, resolve(tauriAndroidProjectDir, "gradle.lockfile"));
    } else {
      const destination = resolve(webDir, "src-tauri/gen/android", relativePath);
      await mkdir(dirname(destination), { recursive: true });
      await copyFile(source, destination);
    }
  }
}

let gradle = await readFile(gradlePath, "utf8");

function pinAndroidSdkLevel(source, property) {
  const expression = new RegExp("\\b" + property + "\\s*=\\s*(\\d+)");
  const match = source.match(expression);
  if (!match) {
    throw new Error("Could not find generated Android " + property + " declaration.");
  }

  const current = Number(match[1]);
  if (current !== 36 && current !== 37) {
    throw new Error(
      "Unexpected Tauri Android " + property + " " + current +
      "; review the template before changing the stable SDK pin.",
    );
  }

  return source.replace(expression, property + " = 36");
}

// Android 17 / API 37 is still a pre-release SDK. Compile and target the latest
// stable platform (Android 16 / API 36) until API 37 reaches the stable channel.
gradle = pinAndroidSdkLevel(gradle, "compileSdk");
gradle = pinAndroidSdkLevel(gradle, "targetSdk");
gradle = gradle.replace(
  /manifestPlaceholders\["usesCleartextTraffic"\]\s*=\s*"?true"?/g,
  'manifestPlaceholders["usesCleartextTraffic"] = "false"',
);
if (/manifestPlaceholders\["usesCleartextTraffic"\]\s*=\s*"?true"?/.test(gradle)) {
  throw new Error("Android debug build still enables unrestricted cleartext traffic.");
}
if (!gradle.includes("net.java.dev.jna:jna:")) {
  const dependencyBlock = gradle.match(/dependencies\s*\{/);
  if (!dependencyBlock) {
    throw new Error("Could not find the Android app dependencies block for JNA.");
  }
  gradle = gradle.replace(
    dependencyBlock[0],
    dependencyBlock[0] + '\n    implementation("net.java.dev.jna:jna:5.19.1@aar")',
  );
}
await writeFile(gradlePath, gradle);

console.log("Miora Android widget and UniFFI Kotlin bindings prepared.");

