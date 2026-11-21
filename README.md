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
npm ci
npm run dev
~~~

Open the Vite URL shown in the terminal. Browser preview data stays in that browser's local storage.

## Run the desktop Tauri app

~~~sh
cd web
npm ci
npm run tauri -- dev
~~~

Install the platform prerequisites listed in the Tauri v2 documentation first.

## Build Android

Install Android Studio, Java 17, Android SDK Platform 36, and Android NDK r30. Configure JAVA_HOME, ANDROID_HOME, and ANDROID_NDK_ROOT.

~~~sh
cd web
npm ci
npm run tauri:android:init
npm run tauri:android:build
~~~

Vite and preview bind to loopback only. Android devices reach the development server through the ADB reverse mappings documented below; no LAN listener is started. The GitHub advisory GHSA-fc5q-39xc-vjpf for `picomatch` was withdrawn on November 18, 2026 and is no longer treated as an active vulnerability or allowlisted exception. CI/release audit now fails on every active high or critical npm advisory, including newly disclosed issues in development dependencies.

The debug APK is generated under:

~~~text
src-tauri/gen/android/app/build/outputs/apk/
~~~

The app supports Android API 26 and newer.

### Encrypted data backup and recovery

Open **Insights → Encrypted backup** in the native app. Create a backup with a passphrase of at least 12 characters, then copy the complete encrypted text into a separate file or password manager. The envelope uses PBKDF2-HMAC-SHA-256 (600,000 iterations) to derive an AES-256-GCM key, with a fresh random salt and nonce for each export. The passphrase is never stored by Miora.

To move data to another device, install Miora there, open Insights, paste the encrypted backup text, enter the passphrase, and confirm restore. Restore replaces the current period and check-in dataset in one SQLite transaction; a failed validation or write leaves the existing database unchanged. Keep the passphrase separately: Miora cannot recover a lost passphrase. The encrypted backup is portable and is not tied to the Android Keystore key used for the on-device database.

### Android home-screen widget

Miora includes a resizable **Daily Check-in** home-screen widget. Add it from the Android launcher widget picker. The widget lets you choose mood and energy (1–5), cycle each symptom through mild, moderate, severe, or off, and save today's entry without opening the app.

The widget calls the Rust check-in API through UniFFI and writes to the same private SQLite repository used by the Tauri app. Rust owns validation and persistence; Kotlin only handles Android widget UI and lifecycle. Unsaved widget drafts are encrypted with an Android Keystore AES-256-GCM key before they are stored in SharedPreferences. Open Miora once after installation to initialize the database. Android widget source files live in `web/android-widget/`; the Android build command generates Kotlin bindings, adds the JNA runtime, copies the widget sources into Tauri's generated Android project, and registers the widget receiver.

## Statistical inference engine

The native Rust core combines several deterministic statistical components:

- **Bayesian online changepoint detection (BOCPD):** tracks posterior run-length probabilities with a Gaussian observation model, fixed known variance, and a constant 8% per-completed-cycle hazard. The resulting run-length posterior downweights observations unlikely to belong to the current regime.
- **Hierarchical partial pooling:** cycle length uses a Normal-Normal individual posterior with a fixed weak population prior (28-day mean, 5-day between-person standard deviation, 4-day within-person standard deviation). Symptom rates use a Beta-Binomial partial-pooling estimate with a fixed strength of four pseudo-observations.
- **Weibull and Gamma survival candidates:** both candidates are grid-fitted by weighted maximum likelihood, including the right-censored current cycle. Their predictive CDFs are combined using normalized relative maximum log-likelihood scores (an equal 50/50 mixture is used below five effective observations), then blended with the hierarchical predictive CDF according to the amount of personal evidence. These weights are likelihood-based mixture weights, not integrated Bayesian model probabilities.
- **Censoring and observation quality:** the active cycle contributes a right-censored observation each day. Completed intervals outside the supported 15–60 day modelling range are classified separately; gaps longer than 60 days are flagged as potentially missed period logs and excluded from the completed-cycle fit.

The native dashboard returns integer-day P10/P50/P90 predictive quantiles. The median/MAD estimator remains available as `estimate_next_period_baseline` for comparison. The core also exposes `evaluate_rolling_origin` and `evaluate_rolling_origin_with` for leakage-safe historical evaluation, reporting empirical interval coverage, width, interval score, and survival-model P10/P50/P90 pinball loss alongside a median/MAD baseline comparison. The bundled tests use synthetic dates only; real calibration still requires an appropriately consented, representative longitudinal dataset. The median/MAD baseline's 68.27% nominal level is an idealized Gaussian one-sigma reference before integer-day rounding; its empirical coverage can differ, so compare the reported coverage and interval width rather than treating 68.27% as guaranteed. BOCPD hazard, distribution priors, grid bounds, and partial-pooling strengths are fixed defaults; they are not estimated from a Miora population cohort. Model quality still requires rolling-origin validation and calibration against an independent, appropriately consented dataset before accuracy claims are made. All inference runs locally in Rust and introduces no new runtime dependency or network requirement.

The browser-only localStorage preview retains its development fallback estimator; the production Android/Tauri application uses the Rust inference engine. The browser fallback is not numerically equivalent to the native Rust predictor.

Cycle-phase classification is currently a separate discrete Dirichlet-categorical model over cycle lengths with a uniform 12–16 day luteal-length assumption. It does not consume the BOCPD/survival predictive distribution, and calendar data alone does not confirm ovulation. Treat the phase distribution and next-period forecast as related but distinct model outputs.

## Rust checks

From the repository root:

~~~sh
cargo fmt --all -- --check
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
~~~

The Tauri host can be checked separately:

~~~sh
cargo check --locked --manifest-path web/src-tauri/Cargo.toml --all-targets
~~~

## Reproducible builds

The repository tracks its Rust and JavaScript dependency lockfiles: `Cargo.lock`, `web/src-tauri/Cargo.lock`, and `web/package-lock.json`. Install JavaScript dependencies with `npm ci`; use Cargo's `--locked` flag for builds and checks. CI verifies these lockfiles with `cargo fetch --locked` before compilation and fails if a manifest change would rewrite them.

The build toolchain is pinned in CI: Rust 1.100.0, Node.js 22.23.4, Temurin Java 17.0.21+9, Android SDK Platform 36 (the latest stable API; generated Tauri projects are explicitly kept off pre-release API 37), Android Build Tools 36.0.0, Android NDK 30.0.16248370, and Gradle 9.6.1 with its official SHA-256 distribution checksum enforced. The Android Gradle project is generated by Tauri rather than stored as source. The Android CI job resolves the complete generated-project lock set and SHA-256 dependency-verification metadata from the exact commit, uploads it as the `miora-android-gradle-lockfiles` workflow artifact, and uses it for the signed smoke build. Tag releases run the full validation workflow, download that artifact from the same workflow run, and apply it in strict dependency-locking mode to the production APK build. The production build therefore consumes the exact Gradle dependency graph already verified by its tag-validation run; the generated lock artifact is not silently re-resolved between smoke and production builds.

## CI and releases

GitHub Actions checks Rust formatting, tests and Clippy; checks Svelte/TypeScript; builds the web frontend; and builds an ARM64 Android debug APK. Tag-triggered releases call the same reusable Rust/Web quality workflow against the exact tag commit before any production signing or publishing step.

A tag matching `vMAJOR.MINOR.PATCH` (optionally with a prerelease suffix) triggers a signed Android release. The tag sets the APK version name; the GitHub Actions release workflow run number sets the monotonically increasing Android version code. The release workflow verifies both values in the APK before publishing. Configure these Actions repository secrets before creating a release tag:

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

RustSec CI scans the Android target (`aarch64-android`). It records two narrowly scoped exceptions: `RUSTSEC-2026-0374` (`basic-toml`) and `RUSTSEC-2024-0370` (`proc-macro-error`) are unmaintained build-time tooling with no available patched release. The shared Tauri lockfile also contains Linux desktop GTK dependencies, so the RustSec report prints `RUSTSEC-2024-0429` for `glib 0.18.5` even when the command is invoked with Android target flags; that GTK dependency is excluded by target-specific compilation from the Android APK, but remains a tracked unsoundness risk for optional Linux desktop builds until Tauri's GTK stack moves to `glib 0.20+`. Miora core and its manifests no longer depend on `chrono` directly. The current Tauri dependency graph still brings `chrono` transitively through `tauri-utils` and `serde_with`; RustSec reports that upstream crate as unmaintained, with no patched release currently available. It is tracked separately from Miora's own Jiff-based date logic. All other actionable Rust advisories fail CI and release builds.

## Data and privacy

The Android app stores its database in Tauri's private app data directory. Android cloud backup and device-to-device data extraction are disabled for the app's private data. Miora does not need an account, cloud service, analytics SDK, or remote API. The browser preview uses localStorage only for local development and is separate from the Android SQLite database.

## License

MIT
