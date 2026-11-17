#!/usr/bin/env python3
"""Run npm audit with one narrowly scoped, dev-only advisory exception.

GHSA-fc5q-39xc-vjpf affects picomatch and is inherited by a known Vite tooling
dependency chain. The exact package versions are allowlisted below and every
affected lockfile node must be development-only. No runtime package or new
version inherits the exception. Remove it when the upstream fix is available.
"""
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
LOCK_PATH = ROOT / "web" / "package-lock.json"
ADVISORY_URL = "https://github.com/advisories/GHSA-fc5q-39xc-vjpf"
# Exact versions observed in the current development-only dependency graph.
# Keep this list narrow: any new package/version/advisory must be reviewed.
ALLOWED_VERSIONS = {
    "picomatch": {"4.0.7"},
    "fdir": {"6.5.0"},
    "tinyglobby": {"0.2.17"},
    "svelte-check": {"4.7.8"},
    "vite": {"6.4.6"},
}
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


def affected_by_picomatch(name: str, vulnerabilities: dict, seen=None) -> bool:
    """Require a real dependency path to the one documented vulnerable crate."""
    if name == "picomatch":
        return True
    seen = set() if seen is None else seen
    if name in seen:
        return False
    seen.add(name)
    vulnerability = vulnerabilities.get(name, {})
    for item in vulnerability.get("via", []):
        if isinstance(item, str) and affected_by_picomatch(item, vulnerabilities, seen):
            return True
        if (
            isinstance(item, dict)
            and item.get("name") == "picomatch"
            and item.get("url") == ADVISORY_URL
        ):
            return True
    return False


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

        expected_versions = ALLOWED_VERSIONS.get(name, set())
        affected_nodes_have_expected_versions = bool(nodes) and all(
            packages.get(node, {}).get("version") in expected_versions
            for node in nodes
        )
        allowed = (
            name in ALLOWED_VERSIONS
            and vulnerability.get("severity") == "high"
            and urls == {ADVISORY_URL}
            and dev_only
            and affected_nodes_have_expected_versions
            and affected_by_picomatch(name, vulnerabilities)
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
