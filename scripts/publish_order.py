#!/usr/bin/env python3
"""Validate and print the crates.io publication order for the OriginLM facade.

The order is derived from ``cargo metadata``, never from a hand-maintained list:

* the publishable set is the normal/build dependency closure of the public
  ``originlm`` facade and the ``originlm-cli`` helper crate;
* every intra-workspace dependency of a publishable crate must declare both the
  workspace version and a path;
* dependencies are emitted before their dependents (stable alphabetical DFS);
* cycles, missing version/path pairs, and a publishable crate marked
  ``publish = false`` fail closed;
* the declared MSRV is read from Cargo metadata instead of guessed.

Nothing here publishes a crate. ``--package`` only runs ``cargo package
--no-verify`` for local verification.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
from collections.abc import Iterable
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FACADE = "originlm"
ROOTS = (FACADE, "originlm-cli")
WORKSPACE_VERSION = "0.1.0-alpha.22"
VERSION_RE = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+(?:[-+][0-9A-Za-z.-]+)?$")


class PublishOrderError(RuntimeError):
    """A publication invariant did not hold."""


@dataclass(frozen=True)
class WorkspacePackage:
    name: str
    version: str
    manifest_path: Path
    publish: list[str] | None
    rust_version: str | None
    normal_dependencies: tuple[str, ...]


def load_metadata(manifest_path: Path = ROOT / "Cargo.toml") -> dict:
    completed = subprocess.run(
        [
            "cargo",
            "metadata",
            "--no-deps",
            "--format-version",
            "1",
            "--manifest-path",
            str(manifest_path),
        ],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    return json.loads(completed.stdout)


def workspace_packages(metadata: dict) -> dict[str, WorkspacePackage]:
    workspace_members = set(metadata["workspace_members"])
    packages: dict[str, WorkspacePackage] = {}
    for package in metadata["packages"]:
        if package["id"] not in workspace_members:
            continue
        dependencies = []
        for dependency in package["dependencies"]:
            if dependency["kind"] in ("dev", "development"):
                continue
            # Only workspace packages are ordered here. Registry dependencies
            # have no path and are not part of the local publish graph.
            if not dependency.get("path"):
                continue
            dependencies.append(dependency["name"])
        packages[package["name"]] = WorkspacePackage(
            name=package["name"],
            version=package["version"],
            manifest_path=Path(package["manifest_path"]),
            publish=package.get("publish"),
            rust_version=package.get("rust_version"),
            normal_dependencies=tuple(sorted(set(dependencies))),
        )
    return packages


def is_publishable(package: WorkspacePackage) -> bool:
    # cargo metadata reports None for the default (publishable) state and an
    # empty list for ``publish = false``.
    return package.publish is None or bool(package.publish)


def publishable_closure(
    packages: dict[str, WorkspacePackage], roots: tuple[str, ...]
) -> list[str]:
    """Stable dependency-first closure of the public roots over workspace packages."""
    for root in roots:
        if root not in packages:
            raise PublishOrderError(f"publish root {root!r} is not a workspace member")

    order: list[str] = []
    state: dict[str, str] = {}

    def visit(name: str, chain: tuple[str, ...]) -> None:
        status = state.get(name)
        if status == "done":
            return
        if status == "visiting":
            cycle = " -> ".join((*chain, name))
            raise PublishOrderError(f"workspace dependency cycle: {cycle}")
        package = packages.get(name)
        if package is None:
            raise PublishOrderError(
                f"publishable dependency {name!r} is not a workspace package"
            )
        if not is_publishable(package):
            raise PublishOrderError(
                f"{name!r} is in the publishable closure but has publish = false"
            )
        state[name] = "visiting"
        for dependency in package.normal_dependencies:
            if dependency in packages:
                visit(dependency, (*chain, name))
        state[name] = "done"
        order.append(name)

    for root in roots:
        visit(root, ())
    return order


def _split_req(req: str) -> tuple[str, str] | None:
    for operator in ("^", "=", "~"):
        if req.startswith(operator):
            return operator, req[len(operator) :]
    return None


def _dependency_version_problem(dependency: dict) -> str | None:
    req = dependency.get("req", "")
    split = _split_req(req)
    version = split[1] if split else req
    if not VERSION_RE.match(version):
        return "must declare an explicit registry version"
    if version != WORKSPACE_VERSION:
        return (
            f"version {req!r} is not the workspace version "
            f"{WORKSPACE_VERSION!r}"
        )
    return None


def validate(metadata: dict) -> tuple[list[str], dict[str, WorkspacePackage]]:
    packages = workspace_packages(metadata)
    order = publishable_closure(packages, ROOTS)
    publishable = set(order)
    problems: list[str] = []
    package_by_manifest_dir = {
        package.manifest_path.parent.resolve(): package for package in packages.values()
    }

    for metadata_package in metadata["packages"]:
        name = metadata_package["name"]
        if name not in packages:
            continue
        for dependency in metadata_package["dependencies"]:
            target = (
                package_by_manifest_dir.get(Path(dependency["path"]).resolve())
                if dependency.get("path")
                else packages.get(dependency["name"])
            )
            if target is None:
                continue
            # Every intra-workspace dependency of a publishable crate must be
            # registry-resolvable, including dev-dependencies.
            if name in publishable:
                problem = _dependency_version_problem(dependency)
                if not dependency.get("path"):
                    problems.append(
                        f"{name} -> {target.name}: intra-workspace dependency must "
                        f'declare path = "../{target.name}" '
                        f"and version = \"{WORKSPACE_VERSION}\""
                    )
                elif problem:
                    problems.append(f"{name} -> {target.name}: {problem}")
        package = packages[name]
        if package.name in publishable and package.version != WORKSPACE_VERSION:
            problems.append(
                f"{package.name}: version {package.version!r} is not the workspace "
                f"version {WORKSPACE_VERSION!r}"
            )
        if package.name not in publishable and is_publishable(package):
            problems.append(
                f"{package.name}: excluded from the facade closure but still publishable; "
                "set publish = false"
            )

    declared_versions = {
        package.rust_version for package in packages.values() if package.rust_version
    }
    if len(declared_versions) != 1:
        problems.append(
            "workspace crates declare inconsistent rust-version values: "
            + ", ".join(sorted(str(version) for version in declared_versions))
        )

    if problems:
        raise PublishOrderError("\n".join(sorted(set(problems))))
    return order, packages


def _cargo_version() -> tuple[int, int, int]:
    completed = subprocess.run(
        ["cargo", "--version"], cwd=ROOT, check=True, capture_output=True, text=True
    )
    match = re.search(r"cargo\s+(\d+)\.(\d+)\.(\d+)", completed.stdout)
    if not match:
        raise PublishOrderError(f"could not read cargo version from {completed.stdout!r}")
    return tuple(int(part) for part in match.groups())  # type: ignore[return-value]


def _rustc_version() -> tuple[int, int, int]:
    completed = subprocess.run(
        ["rustc", "--version"], cwd=ROOT, check=True, capture_output=True, text=True
    )
    match = re.search(r"rustc\s+(\d+)\.(\d+)\.(\d+)", completed.stdout)
    if not match:
        raise PublishOrderError(f"could not read rustc version from {completed.stdout!r}")
    return tuple(int(part) for part in match.groups())  # type: ignore[return-value]


def msrv_report(packages: dict[str, WorkspacePackage], selected: Iterable[str]) -> str:
    versions = sorted(
        {
            packages[name].rust_version
            for name in selected
            if packages[name].rust_version
        }
    )
    if not versions:
        return "declared MSRV: none (cargo metadata has no rust-version)"
    if len(versions) > 1:
        raise PublishOrderError(
            "publishable crates declare different MSRVs: " + ", ".join(versions)
        )
    declared = versions[0]
    parts = tuple(int(part) for part in declared.split("."))
    rustc = _rustc_version()
    cargo = _cargo_version()
    if rustc < parts:
        raise PublishOrderError(
            f"declared MSRV {declared} is not satisfied by rustc "
            f"{'.'.join(map(str, rustc))}"
        )
    return (
        f"declared MSRV {declared} (satisfied); rustc {'.'.join(map(str, rustc))}; "
        f"cargo {'.'.join(map(str, cargo))}"
    )


def package_in_order(order: list[str], packages: dict[str, WorkspacePackage], package_dir: Path | None = None) -> None:
    """Package every publishable crate as if its predecessors were published.

    ``cargo package`` resolves intra-workspace versions against crates.io. The
    temporary ``[patch.crates-io]`` entries below point those versions at the
    local workspace, which is exactly the state a maintainer would have after
    publishing the earlier crates in this order. Nothing is uploaded and no
    patch is written to the repository.
    """
    for name in order:
        print(f"packaging {name}", flush=True)
        command = [
            "cargo",
            "package",
            "-p",
            name,
            "--no-verify",
            "--allow-dirty",
            "--offline",
        ]
        for dependency in _transitive_dependencies(name, packages):
            path = packages[dependency].manifest_path.parent
            command.extend(
                [
                    "--config",
                    f"patch.crates-io.{dependency}.path={json.dumps(str(path))}",
                ]
            )
        subprocess.run(command, cwd=ROOT, check=True)
    if package_dir is not None:
        target = Path(os.environ.get('CARGO_TARGET_DIR', ROOT / 'target'))
        if not target.is_absolute():
            target = ROOT / target
        package_dir.mkdir(parents=True, exist_ok=True)
        records = []
        for name in order:
            crate = target / 'package' / f'{name}-{packages[name].version}.crate'
            if not crate.is_file():
                raise PublishOrderError(f'packaged crate is missing: {crate}')
            destination = package_dir / crate.name
            shutil.copy2(crate, destination)
            records.append({
                'name': name,
                'version': packages[name].version,
                'file': destination.name,
                'bytes': destination.stat().st_size,
                'sha256': hashlib.sha256(destination.read_bytes()).hexdigest(),
            })
        commit = subprocess.run(
            ['git', 'rev-parse', 'HEAD'], cwd=ROOT, check=True, capture_output=True, text=True
        ).stdout.strip()
        (package_dir / 'SHA256SUMS').write_text(
            ''.join(f'{record["sha256"]}  {record["file"]}\n' for record in records),
            encoding='utf-8',
        )
        (package_dir / 'manifest.json').write_text(
            json.dumps({
                'schemaVersion': 1,
                'kind': 'originlm-crate-release-set',
                'sourceCommit': commit,
                'order': order,
                'packages': records,
            }, indent=2, sort_keys=True) + '\n',
            encoding='utf-8',
        )


def _transitive_dependencies(
    name: str, packages: dict[str, WorkspacePackage]
) -> list[str]:
    needed: set[str] = set()

    def visit(current: str) -> None:
        for dependency in packages[current].normal_dependencies:
            if dependency in packages and dependency not in needed:
                needed.add(dependency)
                visit(dependency)

    visit(name)
    return sorted(needed)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json", action="store_true", help="print the order as JSON")
    parser.add_argument(
        "--check", action="store_true", help="validate only (the default action)"
    )
    parser.add_argument(
        "--package",
        action="store_true",
        help="run cargo package --no-verify for every crate in order",
    )
    parser.add_argument(
        "--package-dir",
        type=Path,
        help="copy the packaged .crate files and a hashed manifest to this directory",
    )
    args = parser.parse_args(argv)

    try:
        order, packages = validate(load_metadata())
        if args.json:
            print(json.dumps({"order": order, "msrv": msrv_report(packages, order)}, indent=2))
        else:
            print("\n".join(order))
            print(msrv_report(packages, order), file=sys.stderr)
        if args.package:
            package_in_order(order, packages, args.package_dir)
    except (PublishOrderError, subprocess.CalledProcessError) as error:
        print(f"publish-order check failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
