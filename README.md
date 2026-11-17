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

The debug APK is generated under:

~~~text
src-tauri/gen/android/app/build/outputs/apk/
~~~

The app supports Android API 26 and newer.

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
- ANDROID_KEY_PASSWORD: key and keystore password.

Keep the release keystore private and back it up securely. Never commit it or keystore.properties.

## Data and privacy

The Android app stores its database in Tauri's private app data directory. It does not need an account, cloud service, analytics SDK, or remote API. The browser preview uses localStorage only for local development and is separate from the Android SQLite database.

## License

MIT
