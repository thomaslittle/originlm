#!/usr/bin/env python3
"""Prepare an unsigned update manifest for a release archive.

This does not sign, publish, install, or create a signing credential. The
result must be reviewed, then signed by the approved release signer. The Rust
runtime (`originlm-update`) verifies the signature, platform, protocol, checksum,
staged copy, active-session lease, activation, and rollback.
"""
import argparse
import datetime
import hashlib
import json
import platform
import pathlib
import sys

SCHEMA = 1
MAX_ARCHIVE = 1024 * 1024 * 1024


def sha256(path):
    digest = hashlib.sha256()
    with open(path, "rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def target():
    os_name = {"Windows": "windows", "Darwin": "macos", "Linux": "linux"}.get(platform.system())
    arch = {"AMD64": "x86_64", "x86_64": "x86_64", "arm64": "aarch64", "aarch64": "aarch64"}.get(platform.machine())
    if not os_name or not arch:
        raise SystemExit("Unsupported platform; refuse to invent a target")
    return f"{os_name}-{arch}"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=pathlib.Path)
    parser.add_argument("--version", required=True)
    parser.add_argument("--channel", default="alpha", choices=("alpha", "beta", "stable"))
    parser.add_argument("--protocol-version", default="1.0")
    parser.add_argument("--output", type=pathlib.Path)
    args = parser.parse_args()
    if not args.archive.is_file():
        raise SystemExit("archive is not a regular file")
    if args.archive.stat().st_size > MAX_ARCHIVE:
        raise SystemExit("archive exceeds the bound")
    if not args.version.startswith("0.1.0-alpha") and args.channel == "alpha":
        raise SystemExit("alpha channel requires the 0.1.0-alpha release family")
    payload = {
        "schemaVersion": SCHEMA,
        "version": args.version,
        "channel": args.channel,
        "platform": target(),
        "protocolVersion": args.protocol_version,
        "archive": args.archive.name,
        "archiveBytes": args.archive.stat().st_size,
        "archiveSha256": sha256(args.archive),
        "publishedAt": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "notesUrl": None,
        "signature": "",
    }
    output = args.output or args.archive.with_name("update-manifest.unsigned.json")
    output.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"Wrote unsigned manifest {output}; signing and publication remain approval gates.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
