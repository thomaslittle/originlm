#!/usr/bin/env python3
"""Compare Tokscale discovery knowledge with the pinned OriginLM revision.

Released builds do not run this script. It is a maintainer and CI tool.
Tokscale remains an upstream source of path knowledge; this process never
merges changes into production discovery code.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TOKSCALE_DIR = ROOT / "upstream" / "tokscale"
LOCK_PATH = TOKSCALE_DIR / "lock.json"
SNAPSHOT_PATH = TOKSCALE_DIR / "snapshot.json"
MAPPINGS_PATH = TOKSCALE_DIR / "mappings.json"
SOURCES_PATH = ROOT / "upstream" / "sources.json"
REPOSITORY = "junhoyeo/tokscale"
GIT_URL = f"https://github.com/{REPOSITORY}.git"
API = f"https://api.github.com/repos/{REPOSITORY}"

WATCHED = [
    "crates/tokscale-core/src/clients.rs",
    "crates/tokscale-core/src/paths.rs",
    "crates/tokscale-core/src/scanner.rs",
    "crates/tokscale-core/src/cc_mirror.rs",
    "crates/tokscale-core/src/sessions",
]

ENTRY_START = re.compile(r"([A-Za-z0-9_]+)\s*=\s*(\d+)\s*=>\s*\{")
ENV_ROOT = re.compile(
    r'root:\s*PathRoot::EnvVar\s*\{.*?var:\s*"([^"]+)".*?fallback_relative:\s*"([^"]+)"',
    re.S,
)
SIMPLE_ROOT = re.compile(r"root:\s*PathRoot::([A-Za-z0-9_]+)\s*,")
RELATIVE = re.compile(r'(?<![A-Za-z0-9_])relative:\s*"([^"]*)"')
CLIENT_ID = re.compile(r'\bid:\s*"([^"]+)"')
PATTERN = re.compile(r'pattern:\s*"([^"]*)"')
ENV_CALL = re.compile(r'env::var(?:_os)?\s*\(\s*"([A-Z][A-Z0-9_]*)"')
ENV_FIELD = re.compile(r'\bvar:\s*"([A-Z][A-Z0-9_]*)"')
STRING_LITERAL = re.compile(r'"([^"\n\\]{1,180})"')
ROOT_KINDS = {
    "Home": "home",
    "XdgData": "xdg-data",
    "Config": "config",
    "AppData": "app-data",
    "ReasonixHome": "reasonix",
}


def load_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def strip_rust_tests(text: str) -> str:
    lines = text.splitlines(keepends=True)
    output: list[str] = []
    index = 0
    while index < len(lines):
        if lines[index].lstrip().startswith("#[cfg(test)]"):
            cursor = index + 1
            while cursor < len(lines) and lines[cursor].strip() == "":
                cursor += 1
            if cursor < len(lines) and re.match(r"\s*(pub(\([^)]*\))?\s+)?mod\s+", lines[cursor]):
                depth = 0
                started = False
                while cursor < len(lines):
                    depth += lines[cursor].count("{") - lines[cursor].count("}")
                    if "{" in lines[cursor]:
                        started = True
                    cursor += 1
                    if started and depth <= 0:
                        break
                index = cursor
                continue
        output.append(lines[index])
        index += 1
    return "".join(output)


def parse_clients(text: str) -> list[dict]:
    start = text.find("define_clients!")
    if start < 0:
        raise SystemExit("Tokscale clients.rs has no define_clients! registry")
    body = text[start:]
    clients = []
    for match in ENTRY_START.finditer(body):
        brace = match.end() - 1
        depth = 0
        end = None
        for offset, character in enumerate(body[brace:], brace):
            if character == "{":
                depth += 1
            elif character == "}":
                depth -= 1
                if depth == 0:
                    end = offset
                    break
        if end is None:
            continue
        entry = body[brace : end + 1]
        identity = CLIENT_ID.search(entry)
        relative = RELATIVE.search(entry)
        pattern = PATTERN.search(entry)
        if not identity or not relative or not pattern:
            continue
        env_root = ENV_ROOT.search(entry)
        if env_root:
            root = {
                "kind": "env",
                "var": env_root.group(1),
                "fallback": env_root.group(2),
            }
        else:
            simple = SIMPLE_ROOT.search(entry)
            if not simple:
                continue
            root = {"kind": ROOT_KINDS.get(simple.group(1), simple.group(1))}
        clients.append(
            {
                "enum": match.group(1),
                "index": int(match.group(2)),
                "id": identity.group(1),
                "root": root,
                "relative": relative.group(1),
                "pattern": pattern.group(1),
            }
        )
    if not clients:
        raise SystemExit("Tokscale client registry parsed as empty")
    clients.sort(key=lambda item: item["index"])
    return clients


def discovery_literals(text: str) -> tuple[list[str], list[str]]:
    source = strip_rust_tests(text)
    env_vars = set(ENV_CALL.findall(source))
    env_vars.update(ENV_FIELD.findall(source))
    paths = set()
    for literal in STRING_LITERAL.findall(source):
        if literal.startswith(("http://", "https://")):
            continue
        if "/" not in literal and "\\" not in literal and not literal.startswith("."):
            continue
        if " " in literal and "/" not in literal and "\\" not in literal:
            continue
        paths.add(literal)
    return sorted(env_vars), sorted(paths)


def read_watched(source: Path) -> str:
    chunks = []
    for relative in WATCHED:
        path = source / relative
        if path.is_dir():
            for file in sorted(path.rglob("*.rs")):
                chunks.append(file.read_text(encoding="utf-8", errors="replace"))
        elif path.is_file():
            chunks.append(path.read_text(encoding="utf-8", errors="replace"))
        else:
            raise SystemExit(f"watched Tokscale path is missing: {relative}")
    return "\n".join(chunks)


def extract(source: Path, commit: str) -> dict:
    clients_path = source / "crates" / "tokscale-core" / "src" / "clients.rs"
    clients = parse_clients(clients_path.read_text(encoding="utf-8", errors="replace"))
    env_vars, paths = discovery_literals(read_watched(source))
    return {
        "repository": REPOSITORY,
        "commit": commit,
        "clients": clients,
        "envVars": env_vars,
        "pathLiterals": paths,
    }


def client_key(client: dict) -> str:
    return json.dumps(
        {
            "id": client["id"],
            "root": client["root"],
            "relative": client["relative"],
            "pattern": client["pattern"],
        },
        sort_keys=True,
    )


def diff_indexes(previous: dict, current: dict) -> dict:
    old = {item["id"]: item for item in previous.get("clients", [])}
    new = {item["id"]: item for item in current.get("clients", [])}
    client_changes = []
    for identifier in sorted(set(old) | set(new)):
        if identifier not in old:
            client_changes.append({"id": identifier, "change": "added", "current": new[identifier]})
        elif identifier not in new:
            client_changes.append(
                {"id": identifier, "change": "removed", "previous": old[identifier]}
            )
        elif client_key(old[identifier]) != client_key(new[identifier]):
            client_changes.append(
                {
                    "id": identifier,
                    "change": "modified",
                    "previous": old[identifier],
                    "current": new[identifier],
                }
            )
    old_env = set(previous.get("envVars", []))
    new_env = set(current.get("envVars", []))
    old_paths = set(previous.get("pathLiterals", []))
    new_paths = set(current.get("pathLiterals", []))
    return {
        "clients": client_changes,
        "envAdded": sorted(new_env - old_env),
        "envRemoved": sorted(old_env - new_env),
        "pathsAdded": sorted(new_paths - old_paths),
        "pathsRemoved": sorted(old_paths - new_paths),
    }


def diff_is_relevant(changes: dict) -> bool:
    return any(changes[key] for key in ("clients", "envAdded", "envRemoved", "pathsAdded", "pathsRemoved"))


def display_join(root: str, relative: str, platform: str) -> str:
    separator = "\\" if platform == "windows" else "/"
    parts = [part for part in relative.replace("\\", "/").split("/") if part and part != "."]
    base = root.rstrip("\\/")
    if not parts:
        return base
    return base + separator + separator.join(parts)


def resolve_root(root: dict, home: str, platform: str, env: dict[str, str]) -> str:
    kind = root.get("kind")
    if kind == "home":
        return home
    if kind == "xdg-data":
        return env.get("XDG_DATA_HOME") or display_join(home, ".local/share", platform)
    if kind == "config":
        custom = env.get("TOKSCALE_CONFIG_DIR", "")
        if custom.strip():
            return custom
        if platform == "windows" and env.get("APPDATA"):
            return display_join(env["APPDATA"], "tokscale", platform)
        return display_join(home, ".config/tokscale", platform)
    if kind == "app-data":
        if platform == "windows":
            return env.get("APPDATA") or display_join(home, "AppData/Roaming", platform)
        if platform == "macos":
            return display_join(home, "Library/Application Support", platform)
        return env.get("XDG_CONFIG_HOME") or display_join(home, ".config", platform)
    if kind == "env":
        value = env.get(root.get("var", ""), "")
        if value.strip():
            return value
        return display_join(home, root.get("fallback", ""), platform)
    if kind == "reasonix":
        return display_join(home, ".reasonix", platform)
    return home


def resolve_client(client: dict, home: str, platform: str, env: dict[str, str] | None = None) -> str:
    base = resolve_root(client["root"], home, platform, env or {})
    return display_join(base, client.get("relative", ""), platform)


def describe_root(root: dict) -> str:
    if root.get("kind") == "env":
        return f'{root.get("var")} || ~/{root.get("fallback")}'
    return str(root.get("kind"))


def format_report(
    previous_commit: str,
    current_commit: str,
    changes: dict,
    mappings: dict,
    parity_text: str,
    parity_status: str,
) -> str:
    mapped = mappings.get("clients", {})
    lines = [
        "Tokscale upstream discovery changes",
        "",
        f"Tokscale revision: {previous_commit} -> {current_commit}",
        f"Local parity: {parity_status}",
        "",
    ]
    if not diff_is_relevant(changes):
        lines.append("No discovery-relevant changes.")
        return "\n".join(lines) + "\n"
    homes = {
        "windows": r"C:\Users\User",
        "macos": "/Users/user",
        "linux": "/home/user",
    }
    for change in changes["clients"]:
        identifier = change["id"]
        mapping = mapped.get(identifier, {})
        adapter = mapping.get("localAdapter") or "(no OriginLM adapter)"
        lines.append(f"Provider: {identifier}")
        lines.append(f"Type: {change['change']}")
        if change["change"] == "modified":
            lines.append("Previous:")
            lines.append(
                f"  {describe_root(change['previous']['root'])} / {change['previous']['relative']}"
            )
            lines.append("Current:")
            lines.append(
                f"  {describe_root(change['current']['root'])} / {change['current']['relative']}"
            )
            for platform, home in homes.items():
                lines.append(
                    f"  {platform}: {resolve_client(change['previous'], home, platform)} -> {resolve_client(change['current'], home, platform)}"
                )
        elif change["change"] == "added":
            current = change["current"]
            lines.append("Current:")
            lines.append(f"  {describe_root(current['root'])} / {current['relative']}")
            for platform, home in homes.items():
                lines.append(f"  {platform}: {resolve_client(current, home, platform)}")
        else:
            previous = change["previous"]
            lines.append("Previous:")
            lines.append(f"  {describe_root(previous['root'])} / {previous['relative']}")
        lines.append(f"Affected OriginLM adapter: {adapter}")
        if mapping.get("parity"):
            lines.append(f"Mapping parity: {mapping['parity']}")
        else:
            lines.append("Mapping parity: NEEDS REVIEW (unmapped upstream client)")
        if mapping.get("note"):
            lines.append(f"Note: {mapping['note']}")
        lines.append("")
    if changes["envAdded"] or changes["envRemoved"]:
        lines.append("Environment variables:")
        if changes["envAdded"]:
            lines.append("  added: " + ", ".join(changes["envAdded"]))
        if changes["envRemoved"]:
            lines.append("  removed: " + ", ".join(changes["envRemoved"]))
        lines.append("")
    if changes["pathsAdded"] or changes["pathsRemoved"]:
        lines.append("Path literals:")
        for label, values in (
            ("added", changes["pathsAdded"][:40]),
            ("removed", changes["pathsRemoved"][:40]),
        ):
            extra = ""
            full = changes["pathsAdded" if label == "added" else "pathsRemoved"]
            if len(full) > 40:
                extra = f" ({len(full) - 40} more)"
            if values:
                lines.append(f"  {label}: " + ", ".join(values) + extra)
        lines.append("")
    lines.append("Parity check:")
    lines.append(parity_text.strip() or "(not run)")
    lines.append("")
    lines.append(
        "This report does not change OriginLM discovery. Review the diff, update the pin and adapters deliberately, and do not merge it automatically."
    )
    return "\n".join(lines).rstrip() + "\n"


def github_json(path: str) -> dict:
    request = urllib.request.Request(
        API + path,
        headers={
            "Accept": "application/vnd.github+json",
            "User-Agent": "originlm-upstream-sync",
            **(
                {"Authorization": f"Bearer {token}"}
                if (token := os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN"))
                else {}
            ),
        },
    )
    with urllib.request.urlopen(request, timeout=60) as response:
        return json.loads(response.read().decode("utf-8"))


def tip_commit() -> tuple[str, str]:
    payload = github_json("/commits/main")
    commit = payload["sha"]
    date = payload["commit"]["committer"]["date"]
    return commit, date


def latest_release() -> tuple[str, str]:
    payload = github_json("/releases/latest")
    tag = payload.get("tag_name", "")
    commit = github_json(f"/commits/{tag}")["sha"]
    return tag, commit


def run(command: list[str], cwd: Path | None = None) -> subprocess.CompletedProcess[str]:
    completed = subprocess.run(
        command,
        cwd=cwd,
        check=False,
        text=True,
        capture_output=True,
    )
    if completed.returncode:
        detail = redact((completed.stderr or completed.stdout or "").strip())
        raise RuntimeError(
            f"command failed ({completed.returncode}): {command[0]}\n{detail[:4000]}"
        )
    return completed


def run_gh(command: list[str]) -> subprocess.CompletedProcess[str]:
    completed = subprocess.run(
        command,
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    if completed.returncode:
        detail = redact((completed.stderr or completed.stdout or "").strip())
        if "not permitted to create or approve pull requests" in detail.lower():
            raise RuntimeError(
                "GitHub Actions is not permitted to create pull requests. "
                "The review branch and report were still produced. Minimum fix: "
                "Settings > Actions > General > Workflow permissions > "
                "'Allow GitHub Actions to create and approve pull requests'. "
                f"gh said: {detail[:2000]}"
            )
        raise RuntimeError(f"gh command failed ({completed.returncode}):\n{detail[:4000]}")
    return completed


def materialize(commit: str, dest: Path) -> None:
    if dest.exists():
        shutil.rmtree(dest)
    dest.parent.mkdir(parents=True, exist_ok=True)
    token = os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN")
    url = f"https://x-access-token:{token}@github.com/{REPOSITORY}.git" if token else GIT_URL
    run(["git", "init", str(dest)])
    run(["git", "remote", "add", "origin", url], dest)
    run(["git", "sparse-checkout", "init", "--cone"], dest)
    run(["git", "sparse-checkout", "set", "crates/tokscale-core/src"], dest)
    run(["git", "fetch", "--depth", "1", "origin", commit], dest)
    run(["git", "checkout", "--detach", "FETCH_HEAD"], dest)


def source_commit(path: Path) -> str:
    completed = run(["git", "rev-parse", "HEAD"], path)
    return completed.stdout.strip()


def set_output(key: str, value: str) -> None:
    path = os.environ.get("GITHUB_OUTPUT")
    if not path:
        return
    with open(path, "a", encoding="utf-8") as handle:
        handle.write(f"{key}={value}\n")


def append_summary(text: str) -> None:
    path = os.environ.get("GITHUB_STEP_SUMMARY")
    if not path:
        return
    with open(path, "a", encoding="utf-8") as handle:
        handle.write(text)
        if not text.endswith("\n"):
            handle.write("\n")


def run_parity(snapshot: Path) -> tuple[str, str]:
    env = os.environ.copy()
    env["LAI_UPSTREAM_ORACLE"] = str(snapshot)
    completed = subprocess.run(
        [
            "cargo",
            "test",
            "-p",
            "originlm-discovery",
            "--test",
            "upstream_parity",
            "--",
            "--nocapture",
        ],
        cwd=ROOT,
        text=True,
        capture_output=True,
        env=env,
    )
    output = (completed.stdout + "\n" + completed.stderr).strip()
    if completed.returncode == 0:
        return "PASS", output
    if "DISCOVERY PARITY FAILURE" in output:
        return "FAIL", output
    return "NEEDS REVIEW", output


def write_local_report(report: str, metadata: dict | None = None) -> None:
    review = TOKSCALE_DIR / "reviews" / "latest"
    review.mkdir(parents=True, exist_ok=True)
    (review / "report.md").write_text(report, encoding="utf-8")
    (review / "report.json").write_text(
        json.dumps({"report": report, **(metadata or {})}, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def open_review_pr(report: str, current_commit: str, metadata: dict | None = None) -> str:
    branch = "upstream/tokscale-discovery-review"
    run(["git", "fetch", "origin", "main"])
    existing = subprocess.run(
        ["git", "rev-parse", "--verify", f"origin/{branch}"],
        cwd=ROOT,
        text=True,
        capture_output=True,
    )
    if existing.returncode == 0:
        run(["git", "checkout", "-B", branch, f"origin/{branch}"])
        merge = subprocess.run(
            ["git", "merge", "origin/main", "--no-edit"],
            cwd=ROOT,
            text=True,
            capture_output=True,
        )
        if merge.returncode != 0:
            raise RuntimeError(redact(merge.stderr or "could not update the review branch with main"))
    else:
        run(["git", "checkout", "-B", branch, "origin/main"])
    write_local_report(report, metadata)
    run(["git", "add", "--", "upstream/tokscale/reviews/latest"])
    status = subprocess.run(
        ["git", "diff", "--cached", "--quiet"],
        cwd=ROOT,
        check=False,
    )
    if status.returncode != 0:
        message = (
            "chore(upstream): report Tokscale discovery changes\n\n"
            f"Review {current_commit} against the pinned OriginLM discovery pin. "
            "This commit does not change runtime discovery."
        )
        run(["git", "commit", "-m", message])
    run(["git", "push", "-u", "origin", branch])
    title = "Review Tokscale discovery changes"
    body = (
        "## Summary\n\n"
        "- Tokscale `main` has discovery-relevant changes since the OriginLM pin.\n"
        "- Production discovery behavior is unchanged by this pull request.\n"
        "- Do not merge until a maintainer ports any adopted rules and updates the pin.\n\n"
        "## Upstream diff\n\n"
        "```text\n"
        + report[:12000]
        + "\n```\n\n"
        "## Test plan\n\n"
        "- [ ] Confirm the changed clients and path literals are discovery-relevant\n"
        "- [ ] Decide which rules OriginLM should adopt, with provenance\n"
        "- [ ] Update `upstream/tokscale/lock.json` and `snapshot.json` only after the adapters change\n"
        "- [ ] Re-run `cargo test -p originlm-discovery --test upstream_parity`\n"
    )
    listed = run_gh(
        [
            "gh",
            "pr",
            "list",
            "--head",
            branch,
            "--json",
            "number,url",
            "--jq",
            ".[0].url // \"\"",
        ]
    )
    url = listed.stdout.strip()
    if url:
        run_gh(["gh", "pr", "edit", url, "--title", title, "--body", body])
        return url
    created = run_gh(
        ["gh", "pr", "create", "--base", "main", "--head", branch, "--title", title, "--body", body]
    )
    return created.stdout.strip()


def validate_offline() -> int:
    lock = load_json(LOCK_PATH)
    snapshot = load_json(SNAPSHOT_PATH)
    mappings = load_json(MAPPINGS_PATH)
    sources = load_json(SOURCES_PATH)
    errors = []
    if lock.get("commit") != snapshot.get("commit"):
        errors.append("lock.json commit does not match snapshot.json")
    if lock.get("repository") != REPOSITORY:
        errors.append("lock.json repository is not junhoyeo/tokscale")
    ids = {client["id"] for client in snapshot.get("clients", [])}
    if "codex" not in ids or "claude" not in ids or "gemini" not in ids:
        errors.append("snapshot is missing a priority Tokscale client")
    for identifier, mapping in mappings.get("clients", {}).items():
        if identifier not in ids:
            errors.append(f"mapping {identifier} is not in the Tokscale snapshot")
        if mapping.get("parity") not in {"contained", "intentionally-different"}:
            errors.append(f"mapping {identifier} has an unknown parity mode")
    source_ids = [item.get("id") for item in sources.get("sources", [])]
    if "tokscale" not in source_ids:
        errors.append("sources.json does not list tokscale")
    if errors:
        print("\n".join(errors))
        return 1
    print(
        f"Pinned Tokscale {lock['commit']} matches the reviewed discovery snapshot. "
        "No upstream fetch was performed."
    )
    return 0


def emit_snapshot(source: Path, destination: Path) -> None:
    commit = source_commit(source)
    payload = extract(source, commit)
    payload["extractedAt"] = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
    print(f"Wrote {destination} from {commit} ({len(payload['clients'])} clients)")


def compare_commits(previous: dict, current: dict, mappings: dict, open_pr: bool, report_only: bool = False) -> int:
    changes = diff_indexes(previous, current)
    set_output("upstream_sha", current["commit"])
    if not diff_is_relevant(changes):
        message = (
            f"Tokscale {previous['commit'][:12]} -> {current['commit'][:12]} "
            "has no discovery-relevant changes."
        )
        print(message)
        append_summary(message + "\n")
        set_output("relevant", "false")
        return 0
    candidate = current
    candidate["extractedAt"] = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    with tempfile.TemporaryDirectory() as temporary:
        snapshot_path = Path(temporary) / "snapshot.json"
        snapshot_path.write_text(json.dumps(candidate), encoding="utf-8")
        parity_status, parity_text = run_parity(snapshot_path)
    report = format_report(
        previous["commit"],
        current["commit"],
        changes,
        mappings,
        parity_text,
        parity_status,
    )
    print(report)
    append_summary("```text\n" + report[:8000] + "\n```\n")
    set_output("relevant", "true")
    set_output("parity", parity_status.replace(" ", "_").lower())
    if report_only:
        write_local_report(
            report,
            {
                "previous": previous["commit"],
                "current": current["commit"],
                "parity": parity_status,
                "generatedAt": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            },
        )
        print("Report-only mode: branch and pull request were not modified.")
        set_output("report_only", "true")
        return 0
    if open_pr:
        try:
            url = open_review_pr(
                report,
                current["commit"],
                {
                    "previous": previous["commit"],
                    "current": current["commit"],
                    "parity": parity_status,
                    "generatedAt": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
                },
            )
        except RuntimeError as error:
            set_output("reporting_failure", str(error).replace("\n", " ")[:1000])
            print(f"UPSTREAM REPORTING FAILURE: {error}", file=sys.stderr)
            return 2
        print(f"Review pull request: {url}")
        set_output("pull_request", url)
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--offline",
        action="store_true",
        help="validate the pin and snapshot without contacting GitHub",
    )
    parser.add_argument("--source-dir", type=Path, help="use a local Tokscale checkout as the tip")
    parser.add_argument("--emit-snapshot", type=Path, help="write a discovery snapshot and exit")
    parser.add_argument(
        "--open-pr",
        action="store_true",
        help="when discovery changes exist, push a review branch and open or update a pull request",
    )
    parser.add_argument(
        "--report-only",
        action="store_true",
        help="produce the report and exit without changing a branch or pull request",
    )
    args = parser.parse_args(argv)
    if args.emit_snapshot:
        if not args.source_dir:
            raise SystemExit("--emit-snapshot requires --source-dir")
        emit_snapshot(args.source_dir, args.emit_snapshot)
        return 0
    if args.offline:
        return validate_offline()
    lock = load_json(LOCK_PATH)
    snapshot = load_json(SNAPSHOT_PATH)
    mappings = load_json(MAPPINGS_PATH)
    if snapshot.get("commit") != lock.get("commit"):
        raise SystemExit("snapshot.json is not the locked Tokscale commit")
    if args.source_dir:
        current = extract(args.source_dir, source_commit(args.source_dir))
    else:
        commit, _date = tip_commit()
        if commit == lock["commit"]:
            message = f"Tokscale main matches pinned revision {commit}."
            print(message)
            append_summary(message + "\n")
            set_output("relevant", "false")
            return 0
        with tempfile.TemporaryDirectory() as temporary:
            dest = Path(temporary) / "tokscale"
            materialize(commit, dest)
            current = extract(dest, commit)
    return compare_commits(snapshot, current, mappings, args.open_pr and not args.report_only, args.report_only)


def redact(text: str) -> str:
    secret = os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN") or ""
    if secret and text:
        return text.replace(secret, "***")
    return text


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (RuntimeError, subprocess.SubprocessError, OSError) as error:
        sys.stderr.write("UPSTREAM WATCH FAILURE: " + redact(str(error)) + "\n")
        raise SystemExit(1) from error
