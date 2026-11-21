#!/usr/bin/env python3
"""Fail CI on active high or critical npm advisories."""
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SEVERITIES = {"high", "critical"}


def main() -> int:
    result = subprocess.run(
        ["npm", "audit", "--json", "--audit-level=high", "--prefix", "web"],
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

    if report.get("error"):
        print("npm audit returned an error response:", file=sys.stderr)
        print(json.dumps(report["error"], indent=2), file=sys.stderr)
        return result.returncode or 1

    vulnerabilities = report.get("vulnerabilities", {})
    rejected = [
        {
            "package": name,
            "severity": item.get("severity"),
            "via": item.get("via", []),
            "nodes": item.get("nodes", []),
            "fixAvailable": item.get("fixAvailable"),
        }
        for name, item in vulnerabilities.items()
        if item.get("severity") in SEVERITIES
    ]

    if rejected:
        print("Active high/critical npm advisories found:", file=sys.stderr)
        print(json.dumps(rejected, indent=2), file=sys.stderr)
        return 1

    if result.returncode not in (0, 1):
        sys.stderr.write(result.stderr)
        print(f"npm audit exited unexpectedly with {result.returncode}.", file=sys.stderr)
        return result.returncode

    print("No active high/critical npm advisories found.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
