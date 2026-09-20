# Survival Report #1 — 2026-09-20

Counts of surviving lines from AI-tagged commits in 10 open-source repositories, measured with causari 0.1.5, method v2. Counts, not grades: no rank, no verdict; rows are alphabetical.

Page: https://causari.dev/reports/survival/2026/01/  
Data: https://causari.dev/reports/survival/2026/01/report.json  
Feed: https://causari.dev/reports/survival/feed.xml  
Licence: CC-BY-4.0  

## Aggregate

462,838 of 754,476 lines introduced by 12,349 AI-tagged commits in 10 open-source repositories are still at HEAD (61.3 %). 95 % interval over the sampled repositories: 53.0 % to 64.9 %.

- Repositories aggregated: 10
- Commits in those repositories (no merges): 86,024
- AI-tagged (VERIFIED) commits: 12,349
- Lines introduced by them: 754,476
- Still attributed to them at HEAD: 462,838
- Line-weighted ratio: 61.3 %
- 95 % bootstrap interval over the sampled repositories: 53.0 % to 64.9 %
- Median of per-repository capped ratios: 65.7 %
- 95 % bootstrap interval on that median: 38.6 % to 80.0 %

95 % percentile interval from 2000 bootstrap resamples of the 10 aggregated repositories (with replacement, seed 1). It describes the sampled repositories, not all AI-assisted code, and not the repositories not in this sample.

## Repositories (alphabetical)

| Repository | Commits | AI-tagged | Introduced | Still at HEAD | Line-weighted | Capped | Median per commit | Largest commit | Reproduce |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---|
| Aider-AI/aider | 12,461 | 11,156 | 374,687 | 235,782 | 62.9 % | 41.9 % | 33.3 % | 27 % | `re audit Aider-AI/aider --json` |
| anthropics/anthropic-sdk-python | 1,390 | 38 | 4,852 | 4,043 | 83.3 % | 80.0 % | 83.7 % | 33 % | `re audit anthropics/anthropic-sdk-python --json` |
| anthropics/anthropic-sdk-typescript | 1,321 | 48 | 13,796 | 11,394 | 82.6 % | 84.7 % | 96.1 % | 40 % | `re audit anthropics/anthropic-sdk-typescript --json` |
| browser-use/browser-use | 7,000 | 220 | 18,186 | 3,184 | 17.5 % | 23.4 % | 0.0 % | 21 % | `re audit browser-use/browser-use --json` |
| cline/cline | 7,273 | 154 | 95,301 | 63,063 | 66.2 % | 67.9 % | 72.0 % | 12 % | `re audit cline/cline --json` |
| continuedev/continue | 16,266 | 228 | 27,756 | 18,063 | 65.1 % | 63.5 % | 68.1 % | 11 % | `re audit continuedev/continue --json` |
| croviatrust/causari | 127 | 14 | 6,267 | 5,595 | 89.3 % | 89.3 % | 96.2 % | 25 % | `re audit croviatrust/causari --json` |
| ghostty-org/ghostty | 13,460 | 77 | 3,725 | 2,570 | 69.0 % | 73.4 % | 86.0 % | 13 % | `re audit ghostty-org/ghostty --json` |
| openai/codex | 11,046 | 364 | 183,908 | 109,674 | 59.6 % | 59.1 % | 58.8 % | 6 % | `re audit openai/codex --json` |
| sst/opencode | 15,680 | 50 | 25,998 | 9,470 | 36.4 % | 35.4 % | 39.5 % | 43 % | `re audit sst/opencode --json` |

## By agent, across aggregated repositories (alphabetical)

| Agent | Repositories | Commits | Introduced | Still at HEAD | Line-weighted |
|---|---:|---:|---:|---:|---:|
| ai | 1 | 1 | 28 | 20 | 71.4 % |
| aider | 2 | 11,157 | 374,809 | 235,802 | 62.9 % |
| claude-code | 9 | 602 | 117,299 | 77,229 | 65.8 % |
| cursor | 6 | 165 | 24,979 | 18,204 | 72.9 % |
| gemini | 1 | 1 | 4 | 3 | 75.0 % |
| github-copilot | 6 | 67 | 55,707 | 23,094 | 41.5 % |
| jules | 1 | 1 | 4 | 0 | 0.0 % |
| openai-codex | 3 | 354 | 181,501 | 108,353 | 59.7 % |
| openhands | 1 | 1 | 145 | 133 | 91.7 % |

## Excluded from this report

- Shallow clones (history truncated; method v2 refuses them): none
- Audits that failed in this run: none
- Opted out by their maintainers (https://github.com/croviatrust/causari/blob/main/.github/survival-optout.txt): 0

## Method

Method v2, causari 0.1.5. Detection from commit metadata only; survival from `git blame -w -M -C` at HEAD. Per-commit cap: a commit weighs at most the 95th percentile of per-commit introduced line counts in its repository, and never more than 10,000 lines. Sample floor: 5 VERIFIED commits. VERIFIED only; PROBABLE is listed but never summed. Full clones only. Details, limits and how to contest a number: https://causari.dev/method.

## What this report is, and is not

This report counts lines. For each repository it states how many lines were introduced by commits that carry machine-readable AI authorship metadata (trailers such as Co-Authored-By naming an agent, bot author identities, aider markers, git-ai notes), and how many of those lines git blame still attributes to those commits at HEAD, under method v2 (blame with -w -M -C, a per-commit weight cap, a sample floor, full clones only). Every row is reproducible with one command.

It is not a quality judgement: deleted lines include removed features and rewritten prototypes; surviving lines include dead code. It is not a sample of all AI-assisted code: inline completions leave no trace in git, untagged agent commits are invisible, and the repositories were added by pull request, not drawn at random. The intervals describe the sampled repositories only.

Prior measurement work asks related questions with different instruments. GitClear publishes churn reports built from code-change patterns across the repositories it analyses; arXiv 2601.16809 ("Will It Survive?") follows the modification of agent-authored code in 201 projects with its own detector and finds that such code is modified less often than human-written code. This report does not reproduce either method and does not adjudicate between them: it publishes counts from git metadata alone, with the method version, the tool version and the exact bytes behind every number, so that the three can be read side by side.

## Cite

Crovia Trust. Survival Report #1 (2026-09-20). https://causari.dev/reports/survival/2026/01/
