# Dependency advisory register

Last reviewed: 2026-11-18

This file records the remaining dependency advisories after lockfile and target review. These entries are intentionally visible; the CI allowlists do not imply that an upstream vulnerability has been fixed.

| Advisory | Locked dependency / path | Exposure in Miora | Current treatment |
|---|---|---|---|
| [GHSA-fc5q-39xc-vjpf](https://github.com/advisories/GHSA-fc5q-39xc-vjpf) | `picomatch` 4.0.7 and the affected Vite tooling chain | Development dependencies only; `npm audit --omit=dev --audit-level=high` reports no production dependency finding | Exact package/version/path allowlist in `.github/scripts/audit-npm.py`. The advisory currently lists no patched release. Do not use these glob parsers on attacker-controlled patterns in application runtime code. Re-check the lockfile whenever the frontend toolchain changes. |
| [RUSTSEC-2026-0543](https://rustsec.org/advisories/RUSTSEC-2026-0543.html) | `chrono` 0.4.45 via `serde_with` 3.24.0 → `tauri-utils` 2.10.3 in `web/src-tauri/Cargo.lock` | Transitive Tauri build/runtime utility dependency; Miora's own date and cycle domain uses `jiff` | All `chrono` versions are affected and RustSec lists no patched version. Keep tracking the upstream Tauri dependency graph; do not replace the crate by a fake compatibility patch. |
| [RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429.html) | `glib` 0.18.5 in `web/src-tauri/Cargo.lock` | GTK3 Linux host backend; not used by the Android target | Patched upstream in `glib >=0.20.0`, while the current GTK3 host dependency chain remains on 0.18.5. Reassess when Tauri/Wry changes its Linux GTK dependency chain. |
| RUSTSEC-2026-0374 | `basic-toml` 0.1.10 | Transitive configuration/build tooling dependency | Unmaintained advisory only; narrowly ignored in the Android-target `cargo audit` command until its upstream consumer removes it. |
| RUSTSEC-2024-0370 | `proc-macro-error` 1.0.4 | Transitive procedural-macro tooling dependency | Unmaintained advisory only; narrowly ignored in the Android-target `cargo audit` command until its upstream consumer removes it. |

The Rust lockfiles were also checked against the newly published ZIP advisories RUSTSEC-2026-0546 and RUSTSEC-2026-0548 and the `libz-sys` advisory RUSTSEC-2026-0544: neither `zip` nor `libz-sys` is present in either `Cargo.lock` or `web/src-tauri/Cargo.lock` at the review date.

A clean audit result means there are no unapproved findings in the scoped dependency set. It does not mean the upstream advisories in this register have disappeared. Remove entries only after the lockfile no longer contains the affected dependency or a patched release is verified.
