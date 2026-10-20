#!/usr/bin/env python3
"""Build and publish the Survival Report as a Hugging Face dataset.

    survival_dataset.py build [--reports site/reports/survival] OUT_DIR
    survival_dataset.py push  [--repo CroviaResearch/causari-survival] [--dry-run] OUT_DIR

`build` is deterministic: the same report tree produces the same bytes. It
writes three JSONL tables under `data/`, copies the published report bytes
unchanged under `raw/`, and renders the dataset card from
`docs/survival-dataset.md`. `push` uploads the folder with `huggingface_hub`
and needs `HF_TOKEN` (or a cached login); without `--dry-run` nothing else
is contacted.
"""
from __future__ import annotations

import argparse
import json
import os
import shutil
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_REPORTS = ROOT / "site" / "reports" / "survival"
CARD_TEMPLATE = ROOT / "docs" / "survival-dataset.md"
DEFAULT_REPO = "CroviaResearch/causari-survival"
SCHEMA = "causari.survival_dataset.v1"

CARD_FRONT_MATTER = """---
license: cc-by-4.0
pretty_name: Causari Survival Report
language:
- en
tags:
- code
- software-engineering
- git
- ai-generated-code
- provenance
- ai-code-survival
- crovia
- causari
size_categories:
- n<1K
configs:
- config_name: repositories
  default: true
  data_files: data/repositories.jsonl
- config_name: reports
  data_files: data/reports.jsonl
- config_name: agents
  data_files: data/agents.jsonl
---
"""


def _load(path: Path) -> dict:
    with path.open("rb") as fh:
        return json.loads(fh.read().decode("utf-8"))


def _dump(obj) -> str:
    return json.dumps(obj, sort_keys=True, ensure_ascii=False, separators=(",", ":"))


def report_dirs(reports_root: Path) -> list[Path]:
    """Every published report directory, oldest first, by report id."""
    found = [p.parent for p in reports_root.glob("*/*/report.json")]
    return sorted(found, key=lambda d: _load(d / "report.json")["number"])


def _stats(block: dict | None, prefix: str) -> dict:
    block = block or {}
    out = {}
    for key in (
        "commits",
        "introduced",
        "surviving",
        "survival_rate",
        "capped_survival_rate",
        "median_survival",
        "largest_commit_share",
    ):
        out[f"{prefix}_{key}"] = block.get(key)
    return out


def repository_row(report: dict, row: dict, aggregated: bool, raw_rel: str) -> dict:
    baseline = row.get("baseline") or {}
    gap = baseline.get("age_matched") or {}
    oldest = baseline.get("oldest_surviving") or {}
    coverage = row.get("coverage") or {}
    out = {
        "report_number": report["number"],
        "report_id": report["id"],
        "report_date": report["date"],
        "method_version": coverage.get("method") or report["method"]["version"],
        "repo": row["repo"],
        "url": row.get("url"),
        "aggregated": aggregated,
        "total_commits": row.get("total_commits"),
    }
    out.update(_stats(row.get("verified"), "verified"))
    out.update(_stats(row.get("probable"), "probable"))
    out.update(_stats(baseline.get("untagged"), "untagged"))
    out.update(
        {
            "age_matched_gap": gap.get("gap"),
            "age_matched_tagged_rate": gap.get("tagged_rate"),
            "age_matched_untagged_rate": gap.get("untagged_rate"),
            "oldest_surviving_date": oldest.get("date"),
            "oldest_surviving_commits_before": oldest.get("commits_before"),
            "coverage_shallow": coverage.get("shallow"),
            "coverage_small_sample": coverage.get("small_sample"),
            "coverage_blame_flags": " ".join(coverage.get("blame_flags") or []),
            "by_agent_json": _dump(row.get("by_agent") or {}),
            "by_age_json": _dump(baseline.get("by_age") or []),
            "reproduce": row.get("reproduce"),
            "raw_audit_json": raw_rel,
        }
    )
    return out


def report_row(report: dict) -> dict:
    agg = report.get("aggregate") or {}
    method = report.get("method") or {}
    return {
        "schema": report.get("schema"),
        "number": report["number"],
        "id": report["id"],
        "title": report.get("title"),
        "date": report["date"],
        "generated_at": report.get("generated_at"),
        "url": report.get("url"),
        "doi": report.get("doi"),
        "concept_doi": report.get("concept_doi"),
        "license": report.get("license"),
        "tool_version": (report.get("tool") or {}).get("version"),
        "method_version": method.get("version"),
        "method_url": method.get("url"),
        "method_blame_flags": " ".join(method.get("blame_flags") or []),
        "method_sample_floor": method.get("sample_floor"),
        "selection": method.get("selection"),
        "repositories_aggregated": agg.get("repositories"),
        "repositories_not_aggregated": len(report.get("not_aggregated") or []),
        "total_commits": agg.get("total_commits"),
        "ai_tagged_commits": agg.get("ai_tagged_commits"),
        "introduced": agg.get("introduced"),
        "surviving": agg.get("surviving"),
        "survival_rate": agg.get("survival_rate"),
        "survival_rate_interval_95_json": _dump(agg.get("survival_rate_interval_95")),
        "median_capped_survival_rate": agg.get("median_capped_survival_rate"),
        "median_capped_survival_rate_interval_95_json": _dump(
            agg.get("median_capped_survival_rate_interval_95")
        ),
        "interval_method": agg.get("interval_method"),
        "baseline_json": _dump(agg.get("baseline")),
        "by_agent_json": _dump(report.get("by_agent") or {}),
        "raw_report_json": f"raw/{report['id']}/report.json",
    }


def agent_rows(report: dict) -> list[dict]:
    rows = []
    for agent, block in sorted((report.get("by_agent") or {}).items()):
        rows.append(
            {
                "report_number": report["number"],
                "report_id": report["id"],
                "report_date": report["date"],
                "method_version": report["method"]["version"],
                "agent": agent,
                "repositories": block.get("repositories"),
                "commits": block.get("commits"),
                "introduced": block.get("introduced"),
                "surviving": block.get("surviving"),
                "survival_rate": block.get("survival_rate"),
            }
        )
    return rows


def render_card(reports: list[dict]) -> str:
    first, last = reports[0], reports[-1]
    aggregated = len(last.get("repositories") or [])
    not_aggregated = len(last.get("not_aggregated") or [])
    doi = last.get("doi")
    doi_note = f" · DOI [{doi}](https://doi.org/{doi})" if doi else ""
    body = CARD_TEMPLATE.read_text(encoding="utf-8")
    body = body.format(
        reports=len(reports),
        first_number=first["number"],
        last_number=last["number"],
        first_date=first["date"],
        last_date=last["date"],
        latest_repositories=aggregated + not_aggregated,
        latest_aggregated=aggregated,
        latest_not_aggregated=not_aggregated,
        latest_title=last.get("title") or f"Survival Report #{last['number']}",
        latest_url=last.get("url") or "https://causari.dev/reports/survival/",
        latest_doi_note=doi_note,
    )
    return CARD_FRONT_MATTER + body


def build(reports_root: Path, out: Path) -> dict:
    dirs = report_dirs(reports_root)
    if not dirs:
        raise SystemExit(f"no report.json under {reports_root}")
    if out.exists():
        shutil.rmtree(out)
    (out / "data").mkdir(parents=True)

    reports, repo_rows, agents = [], [], []
    for d in dirs:
        report = _load(d / "report.json")
        reports.append(report)
        raw_dir = out / "raw" / report["id"]
        (raw_dir / "repos").mkdir(parents=True)
        shutil.copyfile(d / "report.json", raw_dir / "report.json")
        for src in sorted((d / "repos").glob("*.json")):
            shutil.copyfile(src, raw_dir / "repos" / src.name)
        for row in report.get("repositories") or []:
            rel = f"raw/{report['id']}/{row.get('audit_file', '')}"
            repo_rows.append(repository_row(report, row, True, rel))
        for row in report.get("not_aggregated") or []:
            rel = f"raw/{report['id']}/{row.get('audit_file', '')}"
            repo_rows.append(repository_row(report, row, False, rel))
        agents.extend(agent_rows(report))

    repo_rows.sort(key=lambda r: (r["report_number"], r["repo"].lower()))
    agents.sort(key=lambda r: (r["report_number"], r["agent"]))

    def write_jsonl(name: str, rows: list[dict]) -> None:
        with (out / "data" / name).open("w", encoding="utf-8", newline="\n") as fh:
            for row in rows:
                fh.write(_dump(row) + "\n")

    write_jsonl("reports.jsonl", [report_row(r) for r in reports])
    write_jsonl("repositories.jsonl", repo_rows)
    write_jsonl("agents.jsonl", agents)
    (out / "README.md").write_text(render_card(reports), encoding="utf-8", newline="\n")
    manifest = {
        "schema": SCHEMA,
        "reports": [r["id"] for r in reports],
        "latest": reports[-1]["id"],
        "rows": {
            "reports": len(reports),
            "repositories": len(repo_rows),
            "agents": len(agents),
        },
        "source": "https://causari.dev/reports/survival/",
        "method": "https://causari.dev/method",
    }
    (out / "manifest.json").write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8", newline="\n"
    )
    return manifest


def push(out: Path, repo_id: str, dry_run: bool) -> None:
    manifest = _load(out / "manifest.json")
    latest = _load(out / "raw" / manifest["latest"] / "report.json")
    message = f"{latest.get('title', 'Survival Report')} ({latest['date']}, method {latest['method']['version']})"
    if dry_run:
        print(json.dumps({"repo": repo_id, "commit_message": message, **manifest}, indent=2))
        return
    from huggingface_hub import HfApi

    api = HfApi(token=os.environ.get("HF_TOKEN") or None)
    api.create_repo(repo_id, repo_type="dataset", exist_ok=True)
    info = api.upload_folder(
        folder_path=str(out),
        repo_id=repo_id,
        repo_type="dataset",
        commit_message=message,
        delete_patterns=["data/*", "raw/**", "manifest.json"],
    )
    print(json.dumps({"repo": repo_id, "commit": getattr(info, "oid", None), "message": message}))


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    b = sub.add_parser("build")
    b.add_argument("--reports", type=Path, default=DEFAULT_REPORTS)
    b.add_argument("out", type=Path)
    p = sub.add_parser("push")
    p.add_argument("--repo", default=DEFAULT_REPO)
    p.add_argument("--dry-run", action="store_true")
    p.add_argument("out", type=Path)
    args = ap.parse_args(argv)
    if args.cmd == "build":
        manifest = build(args.reports, args.out)
        print(json.dumps(manifest["rows"]))
        return 0
    push(args.out, args.repo, args.dry_run)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
