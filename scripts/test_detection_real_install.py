#!/usr/bin/env python3
"""Assert a real-install detection scan found the expected clients.

This is an evidence check for CI, not a discovery implementation. It reads the
`--include-locations` form of `originlm scan --json` and fails with the missing
providers, the commands that were found, and where they were found.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import sys

# Required for the real-install job. Each value is the command users install
# and the file name the scanner is expected to retain.
EXPECTED = {
    "codex": "codex",
    "claude_code": "claude",
    "gemini_cli": "gemini",
    "opencode": "opencode",
    "kiro": "kiro-cli",
}


def normalize(name: str) -> str:
    return name.lower().removesuffix(".exe").removesuffix(".cmd").removesuffix(".bat")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("scan", type=pathlib.Path)
    parser.add_argument(
        "--require",
        default=",".join(EXPECTED),
        help="comma-separated provider ids; defaults to the real-install set",
    )
    args = parser.parse_args()
    document = json.loads(args.scan.read_text(encoding="utf-8"))
    snapshot = document["snapshot"]
    locations = document.get("locations", [])
    providers = {
        item["providerId"] for item in snapshot.get("connections", [])
    }
    executable_by_provider: dict[str, list[str]] = {}
    for location in locations:
        executable = location.get("executable")
        if executable:
            executable_by_provider.setdefault(location["providerId"], []).append(executable)

    required = [part.strip() for part in args.require.split(",") if part.strip()]
    failures: list[str] = []
    for provider in required:
        if provider not in providers:
            failures.append(f"{provider}: provider was not discovered")
            continue
        executable = EXPECTED.get(provider)
        found = executable_by_provider.get(provider, [])
        if not found:
            failures.append(f"{provider}: discovered but no executable location was retained")
            continue
        wrong = [path for path in found if normalize(pathlib.Path(path).name) != executable]
        if wrong:
            failures.append(
                f"{provider}: expected a {executable!r} executable, found {', '.join(wrong)}"
            )
        if len(set(found)) != len(found):
            failures.append(f"{provider}: duplicate executable locations {found}")
        for path in found:
            if not pathlib.Path(path).is_file():
                failures.append(f"{provider}: retained executable does not exist: {path}")
        for location in locations:
            if location.get("providerId") == provider and not location.get("profileHome"):
                failures.append(f"{provider}: profile home was not recorded")

    print("provider\texecutable")
    for provider in sorted(EXPECTED):
        for path in executable_by_provider.get(provider, []):
            print(f"{provider}\t{path}")
    if failures:
        print("\nDETECTION REAL-INSTALL FAILURE", file=sys.stderr)
        for failure in failures:
            print(f"- {failure}", file=sys.stderr)
        return 1
    print(f"\nDetected {len(required)} required clients with executable locations.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
