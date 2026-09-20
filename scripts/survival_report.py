#!/usr/bin/env python3
"""Weekly Survival Report: a numbered, dated, citable report of AI code survival.

Built from the per-repository output of ``re audit <owner/repo> --json``
(method v2) and nothing else. The report states counts and intervals: how
many lines were introduced by commits that carry machine-readable AI
authorship metadata, and how many of them ``git blame`` still attributes to
those commits at HEAD. No rank, no colour, no verdict; rows are alphabetical.
Same pipeline shape as the Crovia Silence Report so the two read as siblings.

Inputs (a run directory)::

    run.json                 {"generated_at", "tool", "tool_version", "method",
                              "command", "repos", "failed", "opted_out"}
    <owner>__<repo>.json     the exact bytes of `re audit --json` per repository

Outputs (under site/reports/survival/)::

    <YYYY>/<NN>/index.html   the report page
    <YYYY>/<NN>/report.json  machine-readable counts, intervals, coverage
    <YYYY>/<NN>/report.md    plain-text version (Zenodo description)
    <YYYY>/<NN>/card.svg     Open Graph card, identity style
    <YYYY>/<NN>/card.png     same card as PNG (needs Pillow; skipped otherwise)
    <YYYY>/<NN>/repos/*.json the per-repository audit bytes every number links to
    index.html               archive, newest first
    feed.xml                 Atom, one entry per report
    latest.json              copy of the newest report.json
    plus managed blocks in site/_redirects and site/sitemap.xml

Usage::

    python3 scripts/survival_report.py build --run /tmp/run [--number N] [--date YYYY-MM-DD]
    python3 scripts/survival_report.py rebuild            # archive, feed, latest, redirects
    python3 scripts/survival_report.py from-existing site/survival-data.json --out /tmp/run

Standard library only; Pillow is optional (card.png).
"""

from __future__ import annotations

import argparse
import datetime as dt
import glob
import html
import json
import random
import re
import shutil
import sys
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(HERE))
from site_version import asset_url  # noqa: E402  content-versioned /styles.css and /app.js
SITE_URL = "https://causari.dev"
REPO_URL = "https://github.com/croviatrust/causari"
REPORTS_REL = "reports/survival"
SCHEMA = "causari.survival_report.v1"
RESAMPLES = 2000
LICENSE = "CC-BY-4.0"
LICENSE_URL = "https://creativecommons.org/licenses/by/4.0/"

# Identity palette (canon/canon.json → glyphs.palette).
INK = "#0b0d10"
PAPER = "#f5f4ef"
GRAPHITE = "#3b4252"
MIST = "#9aa3ad"

REDIRECT_BEGIN = "# survival-report: begin (managed by scripts/survival_report.py)"
REDIRECT_END = "# survival-report: end"
SITEMAP_BEGIN = "<!-- survival-report: begin (managed by scripts/survival_report.py) -->"
SITEMAP_END = "<!-- survival-report: end -->"


# ----------------------------------------------------------------------------- helpers

def load_json(path: Path, default: Any = None) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return default


def dump_json(path: Path, data: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")


def esc(s: Any) -> str:
    return html.escape(str(s), quote=True)


def fmt_int(n: int | None) -> str:
    return "—" if n is None else f"{n:,}"


def fmt_pct(x: float | None) -> str:
    return "—" if x is None else f"{x * 100:.1f} %"


def fmt_share(x: float | None) -> str:
    return "—" if x is None else f"{x * 100:.0f} %"


def repo_slug(repo: str) -> str:
    return repo.replace("/", "__")


def slug_repo(slug: str) -> str:
    return slug.replace("__", "/", 1)


def report_id(year: int, number: int) -> str:
    return f"{year}/{number:02d}"


def report_url(rid: str) -> str:
    return f"{SITE_URL}/{REPORTS_REL}/{rid}/"


def read_optout(root: Path) -> set[str]:
    """`.github/survival-optout.txt`: one owner/repo per line, `#` comments,
    matched case-insensitively. Same semantics as the workflow."""
    path = root / ".github" / "survival-optout.txt"
    out: set[str] = set()
    if not path.exists():
        return out
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if line and not line.startswith("#"):
            out.add(line.lower())
    return out


# ----------------------------------------------------------------------------- statistics

def percentile(sorted_values: list[float], q: float) -> float:
    """Nearest-rank percentile on an already sorted list."""
    if not sorted_values:
        raise ValueError("empty")
    idx = int(round(q * (len(sorted_values) - 1)))
    return sorted_values[max(0, min(idx, len(sorted_values) - 1))]


def bootstrap_rate(pairs: list[tuple[int, int]], seed: int, resamples: int = RESAMPLES) -> dict[str, Any] | None:
    """95 % interval on Σ surviving / Σ introduced, resampling repositories
    with replacement. Seeded so the same report always yields the same bytes.
    Returns None with fewer than two repositories: an interval over one
    repository would be a fiction."""
    pairs = [(i, s) for i, s in pairs if i > 0]
    if len(pairs) < 2:
        return None
    rng = random.Random(seed)
    n = len(pairs)
    rates: list[float] = []
    for _ in range(resamples):
        intro = surv = 0
        for _ in range(n):
            i, s = pairs[rng.randrange(n)]
            intro += i
            surv += s
        rates.append(surv / intro)
    rates.sort()
    return {"low": percentile(rates, 0.025), "high": percentile(rates, 0.975)}


def bootstrap_median(values: list[float], seed: int, resamples: int = RESAMPLES) -> dict[str, Any] | None:
    values = [v for v in values if v is not None]
    if len(values) < 2:
        return None
    rng = random.Random(seed)
    n = len(values)
    meds: list[float] = []
    for _ in range(resamples):
        sample = sorted(values[rng.randrange(n)] for _ in range(n))
        meds.append(sample[n // 2] if n % 2 else (sample[n // 2 - 1] + sample[n // 2]) / 2)
    meds.sort()
    return {"low": percentile(meds, 0.025), "high": percentile(meds, 0.975)}


def median(values: list[float]) -> float | None:
    values = sorted(v for v in values if v is not None)
    if not values:
        return None
    n = len(values)
    return values[n // 2] if n % 2 else (values[n // 2 - 1] + values[n // 2]) / 2


# ----------------------------------------------------------------------------- collect

def is_v2(audit: dict[str, Any]) -> bool:
    cov = audit.get("coverage")
    return isinstance(cov, dict) and "method" in cov and "small_sample" in cov and "shallow" in cov


def stat_view(stat: dict[str, Any]) -> dict[str, Any]:
    return {
        "commits": int(stat.get("commits") or 0),
        "introduced": int(stat.get("introduced") or 0),
        "surviving": int(stat.get("surviving") or 0),
        "survival_rate": stat.get("survival_rate"),
        "median_survival": stat.get("median_survival"),
        "capped_survival_rate": stat.get("capped_survival_rate"),
        "cap_lines": stat.get("cap_lines"),
        "largest_commit_share": stat.get("largest_commit_share"),
    }


def collect(run_dir: Path, number: int, date: str, root: Path) -> dict[str, Any]:
    run = load_json(run_dir / "run.json", None)
    if not isinstance(run, dict):
        raise SystemExit(f"{run_dir}/run.json missing or invalid")
    optout = read_optout(root)

    rows: list[dict[str, Any]] = []
    shallow: list[str] = []
    not_v2: list[str] = []
    # Repositories the workflow already skipped never reach the run dir; those
    # that did reach it are dropped here, so the list is honoured either way.
    opted: set[str] = {str(n).lower() for n in (run.get("opted_out") or [])}
    for fp in sorted(run_dir.glob("*__*.json")):
        repo = slug_repo(fp.stem)
        if repo.lower() in optout:
            opted.add(repo.lower())
            continue
        audit = load_json(fp, None)
        if not isinstance(audit, dict):
            not_v2.append(repo)
            continue
        if not is_v2(audit):
            not_v2.append(repo)
            continue
        cov = audit["coverage"]
        if cov.get("shallow"):
            shallow.append(repo)
            continue
        v = stat_view(audit.get("verified") or {})
        p = stat_view(audit.get("probable") or {})
        rows.append({
            "repo": repo,
            "url": f"https://github.com/{repo}",
            "audit_file": f"repos/{fp.name}",
            "reproduce": f"re audit {repo} --json",
            "total_commits": int(audit.get("total_commits") or 0),
            "verified": v,
            "probable": p,
            "by_agent": {k: stat_view(s) for k, s in sorted((audit.get("by_agent") or {}).items())},
            "coverage": {
                "method": cov.get("method"),
                "blame_flags": list(cov.get("blame_flags") or []),
                "ignore_revs_file": bool(cov.get("ignore_revs_file")),
                "shallow": bool(cov.get("shallow")),
                "sample_floor": int(cov.get("sample_floor") or 0),
                "small_sample": bool(cov.get("small_sample")),
            },
            "aggregated": not cov.get("small_sample") and v["introduced"] > 0,
            "_source": fp,
        })
    opted_out = len(opted)

    # Alphabetical, case-insensitive, and nothing else: never by rate.
    rows.sort(key=lambda r: r["repo"].lower())
    aggregated = [r for r in rows if r["aggregated"]]
    not_aggregated = [r for r in rows if not r["aggregated"]]

    sample_floor = max((r["coverage"]["sample_floor"] for r in rows), default=5)
    blame_flags = next((r["coverage"]["blame_flags"] for r in rows if r["coverage"]["blame_flags"]), [])
    method_version = next((r["coverage"]["method"] for r in rows if r["coverage"]["method"]), run.get("method") or "v2")

    intro = sum(r["verified"]["introduced"] for r in aggregated)
    surv = sum(r["verified"]["surviving"] for r in aggregated)
    ai_commits = sum(r["verified"]["commits"] for r in aggregated)
    total_commits = sum(r["total_commits"] for r in aggregated)
    pairs = [(r["verified"]["introduced"], r["verified"]["surviving"]) for r in aggregated]
    capped = [r["verified"]["capped_survival_rate"] for r in aggregated]

    by_agent: dict[str, dict[str, int]] = {}
    for r in aggregated:
        for agent, s in r["by_agent"].items():
            a = by_agent.setdefault(agent, {"repositories": 0, "commits": 0, "introduced": 0, "surviving": 0})
            a["repositories"] += 1
            a["commits"] += s["commits"]
            a["introduced"] += s["introduced"]
            a["surviving"] += s["surviving"]
    by_agent_out = {
        agent: {**a, "survival_rate": (a["surviving"] / a["introduced"]) if a["introduced"] else None}
        for agent, a in sorted(by_agent.items())
    }

    year = int(date[:4])
    rid = report_id(year, number)
    interval_note = (
        f"95 % percentile interval from {RESAMPLES} bootstrap resamples of the {len(aggregated)} aggregated "
        f"repositories (with replacement, seed {number}). It describes the sampled repositories, "
        "not all AI-assisted code, and not the repositories not in this sample."
    )
    facts = {
        "schema": SCHEMA,
        "title": f"Survival Report #{number}",
        "number": number,
        "id": rid,
        "date": date,
        "generated_at": run.get("generated_at") or dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "url": report_url(rid),
        "license": LICENSE,
        "license_url": LICENSE_URL,
        "publisher": "Crovia Trust",
        "tool": {"name": run.get("tool") or "causari", "version": run.get("tool_version") or "unknown"},
        "method": {
            "version": method_version,
            "url": f"{SITE_URL}/method",
            "command": run.get("command") or "re audit <owner/repo> --json",
            "blame_flags": blame_flags,
            "sample_floor": sample_floor,
            "cap_rule": "a commit weighs at most the 95th percentile of per-commit introduced line counts in its repository, and never more than 10,000 lines",
            "evidence_class": "VERIFIED only; PROBABLE is listed but never summed",
            "aggregation": "repositories with at least sample_floor VERIFIED commits and a full (non-shallow) clone",
        },
        "aggregate": {
            "repositories": len(aggregated),
            "total_commits": total_commits,
            "ai_tagged_commits": ai_commits,
            "introduced": intro,
            "surviving": surv,
            "survival_rate": (surv / intro) if intro else None,
            "survival_rate_interval_95": bootstrap_rate(pairs, seed=number),
            "median_capped_survival_rate": median(capped),
            "median_capped_survival_rate_interval_95": bootstrap_median(capped, seed=number),
            "interval_method": {"kind": "bootstrap over repositories", "resamples": RESAMPLES, "seed": number, "note": interval_note},
        },
        "by_agent": by_agent_out,
        "repositories": [{k: v for k, v in r.items() if not k.startswith("_")} for r in aggregated],
        "not_aggregated": [{k: v for k, v in r.items() if not k.startswith("_")} for r in not_aggregated],
        "excluded": {
            "shallow": sorted(shallow, key=str.lower),
            "not_method_v2": sorted(not_v2, key=str.lower),
            "failed": sorted((run.get("failed") or []), key=str.lower),
            "opted_out": opted_out,
            "optout_file": f"{REPO_URL}/blob/main/.github/survival-optout.txt",
        },
        "doi": None,
        "concept_doi": None,
        "zenodo": None,
        "_sources": {r["repo"]: r["_source"] for r in rows},
    }
    return facts


# ----------------------------------------------------------------------------- text

def headline(f: dict[str, Any]) -> str:
    a = f["aggregate"]
    if not a["repositories"]:
        return "No repository met the aggregation rule in this run."
    s = (
        f"{fmt_int(a['surviving'])} of {fmt_int(a['introduced'])} lines introduced by "
        f"{fmt_int(a['ai_tagged_commits'])} AI-tagged commits in {a['repositories']} open-source repositories "
        f"are still at HEAD ({fmt_pct(a['survival_rate'])})."
    )
    iv = a["survival_rate_interval_95"]
    if iv:
        s += f" 95 % interval over the sampled repositories: {fmt_pct(iv['low'])} to {fmt_pct(iv['high'])}."
    return s


def cite(f: dict[str, Any]) -> str:
    s = f"Crovia Trust. Survival Report #{f['number']} ({f['date']}). {f['url']}"
    if f.get("doi"):
        s += f" DOI {f['doi']}"
    return s


POSITIONING = {
    "is": (
        "This report counts lines. For each repository it states how many lines were introduced by commits "
        "that carry machine-readable AI authorship metadata (trailers such as Co-Authored-By naming an agent, "
        "bot author identities, aider markers, git-ai notes), and how many of those lines git blame still "
        "attributes to those commits at HEAD, under method v2 (blame with -w -M -C, a per-commit weight cap, "
        "a sample floor, full clones only). Every row is reproducible with one command."
    ),
    "is_not": (
        "It is not a quality judgement: deleted lines include removed features and rewritten prototypes; "
        "surviving lines include dead code. It is not a sample of all AI-assisted code: inline completions "
        "leave no trace in git, untagged agent commits are invisible, and the repositories were added by pull "
        "request, not drawn at random. The intervals describe the sampled repositories only."
    ),
    "context": (
        "Prior measurement work asks related questions with different instruments. GitClear publishes churn "
        "reports built from code-change patterns across the repositories it analyses; arXiv 2601.16809 "
        "(\"Will It Survive?\") follows the modification of agent-authored code in 201 projects with its own "
        "detector and finds that such code is modified less often than human-written code. This report does "
        "not reproduce either method and does not adjudicate between them: it publishes counts from git "
        "metadata alone, with the method version, the tool version and the exact bytes behind every number, "
        "so that the three can be read side by side."
    ),
}


def report_md(f: dict[str, Any]) -> str:
    a = f["aggregate"]
    m = f["method"]
    lines = [
        f"# Survival Report #{f['number']} — {f['date']}",
        "",
        f"Counts of surviving lines from AI-tagged commits in {a['repositories']} open-source repositories, "
        f"measured with {f['tool']['name']} {f['tool']['version']}, method {m['version']}. "
        "Counts, not grades: no rank, no verdict; rows are alphabetical.",
        "",
        f"Page: {f['url']}  ",
        f"Data: {f['url']}report.json  ",
        f"Feed: {SITE_URL}/{REPORTS_REL}/feed.xml  ",
        f"Licence: {LICENSE}  ",
    ]
    if f.get("doi"):
        lines.append(f"DOI: https://doi.org/{f['doi']}  ")
    lines += ["", "## Aggregate", "", headline(f), ""]
    if a["repositories"]:
        lines += [
            f"- Repositories aggregated: {a['repositories']}",
            f"- Commits in those repositories (no merges): {fmt_int(a['total_commits'])}",
            f"- AI-tagged (VERIFIED) commits: {fmt_int(a['ai_tagged_commits'])}",
            f"- Lines introduced by them: {fmt_int(a['introduced'])}",
            f"- Still attributed to them at HEAD: {fmt_int(a['surviving'])}",
            f"- Line-weighted ratio: {fmt_pct(a['survival_rate'])}",
        ]
        iv = a["survival_rate_interval_95"]
        if iv:
            lines.append(f"- 95 % bootstrap interval over the sampled repositories: {fmt_pct(iv['low'])} to {fmt_pct(iv['high'])}")
        lines.append(f"- Median of per-repository capped ratios: {fmt_pct(a['median_capped_survival_rate'])}")
        iv2 = a["median_capped_survival_rate_interval_95"]
        if iv2:
            lines.append(f"- 95 % bootstrap interval on that median: {fmt_pct(iv2['low'])} to {fmt_pct(iv2['high'])}")
        lines.append("")
        lines.append(a["interval_method"]["note"])
        lines.append("")
    lines += ["## Repositories (alphabetical)", "",
              "| Repository | Commits | AI-tagged | Introduced | Still at HEAD | Line-weighted | Capped | Median per commit | Largest commit | Reproduce |",
              "|---|---:|---:|---:|---:|---:|---:|---:|---:|---|"]
    for r in f["repositories"]:
        v = r["verified"]
        lines.append(
            f"| {r['repo']} | {fmt_int(r['total_commits'])} | {fmt_int(v['commits'])} | {fmt_int(v['introduced'])} | "
            f"{fmt_int(v['surviving'])} | {fmt_pct(v['survival_rate'])} | {fmt_pct(v['capped_survival_rate'])} | "
            f"{fmt_pct(v['median_survival'])} | {fmt_share(v['largest_commit_share'])} | `{r['reproduce']}` |"
        )
    if f["by_agent"]:
        lines += ["", "## By agent, across aggregated repositories (alphabetical)", "",
                  "| Agent | Repositories | Commits | Introduced | Still at HEAD | Line-weighted |", "|---|---:|---:|---:|---:|---:|"]
        for agent, s in f["by_agent"].items():
            lines.append(f"| {agent} | {s['repositories']} | {fmt_int(s['commits'])} | {fmt_int(s['introduced'])} | {fmt_int(s['surviving'])} | {fmt_pct(s['survival_rate'])} |")
    if f["not_aggregated"]:
        lines += ["", f"## Measured but not aggregated (fewer than {m['sample_floor']} AI-tagged commits)", "",
                  "| Repository | Commits | AI-tagged | Introduced | Still at HEAD | Reproduce |", "|---|---:|---:|---:|---:|---|"]
        for r in f["not_aggregated"]:
            v = r["verified"]
            lines.append(f"| {r['repo']} | {fmt_int(r['total_commits'])} | {fmt_int(v['commits'])} | {fmt_int(v['introduced'])} | {fmt_int(v['surviving'])} | `{r['reproduce']}` |")
    ex = f["excluded"]
    lines += ["", "## Excluded from this report", ""]
    lines.append(f"- Shallow clones (history truncated; method v2 refuses them): {', '.join(ex['shallow']) or 'none'}")
    lines.append(f"- Audits that failed in this run: {', '.join(ex['failed']) or 'none'}")
    if ex["not_method_v2"]:
        lines.append(f"- Outputs not in method v2 format: {', '.join(ex['not_method_v2'])}")
    lines.append(f"- Opted out by their maintainers ({ex['optout_file']}): {ex['opted_out']}")
    lines += ["", "## Method", "",
              f"Method {m['version']}, {f['tool']['name']} {f['tool']['version']}. Detection from commit metadata only; "
              f"survival from `git blame {' '.join(m['blame_flags'])}` at HEAD. Per-commit cap: {m['cap_rule']}. "
              f"Sample floor: {m['sample_floor']} VERIFIED commits. {m['evidence_class']}. Full clones only. "
              f"Details, limits and how to contest a number: {m['url']}.",
              "", "## What this report is, and is not", "", POSITIONING["is"], "", POSITIONING["is_not"], "", POSITIONING["context"],
              "", "## Cite", "", cite(f), ""]
    return "\n".join(lines)


# ----------------------------------------------------------------------------- card

def card_svg(f: dict[str, Any]) -> str:
    a = f["aggregate"]
    mono = "'JetBrains Mono','SF Mono','Cascadia Mono','Fira Code',Consolas,monospace"
    r = 14.5 * 1.3
    discs = ((28, 34), (72, 34), (50, 72))
    disc_svg = "\n".join(
        f'  <circle cx="{72 + x * 1.3:.1f}" cy="{72 + y * 1.3:.1f}" r="{r:.1f}" fill="{PAPER}"/>' for x, y in discs
    )
    big = f"{fmt_int(a['surviving'])} of {fmt_int(a['introduced'])} lines" if a["repositories"] else "no repository aggregated"
    line2 = (f"from {fmt_int(a['ai_tagged_commits'])} AI-tagged commits in {a['repositories']} repositories are still at HEAD"
             if a["repositories"] else "in this run")
    iv = a["survival_rate_interval_95"]
    line3 = (f"{fmt_pct(a['survival_rate'])} · 95 % interval over the sampled repositories {fmt_pct(iv['low'])} – {fmt_pct(iv['high'])}"
             if iv else (fmt_pct(a["survival_rate"]) if a["repositories"] else ""))
    return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1200 630" width="1200" height="630" role="img" aria-label="Survival Report #{f['number']}">
  <rect width="1200" height="630" fill="{INK}"/>
{disc_svg}
  <text x="252" y="170" font-family="{mono}" font-size="72" font-weight="500" letter-spacing="-1" fill="{PAPER}">causari</text>
  <text x="72" y="270" font-family="{mono}" font-size="26" fill="{MIST}">Survival Report #{f['number']} · {esc(f['date'])} · method {esc(f['method']['version'])}</text>
  <text x="72" y="345" font-family="{mono}" font-size="44" fill="{PAPER}">{esc(big)}</text>
  <text x="72" y="400" font-family="{mono}" font-size="{26 if len(line2) > 62 else 30}" fill="{PAPER}">{esc(line2)}</text>
  <text x="72" y="450" font-family="{mono}" font-size="24" fill="{MIST}">{esc(line3)}</text>
  <text x="72" y="540" font-family="{mono}" font-size="24" fill="{MIST}">counts, not grades  ·  re audit &lt;owner/repo&gt; --json  ·  {LICENSE}</text>
  <text x="72" y="580" font-family="{mono}" font-size="24" fill="{MIST}">causari.dev/{REPORTS_REL}</text>
</svg>
"""


def card_png(f: dict[str, Any], out: Path) -> bool:
    try:
        from PIL import Image, ImageDraw
    except ImportError:
        print("survival_report: Pillow not installed; card.png skipped", file=sys.stderr)
        return False
    sys.path.insert(0, str(HERE))
    import identity  # scripts/identity.py: palette, disc geometry, font lookup

    a = f["aggregate"]
    S = 2
    img = Image.new("RGB", (1200 * S, 630 * S), INK)
    d = ImageDraw.Draw(img)

    def fitted(text: str, size: int, max_width: int = 1056 * S):
        # shrink until the line fits the card's text column
        while size > 14 * S and d.textlength(text, font=identity.font(size)) > max_width:
            size -= 1 * S
        return identity.font(size)

    identity.draw_discs(d, 72 * S, 72 * S, 1.3 * S, PAPER)
    d.text((252 * S, 118 * S), "causari", font=identity.font(72 * S), fill=PAPER)
    d.text((72 * S, 246 * S), f"Survival Report #{f['number']} · {f['date']} · method {f['method']['version']}", font=identity.font(26 * S), fill=MIST)
    big = f"{fmt_int(a['surviving'])} of {fmt_int(a['introduced'])} lines" if a["repositories"] else "no repository aggregated"
    d.text((72 * S, 305 * S), big, font=fitted(big, 44 * S), fill=PAPER)
    line2 = (f"from {fmt_int(a['ai_tagged_commits'])} AI-tagged commits in {a['repositories']} repositories are still at HEAD"
             if a["repositories"] else "in this run")
    d.text((72 * S, 372 * S), line2, font=fitted(line2, 30 * S), fill=PAPER)
    iv = a["survival_rate_interval_95"]
    if iv:
        line3 = f"{fmt_pct(a['survival_rate'])} · 95 % interval over the sampled repositories {fmt_pct(iv['low'])} – {fmt_pct(iv['high'])}"
    else:
        line3 = fmt_pct(a["survival_rate"]) if a["repositories"] else ""
    d.text((72 * S, 428 * S), line3, font=fitted(line3, 24 * S), fill=MIST)
    d.text((72 * S, 518 * S), f"counts, not grades  ·  re audit <owner/repo> --json  ·  {LICENSE}", font=identity.font(24 * S), fill=MIST)
    d.text((72 * S, 558 * S), f"causari.dev/{REPORTS_REL}", font=identity.font(24 * S), fill=MIST)
    img = img.resize((1200, 630), Image.LANCZOS)
    out.parent.mkdir(parents=True, exist_ok=True)
    img.save(out, optimize=True)
    return True


# ----------------------------------------------------------------------------- html

def page_head(title: str, desc: str, url: str, image: str, jsonld: dict[str, Any]) -> str:
    return f"""<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width,initial-scale=1,viewport-fit=cover" />
  <title>{esc(title)} — causari</title>
  <meta name="description" content="{esc(desc)}" />
  <meta name="theme-color" content="#0b0d10" />
  <meta name="color-scheme" content="light dark" />
  <link rel="canonical" href="{esc(url)}" />
  <link rel="alternate" type="application/atom+xml" title="Causari Survival Report" href="/{REPORTS_REL}/feed.xml" />
  <meta property="og:type" content="article" />
  <meta property="og:title" content="{esc(title)}" />
  <meta property="og:description" content="{esc(desc)}" />
  <meta property="og:url" content="{esc(url)}" />
  <meta property="og:image" content="{esc(image)}" />
  <meta property="og:image:width" content="1200" />
  <meta property="og:image:height" content="630" />
  <meta property="og:site_name" content="causari" />
  <meta name="twitter:card" content="summary_large_image" />
  <meta name="twitter:title" content="{esc(title)}" />
  <meta name="twitter:description" content="{esc(desc)}" />
  <meta name="twitter:image" content="{esc(image)}" />
  <link rel="icon" type="image/svg+xml" href="/assets/favicon.svg" />
  <link rel="stylesheet" href="{asset_url('styles.css')}" />
  <script type="application/ld+json">{json.dumps(jsonld, ensure_ascii=False).replace('</', '<\\/')}</script>
</head>
<body>

<a class="skip-link" href="#main">Skip to content</a>

<header class="nav" role="banner">
  <div class="container nav-inner">
    <a href="/" class="brand" aria-label="causari home">
      <img src="/assets/mark.svg" alt="" width="26" height="26" class="mark-light" />
      <img src="/assets/mark-white.svg" alt="" width="26" height="26" class="mark-dark" />
      <span class="brand-name" translate="no">causari</span>
    </a>
    <nav class="nav-links" aria-label="Primary">
      <a href="/#audit" class="hide-sm">audit</a>
      <a href="/{REPORTS_REL}/" aria-current="page">reports</a>
      <a href="/method">method</a>
      <a href="{REPO_URL}" rel="noopener">source</a>
      <button class="theme-toggle" id="theme-toggle" aria-label="Toggle light and dark" title="Toggle light and dark">◐</button>
    </nav>
  </div>
</header>

<main id="main">
"""


def page_foot(extra: str = "") -> str:
    return f"""</main>

<footer class="footer">
  <div class="container">
    {extra}
    <div class="foot-bottom">
      <p>© <span id="year">2026</span> <a href="https://croviatrust.com" rel="noopener">Crovia</a> · <em>causari</em> is a trademark of Crovia Trust. Report text and data <a href="{LICENSE_URL}" rel="license noopener">{LICENSE}</a>.</p>
      <p class="muted">Every number reproducible: <code translate="no">re audit &lt;owner/repo&gt; --json</code> · <a href="/method">method</a> · <a href="/{REPORTS_REL}/feed.xml">feed</a> · <a href="/">causari.dev</a></p>
    </div>
  </div>
</footer>

<script src="{asset_url('app.js')}" defer></script>
</body>
</html>
"""


def positioning_html() -> str:
    return f"""<div class="rp-positioning">
      <h3>What this report is, and is not</h3>
      <p>{esc(POSITIONING['is'])}</p>
      <p>{esc(POSITIONING['is_not'])}</p>
      <p>{esc(POSITIONING['context']).replace('arXiv 2601.16809', '<a href="https://arxiv.org/abs/2601.16809" rel="noopener">arXiv 2601.16809</a>')}</p>
    </div>"""


def num_cell(value: str, href: str, title: str) -> str:
    return f'<td><a class="rp-num" href="{esc(href)}" title="{esc(title)}">{value}</a></td>'


def repo_rows(rows: list[dict[str, Any]], full: bool) -> str:
    out = []
    for r in rows:
        v = r["verified"]
        p = r["probable"]
        href = r["audit_file"]
        t = r["reproduce"]
        probable = f' <span class="muted">(+{fmt_int(p["commits"])} probable)</span>' if p["commits"] else ""
        cells = [
            f'<td><a href="{esc(r["url"])}" rel="noopener">{esc(r["repo"])}</a></td>',
            num_cell(fmt_int(r["total_commits"]), href, t),
            num_cell(fmt_int(v["commits"]), href, t).replace("</a></td>", f"</a>{probable}</td>"),
            num_cell(fmt_int(v["introduced"]), href, t),
            num_cell(fmt_int(v["surviving"]), href, t),
        ]
        if full:
            cells += [
                num_cell(fmt_pct(v["survival_rate"]), href, t),
                num_cell(fmt_pct(v["capped_survival_rate"]), href, t),
                num_cell(fmt_pct(v["median_survival"]), href, t),
                num_cell(fmt_share(v["largest_commit_share"]), href, t),
            ]
        else:
            floor = r["coverage"]["sample_floor"]
            cells.append(f'<td><span class="lb-none" title="Fewer than {floor} AI-tagged commits: one commit can dominate, so no ratio is aggregated">n &lt; {floor}</span></td>')
        cells.append(f'<td><code translate="no" class="lb-repro">{esc(t)}</code></td>')
        out.append("<tr>" + "".join(cells) + "</tr>")
    return "\n".join(out)


def agent_rows(by_agent: dict[str, Any]) -> str:
    return "\n".join(
        f"<tr><td>{esc(agent)}</td><td>{s['repositories']}</td><td>{fmt_int(s['commits'])}</td>"
        f"<td>{fmt_int(s['introduced'])}</td><td>{fmt_int(s['surviving'])}</td><td>{fmt_pct(s['survival_rate'])}</td></tr>"
        for agent, s in by_agent.items()
    )


def doi_html(f: dict[str, Any]) -> str:
    if f.get("doi"):
        return f'<a class="rp-doi" id="doi" href="https://doi.org/{esc(f["doi"])}" rel="noopener">DOI {esc(f["doi"])}</a>'
    return '<span class="rp-doi" id="doi">DOI: pending deposit</span>'


def render_report(f: dict[str, Any]) -> str:
    a = f["aggregate"]
    m = f["method"]
    ex = f["excluded"]
    url = f["url"]
    img = url + "card.png"
    title = f"Survival Report #{f['number']} · {f['date']}"
    desc = headline(f) + " Counts, not grades; method public; every number reproducible."
    jsonld = {
        "@context": "https://schema.org", "@type": "Report", "name": title, "headline": headline(f), "url": url,
        "datePublished": f["date"], "dateModified": f["generated_at"], "inLanguage": "en", "image": img,
        "author": {"@type": "Organization", "name": "Crovia Trust", "url": "https://croviatrust.com"},
        "publisher": {"@type": "Organization", "name": "Crovia Trust", "url": "https://croviatrust.com"},
        "license": LICENSE_URL,
        "isBasedOn": [r["url"] for r in f["repositories"]],
        "distribution": {"@type": "DataDownload", "encodingFormat": "application/json", "contentUrl": url + "report.json"},
    }
    if f.get("doi"):
        jsonld["identifier"] = {"@type": "PropertyValue", "propertyID": "DOI", "value": f["doi"]}
    iv = a["survival_rate_interval_95"]
    iv2 = a["median_capped_survival_rate_interval_95"]
    strip = ""
    if a["repositories"]:
        strip = f"""
    <div class="rp-strip">
      <a class="rp-stat" href="report.json"><span class="n">{a['repositories']}</span><span class="l">repositories aggregated</span></a>
      <a class="rp-stat" href="report.json"><span class="n">{fmt_int(a['ai_tagged_commits'])}</span><span class="l">AI-tagged commits · of {fmt_int(a['total_commits'])}</span></a>
      <a class="rp-stat" href="report.json"><span class="n">{fmt_int(a['introduced'])}</span><span class="l">lines introduced by them</span></a>
      <a class="rp-stat" href="report.json"><span class="n">{fmt_int(a['surviving'])}</span><span class="l">still attributed to them at HEAD</span></a>
      <a class="rp-stat" href="report.json"><span class="n">{fmt_pct(a['survival_rate'])}</span><span class="l">line-weighted{' · 95 % interval ' + fmt_pct(iv['low']) + ' – ' + fmt_pct(iv['high']) if iv else ''}</span></a>
      <a class="rp-stat" href="report.json"><span class="n">{fmt_pct(a['median_capped_survival_rate'])}</span><span class="l">median of capped per-repository ratios{' · ' + fmt_pct(iv2['low']) + ' – ' + fmt_pct(iv2['high']) if iv2 else ''}</span></a>
    </div>"""
    not_agg = ""
    if f["not_aggregated"]:
        not_agg = f"""
    <h3 id="not-aggregated">Measured but not aggregated</h3>
    <p class="muted small">Fewer than {m['sample_floor']} AI-tagged commits: the counts are published, the ratio is not, and the repository is left out of the aggregate above.</p>
    <div class="lb-scroll">
      <div class="tbl-scroll">
      <table class="lb-table">
        <thead><tr><th>Repository</th><th>Commits</th><th>AI-tagged</th><th>Lines introduced</th><th>Still at HEAD</th><th>Ratio</th><th>Reproduce</th></tr></thead>
        <tbody>
{repo_rows(f['not_aggregated'], full=False)}
        </tbody>
      </table>
      </div>
    </div>"""
    agents = ""
    if f["by_agent"]:
        agents = f"""
    <h3 id="by-agent">By agent, across the aggregated repositories</h3>
    <p class="muted small">Alphabetical. A commit is attributed to the agent its metadata names; one agent per commit.</p>
    <div class="lb-scroll">
      <div class="tbl-scroll">
      <table class="lb-table">
        <thead><tr><th>Agent</th><th>Repositories</th><th>Commits</th><th>Lines introduced</th><th>Still at HEAD</th><th>Line-weighted</th></tr></thead>
        <tbody>
{agent_rows(f['by_agent'])}
        </tbody>
      </table>
      </div>
    </div>"""
    excluded_items = [
        f"<li><strong>Shallow clones</strong> (history truncated; method {esc(m['version'])} refuses them): {esc(', '.join(ex['shallow'])) if ex['shallow'] else 'none'}.</li>",
        f"<li><strong>Audits that failed</strong> in this run: {esc(', '.join(ex['failed'])) if ex['failed'] else 'none'}.</li>",
    ]
    if ex["not_method_v2"]:
        excluded_items.append(f"<li><strong>Outputs not in method v2 format:</strong> {esc(', '.join(ex['not_method_v2']))}.</li>")
    excluded_items.append(
        f'<li><strong>Opted out</strong> by their maintainers: {ex["opted_out"]}. One line in '
        f'<a href="{REPO_URL}/edit/main/.github/survival-optout.txt" rel="noopener"><code translate="no">.github/survival-optout.txt</code></a> '
        "removes a repository from the next report, no questions asked.</li>"
    )
    body = f"""
<section class="section">
  <div class="container">
    <div class="section-head">
      <p class="eyebrow">survival report #{f['number']} · {esc(f['date'])} · method {esc(m['version'])} · {esc(f['tool']['name'])} {esc(f['tool']['version'])} · unranked</p>
      <h1>Survival Report #{f['number']}</h1>
      <p class="lede">{esc(headline(f))} <strong>These are counts, not grades.</strong> There is no rank, no colour and no verdict on this page; rows are alphabetical. Every number links to the audit bytes behind it and the <a href="/method">method and its limits</a> are public.</p>
      <p class="rp-meta">{doi_html(f)} · <a href="report.json">report.json</a> · <a href="report.md">report.md</a> · <a href="card.png">card</a> · <a href="/{REPORTS_REL}/feed.xml">Atom feed</a> · <a href="/{REPORTS_REL}/">all reports</a></p>
    </div>
{strip}
    <img class="rp-card" src="card.png" alt="Survival Report #{f['number']} card" width="1200" height="630" loading="lazy" />

    <h3 id="repositories">Repositories</h3>
    <p class="muted small">Alphabetical. VERIFIED commits only; PROBABLE counts are shown but never summed. <em>Capped</em>: no commit weighs more than the cap. <em>Median per commit</em>: the middle commit's own ratio. <em>Largest commit</em>: share of introduced lines from the single largest commit.</p>
    <div class="lb-scroll">
      <div class="tbl-scroll">
      <table class="lb-table" id="repos">
        <thead><tr><th>Repository</th><th>Commits</th><th>AI-tagged</th><th>Lines introduced</th><th>Still at HEAD</th><th>Line-weighted</th><th>Capped</th><th>Median per commit</th><th>Largest commit</th><th>Reproduce</th></tr></thead>
        <tbody>
{repo_rows(f['repositories'], full=True)}
        </tbody>
      </table>
      </div>
    </div>
{not_agg}
{agents}
    <h3 id="excluded">Excluded from this report</h3>
    <ul class="rp-list">
      {''.join(excluded_items)}
    </ul>

    <div class="lb-limits" id="method">
      <h3>Method</h3>
      <ul>
        <li><strong>Method {esc(m['version'])}</strong>, {esc(f['tool']['name'])} {esc(f['tool']['version'])}. Detection from commit metadata only; no model, no guess from the diff. Full text and known artefacts at <a href="/method">causari.dev/method</a>.</li>
        <li><strong>Survival</strong>: <code translate="no">git blame {esc(' '.join(m['blame_flags']))}</code> at HEAD, honouring <code translate="no">.git-blame-ignore-revs</code> where present; a line counts for the commit blame attributes it to, capped at that commit's introduced count.</li>
        <li><strong>Cap rule</strong>: {esc(m['cap_rule'])}. The capped ratio is what one bulk commit cannot dominate.</li>
        <li><strong>Sample floor</strong>: {m['sample_floor']} VERIFIED commits. Below it a repository is measured but not aggregated.</li>
        <li><strong>Intervals</strong>: {esc(a['interval_method']['note'])}</li>
        <li><strong>Full clones only</strong>: method {esc(m['version'])} refuses shallow clones; the workflow clones each repository completely before measuring.</li>
        <li><strong>Reproduce or contest</strong>: <code translate="no">{esc(m['command'])}</code> gives the exact bytes behind a row; the bytes of this run are under <code translate="no">repos/</code> next to this page. Open an issue with your JSON if it differs.</li>
      </ul>
    </div>

    <p class="rp-cite">Cite as: <code translate="no">{esc(cite(f))}</code></p>
  </div>
</section>
"""
    return page_head(title, desc, url, img, jsonld) + body + page_foot(positioning_html())


def render_index(archive: list[dict[str, Any]]) -> str:
    url = f"{SITE_URL}/{REPORTS_REL}/"
    title = "Survival Report · weekly"
    latest = archive[0] if archive else None
    desc = ("Weekly, numbered, citable: how many lines from AI-tagged commits are still at HEAD in open-source repositories. "
            "Counts and intervals, no ranks, no verdicts. Every number reproducible with re audit <owner/repo> --json.")
    jsonld = {"@context": "https://schema.org", "@type": "CollectionPage", "name": title, "url": url, "description": desc,
              "publisher": {"@type": "Organization", "name": "Crovia Trust", "url": "https://croviatrust.com"},
              "hasPart": [{"@type": "Report", "name": f"Survival Report #{a['number']}", "url": a["url"], "datePublished": a["date"]} for a in archive[:52]]}
    rows = "\n".join(
        f'<tr><td><a href="/{REPORTS_REL}/{esc(a["id"])}/">Survival Report #{a["number"]}</a></td><td>{esc(a["date"])}</td>'
        f'<td>{a["aggregate"]["repositories"]}</td><td>{fmt_int(a["aggregate"]["ai_tagged_commits"])}</td>'
        f'<td>{fmt_int(a["aggregate"]["introduced"])}</td><td>{fmt_int(a["aggregate"]["surviving"])}</td>'
        f'<td>{fmt_pct(a["aggregate"]["survival_rate"])}</td><td>{esc(a["method"]["version"])}</td>'
        f'<td>{("<a href=\"https://doi.org/" + esc(a["doi"]) + "\" rel=\"noopener\">" + esc(a["doi"]) + "</a>") if a.get("doi") else "<span class=\"muted\">pending</span>"}</td></tr>'
        for a in archive
    )
    latest_block = ""
    if latest:
        latest_block = f"""
    <p class="rp-meta">Latest: <a href="/{REPORTS_REL}/{esc(latest['id'])}/">Survival Report #{latest['number']}</a> · {esc(latest['date'])} — {esc(headline(latest))}</p>
    <a href="/{REPORTS_REL}/{esc(latest['id'])}/"><img class="rp-card" src="/{REPORTS_REL}/{esc(latest['id'])}/card.png" alt="Survival Report #{latest['number']} card" width="1200" height="630" /></a>"""
    body = f"""
<section class="section">
  <div class="container">
    <div class="section-head">
      <p class="eyebrow">measured weekly · git metadata only · unranked · atom feed</p>
      <h1>Survival Report</h1>
      <p class="lede">Every week, one numbered report: for each measured open-source repository, how many lines were introduced by commits that carry machine-readable AI authorship metadata, and how many of those lines <code translate="no">git blame</code> still attributes to them at HEAD. <strong>These are counts, not grades.</strong> No rank, no colour, no verdict; rows are alphabetical; intervals describe the sampled repositories only. Every number is reproducible with one command and the <a href="/method">method and its limits</a> are public.</p>
      <p class="rp-meta"><a href="/{REPORTS_REL}/feed.xml">Atom feed</a> · <a href="/{REPORTS_REL}/latest.json">latest.json</a> · <a href="{REPO_URL}/blob/main/docs/survival-report.md" rel="noopener">how it is made</a> · <a href="{REPO_URL}/edit/main/.github/survival-optout.txt" rel="noopener">opt out</a> (<code translate="no">.github/survival-optout.txt</code>)</p>
    </div>
{latest_block}
    <h3 id="archive">All reports</h3>
    <div class="lb-scroll">
      <div class="tbl-scroll">
      <table class="lb-table" id="archive-table">
        <thead><tr><th>Report</th><th>Date</th><th>Repositories</th><th>AI-tagged commits</th><th>Lines introduced</th><th>Still at HEAD</th><th>Line-weighted</th><th>Method</th><th>DOI</th></tr></thead>
        <tbody>
{rows}
        </tbody>
      </table>
      </div>
    </div>
    <p class="muted small">The report replaced the weekly measurements table in September 2026. Repositories are added by pull request to <a href="{REPO_URL}/blob/main/.github/survival-repos.txt" rel="noopener"><code translate="no">.github/survival-repos.txt</code></a>; maintainers opt out with one line in <a href="{REPO_URL}/edit/main/.github/survival-optout.txt" rel="noopener"><code translate="no">.github/survival-optout.txt</code></a>.</p>
  </div>
</section>
"""
    image = f"{SITE_URL}/{REPORTS_REL}/{latest['id']}/card.png" if latest else f"{SITE_URL}/assets/og.png"
    return page_head(title, desc, url, image, jsonld) + body + page_foot(positioning_html())


def render_feed(archive: list[dict[str, Any]]) -> str:
    feed_url = f"{SITE_URL}/{REPORTS_REL}/feed.xml"
    updated = archive[0]["generated_at"] if archive else dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    entries = []
    for a in archive[:104]:
        summary = headline(a) + " Counts, not grades. " + LICENSE + "."
        if a.get("doi"):
            summary += f" DOI {a['doi']}."
        entries.append(f"""  <entry>
    <title>Survival Report #{a['number']} · {esc(a['date'])}</title>
    <link rel="alternate" type="text/html" href="{esc(a['url'])}"/>
    <link rel="enclosure" type="image/png" href="{esc(a['url'])}card.png"/>
    <link rel="related" type="application/json" href="{esc(a['url'])}report.json"/>
    <id>{esc(a['url'])}</id>
    <published>{esc(a['date'])}T00:00:00Z</published>
    <updated>{esc(a['generated_at'])}</updated>
    <summary>{esc(summary)}</summary>
  </entry>""")
    return f"""<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>causari — Survival Report</title>
  <subtitle>Weekly counts of surviving lines from AI-tagged commits in open-source repositories. No ranks, no verdicts. {LICENSE}.</subtitle>
  <link rel="self" type="application/atom+xml" href="{feed_url}"/>
  <link rel="alternate" type="text/html" href="{SITE_URL}/{REPORTS_REL}/"/>
  <id>{SITE_URL}/{REPORTS_REL}/</id>
  <updated>{esc(updated)}</updated>
  <author><name>Crovia Trust</name><uri>https://croviatrust.com</uri></author>
  <rights>{LICENSE} Crovia Trust</rights>
  <icon>{SITE_URL}/assets/favicon.svg</icon>
{chr(10).join(entries)}
</feed>
"""


# ----------------------------------------------------------------------------- site glue

def replace_block(text: str, begin: str, end: str, body: str, insert_before: str | None) -> str:
    block = f"{begin}\n{body}\n{end}"
    pattern = re.compile(re.escape(begin) + r".*?" + re.escape(end), re.S)
    if pattern.search(text):
        return pattern.sub(lambda _: block, text)
    if insert_before and insert_before in text:
        return text.replace(insert_before, block + "\n" + insert_before, 1)
    return text.rstrip("\n") + "\n\n" + block + "\n"


def update_redirects(site: Path, latest: dict[str, Any] | None) -> None:
    path = site / "_redirects"
    text = path.read_text(encoding="utf-8") if path.exists() else ""
    lines = [
        f"/survival             /{REPORTS_REL}/                 301",
        f"/survival-data.json   /{REPORTS_REL}/latest.json      301",
    ]
    target = f"/{REPORTS_REL}/{latest['id']}/" if latest else f"/{REPORTS_REL}/"
    lines.append(f"/report               {target}   302")
    lines.append(f"/report/latest        {target}   302")
    path.write_text(replace_block(text, REDIRECT_BEGIN, REDIRECT_END, "\n".join(lines), None), encoding="utf-8")


def update_sitemap(site: Path, archive: list[dict[str, Any]]) -> None:
    path = site / "sitemap.xml"
    text = path.read_text(encoding="utf-8") if path.exists() else (
        '<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n</urlset>\n')
    urls = [f"  <url><loc>{SITE_URL}/{REPORTS_REL}/</loc><changefreq>weekly</changefreq><priority>0.8</priority></url>"]
    for a in archive:
        urls.append(f"  <url><loc>{esc(a['url'])}</loc><lastmod>{esc(a['date'])}</lastmod><changefreq>yearly</changefreq><priority>0.6</priority></url>")
    path.write_text(replace_block(text, "  " + SITEMAP_BEGIN, "  " + SITEMAP_END, "\n".join(urls), "</urlset>"), encoding="utf-8")


def load_archive(site: Path) -> list[dict[str, Any]]:
    out = []
    for fp in glob.glob(str(site / REPORTS_REL / "[0-9][0-9][0-9][0-9]" / "*" / "report.json")):
        a = load_json(Path(fp), None)
        if isinstance(a, dict) and a.get("schema") == SCHEMA:
            out.append(a)
    out.sort(key=lambda a: (a["number"], a["date"]), reverse=True)
    return out


def next_number(site: Path) -> int:
    dirs = [d for d in glob.glob(str(site / REPORTS_REL / "[0-9][0-9][0-9][0-9]" / "*")) if Path(d).is_dir() and Path(d).name.isdigit()]
    return len(dirs) + 1


def rebuild(site: Path) -> list[dict[str, Any]]:
    archive = load_archive(site)
    base = site / REPORTS_REL
    base.mkdir(parents=True, exist_ok=True)
    (base / "index.html").write_text(render_index(archive), encoding="utf-8")
    (base / "feed.xml").write_text(render_feed(archive), encoding="utf-8")
    if archive:
        dump_json(base / "latest.json", archive[0])
    update_redirects(site, archive[0] if archive else None)
    update_sitemap(site, archive)
    return archive


def write_report(f: dict[str, Any], site: Path, png: bool = True) -> Path:
    out = site / REPORTS_REL / f["id"]
    if out.exists() and (out / "report.json").exists():
        existing = load_json(out / "report.json", {})
        if existing.get("number") != f["number"]:
            raise SystemExit(f"{out} already holds report #{existing.get('number')}; refusing to overwrite")
    out.mkdir(parents=True, exist_ok=True)
    sources = f.pop("_sources", {})
    (out / "repos").mkdir(exist_ok=True)
    for repo, src in sources.items():
        shutil.copyfile(src, out / "repos" / f"{repo_slug(repo)}.json")
    dump_json(out / "report.json", f)
    (out / "report.md").write_text(report_md(f), encoding="utf-8")
    (out / "card.svg").write_text(card_svg(f), encoding="utf-8")
    if png:
        card_png(f, out / "card.png")
    (out / "index.html").write_text(render_report(f), encoding="utf-8")
    return out


def rerender(report_dir: Path) -> None:
    """Re-render page and markdown from an edited report.json (e.g. after a DOI was written)."""
    f = load_json(report_dir / "report.json", None)
    if not isinstance(f, dict):
        raise SystemExit(f"{report_dir}/report.json missing")
    (report_dir / "index.html").write_text(render_report(f), encoding="utf-8")
    (report_dir / "report.md").write_text(report_md(f), encoding="utf-8")


# ----------------------------------------------------------------------------- converter

def from_existing(src: Path, out: Path) -> int:
    """Turn the retired site/survival-data.json into a run directory, if and
    only if its rows carry the method v2 fields. Method v1 rows (no coverage,
    no capped rate, blame without -w -M -C) cannot be relabelled as v2 and are
    refused: the first report must come from a fresh run instead."""
    data = load_json(src, None)
    if not isinstance(data, dict) or not isinstance(data.get("rows"), list):
        print(f"from-existing: {src} is not a survival-data.json", file=sys.stderr)
        return 2
    rows = data["rows"]
    missing = [r.get("repo") for r in rows if not is_v2(r)]
    if missing:
        print(f"from-existing: {len(missing)} of {len(rows)} rows lack method v2 fields (coverage/small_sample/shallow): "
              f"{', '.join(map(str, missing[:5]))}{'…' if len(missing) > 5 else ''}", file=sys.stderr)
        print("from-existing: refusing to relabel v1 data; generate Report #1 from a fresh run (see docs/survival-report.md)", file=sys.stderr)
        return 3
    out.mkdir(parents=True, exist_ok=True)
    for r in rows:
        repo = r["repo"]
        audit = {k: v for k, v in r.items() if k not in ("repo", "audited_at")}
        dump_json(out / f"{repo_slug(repo)}.json", audit)
    dump_json(out / "run.json", {
        "generated_at": data.get("generated_at"), "tool": data.get("tool") or "causari", "tool_version": data.get("tool_version") or "unknown",
        "method": data.get("method") or "v2", "command": data.get("command") or "re audit <owner/repo> --json",
        "repos": [r["repo"] for r in rows], "failed": [], "opted_out": [],
    })
    print(f"from-existing: wrote {len(rows)} audits to {out}")
    return 0


# ----------------------------------------------------------------------------- main

def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--site", default=str(ROOT / "site"), help="site root (default: <repo>/site)")
    ap.add_argument("--root", default=str(ROOT), help="repository root, for .github/survival-optout.txt")
    sub = ap.add_subparsers(dest="cmd", required=True)
    b = sub.add_parser("build", help="build one report from a run directory, then rebuild archive, feed, latest, redirects")
    b.add_argument("--run", required=True, help="directory with run.json and <owner>__<repo>.json")
    b.add_argument("--number", type=int, help="report number (default: existing report dirs + 1)")
    b.add_argument("--date", help="report date YYYY-MM-DD (default: run.json generated_at, else today UTC)")
    b.add_argument("--no-png", action="store_true", help="skip card.png even if Pillow is present")
    sub.add_parser("rebuild", help="archive index, feed, latest.json, redirects, sitemap from existing report.json files")
    rr = sub.add_parser("rerender", help="re-render one report's page and markdown from its report.json")
    rr.add_argument("report_dir")
    fe = sub.add_parser("from-existing", help="convert the retired survival-data.json into a run directory (v2 rows only)")
    fe.add_argument("src")
    fe.add_argument("--out", required=True)
    args = ap.parse_args(argv)

    site = Path(args.site)
    root = Path(args.root)
    if args.cmd == "from-existing":
        return from_existing(Path(args.src), Path(args.out))
    if args.cmd == "rerender":
        rerender(Path(args.report_dir))
        rebuild(site)
        return 0
    if args.cmd == "rebuild":
        archive = rebuild(site)
        print(f"survival_report: rebuilt archive with {len(archive)} report(s)")
        return 0

    run_dir = Path(args.run)
    number = args.number or next_number(site)
    run = load_json(run_dir / "run.json", {}) or {}
    date = args.date or (run.get("generated_at") or dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"))[:10]
    facts = collect(run_dir, number, date, root)
    out = write_report(facts, site, png=not args.no_png)
    archive = rebuild(site)
    a = facts["aggregate"]
    print(f"survival_report: #{number} {date} → {out} · {a['repositories']} repositories aggregated, "
          f"{len(facts['not_aggregated'])} measured only, {len(facts['excluded']['shallow'])} shallow, "
          f"{len(facts['excluded']['failed'])} failed · {fmt_int(a['surviving'])}/{fmt_int(a['introduced'])} lines "
          f"({fmt_pct(a['survival_rate'])}) · archive {len(archive)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
