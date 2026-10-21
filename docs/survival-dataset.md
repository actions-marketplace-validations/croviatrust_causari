# Causari Survival Report — dataset

How many lines from AI-tagged commits are still alive in open-source repositories, counted every week with a public method. Counts, not grades.

This dataset is the machine-readable form of the weekly [Survival Report](https://causari.dev/reports/survival/) published at causari.dev. Every number here is produced by `re audit <owner/repo> --json`, a free, open-source command ([github.com/croviatrust/causari](https://github.com/croviatrust/causari)) that reads plain git history and asks `git blame` how many lines introduced by metadata-matched commits are still attributed to them at HEAD. Anyone can rerun it on the same commit and compare the bytes.

- Reports included: {reports} (#{first_number} to #{last_number}, {first_date} to {last_date})
- Repositories in the latest report: {latest_repositories} ({latest_aggregated} aggregated, {latest_not_aggregated} measured but below the sample floor)
- Latest report: [{latest_title}]({latest_url}){latest_doi_note}
- Method: [causari.dev/method](https://causari.dev/method). Each row names the method version that produced it. Published reports are not recomputed when the method changes. Rows through report #6 are method v2, method v3, or method v4. None is method v5. The published binary that measured reports #5 and #6 is causari 0.4.1 (commit d4c4949, 2026-10-08), method v4. This source computes method v5 and is not that binary.
- License: CC-BY-4.0. Publisher: Crovia Trust.

## What a row means

**AI-tagged means the metadata matched.** A commit is VERIFIED when its git metadata carries a machine-readable AI authorship signal (a `Co-Authored-By` trailer naming an agent, a bot author identity, an `AI-*` or `Assisted-by:` trailer, or, from method v4, a git-ai note that names a non-empty tool). The rule table is public. Matching is not proof that a model wrote the line, and a person can forge metadata.

**UNKNOWN is not human.** Commits with no such metadata form the untagged baseline. That baseline holds human-written code, inline completions (Copilot, Cursor Tab and similar leave no git trace) and untagged agent commits alike. Nothing in this dataset says who typed a line.

**Survival is a count.** `surviving / introduced` over VERIFIED commits, line-weighted, with `git blame -w -M -C`, capped per commit at what the commit introduced. It is not code quality, developer productivity, total AI usage, authorship, security, correctness, or a causal effect of AI. A rewritten line is a death even when the meaning is unchanged; a surviving line may be dead code.

**Five commits is the comparability floor, not a reliability guarantee.** Rows with `aggregated = false` are measured but kept out of the headline aggregate because the repository has fewer than five VERIFIED commits or was a shallow clone.

**The age-matched gap compares the same repository with itself.** `age_matched_gap` is AI-tagged survival minus untagged survival inside matched line-age windows of the same repository, in ratio units. It is not a comparison of identical timestamps and it is `null` when no window has at least five commits of both kinds.

## Files

| Path | One row per | Notes |
| --- | --- | --- |
| `data/reports.jsonl` | report | aggregate counts, 95% intervals, DOI, method version, selection sentence |
| `data/repositories.jsonl` | repository × report | VERIFIED and untagged counts, age-matched gap, coverage flags, per-agent JSON |
| `data/agents.jsonl` | agent × report | counts per detected agent identity, across the aggregated repositories |
| `raw/<report-id>/report.json` | report | the exact bytes published at causari.dev |
| `raw/<report-id>/repos/*.json` | repository | the exact `re audit --json` bytes behind each row |

The `raw/` tree is byte-identical to the published site. The `data/` tables are derived from it; if they ever disagree, the raw bytes are the record.

## Selection

The repositories were selected, not drawn at random: a hand-picked part and a part found by GitHub commit search as public repositories with at least five commits carrying the same AI authorship metadata and at least 100 stars. The selection rule, the counts behind it and the opt-out file are public. The sample favours popular repositories where AI-attributed commits are visible; it is not a random sample of open source. Each report's `selection` field states how its list was made.

Maintainers who do not want their repository measured add one line to [`.github/survival-optout.txt`](https://github.com/croviatrust/causari/edit/main/.github/survival-optout.txt); the next weekly run drops the row and it is not re-added.

## Reproduce or contest a number

```bash
curl -fsSL https://causari.dev/install.sh | sh
re audit owner/repo --json        # the bytes behind a row
diff <(re audit owner/repo --json) raw/<report-id>/repos/owner__repo.json
```

A repository that has moved has a different HEAD commit, and the export does not reproduce the published number; `repository.head` in each raw file names the commit that was measured. If a reproduction differs or a row is wrong, open an issue with the JSON at [github.com/croviatrust/causari/issues](https://github.com/croviatrust/causari/issues). Corrections are made in public and recorded in the data history.

## Cite

Each report has its own DOI on Zenodo (see `doi` in `data/reports.jsonl`); the concept DOI covers the series. Please cite the report you used, with its method version.

Related: "Will It Survive?" ([arXiv 2601.16809](https://arxiv.org/abs/2601.16809)) measures the same question on 201 projects with a different detector. These counts should be read next to it, not instead of it.

The same limits, for an agent, in one file: [causari.dev/llms.txt](https://causari.dev/llms.txt).
