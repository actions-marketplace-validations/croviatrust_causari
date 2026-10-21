#!/usr/bin/env python3
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "release_check.py"
spec = importlib.util.spec_from_file_location("release_check", SCRIPT)
mod = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mod)

FIXTURES = ROOT / "tests" / "fixtures" / "release-check"


def shipped_fixture() -> dict:
    return json.loads((FIXTURES / "OpenHands__extensions.v4.json").read_text(encoding="utf-8"))


def audit_matching(fx: dict) -> dict:
    """An audit JSON that carries exactly the fixture's expected values."""
    audit: dict = {"method": fx["method"]}
    for key, val in fx["expect"].items():
        cur = audit
        parts = key.split(".")
        for p in parts[:-1]:
            cur = cur.setdefault(p, {})
        cur[parts[-1]] = val
    return audit


class ReleaseCheckTests(unittest.TestCase):
    def test_shipped_fixture_is_well_formed(self):
        fx = shipped_fixture()
        self.assertEqual(fx["schema"], "causari.release_check.v1")
        self.assertEqual(len(fx["commit"]), 40)
        self.assertEqual(fx["expect"]["repository.head"], fx["commit"])
        self.assertEqual(fx["expect"]["coverage.shallow"], False)
        self.assertGreater(fx["expect"]["verified.commits"], 0)
        self.assertLessEqual(fx["expect"]["verified.surviving"], fx["expect"]["verified.introduced"])

    def test_matching_audit_passes(self):
        fx = shipped_fixture()
        rows = mod.compare(audit_matching(fx), fx)
        self.assertTrue(all(r[3] for r in rows))
        self.assertEqual(len(rows), len(fx["expect"]))

    def test_one_changed_count_fails_and_is_named(self):
        fx = shipped_fixture()
        audit = audit_matching(fx)
        audit["verified"]["surviving"] -= 1
        rows = mod.compare(audit, fx)
        bad = [r for r in rows if not r[3]]
        self.assertEqual([r[0] for r in bad], ["verified.surviving"])
        text = mod.render(rows, fx, audit, "test")
        self.assertIn("MISMATCH", text)
        self.assertIn("`verified.surviving`", text)

    def test_absent_field_is_not_a_match(self):
        fx = shipped_fixture()
        audit = audit_matching(fx)
        del audit["baseline"]["untagged"]["surviving"]
        rows = mod.compare(audit, fx)
        row = next(r for r in rows if r[0] == "baseline.untagged.surviving")
        self.assertFalse(row[3])
        self.assertIn("absent", mod.render(rows, fx, audit, "test"))

    def test_fixture_lookup_is_by_method_and_commit(self):
        fx = shipped_fixture()
        self.assertIsNotNone(mod.find_fixture(FIXTURES, fx["method"], fx["commit"]))
        self.assertIsNone(mod.find_fixture(FIXTURES, "v99", fx["commit"]))
        self.assertIsNone(mod.find_fixture(FIXTURES, fx["method"], "0" * 40))

    def test_missing_fixture_exits_one(self):
        fx = shipped_fixture()
        audit = audit_matching(fx)
        audit["method"] = "v99"
        with tempfile.TemporaryDirectory() as td:
            p = Path(td) / "audit.json"
            p.write_text(json.dumps(audit), encoding="utf-8")
            self.assertEqual(mod.main(["release_check.py", str(p), str(FIXTURES)]), 1)
            audit["method"] = fx["method"]
            p.write_text(json.dumps(audit), encoding="utf-8")
            self.assertEqual(mod.main(["release_check.py", str(p), str(FIXTURES)]), 0)


if __name__ == "__main__":
    unittest.main()
