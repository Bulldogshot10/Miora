# Miora

**Understand your rhythm.**

Miora is a lightweight, offline-first menstrual cycle and symptom tracker built with Rust. The initial prototype focuses on reliable local period logging and a small Android-native package.

## Current status

The first prototype includes:

- Slint-based dashboard UI.
- Rust application entry points for desktop development and Android.
- SQLite storage for period start dates.
- Input validation for ISO dates and future dates.
- Local Android database storage through the app's private internal data directory.
- A basic database unit test.

Cycle and symptom predictions are not implemented yet. Miora will only introduce personalized estimates after it has enough real logged history.

## Stack

- Rust
- Slint 1.19
- SQLite through rusqlite
- Chrono for date calculations
- cargo-apk2 for Android packaging

## Run on desktop

Install a recent stable Rust toolchain, then run:

~~~sh
cargo run
~~~

The development database is created in \`.miora/miora.sqlite3\` under the current project directory.

## Build for Android

Install the Rust Android target:

~~~sh
rustup target add aarch64-linux-android
~~~

Install the Android SDK and NDK r30 or newer, then configure:

- \`ANDROID_HOME\`
- \`ANDROID_NDK_ROOT\`
- \`JAVA_HOME\` (when \`javac\` is not already available in \`PATH\`)

Install cargo-apk2:

~~~sh
cargo install cargo-apk2
~~~

Build a release APK:

~~~sh
cargo apk2 build --target aarch64-linux-android --lib --release
~~~

The APK is placed under \`target/release/apk/\`.

To install and launch on a connected device:

~~~sh
cargo apk2 run --target aarch64-linux-android --lib
~~~

Slint's Android backend uses \`NativeActivity\` and requires Android API 26 or newer.

## CI and releases

GitHub Actions runs formatting, unit tests, Clippy, and an Android debug APK build on pushes and pull requests targeting \`main\`.

Pushing a version tag such as \`v0.1.0\` triggers the release workflow. Before creating a release tag, add these repository Actions secrets:

- \`ANDROID_KEYSTORE_BASE64\`: the release keystore file encoded as Base64.
- \`ANDROID_KEYSTORE_PASSWORD\`: the keystore password.

Create a persistent signing keystore once and keep a secure backup. Do not generate a new release key for each build. For example:

~~~sh
keytool -genkeypair -v \\
  -keystore miora-release.jks \\
  -alias miora \\
  -keyalg RSA \\
  -keysize 2048 \\
  -validity 10000
~~~

Encode it for the \`ANDROID_KEYSTORE_BASE64\` secret:

~~~sh
base64 -w 0 miora-release.jks
~~~

The release workflow builds a signed ARM64 APK, calculates its SHA-256 checksum, and attaches both to the GitHub Release.

## Data and privacy

Miora currently stores its SQLite database in the app's private local storage. It does not require an account, network access, analytics, or a remote service.

Uninstalling the app normally removes its private app data. Export and restore functionality will be planned separately.

## Development roadmap

1. Foundation: verify desktop and Android builds, database lifecycle, and basic UI.
2. Period tracking: edit and review period history.
3. Daily check-ins: log symptoms, mood, energy, and notes.
4. Analytics: cycle statistics and symptom timelines.
5. Prediction: evaluate simple statistical baselines before adding more advanced models.
6. Release: backup/export, accessibility, notifications, and UI polish.

## License

MIT
