"""Run the OriginLM Desktop end-to-end gate.

The GPUI tests render the real Playground page. The service/adaptor suites
exercise the real conversation handoff, model switch, resume and HTTP request
shaping that the UI depends on.
"""
from __future__ import annotations

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
COMMANDS = [
    ["cargo", "test", "--locked", "-p", "originlm-desktop", "--all-targets"],
    ["cargo", "test", "--locked", "-p", "originlm-service", "--test", "model_switch"],
    ["cargo", "test", "--locked", "-p", "originlm-adapters", "--test", "http_execution"],
    ["cargo", "test", "--locked", "-p", "originlm-adapters", "--test", "opencode_v2"],
    ["cargo", "test", "--locked", "-p", "originlm-adapters", "--lib", "v2_service"],
]


def main() -> int:
    for command in COMMANDS:
        print("+", " ".join(command), flush=True)
        result = subprocess.run(command, cwd=ROOT)
        if result.returncode:
            return result.returncode
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
