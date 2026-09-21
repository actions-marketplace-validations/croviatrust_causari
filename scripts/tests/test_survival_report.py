#!/usr/bin/env python3
"""Tests for scripts/survival_report.py and scripts/zenodo_deposit.py.

Standard library only (unittest); pytest runs them too:

    python3 -m unittest scripts/tests/test_survival_report.py
    python3 -m pytest scripts/tests -q

Every test builds a report from a synthetic run directory into a scratch
site, so nothing under site/ is touched.
"""

from __future__ import annotations

import json
import re
import sys
import tempfile
import unittest
import xml.etree.ElementTree as ET
from html.parser import HTMLParser
from pathlib import Path

HERE = Path(__file__).resolve().parent
SCRIPTS = HERE.parent
ROOT = SCRIPTS.parent
sys.path.insert(0, str(SCRIPTS))

import survival_report as sr  # noqa: E402
import zenodo_deposit as zd  # noqa: E402

CANON = json.loads((ROOT / "canon" / "canon.json").read_text(encoding="utf-8"))
FORBIDDEN = CANON["forbidden_words"]["words"]


def stat(commits: int, introduced: int, surviving: int) -> dict:
    rate = surviving / introduced if introduced else None
    return {
        "commits": commits, "introduced": introduced, "surviving": surviving, "survival_rate": rate,
        "median_survival": rate, "capped_survival_rate": rate, "cap_lines": 100 if introduced else None,
        "largest_commit_share": 0.3 if introduced else None,
    }


def audit(commits: int, introduced: int, surviving: int, agent: str = "claude-code", shallow: bool = False) -> dict:
    return {
        "method": "v2", "total_commits": commits * 10,
        "verified": stat(commits, introduced, surviving), "probable": stat(0, 0, 0),
        "by_agent": {agent: stat(commits, introduced, surviving)},
        "coverage": {"method": "v2", "blame_flags": ["-w", "-M", "-C"], "ignore_revs_file": False,
                     "shallow": shallow, "sample_floor": 5, "small_sample": commits < 5},
    }


REPOS = {
    "zeta/last": audit(20, 1000, 600),
    "Alpha/first": audit(10, 500, 400, agent="cursor"),
    "mid/one": audit(12, 800, 200, agent="aider"),
    "mid/small": audit(3, 50, 10, agent="aider"),
    "mid/shallow": audit(30, 900, 100, shallow=True),
    "opt/out": audit(9, 100, 50, agent="devin"),
}


class Scratch:
    """A run directory, an empty site and a repo root with an opt-out file."""

    def __init__(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        base = Path(self.tmp.name)
        self.run = base / "run"
        self.site = base / "site"
        self.root = base / "root"
        self.run.mkdir()
        self.site.mkdir()
        (self.root / ".github").mkdir(parents=True)
        for repo, a in REPOS.items():
            (self.run / f"{sr.repo_slug(repo)}.json").write_text(json.dumps(a), encoding="utf-8")
        (self.run / "run.json").write_text(json.dumps({
            "generated_at": "2026-09-21T05:17:00Z", "tool": "causari", "tool_version": "0.1.5", "method": "v2",
            "command": "re audit <owner/repo> --json", "repos": list(REPOS), "failed": ["gone/repo"], "opted_out": ["skipped/early"],
        }), encoding="utf-8")
        (self.root / ".github" / "survival-optout.txt").write_text("# comment\n\nOPT/out\n", encoding="utf-8")
        # the static parts of the real site the generator amends
        (self.site / "_redirects").write_text("/github https://github.com/croviatrust/causari 302\n", encoding="utf-8")
        (self.site / "sitemap.xml").write_text(
            '<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n'
            "  <url><loc>https://causari.dev/</loc></url>\n</urlset>\n", encoding="utf-8")

    def build(self, number: int = 1, date: str = "2026-09-21") -> dict:
        rc = sr.main(["--site", str(self.site), "--root", str(self.root), "build", "--run", str(self.run),
                      "--number", str(number), "--date", date, "--no-png"])
        assert rc == 0
        return json.loads((self.site / "reports" / "survival" / f"{date[:4]}" / f"{number:02d}" / "report.json").read_text(encoding="utf-8"))

    def close(self) -> None:
        self.tmp.cleanup()


class TextOnly(HTMLParser):
    def __init__(self) -> None:
        super().__init__()
        self.parts: list[str] = []

    def handle_data(self, data: str) -> None:
        self.parts.append(data)


def visible_text(html: str) -> str:
    p = TextOnly()
    p.feed(html)
    return "".join(p.parts)


class BuildTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.s = Scratch()
        cls.f = cls.s.build()
        cls.dir = cls.s.site / "reports" / "survival" / "2026" / "01"
        cls.page = (cls.dir / "index.html").read_text(encoding="utf-8")
        cls.archive = (cls.s.site / "reports" / "survival" / "index.html").read_text(encoding="utf-8")

    @classmethod
    def tearDownClass(cls) -> None:
        cls.s.close()

    def test_alphabetical_order_never_by_rate(self) -> None:
        repos = [r["repo"] for r in self.f["repositories"]]
        self.assertEqual(repos, ["Alpha/first", "mid/one", "zeta/last"])
        self.assertEqual(repos, sorted(repos, key=str.lower))
        rates = [r["verified"]["survival_rate"] for r in self.f["repositories"]]
        self.assertNotEqual(rates, sorted(rates))
        self.assertNotEqual(rates, sorted(rates, reverse=True))
        # and the page shows them in the same order
        positions = [self.page.index(f">{r}</a>") for r in repos]
        self.assertEqual(positions, sorted(positions))

    def test_small_sample_measured_but_not_aggregated(self) -> None:
        self.assertEqual([r["repo"] for r in self.f["not_aggregated"]], ["mid/small"])
        self.assertNotIn("mid/small", [r["repo"] for r in self.f["repositories"]])
        self.assertEqual(self.f["aggregate"]["repositories"], 3)
        self.assertEqual(self.f["aggregate"]["introduced"], 1000 + 500 + 800)
        self.assertEqual(self.f["aggregate"]["surviving"], 600 + 400 + 200)
        self.assertIn("Measured but not aggregated", self.page)

    def test_shallow_excluded_with_note(self) -> None:
        self.assertEqual(self.f["excluded"]["shallow"], ["mid/shallow"])
        for group in ("repositories", "not_aggregated"):
            self.assertNotIn("mid/shallow", [r["repo"] for r in self.f[group]])
        self.assertIn("mid/shallow", self.page)
        self.assertIn("Shallow clones", self.page)

    def test_optout_honoured(self) -> None:
        for group in ("repositories", "not_aggregated"):
            self.assertNotIn("opt/out", [r["repo"] for r in self.f[group]])
        self.assertNotIn("opt/out", self.page)
        self.assertEqual(self.f["excluded"]["opted_out"], 2)  # one from the run dir, one the workflow skipped
        self.assertFalse((self.dir / "repos" / "opt__out.json").exists())

    def test_failed_listed(self) -> None:
        self.assertEqual(self.f["excluded"]["failed"], ["gone/repo"])
        self.assertIn("gone/repo", self.page)

    def test_no_rank_column_no_colour(self) -> None:
        header = re.search(r'<table class="lb-table" id="repos">.*?</thead>', self.page, re.S).group(0)
        self.assertNotIn("Rank", header)
        self.assertNotIn("#</th>", header)
        for token in ("color:", "background:", "🟢", "🟡", "🔴", "class=\"good\"", "class=\"bad\""):
            self.assertNotIn(token, self.page)

    def test_forbidden_words_absent_from_generated_html(self) -> None:
        for html in (self.page, self.archive):
            text = visible_text(html)
            for word in FORBIDDEN:
                for m in re.finditer(re.escape(word), text):
                    before = text[max(0, m.start() - 1):m.start()]
                    self.assertIn(before, ('"', "'", "\u201c", "\u2018", "`", "\u00ab"),
                                  f"forbidden word {word!r} used bare in generated HTML")

    def test_every_number_links_to_reproduction(self) -> None:
        for r in self.f["repositories"] + self.f["not_aggregated"]:
            self.assertEqual(r["reproduce"], f"re audit {r['repo']} --json")
            self.assertTrue((self.dir / r["audit_file"]).exists())
            self.assertIn(f'href="{r["audit_file"]}"', self.page)
            self.assertIn(f"re audit {r['repo']} --json", self.page)

    def test_method_section(self) -> None:
        self.assertIn('href="/method"', self.page)
        self.assertIn("method v2", self.page)
        self.assertIn("causari 0.1.5", self.page)
        self.assertIn("git blame -w -M -C", self.page)
        self.assertIn("95th percentile", self.page)
        self.assertIn("10,000 lines", self.page)
        self.assertIn("counts, not grades", self.page)
        self.assertIn("survival-optout.txt", self.page)

    def test_interval_labelled_as_sample_not_population(self) -> None:
        note = self.f["aggregate"]["interval_method"]["note"]
        self.assertIn("sampled repositories", note)
        self.assertIn("not all AI-assisted code", note)
        self.assertIn("seed 1", note)
        self.assertEqual(self.f["aggregate"]["interval_method"]["resamples"], 2000)

    def test_by_agent_alphabetical_and_summed(self) -> None:
        self.assertEqual(list(self.f["by_agent"]), ["aider", "claude-code", "cursor"])
        self.assertEqual(self.f["by_agent"]["aider"]["introduced"], 800)  # mid/small is not aggregated

    def test_outputs_exist(self) -> None:
        for name in ("index.html", "report.json", "report.md", "card.svg"):
            self.assertTrue((self.dir / name).exists(), name)
        base = self.s.site / "reports" / "survival"
        for name in ("index.html", "feed.xml", "latest.json"):
            self.assertTrue((base / name).exists(), name)
        latest = json.loads((base / "latest.json").read_text(encoding="utf-8"))
        self.assertEqual(latest["id"], "2026/01")
        ET.fromstring((self.dir / "card.svg").read_text(encoding="utf-8"))

    def test_atom_feed_valid(self) -> None:
        ns = {"a": "http://www.w3.org/2005/Atom"}
        root = ET.parse(self.s.site / "reports" / "survival" / "feed.xml").getroot()
        self.assertEqual(root.tag, "{http://www.w3.org/2005/Atom}feed")
        for tag in ("title", "id", "updated"):
            self.assertIsNotNone(root.find(f"a:{tag}", ns), tag)
        self.assertEqual(root.find("a:link[@rel='self']", ns).get("href"), "https://causari.dev/reports/survival/feed.xml")
        entries = root.findall("a:entry", ns)
        self.assertEqual(len(entries), 1)
        e = entries[0]
        self.assertEqual(e.find("a:id", ns).text, "https://causari.dev/reports/survival/2026/01/")
        self.assertTrue(e.find("a:link[@rel='alternate']", ns).get("href").startswith("https://causari.dev/"))
        self.assertIn("Survival Report #1", e.find("a:title", ns).text)

    def test_redirects_and_sitemap(self) -> None:
        redirects = (self.s.site / "_redirects").read_text(encoding="utf-8")
        for pattern in (r"^/survival\s+/reports/survival/\s+301$",
                        r"^/report\s+/reports/survival/2026/01/\s+302$",
                        r"^/survival-data\.json\s+/reports/survival/latest\.json\s+301$"):
            self.assertIsNotNone(re.search(pattern, redirects, re.M), pattern)
        self.assertIn("/github https://github.com/croviatrust/causari 302", redirects)  # untouched
        sitemap = ET.parse(self.s.site / "sitemap.xml").getroot()
        locs = [u.find("{http://www.sitemaps.org/schemas/sitemap/0.9}loc").text for u in sitemap]
        self.assertIn("https://causari.dev/reports/survival/", locs)
        self.assertIn("https://causari.dev/reports/survival/2026/01/", locs)
        self.assertIn("https://causari.dev/", locs)

    def test_rebuild_is_idempotent(self) -> None:
        before = {p.name: p.read_bytes() for p in (self.s.site / "reports" / "survival").iterdir() if p.is_file()}
        sr.rebuild(self.s.site)
        after = {p.name: p.read_bytes() for p in (self.s.site / "reports" / "survival").iterdir() if p.is_file()}
        self.assertEqual(before, after)
        redirects = (self.s.site / "_redirects").read_text(encoding="utf-8")
        self.assertEqual(redirects.count(sr.REDIRECT_BEGIN), 1)


class RepoPageTests(unittest.TestCase):
    """site/r/<owner>/<repo>/: page, badge, latest.json; site/r/index.html."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.s = Scratch()
        cls.f = cls.s.build()
        cls.r = cls.s.site / "r"
        cls.page = (cls.r / "alpha" / "first" / "index.html").read_text(encoding="utf-8")
        cls.index = (cls.r / "index.html").read_text(encoding="utf-8")
        cls.report_page = (cls.s.site / "reports" / "survival" / "2026" / "01" / "index.html").read_text(encoding="utf-8")

    @classmethod
    def tearDownClass(cls) -> None:
        cls.s.close()

    def test_tree_for_every_measured_repository_and_no_other(self) -> None:
        measured = [r["repo"] for r in self.f["repositories"] + self.f["not_aggregated"]]
        self.assertEqual(sorted(measured, key=str.lower), ["Alpha/first", "mid/one", "mid/small", "zeta/last"])
        for repo in measured:
            d = self.r / repo.lower()
            for name in ("index.html", "badge.svg", "badge-dark.svg", "latest.json"):
                self.assertTrue((d / name).exists(), f"{repo}: {name}")
        for absent in ("mid/shallow", "opt/out", "gone/repo"):
            self.assertFalse((self.r / absent).exists(), absent)
        self.assertFalse((self.r / "Alpha").exists(), "paths are lowercase only")

    def test_lowercase_paths_and_cased_redirect(self) -> None:
        self.assertEqual(sr.repo_path("Alpha/first"), "/r/alpha/first/")
        self.assertEqual(sr.repo_path("mid/one"), "/r/mid/one/")
        redirects = (self.s.site / "_redirects").read_text(encoding="utf-8")
        self.assertIsNotNone(re.search(r"^/r/Alpha/first/\s+/r/alpha/first/\s+301$", redirects, re.M))
        self.assertNotIn("/r/mid/one/ ", redirects)  # already lowercase: no redirect needed
        for href in re.findall(r'href="(/r/[^"]+)"', self.report_page + self.index + self.page):
            self.assertEqual(href, href.lower(), href)
        # the report page's repository names link to the repository pages; the numbers still link to the bytes
        self.assertIn('href="/r/alpha/first/"', self.report_page)
        self.assertIn('href="repos/Alpha__first.json"', self.report_page)

    def test_page_with_one_report(self) -> None:
        self.assertIn('<h1><a href="https://github.com/Alpha/first" rel="noopener" translate="no">Alpha/first</a></h1>', self.page)
        self.assertIn("In Survival Report #1 (2026-09-21, method v2): 400 of 500 lines introduced by 10 AI-tagged commits are still at HEAD, 80.0 %.", self.page)
        self.assertIn("re audit Alpha/first --json", self.page)
        self.assertIn('href="/reports/survival/2026/01/repos/Alpha__first.json"', self.page)
        self.assertIn("[![AI code survival](https://causari.dev/r/alpha/first/badge.svg)](https://causari.dev/r/alpha/first/)", self.page)
        self.assertIn('data-copy="badge-md"', self.page)
        self.assertIn("cursor", self.page)  # by agent
        self.assertNotIn('class="spark"', self.page)  # one point is not a line
        self.assertEqual(self.page.count("<tr><td><a href=\"/reports/survival/"), 1)  # one history row
        self.assertIn("A line appears once the repository has been measured in two reports.", self.page)
        self.assertIn('"@type": "Dataset"', self.page)
        self.assertIn('"codeRepository": "https://github.com/Alpha/first"', self.page)
        for m in re.finditer(r"<code\b[^>]*>", self.page):
            self.assertIn('translate="no"', m.group(0), m.group(0))
        for token in ("color:", "background:", "🟢", "🟡", "🔴", "Rank", "rank "):
            self.assertNotIn(token, self.page)
        for other in ("mid/one", "zeta/last", "mid/small"):
            self.assertNotIn(other, self.page)  # no comparison with other repositories
        text = visible_text(self.page)
        for word in FORBIDDEN:
            self.assertNotIn(word, text)

    def test_small_sample_page_publishes_counts_not_ratio(self) -> None:
        page = (self.r / "mid" / "small" / "index.html").read_text(encoding="utf-8")
        self.assertIn("10 of 50 lines introduced by 3 AI-tagged commits are still at HEAD. Fewer than 5 AI-tagged commits", page)
        self.assertNotIn("20.0 %", page)
        self.assertIn('<span class="n">n &lt; 5</span>', page)
        self.assertNotIn("n < 5", page)
        badge = (self.r / "mid" / "small" / "badge.svg").read_text(encoding="utf-8")
        self.assertIn("AI code survival  n &lt; 5", badge)
        self.assertNotIn("20.0", badge)

    def test_badge_text_width_and_colours(self) -> None:
        light = (self.r / "alpha" / "first" / "badge.svg").read_text(encoding="utf-8")
        dark = (self.r / "alpha" / "first" / "badge-dark.svg").read_text(encoding="utf-8")
        root = ET.fromstring(light)
        ns = "{http://www.w3.org/2000/svg}"
        texts = [t.text for t in root.iter(f"{ns}text")]
        self.assertEqual(texts, ["AI code survival  80.0 %", "causari · #1"])
        left = round(len(texts[0]) * sr.BADGE_CHAR + 2 * sr.BADGE_PAD)
        right = round(len(texts[1]) * sr.BADGE_CHAR + 2 * sr.BADGE_PAD)
        self.assertEqual(int(root.get("width")), left + right)
        self.assertEqual(int(root.get("height")), 20)
        title = root.find(f"{ns}title").text
        self.assertIn("Alpha/first: In Survival Report #1 (2026-09-21, method v2): 400 of 500 lines", title)
        colours = set(re.findall(r'fill="(#[0-9a-f]{6})"', light + dark))
        self.assertEqual(colours, {sr.INK, sr.PAPER})
        for word in ("red", "green", "#4c1", "#e05d44", "stroke="):
            self.assertNotIn(word, light + dark)
        # dark is the same badge with the two values swapped
        self.assertEqual(dark.replace(sr.INK, "X").replace(sr.PAPER, sr.INK).replace("X", sr.PAPER), light)
        self.assertIn("monospace", root.find(f"{ns}g").get("font-family"))
        ET.fromstring(dark)

    def test_latest_json_schema(self) -> None:
        latest = json.loads((self.r / "alpha" / "first" / "latest.json").read_text(encoding="utf-8"))
        self.assertEqual(latest["schema"], sr.REPO_SCHEMA)
        self.assertEqual(latest["repo"], "Alpha/first")
        self.assertEqual(latest["url"], "https://causari.dev/r/alpha/first/")
        self.assertEqual(latest["badge"], "https://causari.dev/r/alpha/first/badge.svg")
        self.assertEqual(latest["report"], {"number": 1, "id": "2026/01", "date": "2026-09-21", "url": "https://causari.dev/reports/survival/2026/01/"})
        self.assertEqual(latest["method"], "v2")
        self.assertEqual(latest["verified"]["introduced"], 500)
        self.assertEqual(latest["verified"]["surviving"], 400)
        self.assertAlmostEqual(latest["verified"]["survival_rate"], 0.8)
        self.assertIn("interval_95", latest)
        self.assertTrue(latest["aggregated"])
        self.assertEqual(latest["reports"], 1)
        self.assertEqual(latest["bytes"], "https://causari.dev/reports/survival/2026/01/repos/Alpha__first.json")
        self.assertEqual(latest["reproduce"], "re audit Alpha/first --json")
        self.assertFalse(json.loads((self.r / "mid" / "small" / "latest.json").read_text(encoding="utf-8"))["aggregated"])

    def test_index_lists_every_repository_alphabetically(self) -> None:
        repos = ["Alpha/first", "mid/one", "mid/small", "zeta/last"]
        for repo in repos:
            self.assertIn(f'href="{sr.repo_path(repo)}"', self.index)
            self.assertIn(f">{repo}</a>", self.index)
        positions = [self.index.index(f">{r}</a>") for r in repos]
        self.assertEqual(positions, sorted(positions))
        self.assertIn("counts, not grades", self.index)
        self.assertIn("n &lt; 5", self.index)
        self.assertIn("80.0 %", self.index)
        self.assertNotIn("Rank", self.index)
        text = visible_text(self.index)
        for word in FORBIDDEN:
            self.assertNotIn(word, text)
        # linked from the archive and listed in the sitemap
        archive = (self.s.site / "reports" / "survival" / "index.html").read_text(encoding="utf-8")
        self.assertIn('<a href="/r/">Every repository has a page and a badge</a>', archive)
        sitemap = ET.parse(self.s.site / "sitemap.xml").getroot()
        locs = [u.find("{http://www.sitemaps.org/schemas/sitemap/0.9}loc").text for u in sitemap]
        self.assertIn("https://causari.dev/r/", locs)
        self.assertIn("https://causari.dev/r/alpha/first/", locs)

    def test_second_report_adds_history_and_sparkline(self) -> None:
        s = Scratch()
        try:
            s.build(number=1, date="2026-09-21")
            # the second run measures a different HEAD: fewer surviving lines
            (s.run / "Alpha__first.json").write_text(json.dumps(audit(12, 600, 300, agent="cursor")), encoding="utf-8")
            s.build(number=2, date="2026-09-28")
            page = (s.site / "r" / "alpha" / "first" / "index.html").read_text(encoding="utf-8")
            self.assertIn("In Survival Report #2 (2026-09-28, method v2): 300 of 600 lines introduced by 12 AI-tagged commits are still at HEAD, 50.0 %.", page)
            self.assertIn('class="spark"', page)
            self.assertIn("#1 80.0 %; #2 50.0 %", page)
            rows = re.findall(r'<tr><td><a href="/reports/survival/(\d{4}/\d{2})/">', page)
            self.assertEqual(rows, ["2026/01", "2026/02"])  # oldest first
            badge = (s.site / "r" / "alpha" / "first" / "badge.svg").read_text(encoding="utf-8")
            self.assertIn("AI code survival  50.0 %", badge)
            self.assertIn("causari · #2", badge)
            latest = json.loads((s.site / "r" / "alpha" / "first" / "latest.json").read_text(encoding="utf-8"))
            self.assertEqual(latest["report"]["number"], 2)
            self.assertEqual(latest["reports"], 2)
            # a repository measured once keeps one row and no line
            other = (s.site / "r" / "mid" / "one" / "index.html").read_text(encoding="utf-8")
            self.assertIn('class="spark"', other)  # measured in both reports
            self.assertEqual(sr.sparkline_svg([("1", 0.5)]), "")
            self.assertEqual(sr.sparkline_svg([("1", 0.5), ("2", None)]), "")
            self.assertIn("<polyline", sr.sparkline_svg([("1", 0.5), ("2", 0.6)]))
        finally:
            s.close()

    def test_rebuild_is_idempotent_for_repo_tree(self) -> None:
        snap = lambda: {str(p.relative_to(self.r)): p.read_bytes() for p in self.r.rglob("*") if p.is_file()}
        before = snap()
        sr.rebuild(self.s.site)
        self.assertEqual(before, snap())
        self.assertEqual(self.report_page, (self.s.site / "reports" / "survival" / "2026" / "01" / "index.html").read_text(encoding="utf-8"))


class IntervalTests(unittest.TestCase):
    def test_reproducible_with_seed(self) -> None:
        pairs = [(1000 + 37 * k, 600 - 41 * k + 13 * (k % 3)) for k in range(12)]
        a = sr.bootstrap_rate(pairs, seed=7)
        b = sr.bootstrap_rate(pairs, seed=7)
        self.assertEqual(a, b)
        c = sr.bootstrap_rate(pairs, seed=8)
        self.assertNotEqual(a, c)
        self.assertLessEqual(a["low"], sum(s for _, s in pairs) / sum(i for i, _ in pairs))
        self.assertGreaterEqual(a["high"], sum(s for _, s in pairs) / sum(i for i, _ in pairs))

    def test_same_report_number_same_bytes(self) -> None:
        s1, s2 = Scratch(), Scratch()
        try:
            f1, f2 = s1.build(number=3), s2.build(number=3)
            self.assertEqual(f1["aggregate"], f2["aggregate"])
            self.assertEqual(f1["aggregate"]["interval_method"]["seed"], 3)
            j1 = (s1.site / "reports" / "survival" / "2026" / "03" / "report.json").read_bytes()
            j2 = (s2.site / "reports" / "survival" / "2026" / "03" / "report.json").read_bytes()
            self.assertEqual(j1, j2)
        finally:
            s1.close()
            s2.close()

    def test_no_interval_for_one_repository(self) -> None:
        self.assertIsNone(sr.bootstrap_rate([(100, 50)], seed=1))
        self.assertIsNone(sr.bootstrap_median([0.5], seed=1))


class GuardTests(unittest.TestCase):
    def test_refuses_to_overwrite_a_different_report(self) -> None:
        s = Scratch()
        try:
            s.build(number=1)
            # the directory of #7 already holds report #1: refuse, never overwrite
            (s.site / "reports" / "survival" / "2026" / "01").rename(s.site / "reports" / "survival" / "2026" / "07")
            with self.assertRaises(SystemExit):
                sr.main(["--site", str(s.site), "--root", str(s.root), "build", "--run", str(s.run),
                         "--number", "7", "--date", "2026-09-21", "--no-png"])
            self.assertEqual(json.loads((s.site / "reports" / "survival" / "2026" / "07" / "report.json").read_text())["number"], 1)
        finally:
            s.close()

    def test_next_number_counts_directories(self) -> None:
        s = Scratch()
        try:
            self.assertEqual(sr.next_number(s.site), 1)
            s.build(number=1)
            self.assertEqual(sr.next_number(s.site), 2)
        finally:
            s.close()

    def test_from_existing_refuses_v1_data(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            src = Path(tmp) / "survival-data.json"
            src.write_text(json.dumps({"generated_at": "2026-08-25T18:00:49Z", "rows": [
                {"repo": "a/b", "total_commits": 10, "verified": stat(6, 100, 50), "probable": stat(0, 0, 0), "by_agent": {}}]}))
            self.assertEqual(sr.from_existing(src, Path(tmp) / "out"), 3)
            self.assertFalse((Path(tmp) / "out" / "run.json").exists())

    def test_from_existing_accepts_v2_rows(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            src = Path(tmp) / "survival-data.json"
            src.write_text(json.dumps({"generated_at": "2026-09-01T00:00:00Z", "tool_version": "0.1.5",
                                       "rows": [{"repo": "a/b", "audited_at": "x", **audit(6, 100, 50)}]}))
            self.assertEqual(sr.from_existing(src, Path(tmp) / "out"), 0)
            self.assertTrue((Path(tmp) / "out" / "a__b.json").exists())
            self.assertEqual(json.loads((Path(tmp) / "out" / "run.json").read_text())["repos"], ["a/b"])


class ScaleTests(unittest.TestCase):
    """One hundred repositories, as the discovered list yields: every surface
    carries all of them, the copy says "100 repositories", nothing assumes a
    handful of rows."""

    N = 100

    @classmethod
    def setUpClass(cls) -> None:
        cls.s = Scratch()
        for p in cls.s.run.glob("*__*.json"):
            p.unlink()
        cls.repos = []
        for k in range(cls.N):
            owner = f"org{k % 7}" if k % 3 else f"Org{k % 5}"  # mixed case, several owners
            repo = f"{owner}/repo-{k:03d}"
            cls.repos.append(repo)
            intro = 1_000 + 137 * k
            a = audit(6 + k % 40, intro, intro - (intro * (k % 10)) // 10 - (3 if k % 10 else 0), agent=["claude-code", "aider", "cursor", "openai-codex"][k % 4])
            (cls.s.run / f"{sr.repo_slug(repo)}.json").write_text(json.dumps(a), encoding="utf-8")
        (cls.s.run / "run.json").write_text(json.dumps({
            "generated_at": "2026-10-05T05:17:00Z", "tool": "causari", "tool_version": "0.2.1", "method": "v2",
            "command": "re audit <owner/repo> --json", "repos": cls.repos, "failed": [f"gone/repo-{k}" for k in range(12)], "opted_out": [],
        }), encoding="utf-8")
        (cls.s.root / ".github" / "survival-discovery.json").write_text(json.dumps({
            "schema": "causari.survival_discovery.v1", "discovered_at": "2026-10-01T04:23:00Z",
            "selection": {"floor": 5, "limit": 100},
            "repositories": [{"repo": r, "seed": k < 30} for k, r in enumerate(cls.repos)],
        }), encoding="utf-8")
        cls.f = cls.s.build(number=2, date="2026-10-05")
        cls.dir = cls.s.site / "reports" / "survival" / "2026" / "02"
        cls.page = (cls.dir / "index.html").read_text(encoding="utf-8")
        cls.md = (cls.dir / "report.md").read_text(encoding="utf-8")

    @classmethod
    def tearDownClass(cls) -> None:
        cls.s.close()

    def test_all_hundred_aggregated_alphabetically(self) -> None:
        repos = [r["repo"] for r in self.f["repositories"]]
        self.assertEqual(len(repos), self.N)
        self.assertEqual(repos, sorted(repos, key=str.lower))
        self.assertEqual(self.f["aggregate"]["repositories"], self.N)
        self.assertEqual(self.f["aggregate"]["introduced"], sum(1_000 + 137 * k for k in range(self.N)))
        self.assertEqual(self.page.count('<tr><td><a href="https://github.com/'), self.N)
        self.assertEqual(sum(1 for l in self.md.splitlines() if l.startswith("| ") and "re audit " in l), self.N)
        self.assertEqual(len(self.f["excluded"]["failed"]), 12)

    def test_copy_counts_one_hundred(self) -> None:
        self.assertIn("in 100 open-source repositories", sr.headline(self.f))
        self.assertIn("in 100 repositories are still at HEAD", (self.dir / "card.svg").read_text(encoding="utf-8"))
        self.assertIn("of the 100 aggregated repositories", self.f["aggregate"]["interval_method"]["note"])
        self.assertIn(">100</span><span class=\"l\">repositories aggregated", self.page)
        iv = self.f["aggregate"]["survival_rate_interval_95"]
        self.assertLess(iv["low"], self.f["aggregate"]["survival_rate"])
        self.assertGreater(iv["high"], self.f["aggregate"]["survival_rate"])
        ET.fromstring((self.dir / "card.svg").read_text(encoding="utf-8"))

    def test_selection_sentence_from_discovery(self) -> None:
        sel = self.f["method"]["selection"]
        self.assertIn("30 hand-picked and 70 found by GitHub commit search", sel)
        self.assertIn("at least 5 commits", sel)
        self.assertIn("discovered 2026-10-01", sel)
        self.assertIn(sel, self.page)
        self.assertIn(sel, self.md)
        self.assertNotIn("added by pull request", self.page)
        for word in FORBIDDEN:
            self.assertNotIn(word, sel)

    def test_sizes_stay_reasonable(self) -> None:
        self.assertLess((self.dir / "report.json").stat().st_size, 400_000)
        self.assertLess((self.dir / "index.html").stat().st_size, 400_000)
        self.assertEqual(len(list((self.dir / "repos").glob("*.json"))), self.N)
        self.assertEqual(len(self.f["by_agent"]), 4)


class ShardTests(unittest.TestCase):
    def frag(self, k: int, repos: list[str], failed: list[str] = (), opted: list[str] = (), version: str = "0.2.1", at: str = "T05:20:00Z") -> dict:
        return {"generated_at": f"2026-10-05{at}", "tool": "causari", "tool_version": version, "method": "v2",
                "command": "re audit <owner/repo> --json", "repos": repos, "failed": list(failed), "opted_out": list(opted), "shard": k}

    def test_merge_unions_fragments_and_records_unreported_repositories(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp) / "root"
            run = Path(tmp) / "run"
            (root / ".github").mkdir(parents=True)
            run.mkdir()
            (root / ".github" / "survival-repos.txt").write_text(
                "# list\na/one\nb/two\nc/three\nd/four\nOpt/Out\ne/five\n", encoding="utf-8")
            (root / ".github" / "survival-optout.txt").write_text("opt/out\n", encoding="utf-8")
            # shard 0: a/one audited, d/four failed; shard 1: b/two audited, c/three has no audit and no failure
            # record (killed mid-run); shard 2 (e/five, Opt/Out) never uploaded at all
            (run / "a__one.json").write_text(json.dumps(audit(6, 100, 50)), encoding="utf-8")
            (run / "b__two.json").write_text(json.dumps(audit(6, 100, 50)), encoding="utf-8")
            (run / "run-shard-0.json").write_text(json.dumps(self.frag(0, ["a/one", "d/four"], failed=["d/four"], at="T05:17:00Z")), encoding="utf-8")
            (run / "run-shard-1.json").write_text(json.dumps(self.frag(1, ["b/two", "c/three"], opted=["Opt/Out"])), encoding="utf-8")
            import contextlib
            import io
            err = io.StringIO()
            with contextlib.redirect_stderr(err), contextlib.redirect_stdout(io.StringIO()):
                merged = sr.merge_shards(run, root)
            on_disk = json.loads((run / "run.json").read_text(encoding="utf-8"))
            self.assertEqual(on_disk, merged)
            self.assertEqual(set(merged) >= {"generated_at", "tool", "tool_version", "method", "command", "repos", "failed", "opted_out"}, True)
            self.assertEqual(merged["generated_at"], "2026-10-05T05:17:00Z")  # earliest shard
            self.assertEqual(merged["tool_version"], "0.2.1")
            self.assertEqual(merged["repos"], ["a/one", "b/two", "c/three", "d/four", "e/five"])
            self.assertEqual(merged["failed"], ["c/three", "d/four", "e/five"])
            self.assertEqual(merged["opted_out"], ["Opt/Out"])
            self.assertIn("e/five", err.getvalue())
            self.assertIn("c/three", err.getvalue())
            # and the generator builds from the merged run: two rows, three failures, one opt-out
            site = Path(tmp) / "site"
            site.mkdir()
            rc = sr.main(["--site", str(site), "--root", str(root), "build", "--run", str(run), "--number", "1", "--date", "2026-10-05", "--no-png"])
            self.assertEqual(rc, 0)
            f = json.loads((site / "reports" / "survival" / "2026" / "01" / "report.json").read_text(encoding="utf-8"))
            self.assertEqual([r["repo"] for r in f["repositories"]], ["a/one", "b/two"])
            self.assertEqual(f["excluded"]["failed"], ["c/three", "d/four", "e/five"])
            self.assertEqual(f["excluded"]["opted_out"], 1)

    def test_merge_refuses_without_fragments(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaises(SystemExit):
                sr.merge_shards(Path(tmp), Path(tmp))

    def test_shard_split_is_index_mod_n(self) -> None:
        # the same rule the workflow applies in bash: shard k takes indices i with i % N == k
        repos = [f"o/r{i}" for i in range(23)]
        shards = [[r for i, r in enumerate(repos) if i % 10 == k] for k in range(10)]
        self.assertEqual(sorted(sum(shards, [])), sorted(repos))
        self.assertEqual(shards[0], ["o/r0", "o/r10", "o/r20"])
        self.assertEqual(shards[3], ["o/r3", "o/r13"])


class ZenodoTests(unittest.TestCase):
    def test_dry_run_payload_without_token(self) -> None:
        s = Scratch()
        try:
            s.build()
            d = s.site / "reports" / "survival" / "2026" / "01"
            import contextlib
            import io
            out = io.StringIO()
            with contextlib.redirect_stdout(out):
                rc = zd.main(["--site", str(s.site), "--dry-run", str(d)])
            self.assertEqual(rc, 0)
            payload = json.loads(out.getvalue())
            self.assertTrue(payload["dry_run"])
            meta = payload["metadata"]
            self.assertEqual(meta["creators"], [{"name": "Crovia Trust", "affiliation": "Crovia Trust"}])
            self.assertEqual(meta["license"], "cc-by-4.0")
            self.assertEqual(meta["upload_type"], "publication")
            self.assertIn("Survival Report #1", meta["title"])
            self.assertEqual(payload["version"], "#1")
            paths = {f["path"] for f in payload["files"]}
            self.assertTrue({"report.json", "report.md", "MANIFEST.json"} <= paths)
            self.assertIn("repos/Alpha__first.json", paths)
            # no token, not a dry run: refused, nothing written
            with contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(zd.main(["--site", str(s.site), str(d)]), 2)
        finally:
            s.close()

    def test_write_back_puts_doi_on_every_surface(self) -> None:
        s = Scratch()
        try:
            f = s.build()
            d = s.site / "reports" / "survival" / "2026" / "01"
            rec = {"doi": "10.5281/zenodo.99999", "concept_doi": "10.5281/zenodo.99998", "html": "https://zenodo.org/records/99999",
                   "id": 99999, "version": "#1", "sandbox": False, "published_at": "2026-09-21T06:00:00Z"}
            zd.write_back(d, f, rec, s.site)
            self.assertEqual(json.loads((d / "report.json").read_text())["doi"], "10.5281/zenodo.99999")
            self.assertIn("https://doi.org/10.5281/zenodo.99999", (d / "index.html").read_text())
            self.assertIn("10.5281/zenodo.99999", (d / "report.md").read_text())
            self.assertIn("10.5281/zenodo.99999", (s.site / "reports" / "survival" / "index.html").read_text())
            self.assertEqual(json.loads((s.site / "reports" / "survival" / "latest.json").read_text())["doi"], "10.5281/zenodo.99999")
            self.assertIn("10.5281/zenodo.99999", (s.site / "reports" / "survival" / "feed.xml").read_text())
        finally:
            s.close()

    def test_content_hash_ignores_manifest(self) -> None:
        files = {"report.json": b"{}", "MANIFEST.json": b"a"}
        self.assertEqual(zd.content_hash(files), zd.content_hash({"report.json": b"{}", "MANIFEST.json": b"b"}))
        self.assertNotEqual(zd.content_hash(files), zd.content_hash({"report.json": b"{ }"}))


if __name__ == "__main__":
    unittest.main()
