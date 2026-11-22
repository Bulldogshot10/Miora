#!/usr/bin/env python3
"""Record AGP 9.3.1's internal, dependency-free androidApis lock state.

AGP consumes this internal lock identity while serializing
ParseLibraryResourcesTask. It is not exposed as a resolvable Project
configuration, so Gradle's --write-locks cannot discover it. The Android API
classpath comes from the installed SDK, not an external module dependency.
Both the application and Tauri Android library are separate Gradle projects,
so each needs its own explicit empty lock state. This preserves strict locking.
"""

import sys
from pathlib import Path


def record_empty_android_apis(lockfile: Path) -> None:
    if not lockfile.is_file():
        raise SystemExit(f"Generated Gradle lockfile not found: {lockfile}")

    lines = lockfile.read_text(encoding="utf-8").splitlines()
    empty_indexes = [index for index, line in enumerate(lines) if line.startswith("empty=")]
    if len(empty_indexes) != 1:
        raise SystemExit(
            f"Expected exactly one empty-configuration entry in {lockfile}, "
            f"found {len(empty_indexes)}."
        )

    index = empty_indexes[0]
    configurations = set(filter(None, lines[index][len("empty="):].split(",")))
    configurations.add("androidApis")
    lines[index] = "empty=" + ",".join(sorted(configurations))
    lockfile.write_text("\n".join(lines) + "\n", encoding="utf-8")

    persisted_lines = lockfile.read_text(encoding="utf-8").splitlines()
    persisted_empty = [
        line[len("empty="):].split(",")
        for line in persisted_lines
        if line.startswith("empty=")
    ]
    if len(persisted_empty) != 1 or "androidApis" not in persisted_empty[0]:
        raise SystemExit(
            f"Failed to persist {lockfile}:androidApis as an empty lock configuration."
        )

    print(f"Recorded {lockfile}:androidApis as an empty, strictly locked configuration.")


def main() -> None:
    lockfiles = [Path(argument) for argument in sys.argv[1:]]
    if len(lockfiles) != 2:
        raise SystemExit(
            "Usage: record-androidapis-lock.py APP_LOCKFILE TAURI_ANDROID_LOCKFILE"
        )

    for lockfile in lockfiles:
        record_empty_android_apis(lockfile)


if __name__ == "__main__":
    main()
