#!/usr/bin/env python3
"""Version the site's shared assets by content.

causari.dev is zero-build. `/styles.css` and `/app.js` are shared by every
page and cached by browsers for a day; a page that needs a class added
today would otherwise render unstyled for anyone who visited yesterday.
Every page therefore references them with the SHA-256 prefix of their
current bytes as a query string, and the assets themselves are cached as
immutable: a new stylesheet is a new URL.

    python3 scripts/site_version.py           # rewrite every page in site/
    python3 scripts/site_version.py --check   # exit 1 if any page is stale

`scripts/survival_report.py` imports `asset_url` so generated report pages
carry the same versions. `scripts/audit_surfaces.py` runs `--check`.
"""

from __future__ import annotations

import argparse
import hashlib
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SITE = ROOT / "site"
ASSETS = ("styles.css", "app.js")


def asset_version(name: str, site: Path = SITE) -> str:
    return hashlib.sha256((site / name).read_bytes()).hexdigest()[:10]


def asset_url(name: str, site: Path = SITE) -> str:
    return f"/{name}?v={asset_version(name, site)}"


def _pattern(name: str) -> re.Pattern[str]:
    # href="/styles.css" or href="/styles.css?v=…", same for src=.
    return re.compile(r'((?:href|src)=")/' + re.escape(name) + r'(?:\?v=[0-9a-f]+)?(")')


def pages(site: Path = SITE) -> list[Path]:
    return sorted(p for p in site.rglob("*.html") if ".git" not in p.parts)


def render(text: str, site: Path = SITE) -> str:
    for name in ASSETS:
        text = _pattern(name).sub(lambda m, n=name: m.group(1) + asset_url(n, site) + m.group(2), text)
    return text


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--check", action="store_true", help="report stale pages, change nothing")
    ap.add_argument("--site", default=str(SITE))
    args = ap.parse_args(argv)
    site = Path(args.site)
    stale: list[Path] = []
    for page in pages(site):
        before = page.read_text(encoding="utf-8")
        after = render(before, site)
        if before != after:
            stale.append(page)
            if not args.check:
                page.write_text(after, encoding="utf-8")
    versions = " ".join(f"{n}?v={asset_version(n, site)}" for n in ASSETS)
    if args.check:
        for p in stale:
            print(f"stale asset version: {p.relative_to(site.parent)}", file=sys.stderr)
        if stale:
            print(f"current: {versions}", file=sys.stderr)
            return 1
        print(f"site assets current: {versions}")
        return 0
    print(f"{len(stale)} page(s) updated · {versions}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
