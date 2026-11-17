# Miora

**Understand your rhythm.**

Miora is a lightweight, offline-first menstrual cycle and symptom tracker built with Rust and Slint. It is designed around quick daily interactions, personal cycle history, and transparent estimates.

## Product principles

- Local-first: personal cycle and symptom data stays in the app's private storage.
- Low-friction: common tracking actions should take a few taps.
- Evidence-led: estimates are derived from the user's own logged history.
- Transparent: predictions are shown as windows when the data supports them.
- Accessible: clear text contrast, large controls, and labels alongside color.

## Architecture

~~~text
src/
├── main.rs
├── lib.rs                 # Android cdylib entry point and app composition
├── app/
│   ├── navigation.rs
│   └── state.rs
├── domain/
│   ├── cycle.rs
│   ├── period.rs
│   ├── symptom.rs
│   └── prediction.rs
├── engine/
│   ├── cycle_estimator.rs
│   ├── symptom_model.rs
│   ├── bayesian.rs
│   └── statistics.rs
├── storage/
│   ├── database.rs
│   ├── migrations.rs
│   └── repository.rs
└── ui/
    ├── dashboard.slint
    ├── calendar.slint
    ├── checkin.slint
    └── insights.slint

tests/
├── cycle_tests.rs
├── prediction_tests.rs
└── symptom_tests.rs
~~~

The extra lib.rs file is the Android shared-library adapter required by the cdylib target. Product logic remains organized in the modules above.

## UI/UX direction

Miora uses a mobile-first, soft editorial visual system with a persistent four-tab navigation: Home, Calendar, Check-in, and Insights.

The design is informed by menstrual-health tracking research from CHI 2024, which identifies five functional requirements: cycle-event and symptom predictions, support for multiple signals, inspection of individual signals, comparison between signals, and comparison across cycles.

The default overview uses a linear timeline and calendar dates for planning. Insights are reserved for historical comparisons once enough real data exists.

Accessibility targets:
- At least 48 × 48 dp touch targets.
- At least 4.5:1 contrast for normal text.
- At least 3:1 contrast for large text.
- Phase labels accompany phase colors.

## Prediction engine

The initial cycle estimator uses robust statistics over completed cycle intervals. It waits for at least two completed intervals before producing a prediction window.

A discrete Bayesian posterior is included for experimentation with cycle-length distributions. Symptom probability estimates use cycle-day-specific observations with Laplace smoothing.

The app does not fabricate predictions when there is insufficient personal history.

## Stack

- Rust
- Slint 1.19
- SQLite via rusqlite
- Chrono
- cargo-apk2

## Run on desktop

~~~sh
cargo run
~~~

The local development database is stored in .miora/miora.sqlite3.

## Test and lint

~~~sh
cargo fmt --all -- --check
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
~~~

## Build for Android

Install the Android Rust target:

~~~sh
rustup target add aarch64-linux-android
~~~

Install the Android SDK, NDK r30, and Java 17. Configure ANDROID_HOME, ANDROID_NDK_ROOT, and JAVA_HOME.

~~~sh
cargo install cargo-apk2
cargo apk2 build --target aarch64-linux-android --lib
~~~

The project targets Android API 36 and supports Android API 26 and newer.

## CI and releases

GitHub Actions runs formatting, tests, Clippy, and an ARM64 Android debug APK build on pushes and pull requests to main.

A tag matching v* triggers the signed Android release workflow. Configure these repository Actions secrets before creating a release tag:

- ANDROID_KEYSTORE_BASE64
- ANDROID_KEYSTORE_PASSWORD

Keep one persistent release keystore and back it up securely. The release workflow attaches the signed APK and SHA-256 checksum to the GitHub Release.

## Data and privacy

The app uses local SQLite storage and does not require an account, cloud service, analytics SDK, or network permission. The Android database is kept in the app's private internal data directory.

## License

MIT
