#!/usr/bin/env python3
"""Compare one `re audit --json` output with the recorded expectation for the
same repository, commit and method.

    python3 scripts/release_check.py audit.json tests/fixtures/release-check

The fixture is chosen by the `method` the binary reports and the `repository.head`
it audited. No fixture for that pair is a failure with a message naming what to
record, not a pass: a new method needs new numbers before the check means anything.

Exit 0 when every expected field matches, 1 otherwise. A Markdown table goes to
stdout (and to $GITHUB_STEP_SUMMARY when set)."""
from __future__ import annotations

import json
import os
import sys
from pathlib import Path


def dig(obj, dotted: str):
    cur = obj
    for part in dotted.split("."):
        if not isinstance(cur, dict) or part not in cur:
            return ("<absent>",)
        cur = cur[part]
    return cur


def find_fixture(fixtures: Path, method: str, head: str) -> Path | None:
    for p in sorted(fixtures.glob("*.json")):
        fx = json.loads(p.read_text(encoding="utf-8"))
        if fx.get("schema") != "causari.release_check.v1":
            continue
        if fx.get("method") == method and fx.get("commit") == head:
            return p
    return None


def compare(audit: dict, fixture: dict) -> list[tuple[str, object, object, bool]]:
    rows = []
    for key, want in fixture["expect"].items():
        got = dig(audit, key)
        rows.append((key, want, got, got == want))
    return rows


def render(rows, fixture: dict, audit: dict, binary: str) -> str:
    ok = all(r[3] for r in rows)
    lines = [
        f"## Release check: {fixture['repository']} @ {fixture['commit'][:7]} — "
        + ("every field reproduced" if ok else "MISMATCH"),
        "",
        f"Binary: {binary}. Method reported: {audit.get('method')}. "
        f"Fixture recorded with causari {fixture['recorded_with']['causari']} "
        f"on {fixture['recorded_with']['os']}, git {fixture['recorded_with']['git']}, "
        f"{fixture['recorded_with']['date']}.",
        "",
        "| Field | Expected | Got | |",
        "|---|---:|---:|---|",
    ]
    for key, want, got, same in rows:
        got_s = "absent" if got == ("<absent>",) else json.dumps(got)
        lines.append(f"| `{key}` | `{json.dumps(want)}` | `{got_s}` | {'ok' if same else 'differs'} |")
    lines.append("")
    lines.append(
        "A differing count is a finding about reproducibility, not a grade of the repository. "
        "The usual causes are a different git version's blame heuristics, a fetched "
        "`refs/notes/ai`, or a binary that is not the recorded release."
    )
    return "\n".join(lines)


def main(argv: list[str]) -> int:
    if len(argv) != 3:
        print(__doc__, file=sys.stderr)
        return 2
    audit_path, fixtures = Path(argv[1]), Path(argv[2])
    audit = json.loads(audit_path.read_text(encoding="utf-8"))
    method = audit.get("method") or (audit.get("coverage") or {}).get("method")
    head = (audit.get("repository") or {}).get("head", "")
    binary = os.environ.get("RELEASE_CHECK_BINARY", "unknown")
    fx_path = find_fixture(fixtures, str(method), head)
    if fx_path is None:
        msg = (
            f"::error title=No release-check fixture::no fixture in {fixtures} for method {method!r} "
            f"at commit {head[:7] or '?'}. Record one with the release that computes this method "
            f"before trusting the check; a missing fixture is not a pass."
        )
        print(msg)
        return 1
    fixture = json.loads(fx_path.read_text(encoding="utf-8"))
    rows = compare(audit, fixture)
    out = render(rows, fixture, audit, binary)
    print(out)
    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary:
        try:
            with open(summary, "a", encoding="utf-8") as fh:
                fh.write(out + "\n")
        except OSError as e:
            print(f"::warning::could not append to GITHUB_STEP_SUMMARY: {e}")
    return 0 if all(r[3] for r in rows) else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
