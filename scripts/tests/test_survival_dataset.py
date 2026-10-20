"""The Hugging Face export is a faithful, deterministic view of the published reports."""
import hashlib
import json
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

import survival_dataset as sd  # noqa: E402

REPORTS = ROOT / "site" / "reports" / "survival"


def tree_digest(root: Path) -> str:
    h = hashlib.sha256()
    for p in sorted(root.rglob("*")):
        if p.is_file():
            h.update(str(p.relative_to(root)).encode())
            h.update(p.read_bytes())
    return h.hexdigest()


def jsonl(path: Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line]


class BuildTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        cls.out = Path(cls.tmp.name) / "ds"
        cls.manifest = sd.build(REPORTS, cls.out)
        cls.dirs = sd.report_dirs(REPORTS)

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def test_one_row_per_report_and_per_measured_repository(self):
        reports = jsonl(self.out / "data" / "reports.jsonl")
        repos = jsonl(self.out / "data" / "repositories.jsonl")
        self.assertEqual(len(reports), len(self.dirs))
        expected = 0
        for d in self.dirs:
            r = json.loads((d / "report.json").read_text(encoding="utf-8"))
            expected += len(r["repositories"]) + len(r.get("not_aggregated") or [])
        self.assertEqual(len(repos), expected)
        self.assertEqual(self.manifest["rows"]["repositories"], expected)
        numbers = [r["number"] for r in reports]
        self.assertEqual(numbers, sorted(numbers))

    def test_rows_carry_method_version_and_evidence_fields(self):
        for row in jsonl(self.out / "data" / "repositories.jsonl"):
            self.assertRegex(row["method_version"], r"^v\d+$")
            self.assertIn(row["aggregated"], (True, False))
            for key in ("verified_commits", "verified_introduced", "verified_surviving",
                        "untagged_commits", "age_matched_gap", "coverage_shallow", "by_agent_json"):
                self.assertIn(key, row)
            json.loads(row["by_agent_json"])
            self.assertTrue((self.out / row["raw_audit_json"]).is_file(), row["raw_audit_json"])

    def test_not_aggregated_rows_are_marked_not_dropped(self):
        rows = jsonl(self.out / "data" / "repositories.jsonl")
        latest = json.loads((self.dirs[-1] / "report.json").read_text(encoding="utf-8"))
        below = {r["repo"] for r in latest.get("not_aggregated") or []}
        if not below:
            self.skipTest("latest report has no below-floor rows")
        marked = {r["repo"] for r in rows if r["report_id"] == latest["id"] and not r["aggregated"]}
        self.assertEqual(marked, below)

    def test_raw_bytes_are_identical_to_the_published_site(self):
        for d in self.dirs:
            rid = json.loads((d / "report.json").read_text(encoding="utf-8"))["id"]
            self.assertEqual((d / "report.json").read_bytes(), (self.out / "raw" / rid / "report.json").read_bytes())
            for src in (d / "repos").glob("*.json"):
                self.assertEqual(src.read_bytes(), (self.out / "raw" / rid / "repos" / src.name).read_bytes())

    def test_build_is_deterministic(self):
        with tempfile.TemporaryDirectory() as other:
            sd.build(REPORTS, Path(other) / "ds")
            self.assertEqual(tree_digest(self.out), tree_digest(Path(other) / "ds"))

    def test_card_states_the_limits_and_the_viewer_configs(self):
        card = (self.out / "README.md").read_text(encoding="utf-8")
        self.assertTrue(card.startswith("---\nlicense: cc-by-4.0\n"))
        for phrase in (
            "Counts, not grades",
            "AI-tagged means the metadata matched",
            "UNKNOWN is not human",
            "not a reliability guarantee",
            "causari.dev/method",
            "data/repositories.jsonl",
            "config_name: repositories",
        ):
            self.assertIn(phrase, card)
        for banned in ("AI-written code survives", "healthy", "proves that", "{reports}", "{latest_"):
            self.assertNotIn(banned, card)
        latest = json.loads((self.dirs[-1] / "report.json").read_text(encoding="utf-8"))
        self.assertIn(latest["title"], card)
        if latest.get("doi"):
            self.assertIn(latest["doi"], card)

    def test_push_dry_run_names_the_latest_report(self):
        import contextlib
        import io

        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            sd.push(self.out, "CroviaResearch/causari-survival", dry_run=True)
        payload = json.loads(buf.getvalue())
        self.assertEqual(payload["repo"], "CroviaResearch/causari-survival")
        self.assertEqual(payload["latest"], self.manifest["latest"])
        self.assertIn("method v", payload["commit_message"])


if __name__ == "__main__":
    unittest.main()
