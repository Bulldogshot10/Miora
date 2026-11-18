#!/usr/bin/env python3
"""Record AGP 9.3.1's internal, dependency-free androidApis lock state.

AGP consumes this internal lock identity while serializing
ParseLibraryResourcesTask. It is not exposed as a resolvable Project
configuration, so Gradle's --write-locks cannot discover it. The Android API
classpath comes from the installed SDK, not an external module dependency.
Keeping it in the empty configuration set preserves strict dependency locking.
"""

from pathlib import Path

LOCKFILE = Path("app/gradle.lockfile")


def main() -> None:
    if not LOCKFILE.is_file():
        raise SystemExit(f"Generated app lockfile not found: {LOCKFILE}")

    lines = LOCKFILE.read_text(encoding="utf-8").splitlines()
    empty_indexes = [index for index, line in enumerate(lines) if line.startswith("empty=")]
    if len(empty_indexes) != 1:
        raise SystemExit(
            f"Expected exactly one empty-configuration entry in {LOCKFILE}, "
            f"found {len(empty_indexes)}."
        )

    index = empty_indexes[0]
    configurations = set(filter(None, lines[index][len("empty="):].split(",")))
    configurations.add("androidApis")
    lines[index] = "empty=" + ",".join(sorted(configurations))

    LOCKFILE.write_text("\n".join(lines) + "\n", encoding="utf-8")

    persisted_lines = LOCKFILE.read_text(encoding="utf-8").splitlines()
    persisted_empty = [
        line[len("empty="):].split(",")
        for line in persisted_lines
        if line.startswith("empty=")
    ]
    if len(persisted_empty) != 1 or "androidApis" not in persisted_empty[0]:
        raise SystemExit("Failed to persist :app:androidApis as an empty lock configuration.")

    print("Recorded :app:androidApis as an empty, strictly locked configuration.")


if __name__ == "__main__":
    main()
