# Launch drafts

Texts for the maintainer to post, if and when he chooses. Nothing in this
folder is published by anyone else, ever: the agent drafts, the owner posts
(`GROWTH.md` §4, "What the agent will not do"). The second rule from the
same section applies to every line here: no figure without its bound.

| File | Where | Limits kept |
|---|---|---|
| `show-hn.md` | news.ycombinator.com, "Show HN" | title ≤ 80 chars, body ≤ 250 words, first person |
| `x-thread.md` | X / Bluesky / Mastodon | 7 posts, each ≤ 280 chars |
| `linkedin.md` | LinkedIn | ≤ 180 words |
| `reddit-r-programming.md` | r/programming | title + ≤ 200 words; method first, criticism invited |
| `maintainer-note.md` | e-mail or issue to a maintainer whose repository is in the report | template; no ask for a star |

## Numbers

Every number is taken from Survival Report #1 (2026-09-20),
`site/reports/survival/2026/01/report.md`, and is quoted with its 95 %
bootstrap interval over the sampled repositories. When a newer report is
out, replace the numbers from that report's `report.md` and nothing else;
the interval travels with the figure. Per-agent and per-repository rows
carry no interval and are therefore not quoted here.

## What the texts do not do

- No verdict adjectives about any repository, agent or vendor.
- No claim about inline completions: they leave no git trace and the texts
  say so.
- The maintainer note links the repository's own page (`/r/<owner>/<repo>/`)
  and mentions the badge that page offers, once, as an option. It never asks
  for it.
- Forbidden wording is the list in `canon/canon.json → forbidden_words`;
  the drafts were checked against it.
