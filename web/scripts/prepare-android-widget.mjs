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

const manifest = await readFile(manifestPath, "utf8");
const receiverName = packageName + ".MioraWidgetProvider";
const receiverDeclaration = [
  '        <receiver android:name="' + receiverName + '" android:exported="true" android:label="Miora Daily Check-in">',
  '            <intent-filter>',
  '                <action android:name="android.appwidget.action.APPWIDGET_UPDATE" />',
  '            </intent-filter>',
  '            <meta-data android:name="android.appwidget.provider" android:resource="@xml/miora_daily_checkin_widget_info" />',
  '        </receiver>',
].join("\n");

if (!manifest.includes('android:name="' + receiverName + '"')) {
  if (!manifest.includes("</application>")) {
    throw new Error("Could not find the application element in the generated Android manifest.");
  }
  await writeFile(manifestPath, manifest.replace("</application>", receiverDeclaration + "\n    </application>"));
}

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
const gradlePath = resolve(webDir, "src-tauri/gen/android/app/build.gradle");

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
await copyFile(
  resolve(sourceDir, "miora_daily_checkin_widget.xml"),
  resolve(layoutDir, "miora_daily_checkin_widget.xml"),
);
await copyFile(
  resolve(sourceDir, "miora_daily_checkin_widget_info.xml"),
  resolve(xmlDir, "miora_daily_checkin_widget_info.xml"),
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
await rm(bindingsOutput, { recursive: true, force: true });
await mkdir(bindingsOutput, { recursive: true });
run(
  "cargo",
  [
    "run",
    "--manifest-path",
    resolve(webDir, "src-tauri/Cargo.toml"),
    "--features",
    "uniffi-cli",
    "--bin",
    "uniffi-bindgen",
    "--",
    "generate",
    "src:miora_lib",
    "--language",
    "kotlin",
    "--out-dir",
    bindingsOutput,
  ],
  resolve(webDir, "src-tauri"),
);
const generatedBindings = await copyGeneratedKotlin(bindingsOutput, javaRoot);
await rm(bindingsOutput, { recursive: true, force: true });
if (generatedBindings === 0) {
  throw new Error("UniFFI did not generate any Kotlin source files.");
}

let gradle = await readFile(gradlePath, "utf8");
if (!gradle.includes("net.java.dev.jna:jna:")) {
  const dependencyBlock = gradle.match(/dependencies\s*\{/);
  if (!dependencyBlock) {
    throw new Error("Could not find the Android app dependencies block for JNA.");
  }
  gradle = gradle.replace(
    dependencyBlock[0],
    dependencyBlock[0] + "\n    implementation 'net.java.dev.jna:jna:5.19.1@aar'",
  );
  await writeFile(gradlePath, gradle);
}

console.log("Miora Android widget and UniFFI Kotlin bindings prepared.");

