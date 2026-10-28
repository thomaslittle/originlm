#!/usr/bin/env python3
"""Compare T3 Code's provider surface with the pinned OriginLM revision.

Released builds do not run this script. It diffs the paths recorded in
upstream/t3code/snapshot.json: local model providers, cloud CLI providers,
installed CLI detection, and the ChatGPT OAuth sign-in flow. A difference
can open a review pull request. It never changes adapter behavior.
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
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SNAPSHOT_PATH = ROOT / "upstream" / "t3code" / "snapshot.json"
STATE_PATH = ROOT / "upstream" / "t3code" / "BASELINE.md"
SOURCES_PATH = ROOT / "upstream" / "sources.json"
REPOSITORY = "pingdotgg/t3code"
GIT_URL = f"https://github.com/{REPOSITORY}.git"
API = f"https://api.github.com/repos/{REPOSITORY}"
NIGHTLY_TAG = re.compile(r"^v(\d+)\.(\d+)\.(\d+)-nightly\.(\d{8})\.(\d+)$")

# Every daily pass must still be able to see these areas. The snapshot may
# list more paths; it may not drop one of these.
REQUIRED_PATHS = {
    "local-and-cloud-catalog": "apps/server/src/provider/builtInDrivers.ts",
    "cloud-relay": "apps/server/src/cloud",
    "cli-codex": "apps/server/src/provider/CodexInstallation.ts",
    "cli-claude-home": "apps/server/src/provider/Drivers/ClaudeHome.ts",
    "cli-claude-executable": "apps/server/src/provider/Drivers/ClaudeExecutable.ts",
    "cli-opencode": "packages/provider-opencode/src/server/driver.ts",
    "cli-pi": "packages/provider-pi/src/server/usage.ts",
    "cli-grok": "packages/provider-grok/src/server",
    "cli-cursor": "packages/provider-cursor/src/settings.ts",
    "cli-acp": "packages/provider-acp-registry/src/server",
    "chatgpt-oauth": "apps/server/src/provider/CodexChatGptAuth.ts",
    "chatgpt-handoff": "apps/server/src/provider/CodexChatGptHandoff.ts",
    "shell-shared": "packages/shared/src/shell.ts",
    "shell-desktop": "apps/desktop/src/shell/DesktopShellEnvironment.ts",
}
REQUIRED_AREAS = {
    "local-model-runtimes",
    "cloud-llm-apis",
    "chatgpt-multi-account",
    "acp-registry-install",
    "codex-managed-download",
    "gemini-cli",
    "claude-windows-shim",
}
STATE_MARKERS = (
    "Local model providers",
    "Cloud model providers",
    "Installed CLI and tool detection",
    "ChatGPT OAuth",
)


def load_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def snapshot_commit(snapshot: dict) -> str:
    return snapshot["source"]["commit"]


def watched_paths(snapshot: dict) -> list[str]:
    paths = snapshot.get("watchedPaths") or []
    if not isinstance(paths, list) or not all(isinstance(item, str) and item for item in paths):
        raise SystemExit("snapshot watchedPaths must be a list of relative paths")
    return paths


def validate_offline() -> int:
    snapshot = load_json(SNAPSHOT_PATH)
    sources = load_json(SOURCES_PATH)
    state = STATE_PATH.read_text(encoding="utf-8")
    errors: list[str] = []
    source = snapshot.get("source") or {}
    commit = source.get("commit", "")
    if source.get("repository") != REPOSITORY:
        errors.append("snapshot repository is not pingdotgg/t3code")
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        errors.append("snapshot commit is not a full SHA")
    if commit not in state:
        errors.append("upstream/t3code/BASELINE.md does not record the snapshot commit")
    nightly = source.get("nightlyCommit", "")
    if nightly and nightly not in state:
        errors.append("upstream/t3code/BASELINE.md does not record the nightly commit")
    for marker in STATE_MARKERS:
        if marker not in state:
            errors.append(f"upstream/t3code/BASELINE.md is missing the {marker} section")
    paths = set(watched_paths(snapshot))
    for area, path in REQUIRED_PATHS.items():
        if path not in paths:
            errors.append(f"watched paths dropped {area}: {path}")
    areas = {item.get("area") for item in snapshot.get("reviewedNotAdopted", [])}
    missing = REQUIRED_AREAS - areas
    if missing:
        errors.append("reviewedNotAdopted is missing " + ", ".join(sorted(missing)))
    source_ids = [item.get("id") for item in sources.get("sources", [])]
    if "t3code" not in source_ids:
        errors.append("sources.json does not list t3code")
    if errors:
        print("\n".join(errors))
        return 1
    print(
        f"Pinned T3 Code {commit} records local model providers, cloud CLI "
        "providers, installed CLI detection, and ChatGPT OAuth. No upstream "
        "fetch was performed."
    )
    return 0


def nightly_sort_key(tag: str) -> tuple[int, ...] | None:
    match = NIGHTLY_TAG.fullmatch(tag)
    if not match:
        return None
    major, minor, patch, day, sequence = (int(part) for part in match.groups())
    return (day, sequence, major, minor, patch)


def newest_nightly(tags: list[str]) -> str | None:
    ranked = [(nightly_sort_key(tag), tag) for tag in tags]
    ranked = [(key, tag) for key, tag in ranked if key is not None]
    if not ranked:
        return None
    return max(ranked)[1]


def diff_is_relevant(text: str) -> bool:
    return bool(text.strip())


PROVIDER_PREFIXES = (
    "apps/server/src/provider/",
    "packages/provider-",
    "packages/effect-acp/src/",
    "packages/shared/src/shell.ts",
    "apps/desktop/src/shell/DesktopShellEnvironment.ts",
)
ORCHESTRATION_PREFIXES = (
    "apps/server/src/cloud/",
    ".github/workflows/",
    "docs/operations/release.md",
)
RELEVANT_NAME_MARKERS = (
    "provider", "driver", "installation", "auth", "login", "usage", "quota",
    "model", "catalog", "settings", "detect", "environment", "shell", "acp",
    "codex", "claude", "grok", "muse", "opencode", "cursor", "antigravity", "pi", "fx",
)


def classify_path(path: str) -> str:
    """Separate provider facts from release/orchestration noise."""
    if path.startswith(ORCHESTRATION_PREFIXES):
        return "orchestration"
    if path.startswith(PROVIDER_PREFIXES):
        return "provider"
    if path.startswith(("docs/",)):
        return "documentation"
    return "other"


def classify_diff_files(text: str) -> dict[str, list[str]]:
    groups = {"provider": [], "orchestration": [], "documentation": [], "other": []}
    for line in text.splitlines():
        if not line.startswith("diff --git "):
            continue
        parts = line.split()
        if len(parts) < 4:
            continue
        path = parts[3].removeprefix("b/")
        groups[classify_path(path)].append(path)
    return {name: sorted(set(paths)) for name, paths in groups.items()}


def diff_actions(text: str) -> list[str]:
    actions = []
    for line in text.splitlines():
        if line.startswith(("new file mode", "deleted file mode", "rename from", "rename to")):
            actions.append(line)
    return actions


def coverage_candidates(
    repo: Path, base: str, tip: str, watched: list[str]
) -> list[str]:
    """Changed provider-shaped files that are not in the watched set.

    A new module must be visible even before a maintainer adds it to the
    snapshot. This is a review prompt, not an automatic adapter change.
    """
    completed = subprocess.run(
        [
            "git",
            "diff",
            "--name-status",
            f"{base}..{tip}",
            "--",
            "apps/server/src/provider",
            "apps/desktop/src/shell",
            "packages",
        ],
        cwd=repo,
        text=True,
        capture_output=True,
    )
    if completed.returncode:
        return [f"coverage scan failed: {redact(completed.stderr.strip())[:500]}"]
    watched_set = set(watched)
    candidates = []
    for line in completed.stdout.splitlines():
        parts = line.split("\t")
        if len(parts) < 2:
            continue
        path = parts[-1]
        if path in watched_set or any(path.startswith(item.rstrip("/") + "/") for item in watched if item.endswith("/")):
            continue
        lowered = path.lower()
        if any(marker in lowered for marker in RELEVANT_NAME_MARKERS):
            candidates.append(f"{parts[0]} {path}")
    return sorted(set(candidates))[:40]


def missing_watched(repo: Path, watched: list[str]) -> list[str]:
    return [path for path in watched if not (repo / path).exists()]


def format_report(
    pinned: str,
    main_commit: str,
    main_diff: str,
    nightly_tag: str,
    nightly_base: str,
    nightly_commit: str,
    nightly_diff: str,
    main_coverage: dict | None = None,
    nightly_coverage: dict | None = None,
    missing: list[str] | None = None,
) -> str:
    def section(title: str, body: str) -> str:
        if not diff_is_relevant(body):
            return f"## {title}\n\nNo watched-path changes.\n"
        clipped = body if len(body) <= 12000 else body[:12000] + "\n… diff truncated …\n"
        return f"## {title}\n\n```diff\n{clipped.rstrip()}\n```\n"

    def grouped(label: str, diff: str, coverage: dict | None) -> str:
        groups = classify_diff_files(diff)
        lines = [f"### {label}", ""]
        lines.append("Provider-relevant files: " + (", ".join(groups["provider"]) or "(none)"))
        lines.append("Orchestration/release files: " + (", ".join(groups["orchestration"]) or "(none)"))
        lines.append("Documentation files: " + (", ".join(groups["documentation"]) or "(none)"))
        actions = diff_actions(diff)
        if actions:
            lines.append("Structural changes: " + "; ".join(actions[:20]))
        candidates = (coverage or {}).get("candidates", [])
        if candidates:
            lines.append("Unwatched provider-shaped candidates (review): " + ", ".join(candidates))
        gaps = (coverage or {}).get("missing", [])
        if gaps:
            lines.append("COVERAGE LOSS: watched paths missing at the tip: " + ", ".join(gaps))
        failures = (coverage or {}).get("failure")
        if failures:
            lines.append("Coverage scan failure: " + failures)
        lines.append("")
        return "\n".join(lines)

    return (
        f"# T3 Code provider review\n\n"
        f"Pinned commit: `{pinned}`\n\n"
        f"Compared `main` `{main_commit}` and nightly `{nightly_tag}` "
        f"(`{nightly_commit}`) against nightly pin `{nightly_base}`.\n\n"
        "Watched areas: local model providers, cloud CLI providers, installed "
        "CLI detection, ChatGPT OAuth, and desktop/shared shell resolution. "
        "This report does not change OriginLM behavior.\n\n"
        + grouped("Change classification · main", main_diff, main_coverage)
        + "\n"
        + grouped("Change classification · nightly", nightly_diff, nightly_coverage)
        + "\n"
        + section(f"main ({pinned[:12]}..{main_commit[:12]})", main_diff)
        + "\n"
        + section(f"nightly ({nightly_tag})", nightly_diff)
        + "\nDo not move the pin until a maintainer ports a fact and updates "
        "the snapshot in the same change.\n"
        + (
            "\nCOVERAGE LOSS: " + ", ".join(missing) + "\n"
            if missing
            else ""
        )
    )


def github_json(path: str):
    import urllib.request

    request = urllib.request.Request(
        API + path,
        headers={
            "Accept": "application/vnd.github+json",
            "User-Agent": "originlm-upstream-t3code",
            **(
                {"Authorization": f"Bearer {token}"}
                if (token := os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN"))
                else {}
            ),
        },
    )
    with urllib.request.urlopen(request, timeout=60) as response:
        return json.loads(response.read().decode("utf-8"))


def tip_commit() -> str:
    return github_json("/commits/main")["sha"]


def list_nightly_tags() -> list[str]:
    # Tags come from git so a paged releases response cannot hide a nightly.
    completed = run(["git", "ls-remote", "--tags", GIT_URL])
    tags = []
    for line in completed.stdout.splitlines():
        _sha, ref = line.split("\t", 1)
        name = ref.removeprefix("refs/tags/")
        if name.endswith("^{}"):
            continue
        if NIGHTLY_TAG.fullmatch(name):
            tags.append(name)
    return tags


def commit_for_tag(tag: str) -> str:
    completed = run(["git", "ls-remote", GIT_URL, f"refs/tags/{tag}"])
    lines = [line for line in completed.stdout.splitlines() if line.strip()]
    if not lines:
        raise SystemExit(f"nightly tag {tag} was not found")
    # An annotated tag lists the tag object, then the peeled commit as tag^{}.
    peeled = [line for line in lines if line.endswith("^{}")]
    chosen = peeled[-1] if peeled else lines[-1]
    return chosen.split("\t", 1)[0]


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


def materialize(commit: str, dest: Path, paths: list[str]) -> None:
    if dest.exists():
        shutil.rmtree(dest)
    dest.mkdir(parents=True)
    token = os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN")
    url = f"https://x-access-token:{token}@github.com/{REPOSITORY}.git" if token else GIT_URL
    run(["git", "init", str(dest)])
    run(["git", "remote", "add", "origin", url], dest)
    run(["git", "sparse-checkout", "init", "--cone"], dest)
    run(["git", "sparse-checkout", "set", *dict.fromkeys(paths)], dest)
    run(["git", "fetch", "--filter=blob:none", "--depth", "1", "origin", commit], dest)
    run(["git", "checkout", "--detach", "FETCH_HEAD"], dest)


def diff_commits(repo: Path, base: str, tip: str, paths: list[str]) -> str:
    run(["git", "fetch", "--filter=blob:none", "origin", base, tip], repo)
    completed = subprocess.run(
        ["git", "diff", "--stat", "--patch", f"{base}..{tip}", "--", *paths],
        cwd=repo,
        check=False,
        text=True,
        capture_output=True,
    )
    if completed.returncode:
        raise RuntimeError(redact(completed.stderr or "diff failed"))
    return completed.stdout


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


def redact(text: str) -> str:
    secret = os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN") or ""
    if secret and text:
        return text.replace(secret, "***")
    return text


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


def write_local_report(report: str, metadata: dict | None = None) -> None:
    review = ROOT / "upstream" / "t3code" / "reviews" / "latest"
    review.mkdir(parents=True, exist_ok=True)
    (review / "report.md").write_text(report, encoding="utf-8")
    (review / "report.json").write_text(
        json.dumps({"report": report, **(metadata or {})}, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def open_review_pr(report: str, main_commit: str, metadata: dict | None = None) -> str:
    branch = "upstream/t3code-provider-review"
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
    run(["git", "add", "--", "upstream/t3code/reviews/latest"])
    status = subprocess.run(["git", "diff", "--cached", "--quiet"], cwd=ROOT, check=False)
    if status.returncode != 0:
        message = (
            "chore(upstream): report T3 Code provider changes\n\n"
            f"Review {main_commit} against the pinned provider surface. "
            "This commit does not change runtime discovery or ChatGPT auth."
        )
        run(["git", "commit", "-m", message])
    run(["git", "push", "-u", "origin", branch])
    title = "Review T3 Code provider changes"
    body = (
        "## Summary\n\n"
        "- T3 Code `main` or a newer nightly changed a provider-relevant watched path, or coverage was lost.\n"
        "- The watched surface includes local model providers, cloud CLI providers, "
        "installed CLI detection, ChatGPT OAuth, and desktop/shared shell resolution.\n"
        "- Production behavior is unchanged by this pull request.\n\n"
        "## Upstream diff\n\n"
        "```text\n"
        + report[:12000]
        + "\n```\n\n"
        "## Test plan\n\n"
        "- [ ] Confirm the diff is provider-relevant; orchestration-only changes do not open this PR\n"
        "- [ ] Decide which facts to adopt without copying T3 Code source\n"
        "- [ ] Update `upstream/t3code/snapshot.json` and `upstream/t3code/BASELINE.md` only with the adopted pin\n"
        "- [ ] Re-run `python scripts/upstream_t3code.py --offline`\n"
    )
    listed = run_gh(
        ["gh", "pr", "list", "--head", branch, "--json", "number,url", "--jq", ".[0].url // \"\""]
    )
    url = listed.stdout.strip()
    if url:
        run_gh(["gh", "pr", "edit", url, "--title", title, "--body", body])
        return url
    created = run_gh(
        ["gh", "pr", "create", "--base", "main", "--head", branch, "--title", title, "--body", body]
    )
    return created.stdout.strip()


def compare(open_pr: bool, report_only: bool = False) -> int:
    snapshot = load_json(SNAPSHOT_PATH)
    pinned = snapshot_commit(snapshot)
    nightly_base = snapshot["source"]["nightlyCommit"]
    paths = watched_paths(snapshot)
    main_commit = tip_commit()
    tag = newest_nightly(list_nightly_tags())
    if tag is None:
        raise SystemExit("T3 Code has no nightly tags")
    nightly_commit = commit_for_tag(tag)
    set_output("upstream_sha", main_commit)
    set_output("nightly_tag", tag)
    needs_main = main_commit != pinned
    needs_nightly = nightly_commit != nightly_base
    if not needs_main and not needs_nightly:
        message = f"T3 Code main and nightly {tag} match the pin."
        print(message)
        append_summary(message + "\n")
        set_output("relevant", "false")
        return 0
    with tempfile.TemporaryDirectory() as temporary:
        dest = Path(temporary) / "t3code"
        materialize(main_commit, dest, paths)
        main_diff = diff_commits(dest, pinned, main_commit, paths) if needs_main else ""
        nightly_diff = (
            diff_commits(dest, nightly_base, nightly_commit, paths) if needs_nightly else ""
        )
        main_missing = missing_watched(dest, paths)
        main_candidates = coverage_candidates(dest, pinned, main_commit, paths) if needs_main else []
        nightly_candidates = (
            coverage_candidates(dest, nightly_base, nightly_commit, paths) if needs_nightly else []
        )
    main_coverage = {"candidates": main_candidates, "missing": main_missing}
    nightly_coverage = {"candidates": nightly_candidates, "missing": []}
    groups = classify_diff_files(main_diff + "\n" + nightly_diff)
    provider_relevant = bool(groups["provider"])
    coverage_gap = bool(main_missing) or any(
        candidate and not candidate.startswith("coverage scan failed")
        for candidate in main_candidates + nightly_candidates
    )
    if not provider_relevant and not coverage_gap:
        detail = (
            "orchestration/release changes were present but were filtered"
            if groups["orchestration"]
            else "no watched-path changes"
        )
        message = (
            f"T3 Code {pinned[:12]}..{main_commit[:12]} and nightly {tag}: "
            f"no provider-relevant changes ({detail})."
        )
        print(message)
        append_summary(message + "\n")
        set_output("relevant", "false")
        set_output("filtered_non_provider", "true" if groups["orchestration"] else "false")
        return 0
    report = format_report(
        pinned,
        main_commit,
        main_diff,
        tag,
        nightly_base,
        nightly_commit,
        nightly_diff,
        main_coverage,
        nightly_coverage,
        main_missing,
    )
    print(report)
    append_summary("```text\n" + report[:8000] + "\n```\n")
    set_output("relevant", "true")
    metadata = {
        "pinned": pinned,
        "main": main_commit,
        "nightlyTag": tag,
        "nightly": nightly_commit,
        "providerRelevant": provider_relevant,
        "coverageGap": coverage_gap,
        "missingWatchedPaths": main_missing,
        "coverageCandidates": main_candidates + nightly_candidates,
        "generatedAt": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    }
    if report_only:
        write_local_report(report, metadata)
        print("Report-only mode: branch and pull request were not modified.")
        set_output("report_only", "true")
        return 0
    if open_pr:
        try:
            url = open_review_pr(report, main_commit, metadata)
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
        help="validate the pin, watched areas, and sync state without contacting GitHub",
    )
    parser.add_argument(
        "--open-pr",
        action="store_true",
        help="when provider-relevant changes exist, push a review branch and open or update a pull request",
    )
    parser.add_argument(
        "--report-only",
        action="store_true",
        help="produce the report and exit without changing a branch or pull request",
    )
    args = parser.parse_args(argv)
    if args.offline:
        return validate_offline()
    return compare(args.open_pr, args.report_only)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (RuntimeError, subprocess.SubprocessError, OSError) as error:
        print(f"UPSTREAM WATCH FAILURE: {error}", file=sys.stderr)
        sys.exit(1)
