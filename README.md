# Miora

**Understand your rhythm.**

Miora is a private, offline-first menstrual cycle and symptom tracker. The Android app uses a Svelte web frontend inside Tauri, while Rust and SQLite handle the core logic and local data.

## Product principles

- Local-first: cycle and symptom data stays in the app's private storage.
- Low-friction: everyday tracking should take only a few taps.
- Evidence-led: estimates are based on the user's own recorded history.
- Transparent: prediction windows are shown only when there is enough history.
- Accessible: responsive controls, readable contrast, and labels alongside color.

## Architecture

~~~text
miora/
├── Cargo.toml                  # Rust domain, prediction and storage core
├── src/
│   ├── app/
│   ├── domain/
│   ├── engine/
│   └── storage/
└── web/
    ├── package.json
    ├── src/
    │   ├── App.svelte
    │   ├── app.css
    │   ├── components/
    │   ├── screens/
    │   └── lib/
    └── src-tauri/
        ├── Cargo.toml
        ├── tauri.conf.json
        └── src/
~~~

The frontend uses Svelte 5, Vite, Tailwind CSS v4, Lucide icons, and shadcn-svelte conventions/components. The visual layout is custom-built for a cycle tracker rather than an admin dashboard.

Tauri v2 embeds the web frontend in the platform WebView and exposes Rust commands for SQLite operations. The browser preview uses localStorage as a local-only development fallback; the Android app uses the Rust SQLite repository.

## UI and theme

Miora is designed mobile-first. The four primary screens are Home, Calendar, Check-in, and Insights. The layout uses a fluid content width, a seven-column calendar grid, a four-column mobile bottom navigation, and Android safe-area spacing.

Light and dark modes are supported. The initial theme follows the device setting, and the theme button can switch modes manually. The selected mode is persisted by mode-watcher.

The component setup is compatible with shadcn-svelte's CLI and token conventions. To add more primitives from the registry, run the shadcn-svelte CLI from the web directory.

## Run the web preview

~~~sh
cd web
npm install
npm run dev
~~~

Open the Vite URL shown in the terminal. Browser preview data stays in that browser's local storage.

## Run the desktop Tauri app

~~~sh
cd web
npm install
npm run tauri -- dev
~~~

Install the platform prerequisites listed in the Tauri v2 documentation first.

## Build Android

Install Android Studio, Java 17, Android SDK Platform 36, and Android NDK r30. Configure JAVA_HOME, ANDROID_HOME, and ANDROID_NDK_ROOT.

~~~sh
cd web
npm install
npm run tauri:android:init
npm run tauri:android:build
~~~

Vite and preview bind to loopback only. Android devices reach the development server through the ADB reverse mappings documented below; no LAN listener is started. The dependency audit documents one unresolved high-severity ReDoS advisory in the transitive, development-only `picomatch` dependency (GHSA-fc5q-39xc-vjpf). No fixed upstream release is currently published. CI/release audit permits only that advisory for the reviewed, exact versions of `picomatch`, `fdir`, `tinyglobby`, `svelte-check`, and `vite`; all affected nodes must be development-only and trace back to `picomatch`. Every other high/critical npm advisory fails the build.

The debug APK is generated under:

~~~text
src-tauri/gen/android/app/build/outputs/apk/
~~~

The app supports Android API 26 and newer.

### Android home-screen widget

Miora includes a resizable **Daily Check-in** home-screen widget. Add it from the Android launcher widget picker. The widget lets you choose mood and energy (1–5), cycle each symptom through mild, moderate, severe, or off, and save today's entry without opening the app.

The widget calls the Rust check-in API through UniFFI and writes to the same private SQLite repository used by the Tauri app. Rust owns validation and persistence; Kotlin only handles Android widget UI and lifecycle. Open Miora once after installation to initialize the database. Android widget source files live in `web/android-widget/`; the Android build command generates Kotlin bindings, adds the JNA runtime, copies the widget sources into Tauri's generated Android project, and registers the widget receiver.

## Rust checks

From the repository root:

~~~sh
cargo fmt --all -- --check
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
~~~

The Tauri host can be checked separately:

~~~sh
cargo check --manifest-path web/src-tauri/Cargo.toml --all-targets
~~~

## CI and releases

GitHub Actions checks Rust formatting, tests and Clippy; checks Svelte/TypeScript; builds the web frontend; and builds an ARM64 Android debug APK.

A tag matching v* triggers a signed Android release. Configure these Actions repository secrets before creating a release tag:

- ANDROID_KEY_BASE64: release keystore encoded as Base64.
- ANDROID_KEY_ALIAS: key alias inside the keystore.
- ANDROID_KEY_PASSWORD: password for the signing key.
- ANDROID_KEY_STORE_PASSWORD: optional keystore password; when omitted, Miora uses ANDROID_KEY_PASSWORD for both.

Keep the release keystore private and back it up securely. Never commit it or keystore.properties.

### Network and data protection

The Vite dev server and preview server bind only to `127.0.0.1`; HMR is also pinned to loopback. For Android development on a USB-connected device, use ADB reverse forwarding instead of exposing Vite on the LAN:

~~~sh
adb reverse tcp:5173 tcp:5173
adb reverse tcp:5174 tcp:5174
~~~

Android release builds remove the `INTERNET` permission and set cleartext traffic to false. The Android network security config permits cleartext only for `localhost`; production CSP only permits Tauri IPC and same-origin resources.

SQLite uses SQLCipher with a 256-bit random key. Android wraps that key with AES-256-GCM using a non-exportable Android Keystore key; the wrapped key file is stored in `noBackupFilesDir`. Existing plaintext SQLite databases are exported to SQLCipher, integrity-checked, and atomically replaced on first encrypted open. If the Android Keystore key or wrapped key is missing while an encrypted database remains, Miora fails closed rather than generating a replacement key.

For desktop Tauri development, provide a persistent 64-character hexadecimal key through `MIORA_DATABASE_KEY`. Desktop mode also fails closed if the key is missing.

RustSec CI scans the Android target (`aarch64-android`). It records three narrowly scoped exceptions: `RUSTSEC-2026-0374` (`basic-toml`) and `RUSTSEC-2024-0370` (`proc-macro-error`) are unmaintained build-time tooling with no available patched release; `RUSTSEC-2024-0429` is an unsound iterator in Tauri's Linux GTK3 desktop backend (`glib 0.18.x`), which is not part of the Android APK. The Android target audit does not include the Linux-only GTK `glib` dependency; its `RUSTSEC-2024-0429` unsound iterator remains a tracked risk for optional Linux desktop builds until Tauri's GTK stack moves to `glib 0.20+`. No `chrono` dependency remains in Miora's Rust source or manifests. All other Rust advisories fail CI and release builds.

## Data and privacy

The Android app stores its database in Tauri's private app data directory. Android cloud backup and device-to-device data extraction are disabled for the app's private data. Miora does not need an account, cloud service, analytics SDK, or remote API. The browser preview uses localStorage only for local development and is separate from the Android SQLite database.

## License

MIT
