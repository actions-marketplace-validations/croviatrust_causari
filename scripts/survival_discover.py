#!/usr/bin/env python3
"""Find the public repositories where AI writes code, for the Survival Report.

The report measures the repositories listed in ``.github/survival-repos.txt``.
This script fills that list from the GitHub commit search API: it looks for
public commits that carry the same trailers and author identities that
``re audit`` classifies as VERIFIED (``src/audit.rs``, method page §1),
counts the matching commits per repository, and keeps the repositories with
the most of them. The selection rule is stated here, written into
``.github/survival-discovery.json`` next to the counts it was made from, and
disclosed on the method page. It selects the sample; it never appears on a
report page, where rows stay alphabetical.

Selection rule
    1. Sample. For every signal below, fetch the most recent matching
       public, non-merge commits (``GET /search/commits``,
       ``sort=author-date``, up to ``--pages`` × ``--per-page`` results; the
       API stops at 1,000). Aggregate by ``repository.full_name``
       (case-insensitive), counting distinct commit SHAs.
    2. Count. A repository seen at least ``--candidate-min`` times in the
       sample is a candidate (at most ``--verify-max`` of them, most
       sampled first). For each candidate and each signal it was seen with,
       ask ``GET /search/commits?q=repo:<owner/repo> <signal>`` for its
       ``total_count``: the number of matching commits in that repository,
       not just in the sample. A repository's count is the sum over its
       signals (a commit carrying two signals counts twice).
    3. Floor. Keep repositories with at least ``--floor`` matching commits
       (the method's sample floor, 5). Drop forks, archived repositories
       and every line of ``.github/survival-optout.txt``.
    4. Order by matching commits, then ``stargazers_count``, then name;
       keep the hand-picked seeds already in the list; fill up to
       ``--limit`` repositories.

Outputs
    .github/survival-repos.txt        header and seeds kept verbatim, then a
                                      ``# discovered <date> …`` line and the
                                      discovered repositories, alphabetical
    .github/survival-discovery.json   per repository: commits found per
                                      signal, sampled commits, stars,
                                      discovered_at, and what was dropped

Usage::

    GITHUB_TOKEN=… python3 scripts/survival_discover.py [--limit 100] [--floor 5] [--pages 10] [--dry-run]

Standard library only. Search endpoints allow 30 requests per minute with a
token (10 without); the script paces itself and sleeps on 403/429 as the
``Retry-After`` / ``X-RateLimit-Reset`` headers say. A full run is a few
hundred search requests, so it takes on the order of half an hour.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Any, Callable

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(HERE))
from survival_report import read_optout  # noqa: E402  same opt-out semantics as the report

API = "https://api.github.com"
SCHEMA = "causari.survival_discovery.v1"
USER_AGENT = "causari-survival-discover/1.0 (+https://causari.dev/method)"
MARKER = "# discovered "
FLOOR = 5
LIMIT = 100
PAGES = 10
PER_PAGE = 100
CANDIDATE_MIN = 2
VERIFY_MAX = 400
SEARCH_PACE_S = 2.1  # 30 requests per minute, with a margin
MAX_WAIT_S = 300

# The signals `re audit` classifies as VERIFIED (src/audit.rs: detect_ai,
# coauthor_agent, coauthor_is_vendor_identity; site/method.html §1), as
# commit-search queries. Free text matches the commit message; `author-*`
# qualifiers match the author identity. A search hit is a candidate, not a
# measurement: the audit re-reads every commit of the clone.
SIGNALS: dict[str, str] = {
    # Co-Authored-By naming a known agent identity
    "co-authored-by:claude": '"Co-Authored-By: Claude"',
    "co-authored-by:copilot": '"Co-Authored-By: Copilot"',
    "co-authored-by:aider": '"Co-Authored-By: aider"',
    "co-authored-by:codex": '"Co-Authored-By: Codex"',
    "co-authored-by:chatgpt": '"Co-Authored-By: ChatGPT"',
    "co-authored-by:openhands": '"Co-Authored-By: openhands"',
    "co-authored-by:cursor-agent": '"Co-Authored-By: Cursor Agent"',
    "co-authored-by:devin-ai-integration": '"Co-Authored-By: devin-ai-integration[bot]"',
    "co-authored-by:google-labs-jules": '"Co-Authored-By: google-labs-jules[bot]"',
    "co-authored-by:gemini-code-assist": '"Co-Authored-By: gemini-code-assist[bot]"',
    # Structured provenance trailers
    "trailer:drafted-with": '"Drafted-With:"',
    "trailer:executed-by": '"Executed-By:"',
    "trailer:ai-model": '"AI-Model:"',
    "trailer:ai-agent": '"AI-Agent:"',
    "trailer:ai-tool": '"AI-Tool:"',
    "trailer:ai-assisted": '"AI-Assisted: yes"',
    "trailer:assisted-by": '"Assisted-by:"',
    # Author identity of a known agent
    "author:noreply@anthropic.com": "author-email:noreply@anthropic.com",
    "author:aider": "author-name:aider",
    "author:copilot-swe-agent": 'author-name:"copilot-swe-agent[bot]"',
    "author:devin-ai-integration": 'author-name:"devin-ai-integration[bot]"',
    "author:openhands": "author-email:openhands@all-hands.dev",
    "author:cursoragent": "author-email:cursoragent@cursor.com",
    "author:google-labs-jules": 'author-name:"google-labs-jules[bot]"',
}
QUALIFIERS = "is:public merge:false"

Fetch = Callable[[str], tuple[int, dict[str, str], bytes]]


# ----------------------------------------------------------------------------- http

def now_utc() -> str:
    return dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def token_from_env() -> str | None:
    return os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN") or None


def urllib_fetch(url: str, token: str | None = None) -> tuple[int, dict[str, str], bytes]:
    headers = {"Accept": "application/vnd.github+json", "User-Agent": USER_AGENT, "X-GitHub-Api-Version": "2022-11-28"}
    if token:
        headers["Authorization"] = f"Bearer {token}"
    req = urllib.request.Request(url, headers=headers)
    try:
        with urllib.request.urlopen(req, timeout=60) as resp:
            return resp.status, {k.lower(): v for k, v in resp.headers.items()}, resp.read()
    except urllib.error.HTTPError as e:
        return e.code, {k.lower(): v for k, v in e.headers.items()}, e.read() if e.fp else b""
    except (urllib.error.URLError, TimeoutError, OSError) as e:
        return 0, {"error": str(e)}, b""


def sample_url(query: str, page: int, per_page: int, until: str) -> str:
    # `author-date:<=today` keeps commits with a clock set in the future from
    # sitting at the top of a newest-first list forever.
    q = urllib.parse.urlencode({"q": f"{query} {QUALIFIERS} author-date:<={until}", "sort": "author-date", "order": "desc",
                                "per_page": per_page, "page": page})
    return f"{API}/search/commits?{q}"


def count_url(full_name: str, query: str) -> str:
    q = urllib.parse.urlencode({"q": f"repo:{full_name} {query} {QUALIFIERS}", "per_page": 1})
    return f"{API}/search/commits?{q}"


def repo_url(full_name: str) -> str:
    return f"{API}/repos/{full_name}"


class Client:
    """GET JSON from the API with pacing, rate-limit sleeps and retries.
    `fetch` and `sleep` are injectable so the tests run without network."""

    def __init__(self, fetch: Fetch, sleep: Callable[[float], None] | None = None, pace_s: float = SEARCH_PACE_S,
                 log: Callable[[str], None] | None = None) -> None:
        self.fetch = fetch
        self.sleep = sleep or (lambda s: time.sleep(s))
        self.pace_s = pace_s
        self.log = log or (lambda s: print(s, file=sys.stderr))
        self.requests = 0
        self.search_requests = 0
        self.waited_s = 0.0

    def wait_for(self, status: int, headers: dict[str, str]) -> float | None:
        """Seconds to sleep before retrying, from Retry-After or X-RateLimit-Reset;
        None when the response is not a rate limit."""
        if status not in (403, 429):
            return None
        remaining = headers.get("x-ratelimit-remaining")
        retry_after = headers.get("retry-after")
        reset = headers.get("x-ratelimit-reset")
        if retry_after and retry_after.strip().isdigit():
            return min(MAX_WAIT_S, max(1.0, float(retry_after)))
        if reset and reset.strip().isdigit() and (remaining is None or remaining.strip() == "0"):
            return min(MAX_WAIT_S, max(1.0, float(reset) - time.time() + 1))
        if remaining is not None and remaining.strip() == "0":
            return 60.0
        return None

    def get_json(self, url: str, pace: bool = False, attempts: int = 4) -> tuple[int, Any]:
        for attempt in range(attempts):
            if pace:
                if self.search_requests:
                    self.sleep(self.pace_s)
                self.search_requests += 1
            self.requests += 1
            status, headers, body = self.fetch(url)
            wait = self.wait_for(status, headers)
            if wait is not None:
                self.log(f"survival_discover: rate limited (HTTP {status}); sleeping {wait:.0f} s")
                self.waited_s += wait
                self.sleep(wait)
                continue
            if status >= 500 or status == 0:
                self.log(f"survival_discover: HTTP {status} for {url.split('?')[0]}; retry {attempt + 1}/{attempts}")
                self.sleep(min(60.0, 5.0 * (attempt + 1)))
                continue
            try:
                return status, json.loads(body.decode("utf-8")) if body else None
            except ValueError:
                return status, None
        return 0, None


# ----------------------------------------------------------------------------- stage 1: sample

def search_signal(client: Client, name: str, query: str, pages: int, per_page: int, until: str) -> tuple[list[dict[str, Any]], bool]:
    """All commit items for one signal, newest first, and whether GitHub
    flagged the result as incomplete or the paging stopped early."""
    items: list[dict[str, Any]] = []
    incomplete = False
    for page in range(1, pages + 1):
        status, data = client.get_json(sample_url(query, page, per_page, until), pace=True)
        if status != 200 or not isinstance(data, dict):
            # 422 past the 1,000th result, 403 that never cleared, 5xx: keep
            # what was found so far and say so.
            if status != 422:
                client.log(f"survival_discover: {name}: page {page} returned HTTP {status}; stopping this signal")
                incomplete = True
            break
        batch = data.get("items") or []
        incomplete = incomplete or bool(data.get("incomplete_results"))
        items.extend(i for i in batch if isinstance(i, dict))
        if len(batch) < per_page:
            break
    return items, incomplete


def aggregate(found: dict[str, list[dict[str, Any]]]) -> dict[str, dict[str, Any]]:
    """Per repository (lower-cased key): distinct sampled commits, sampled
    commits per signal, the name as GitHub writes it, the fork flag and
    whatever star count or archived flag the search result carried."""
    repos: dict[str, dict[str, Any]] = {}
    for signal, items in found.items():
        for item in items:
            repo = item.get("repository") or {}
            full = repo.get("full_name")
            sha = item.get("sha")
            if not isinstance(full, str) or "/" not in full or not isinstance(sha, str):
                continue
            key = full.lower()
            r = repos.setdefault(key, {"repo": full, "shas": set(), "sampled_by_signal": {}, "fork": bool(repo.get("fork")),
                                       "stars": repo.get("stargazers_count"), "archived": repo.get("archived")})
            if sha in r["shas"]:
                continue
            r["shas"].add(sha)
            r["sampled_by_signal"][signal] = r["sampled_by_signal"].get(signal, 0) + 1
            if r["stars"] is None and repo.get("stargazers_count") is not None:
                r["stars"] = repo.get("stargazers_count")
            if r["archived"] is None and repo.get("archived") is not None:
                r["archived"] = repo.get("archived")
    for r in repos.values():
        r["sampled"] = len(r["shas"])
        r["sampled_by_signal"] = dict(sorted(r["sampled_by_signal"].items()))
        # Without stage 2 the sampled counts are the counts.
        r["commits"] = r["sampled"]
        r["by_signal"] = dict(r["sampled_by_signal"])
        del r["shas"]
    return repos


# ----------------------------------------------------------------------------- stage 2: count

def candidates(repos: dict[str, dict[str, Any]], candidate_min: int, verify_max: int) -> list[dict[str, Any]]:
    cands = [r for r in repos.values() if r["sampled"] >= candidate_min and not r.get("fork")]
    cands.sort(key=lambda r: (-r["sampled"], r["repo"].lower()))
    return cands[:verify_max]


def verify_counts(client: Client, cands: list[dict[str, Any]], signals: dict[str, str]) -> list[str]:
    """Replace each candidate's sampled counts by the repository-wide
    `total_count` of every signal it was sampled with. Returns the
    repositories whose counts could not be verified (kept with sampled
    counts, flagged)."""
    unverified: list[str] = []
    for r in cands:
        by_signal: dict[str, int] = {}
        ok = True
        for name in r["sampled_by_signal"]:
            status, data = client.get_json(count_url(r["repo"], signals[name]), pace=True)
            if status != 200 or not isinstance(data, dict) or not isinstance(data.get("total_count"), int):
                ok = False
                break
            by_signal[name] = data["total_count"]
            if data.get("incomplete_results"):
                r["incomplete"] = True
        if ok:
            r["by_signal"] = by_signal
            r["commits"] = sum(by_signal.values())
            r["verified"] = True
        else:
            r["verified"] = False
            unverified.append(r["repo"])
    return unverified


# ----------------------------------------------------------------------------- details and selection

def complete_details(client: Client, rows: list[dict[str, Any]]) -> list[str]:
    """Search results carry `fork` but not `stargazers_count` or `archived`;
    GET /repos/<full_name> fills them in (core limit, 5,000/h). Returns the
    repositories that no longer resolve (renamed away, deleted, private):
    they are dropped."""
    unavailable: list[str] = []
    for r in rows:
        if r["stars"] is not None and r["archived"] is not None:
            continue
        status, data = client.get_json(repo_url(r["repo"]))
        if status in (404, 451):
            unavailable.append(r["repo"])
            continue
        if status != 200 or not isinstance(data, dict):
            # keep the candidate with what the search said; stars 0 means unknown here
            r["stars"] = r["stars"] if r["stars"] is not None else 0
            r["archived"] = bool(r["archived"])
            r["details"] = f"HTTP {status}"
            continue
        r["stars"] = int(data.get("stargazers_count") or 0)
        r["archived"] = bool(data.get("archived"))
        r["fork"] = bool(data.get("fork")) or r["fork"]
        if data.get("private"):
            unavailable.append(r["repo"])
            continue
        full = data.get("full_name")
        if isinstance(full, str) and full.lower() != r["repo"].lower():
            r["renamed_from"] = r["repo"]
            r["repo"] = full
    return unavailable


def rank_key(r: dict[str, Any]) -> tuple[int, int, str]:
    return (-int(r["commits"]), -int(r.get("stars") or 0), r["repo"].lower())


def select(rows: list[dict[str, Any]], seeds: list[str], optout: set[str], floor: int, limit: int,
           unavailable: list[str]) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    """Apply floor, exclusions and the limit. Returns the discovered
    repositories in selection order (seeds excluded) and an account of what
    was dropped."""
    seed_keys = {s.lower() for s in seeds}
    gone = {u.lower() for u in unavailable}
    dropped: dict[str, Any] = {"below_floor": 0, "fork": [], "archived": [], "opted_out": [],
                               "unavailable": sorted(unavailable, key=str.lower), "over_limit": []}
    eligible: list[dict[str, Any]] = []
    for r in rows:
        if r["commits"] < floor:
            dropped["below_floor"] += 1
            continue
        if r["repo"].lower() in gone or (r.get("renamed_from") or "").lower() in gone:
            continue
        if r.get("fork"):
            dropped["fork"].append(r["repo"])
            continue
        if r.get("archived"):
            dropped["archived"].append(r["repo"])
            continue
        if r["repo"].lower() in optout:
            dropped["opted_out"].append(r["repo"])
            continue
        eligible.append(r)
    # Case-insensitive dedupe, also after renames resolved through GET /repos.
    seen: dict[str, dict[str, Any]] = {}
    for r in sorted(eligible, key=rank_key):
        k = r["repo"].lower()
        if k in seen:
            seen[k]["commits"] += r["commits"]
            for s, n in r["by_signal"].items():
                seen[k]["by_signal"][s] = seen[k]["by_signal"].get(s, 0) + n
        else:
            seen[k] = r
    ranked = sorted(seen.values(), key=rank_key)
    room = max(0, limit - len(seed_keys))
    discovered = [r for r in ranked if r["repo"].lower() not in seed_keys]
    dropped["over_limit"] = [r["repo"] for r in discovered[room:]]
    for k in ("fork", "archived", "opted_out"):
        dropped[k].sort(key=str.lower)
    return discovered[:room], dropped


# ----------------------------------------------------------------------------- the list file

def split_list(text: str) -> tuple[list[str], list[str], list[str]]:
    """(head lines kept verbatim, seed repositories, previously discovered
    repositories). Everything before the `# discovered` marker is the hand-
    picked part of the list; everything after it was written by this script."""
    lines = text.splitlines()
    idx = next((i for i, l in enumerate(lines) if l.startswith(MARKER)), len(lines))
    head = lines[:idx]
    seeds = [l.strip() for l in head if l.strip() and not l.strip().startswith("#")]
    previous = [l.strip() for l in lines[idx:] if l.strip() and not l.strip().startswith("#")]
    while head and not head[-1].strip():
        head.pop()
    return head, seeds, previous


def render_list(head: list[str], discovered: list[str], date: str, floor: int) -> str:
    names = sorted(discovered, key=str.lower)
    out = list(head)
    out.append(f"{MARKER}{date} by scripts/survival_discover.py: {len(names)} repositories, "
               f"≥{floor} AI-attributed commits found via GitHub commit search")
    out.append("# Alphabetical. Counts per signal and the selection rule: .github/survival-discovery.json")
    out.extend(names)
    return "\n".join(out) + "\n"


# ----------------------------------------------------------------------------- run

def discover(client: Client, root: Path, floor: int = FLOOR, limit: int = LIMIT, pages: int = PAGES, per_page: int = PER_PAGE,
             candidate_min: int = CANDIDATE_MIN, verify_max: int = VERIFY_MAX, verify: bool = True,
             signals: dict[str, str] | None = None, now: str | None = None) -> dict[str, Any]:
    signals = signals or SIGNALS
    now = now or now_utc()
    today = now[:10]
    list_path = root / ".github" / "survival-repos.txt"
    json_path = root / ".github" / "survival-discovery.json"
    head, seeds, _previous = split_list(list_path.read_text(encoding="utf-8") if list_path.exists() else "")
    optout = read_optout(root)
    previous_json: dict[str, Any] = {}
    try:
        previous_json = json.loads(json_path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        pass
    first_seen = {r["repo"].lower(): r.get("discovered_at") for r in (previous_json.get("repositories") or [])
                  if isinstance(r, dict) and r.get("repo")}

    # 1. sample
    found: dict[str, list[dict[str, Any]]] = {}
    incomplete: list[str] = []
    for name, query in signals.items():
        items, inc = search_signal(client, name, query, pages, per_page, today)
        found[name] = items
        if inc:
            incomplete.append(name)
        client.log(f"survival_discover: sample {name}: {len(items)} commits{' (incomplete)' if inc else ''}")
    repos = aggregate(found)

    # 2. count
    cands = candidates(repos, candidate_min, verify_max)
    unverified: list[str] = []
    if verify:
        unverified = verify_counts(client, cands, signals)
        client.log(f"survival_discover: counted {len(cands)} candidates repository-wide ({len(unverified)} unverified)")
    rows = [r for r in cands if r["commits"] >= floor]

    # 3. + 4. floor, exclusions, order, limit
    unavailable = complete_details(client, rows)
    discovered, dropped = select(rows, seeds, optout, floor, limit, unavailable)
    dropped["below_floor"] += len(cands) - len(rows)
    dropped["not_candidates"] = len(repos) - len(cands)
    dropped["unverified"] = sorted(unverified, key=str.lower)

    def row(r: dict[str, Any], seed: bool) -> dict[str, Any]:
        out = {"repo": r["repo"], "seed": seed, "commits": r["commits"], "by_signal": r["by_signal"], "sampled": r["sampled"],
               "counts": "repository-wide" if r.get("verified") else "sampled",
               "stars": r.get("stars"), "discovered_at": first_seen.get(r["repo"].lower()) or now}
        for k in ("renamed_from", "incomplete", "details"):
            if r.get(k):
                out[k] = r[k]
        return out

    rows_out = [row(r, False) for r in discovered]
    for s in seeds:
        r = repos.get(s.lower())
        rows_out.append({**row(r, True), "repo": s} if r else
                        {"repo": s, "seed": True, "commits": 0, "by_signal": {}, "sampled": 0, "counts": "not sampled", "stars": None, "discovered_at": None})
    rows_out.sort(key=lambda r: r["repo"].lower())

    result = {
        "schema": SCHEMA,
        "discovered_at": now,
        "tool": "scripts/survival_discover.py",
        "source": "GitHub REST commit search (GET /search/commits): default branch, public, non-merge commits; "
                  "at most 1,000 results per query; repository-wide counts from total_count of repo:-scoped queries",
        "selection": {
            "rule": "sample the most recent matching commits per signal; a repository sampled at least `candidate_min` times is a "
                    "candidate (at most `verify_max`); its count is the sum over its signals of the repository-wide total_count "
                    "(a commit carrying two signals counts twice); keep repositories with at least `floor` commits; drop forks, "
                    "archived repositories and .github/survival-optout.txt; order by commits, then stargazers_count, then name; "
                    "keep the hand-picked seeds; fill up to `limit`. Rows below are alphabetical.",
            "floor": floor, "limit": limit, "pages": pages, "per_page": per_page, "candidate_min": candidate_min,
            "verify_max": verify_max, "verified": bool(verify), "sort": "author-date desc (most recent first)",
            "qualifiers": f"{QUALIFIERS} author-date:<={today}",
        },
        "signals": dict(signals),
        "sampled_commits": {name: len(items) for name, items in found.items()},
        "incomplete_signals": incomplete,
        "requests": client.requests,
        "search_requests": client.search_requests,
        "seeds": list(seeds),
        "repositories": rows_out,
        "candidates": len(cands),
        "repositories_sampled": len(repos),
        "dropped": dropped,
    }
    result["_list_text"] = render_list(head, [r["repo"] for r in discovered], today, floor)
    return result


def write_outputs(result: dict[str, Any], root: Path) -> None:
    text = result.pop("_list_text")
    (root / ".github" / "survival-repos.txt").write_text(text, encoding="utf-8")
    (root / ".github" / "survival-discovery.json").write_text(json.dumps(result, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")


def summary(result: dict[str, Any]) -> str:
    n_disc = sum(1 for r in result["repositories"] if not r["seed"])
    d = result["dropped"]
    return (f"survival_discover: {result['repositories_sampled']} repositories sampled, {result['candidates']} counted · "
            f"{n_disc} discovered + {len(result['seeds'])} seeds = {n_disc + len(result['seeds'])} listed "
            f"(limit {result['selection']['limit']}) · dropped: {d['below_floor']} below floor, {len(d['fork'])} forks, "
            f"{len(d['archived'])} archived, {len(d['opted_out'])} opted out, {len(d['unavailable'])} unavailable, "
            f"{len(d['over_limit'])} over the limit · {result['requests']} requests"
            + (f" · incomplete: {', '.join(result['incomplete_signals'])}" if result["incomplete_signals"] else ""))


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--root", default=str(ROOT), help="repository root (default: the checkout this script lives in)")
    ap.add_argument("--limit", type=int, default=LIMIT, help=f"repositories in the list, seeds included (default {LIMIT})")
    ap.add_argument("--floor", type=int, default=FLOOR, help=f"minimum matching commits per repository (default {FLOOR}, the sample floor)")
    ap.add_argument("--pages", type=int, default=PAGES, help=f"sample pages per signal (default {PAGES}; the API stops at 1,000 results)")
    ap.add_argument("--per-page", type=int, default=PER_PAGE, help=f"results per page, max 100 (default {PER_PAGE})")
    ap.add_argument("--candidate-min", type=int, default=CANDIDATE_MIN, help=f"sampled commits that make a repository a candidate (default {CANDIDATE_MIN})")
    ap.add_argument("--verify-max", type=int, default=VERIFY_MAX, help=f"candidates counted repository-wide, most sampled first (default {VERIFY_MAX})")
    ap.add_argument("--no-verify", action="store_true", help="rank by sampled commits only (cheaper, noisier)")
    ap.add_argument("--pace", type=float, default=SEARCH_PACE_S, help=f"seconds between search requests (default {SEARCH_PACE_S})")
    ap.add_argument("--dry-run", action="store_true", help="print the list and the summary; write nothing")
    args = ap.parse_args(argv)

    token = token_from_env()
    if not token:
        print("survival_discover: no GITHUB_TOKEN/GH_TOKEN; unauthenticated search allows 10 requests per minute", file=sys.stderr)
    client = Client(lambda url: urllib_fetch(url, token), pace_s=args.pace if token else 6.5)
    root = Path(args.root)
    result = discover(client, root, floor=args.floor, limit=args.limit, pages=args.pages, per_page=min(100, max(1, args.per_page)),
                      candidate_min=args.candidate_min, verify_max=args.verify_max, verify=not args.no_verify)
    if args.dry_run:
        sys.stdout.write(result["_list_text"])
        print()
        print(summary(result))
        return 0
    write_outputs(result, root)
    print(summary(result))
    return 0


if __name__ == "__main__":
    sys.exit(main())
