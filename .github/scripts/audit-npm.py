#!/usr/bin/env python3
"""Run npm audit and allow only one documented, dev-only, unpatched advisory.

picomatch's GHSA-fc5q-39xc-vjpf currently has no patched release. Miora only
uses picomatch through Vite/fdir in the development toolchain. The dev server
binds to loopback only; Android devices use ADB reverse forwarding rather than
exposing a LAN listener. No app/runtime package may inherit this
exception. Remove this exception as soon as picomatch publishes a fix.
"""
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
LOCK_PATH = ROOT / "web" / "package-lock.json"
ADVISORY_URL = "https://github.com/advisories/GHSA-fc5q-39xc-vjpf"
ALLOWED_PACKAGES = {"picomatch", "fdir"}
SEVERITIES = {"high", "critical"}


def advisory_urls(name: str, vulnerability: dict, all_vulnerabilities: dict, seen=None):
    seen = set() if seen is None else seen
    if name in seen:
        return set()
    seen.add(name)
    urls = set()
    for item in vulnerability.get("via", []):
        if isinstance(item, dict):
            url = item.get("url")
            if url:
                urls.add(url)
        elif isinstance(item, str) and item in all_vulnerabilities:
            urls.update(
                advisory_urls(item, all_vulnerabilities[item], all_vulnerabilities, seen)
            )
    return urls


def node_is_dev_only(node: str, packages: dict) -> bool:
    package = packages.get(node)
    return isinstance(package, dict) and package.get("dev") is True


def version_at_or_below_4_0_7(version: str) -> bool:
    try:
        parts = tuple(int(part) for part in version.split(".")[:3])
    except (TypeError, ValueError):
        return False
    return parts <= (4, 0, 7)


def main() -> int:
    lock = json.loads(LOCK_PATH.read_text())
    packages = lock.get("packages", {})
    result = subprocess.run(
        ["npm", "audit", "--json", "--prefix", "web"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    try:
        report = json.loads(result.stdout)
    except json.JSONDecodeError:
        sys.stderr.write(result.stderr)
        print("npm audit did not return valid JSON.", file=sys.stderr)
        return result.returncode or 1

    vulnerabilities = report.get("vulnerabilities", {})
    accepted = []
    rejected = []

    for name, vulnerability in vulnerabilities.items():
        if vulnerability.get("severity") not in SEVERITIES:
            continue

        nodes = vulnerability.get("nodes", [])
        dev_only = bool(nodes) and all(node_is_dev_only(node, packages) for node in nodes)
        urls = advisory_urls(name, vulnerability, vulnerabilities)

        allowed = (
            name in ALLOWED_PACKAGES
            and vulnerability.get("severity") == "high"
            and urls == {ADVISORY_URL}
            and dev_only
        )

        if name == "picomatch":
            picomatch = packages.get("node_modules/picomatch", {})
            allowed = allowed and version_at_or_below_4_0_7(picomatch.get("version"))
        elif name == "fdir":
            picomatch = packages.get("node_modules/picomatch", {})
            picomatch_is_dev = node_is_dev_only("node_modules/picomatch", packages)
            allowed = (
                allowed
                and picomatch_is_dev
                and version_at_or_below_4_0_7(picomatch.get("version"))
                and any(
                    item == "picomatch"
                    or (
                        isinstance(item, dict)
                        and item.get("name") == "picomatch"
                        and item.get("url") == ADVISORY_URL
                    )
                    for item in vulnerability.get("via", [])
                )
            )

        if allowed:
            accepted.append(name)
        else:
            rejected.append(
                {
                    "package": name,
                    "severity": vulnerability.get("severity"),
                    "urls": sorted(urls),
                    "nodes": nodes,
                    "dev_only": dev_only,
                }
            )

    if rejected:
        print("Unapproved high/critical npm advisories:", file=sys.stderr)
        print(json.dumps(rejected, indent=2), file=sys.stderr)
        return 1

    if result.returncode not in (0, 1):
        sys.stderr.write(result.stderr)
        print(f"npm audit exited unexpectedly with {result.returncode}.", file=sys.stderr)
        return result.returncode

    if accepted:
        print(
            "Accepted dev-only unpatched advisory GHSA-fc5q-39xc-vjpf "
            f"for {', '.join(sorted(accepted))}; all affected lockfile nodes are dev-only."
        )
    else:
        print("No high/critical npm advisories found.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
