import { copyFile, mkdir, readFile, writeFile } from "node:fs/promises";
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

console.log("Miora Android home-screen widget prepared.");
