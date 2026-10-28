import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("upstream_sync", ROOT / "scripts" / "upstream_sync.py")
sync = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(sync)


class UpstreamSyncTests(unittest.TestCase):
    def test_parser_does_not_treat_fallback_relative_as_the_session_path(self):
        text = (ROOT / "scripts" / "fixtures" / "upstream_sync" / "clients.rs").read_text(
            encoding="utf-8"
        )
        clients = {item["id"]: item for item in sync.parse_clients(text)}
        self.assertEqual(clients["codex"]["root"]["var"], "CODEX_HOME")
        self.assertEqual(clients["codex"]["root"]["fallback"], ".codex")
        self.assertEqual(clients["codex"]["relative"], "sessions")
        self.assertEqual(clients["cursor"]["relative"], ".config/tokscale/cursor-cache")

    def test_unchanged_registry_is_not_relevant(self):
        text = (ROOT / "scripts" / "fixtures" / "upstream_sync" / "clients.rs").read_text(
            encoding="utf-8"
        )
        clients = sync.parse_clients(text)
        previous = {"clients": clients, "envVars": ["CODEX_HOME"], "pathLiterals": [".codex"]}
        current = json.loads(json.dumps(previous))
        self.assertFalse(sync.diff_is_relevant(sync.diff_indexes(previous, current)))

    def test_path_change_is_reported_with_native_separators(self):
        previous = {
            "clients": [
                {
                    "id": "codex",
                    "root": {"kind": "env", "var": "CODEX_HOME", "fallback": ".codex"},
                    "relative": "sessions",
                    "pattern": "*.jsonl",
                }
            ],
            "envVars": [],
            "pathLiterals": [],
        }
        current = json.loads(json.dumps(previous))
        current["clients"][0]["relative"] = "session-store"
        changes = sync.diff_indexes(previous, current)
        self.assertTrue(sync.diff_is_relevant(changes))
        report = sync.format_report(
            "abc123",
            "def456",
            changes,
            {"clients": {"codex": {"localAdapter": "codex", "parity": "contained"}}},
            "DISCOVERY PARITY FAILURE\n\nClient: Codex\n",
            "FAIL",
        )
        self.assertIn("abc123 -> def456", report)
        self.assertIn("Affected OriginLM adapter: codex", report)
        self.assertIn(r"C:\Users\User\.codex\sessions", report)
        self.assertIn(r"C:\Users\User\.codex\session-store", report)
        self.assertIn("Local parity: FAIL", report)

    def test_offline_pin_matches_snapshot(self):
        self.assertEqual(sync.validate_offline(), 0)

    def test_emit_round_trip_uses_the_checkout_commit(self):
        with tempfile.TemporaryDirectory() as temporary:
            source = Path(temporary)
            destination = source / "out.json"
            crate = source / "crates" / "tokscale-core" / "src"
            crate.mkdir(parents=True)
            fixture = ROOT / "scripts" / "fixtures" / "upstream_sync" / "clients.rs"
            (crate / "clients.rs").write_text(fixture.read_text(encoding="utf-8"), encoding="utf-8")
            for name in ("paths.rs", "scanner.rs", "cc_mirror.rs"):
                (crate / name).write_text("fn example() {}\n", encoding="utf-8")
            (crate / "sessions").mkdir()
            (crate / "sessions" / "mod.rs").write_text(
                'std::env::var("CODEX_HOME");\nlet _ = ".codex/sessions";\n',
                encoding="utf-8",
            )
            subprocess_dir = source
            import subprocess

            subprocess.run(["git", "init"], cwd=subprocess_dir, check=True, capture_output=True)
            subprocess.run(["git", "add", "."], cwd=subprocess_dir, check=True, capture_output=True)
            subprocess.run(
                [
                    "git",
                    "-c",
                    "user.email=sync@example.com",
                    "-c",
                    "user.name=sync",
                    "commit",
                    "-m",
                    "fixture",
                ],
                cwd=subprocess_dir,
                check=True,
                capture_output=True,
            )
            sync.emit_snapshot(source, destination)
            payload = json.loads(destination.read_text(encoding="utf-8"))
            self.assertEqual(payload["clients"][0]["id"], "codex")
            self.assertIn("CODEX_HOME", payload["envVars"])
            self.assertIn(".codex/sessions", payload["pathLiterals"])


if __name__ == "__main__":
    unittest.main()
