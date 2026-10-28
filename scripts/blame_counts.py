#!/usr/bin/env python3
"""Per-file, per-commit line counts from `git blame` at HEAD.

The release check compares aggregate counts. When one differs between two
machines, the aggregate cannot say which file or which commit moved. This
script writes the raw material the aggregate is made of, with the same blame
flags the audit uses, so two runs' artifacts can be diffed down to a path.

    python3 scripts/blame_counts.py <repo> <out.json>

Output: {"git": "<git --version>", "flags": [...], "files": {path: {"lines":
{commit: n}} | {"error": "<stderr>"}}}. Blame errors are recorded, not hidden,
because a file the binary silently skipped is one explanation for a lower count.
"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

BLAME_FLAGS = ["-w", "-M", "-C"]


def count_owners(porcelain: str) -> dict[str, int]:
    """Count lines per owning commit in `--line-porcelain` output.

    A record starts with "<40-hex> <orig> <final> [<n>]" and ends with the
    content line, which begins with a tab. Metadata lines in between are not
    headers even when a value happens to be forty hex characters.
    """
    counts: dict[str, int] = {}
    expect_header = True
    for line in porcelain.splitlines():
        if expect_header:
            head = line.split(" ", 1)[0]
            if len(head) == 40 and all(c in "0123456789abcdef" for c in head):
                counts[head] = counts.get(head, 0) + 1
                expect_header = False
        elif line.startswith("\t"):
            expect_header = True
    return counts


def tracked_paths(repo: Path) -> list[str]:
    raw = subprocess.run(
        ["git", "ls-files", "-z"], cwd=repo, check=True, capture_output=True
    ).stdout
    return [p.decode("utf-8", "surrogateescape") for p in raw.split(b"\0") if p]


def blame_file(repo: Path, path: str) -> dict:
    proc = subprocess.run(
        ["git", "blame", *BLAME_FLAGS, "--line-porcelain", "HEAD", "--", path],
        cwd=repo,
        capture_output=True,
    )
    if proc.returncode != 0:
        return {"error": proc.stderr.decode("utf-8", "replace").strip()}
    return {"lines": count_owners(proc.stdout.decode("utf-8", "replace"))}


def main(argv: list[str]) -> int:
    if len(argv) != 3:
        print(__doc__.strip().splitlines()[0], file=sys.stderr)
        return 2
    repo, out = Path(argv[1]), Path(argv[2])
    git_version = subprocess.run(
        ["git", "--version"], check=True, capture_output=True, text=True
    ).stdout.strip()
    files = {p: blame_file(repo, p) for p in tracked_paths(repo)}
    total = sum(sum(f["lines"].values()) for f in files.values() if "lines" in f)
    errors = sum(1 for f in files.values() if "error" in f)
    out.write_text(
        json.dumps(
            {"git": git_version, "flags": BLAME_FLAGS, "files": files},
            indent=1,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    print(f"blame counts: {len(files)} files, {total} lines, {errors} blame errors -> {out}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
