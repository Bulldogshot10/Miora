import { copyFile, mkdir, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const webDir = resolve(scriptDir, "..");
const configPath = resolve(webDir, "src-tauri/tauri.conf.json");
const generatedAndroidDir = resolve(webDir, "src-tauri/gen/android/app/src/main");
const manifestPath = resolve(generatedAndroidDir, "AndroidManifest.xml");
const config = JSON.parse(await readFile(configPath, "utf8"));
const packageName = config.identifier;

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
run("cargo", ["build", "--manifest-path", cargoManifest, "--lib"], cargoWorkingDir);
run(
  "cargo",
  [
    "run",
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
    importMarker + "\\nimport " + packageName + ".uniffi.configureDatabaseKey as configureRustDatabaseKey",
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

let gradle = await readFile(gradlePath, "utf8");
if (!gradle.includes("net.java.dev.jna:jna:")) {
  const dependencyBlock = gradle.match(/dependencies\s*\{/);
  if (!dependencyBlock) {
    throw new Error("Could not find the Android app dependencies block for JNA.");
  }
  gradle = gradle.replace(
    dependencyBlock[0],
    dependencyBlock[0] + '\n    implementation("net.java.dev.jna:jna:5.19.1@aar")',
  );
  await writeFile(gradlePath, gradle);
}

console.log("Miora Android widget and UniFFI Kotlin bindings prepared.");

