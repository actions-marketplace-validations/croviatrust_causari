#!/usr/bin/env python3
import importlib.util
import json
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "blame_counts.py"
spec = importlib.util.spec_from_file_location("blame_counts", SCRIPT)
mod = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mod)

A = "a" * 40
B = "b" * 40


def record(sha: str, content: str, extra: str = "") -> str:
    return f"{sha} 1 1 1\nauthor x\n{extra}filename f\n\t{content}\n"


class CountOwners(unittest.TestCase):
    def test_counts_one_line_per_record(self):
        porcelain = record(A, "x") + record(B, "y") + record(A, "z")
        self.assertEqual(mod.count_owners(porcelain), {A: 2, B: 1})

    def test_forty_hex_metadata_is_not_a_header(self):
        # A metadata line whose value is 40 hex chars must not be counted.
        porcelain = record(A, "x", extra=f"summary {B}\n")
        self.assertEqual(mod.count_owners(porcelain), {A: 1})

    def test_crlf_in_content_does_not_break_records(self):
        porcelain = record(A, "x\r") + record(A, "y\r")
        self.assertEqual(mod.count_owners(porcelain), {A: 2})

    def test_same_flags_as_the_audit(self):
        self.assertEqual(mod.BLAME_FLAGS, ["-w", "-M", "-C"])


class EndToEnd(unittest.TestCase):
    def test_real_repository_counts_and_errors(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp) / "r"
            repo.mkdir()
            env = {
                "GIT_AUTHOR_NAME": "t",
                "GIT_AUTHOR_EMAIL": "t@x",
                "GIT_COMMITTER_NAME": "t",
                "GIT_COMMITTER_EMAIL": "t@x",
                "GIT_CONFIG_GLOBAL": "/dev/null",
                "GIT_CONFIG_NOSYSTEM": "1",
                "PATH": __import__("os").environ["PATH"],
            }

            def git(*a):
                subprocess.run(["git", *a], cwd=repo, check=True, env=env, capture_output=True)

            git("init", "-q", "-b", "main")
            (repo / "f.txt").write_text("1\n2\n3\n", encoding="utf-8")
            git("add", "f.txt")
            git("commit", "-q", "-m", "one")
            sha = subprocess.run(
                ["git", "rev-parse", "HEAD"], cwd=repo, check=True, env=env, capture_output=True, text=True
            ).stdout.strip()
            out = Path(tmp) / "out.json"
            rc = mod.main(["blame_counts.py", str(repo), str(out)])
            self.assertEqual(rc, 0)
            data = json.loads(out.read_text(encoding="utf-8"))
            self.assertIn("git version", data["git"])
            self.assertEqual(data["files"], {"f.txt": {"lines": {sha: 3}}})


if __name__ == "__main__":
    unittest.main()
