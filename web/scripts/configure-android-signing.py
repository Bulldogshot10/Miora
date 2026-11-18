#!/usr/bin/env python3
"""Configure Tauri's generated Android Gradle project for signed release builds."""
from __future__ import annotations

import os
import sys
from pathlib import Path


def java_property_value(value: str) -> str:
    if "\r" in value or "\n" in value:
        raise ValueError("Android signing values must not contain newline characters.")
    encoded = value.encode("utf-16-be")
    escaped: list[str] = []
    for offset in range(0, len(encoded), 2):
        code_unit = int.from_bytes(encoded[offset : offset + 2], "big")
        character = chr(code_unit)
        if character == "\\":
            escaped.append("\\\\")
        elif character == " ":
            escaped.append("\\ " if not escaped else " ")
        elif character == "\t":
            escaped.append("\\t")
        elif character == "\f":
            escaped.append("\\f")
        elif character in "#!:=":
            escaped.append("\\" + character)
        elif code_unit < 0x20 or code_unit > 0x7E:
            escaped.append(f"\\u{code_unit:04X}")
        else:
            escaped.append(character)
    return "".join(escaped)


def main() -> int:
    keystore_path = Path(os.environ["ANDROID_KEYSTORE_PATH"]).resolve(strict=True)
    alias = os.environ["ANDROID_KEY_ALIAS"]
    key_password = os.environ["ANDROID_KEY_PASSWORD"]
    store_password = os.environ.get("ANDROID_KEY_STORE_PASSWORD") or key_password

    if not keystore_path.is_file() or keystore_path.stat().st_size == 0:
        raise SystemExit("Android signing keystore is missing or empty.")
    if not alias or not key_password or not store_password:
        raise SystemExit("Android signing alias and passwords must be non-empty.")

    project = Path("web/src-tauri/gen/android")
    gradle_file = project / "app/build.gradle.kts"
    if not gradle_file.is_file():
        raise SystemExit("Generated Android Gradle app/build.gradle.kts is missing.")

    properties_file = project / "keystore.properties"
    rows = [
        "keyAlias=" + java_property_value(alias),
        "keyPassword=" + java_property_value(key_password),
        "storePassword=" + java_property_value(store_password),
        "storeFile=" + java_property_value(str(keystore_path)),
    ]
    temporary = properties_file.with_suffix(".properties.tmp")
    temporary.write_text("\n".join(rows) + "\n", encoding="ascii")
    temporary.chmod(0o600)
    temporary.replace(properties_file)
    properties_file.chmod(0o600)

    source = gradle_file.read_text(encoding="utf-8")
    imports = []
    if "import java.io.FileInputStream" not in source:
        imports.append("import java.io.FileInputStream")
    if "import java.util.Properties" not in source:
        imports.append("import java.util.Properties")
    if imports:
        source = "\n".join(imports) + "\n" + source

    signing_block = '''signingConfigs {
        create("release") {
            val keystorePropertiesFile = rootProject.file("keystore.properties")
            val keystoreProperties = Properties()
            keystoreProperties.load(FileInputStream(keystorePropertiesFile))
            keyAlias = keystoreProperties["keyAlias"] as String
            keyPassword = keystoreProperties["keyPassword"] as String
            storeFile = file(keystoreProperties["storeFile"] as String)
            storePassword = keystoreProperties["storePassword"] as String
        }
    }

'''
    if 'create("release")' not in source:
        marker = "buildTypes {"
        if marker not in source:
            raise SystemExit("Could not find Gradle buildTypes; refusing to produce an unsigned release.")
        source = source.replace(marker, signing_block + marker, 1)

    release_marker = 'getByName("release") {'
    if release_marker not in source:
        raise SystemExit("Could not find release build type; refusing to produce an unsigned release.")

    signing_line = 'signingConfig = signingConfigs.getByName("release")'
    if signing_line not in source:
        source = source.replace(release_marker, release_marker + "\n            " + signing_line, 1)

    if source.count(signing_line) != 1 or 'create("release")' not in source:
        raise SystemExit("Gradle release signing configuration was not applied exactly once.")

    gradle_file.write_text(source, encoding="utf-8")
    print("Android release signing configured.")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (KeyError, OSError, ValueError) as error:
        print(f"Android signing configuration failed: {error}", file=sys.stderr)
        raise SystemExit(1)
