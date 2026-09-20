# The Survival Report

Weekly, numbered, dated, citable. Published at
[causari.dev/reports/survival/](https://causari.dev/reports/survival/) as a
static page, an Open Graph card, an Atom feed, a JSON dataset and a Zenodo
deposit with a DOI. Same pipeline shape as the
[Crovia Silence Report](https://croviatrust.com/report/), so the two read
as siblings.

## What it is

Counts of surviving lines from AI-tagged commits in N open-source
repositories, under method v2. For each repository the report states how
many lines were introduced by commits that carry machine-readable AI
authorship metadata (`Co-Authored-By` trailers naming an agent, bot author
identities, aider markers, `Assisted-by:`, git-ai notes) and how many of
those lines `git blame -w -M -C` still attributes to those commits at HEAD.
Alongside the line-weighted ratio it gives the capped ratio (no commit
weighs more than the 95th percentile of per-commit introduced counts in its
repository, never more than 10,000 lines), the median per-commit ratio and
the share of the largest commit. The aggregate carries a 95 % bootstrap
interval computed by resampling repositories (2,000 resamples, seeded from
the report number so the bytes are reproducible).

Every number links to the audit bytes behind it (`repos/<owner>__<repo>.json`)
and to the command that reproduces it: `re audit <owner/repo> --json`.

## What it is not

It is not a quality judgement. Deleted lines include removed features,
moved code and rewritten prototypes; surviving lines include dead code.
There is no rank, no colour, no verdict; rows are alphabetical.

It is not a sample of "all AI code". Inline completions (Copilot, Cursor
Tab, Windsurf) leave no trace in git and are invisible. Untagged agent
commits are UNKNOWN and never counted. The repositories were added by pull
request to [`.github/survival-repos.txt`](../.github/survival-repos.txt),
not drawn at random. The intervals describe the sampled repositories only,
not a population.

## Prior measurement work

GitClear publishes churn reports built from code-change patterns across the
repositories it analyses. arXiv 2601.16809 ("Will It Survive?") follows the
modification of agent-authored code in 201 projects with its own detector
and finds that such code is modified less often than human-written code.
The Survival Report does not reproduce either method and does not
adjudicate between them: it publishes counts from git metadata alone, with
the method version, the tool version and the exact bytes behind every
number, so that the three can be read side by side.

## Rules baked into the generator

`scripts/survival_report.py` enforces what the 2026-09-20 reviews asked
for, so no editor has to remember it:

| Rule | Where |
|---|---|
| Rows alphabetical (case-insensitive), never by rate | `collect()` |
| No rank column, no per-row colour, no adjectives | templates; `scripts/audit_surfaces.py --gate` checks the forbidden words of `canon/canon.json` on the generated pages |
| VERIFIED only; PROBABLE listed, never summed | `collect()` |
| `coverage.small_sample` (fewer than 5 VERIFIED commits) → "measured but not aggregated" | `collect()` |
| `coverage.shallow` → excluded with a note | `collect()` |
| Opt-out list honoured (`.github/survival-optout.txt`, case-insensitive, `#` comments) | workflow skips them; generator drops them again |
| Bootstrap interval over repositories, 2,000 resamples, seed = report number, labelled as an interval over the sample | `bootstrap_rate()`, `bootstrap_median()` |
| Method section states method version, tool version, blame flags, cap rule, sample floor; links `/method` | `render_report()` |
| Report directories are append-only; a directory holding a different report is never overwritten | `write_report()` |
| Old method v1 data cannot be relabelled as v2 | `from_existing()` refuses rows without a `coverage` block |

## Files

```
site/reports/survival/
  index.html                 archive, newest first
  feed.xml                   Atom, one entry per report
  latest.json                copy of the newest report.json
  zenodo.json                Concept DOI and one record per report (written by the deposit)
  <YYYY>/<NN>/
    index.html               the report page
    report.json              counts, intervals, coverage, DOI (schema causari.survival_report.v1)
    report.md                plain-text version, also the Zenodo description
    card.svg, card.png       Open Graph card, identity style
    repos/<owner>__<repo>.json   the exact `re audit --json` bytes per repository
```

`site/_redirects` and `site/sitemap.xml` contain a block managed by the
generator: `/survival` → `/reports/survival/`, `/survival-data.json` →
`latest.json`, `/report` → the latest report.

## How a report is made

[`.github/workflows/survival-report.yml`](../.github/workflows/survival-report.yml),
Mondays 05:17 UTC or on demand:

1. Install the latest release (`causari` and `re`), checksum-verified.
2. For every repository in `.github/survival-repos.txt` not in
   `.github/survival-optout.txt`: full `git clone` (method v2 refuses shallow
   clones), `re audit <clone> --json`, keep the bytes.
3. `python3 scripts/survival_report.py build --run /tmp/run`: report number =
   existing report directories + 1; writes the report, the archive, the feed,
   `latest.json`, the redirect and sitemap blocks. `scripts/audit_surfaces.py
   --gate` runs on the result.
4. Commit `site/reports/survival/**` to `main`: plain commit, never a
   force-push. A concurrency group keeps two runs from racing.
5. If `ZENODO_TOKEN` is set: `python3 scripts/zenodo_deposit.py <report dir>`
   publishes the record, writes the DOI into `report.json`, re-renders the page
   and the archive, and commits again. Without the secret the step prints a
   notice and the dry-run payload; the report is published without a DOI and
   the page says "DOI: pending deposit".

Locally:

```sh
mkdir -p /tmp/run && for r in owner/repo …; do
  git clone --single-branch "https://github.com/$r" "/tmp/clones/${r/\//__}"
  re audit "/tmp/clones/${r/\//__}" --json > "/tmp/run/${r/\//__}.json"
done
# run.json: generated_at, tool, tool_version, method, command, repos, failed, opted_out
python3 scripts/survival_report.py build --run /tmp/run
python3 scripts/zenodo_deposit.py --dry-run site/reports/survival/2026/01
python3 -m pytest scripts/tests -q
```

## Zenodo

The deposit mirrors `ops/phase0/zenodo_deposit_weekly.py` of the Silence
Report: `upload_type` publication / report, creators Crovia Trust, licence
CC-BY-4.0, keywords, related identifiers back to the page, the JSON, the
method page and the repository. One Concept DOI for the series; each report
is a version of it. Idempotent per report: same bytes → skip; changed bytes →
new version of the same record, never a duplicate.

Setup for the repository owner:

- Create a personal access token on [zenodo.org](https://zenodo.org/account/settings/applications/)
  with `deposit:write` and `deposit:actions`, add it as the repository secret
  `ZENODO_TOKEN`.
- To rehearse against the sandbox first, create a token on
  [sandbox.zenodo.org](https://sandbox.zenodo.org/) and set the repository
  variable `ZENODO_SANDBOX=1`; sandbox records are kept apart in
  `zenodo.json` so a sandbox concept is never reused live.
- The first successful deposit creates the Concept DOI; it is then shown on
  every report page and in the archive.

## Report #1

Report #1 (2026-09-20) was generated from fresh `re audit --json` runs on
full clones of ten repositories from the measured list. The retired
`survival-data.json` of 2026-08-25 was method v1 (blame without `-w -M -C`,
no cap, no coverage block) and was therefore not relabelled as Report #1;
`survival_report.py from-existing` refuses such data by design.
