# Changelog

Each section is the release note of the tag with the same number; the
release workflow copies it verbatim. Counts, not adjectives.

## 0.2.0 — 2026-09-20

The first release after the 2026-09-20 review (`docs/review-2026-09-20/`).
Decisions in `ROADMAP.md`, thesis in `MANIFESTO.md`.

### Measure

- `re audit` method v2: `git blame -w -M -C`, `.git-blame-ignore-revs`
  honoured, only the git trailer block is parsed, `Assisted-by:` and
  `copilot-swe-agent[bot]` recognised, whole-word agent names, per-commit
  cap (p95, ceiling 10,000 lines), median and largest-commit share,
  `coverage` block in every output, shallow clones refused unless
  `--allow-shallow`.
- No verdicts anywhere: no rank, no colour, no "healthy" in `audit`,
  `churn`, `guard`, the Action or the site. `re churn --json`,
  `--fail-below`; `re guard --json`, `--fail-on alert|warning`.
- The public leaderboard is gone. Weekly measurements are published as
  counts, opt-out honoured, no bot opens issues on other people's repos.

### Record

- Proxy parses `tool_calls` (OpenAI), `tool_use` (Anthropic) and the
  Responses API into `response_text`; requests usage on OpenAI streams;
  records exchanges cut short by a disconnect as `truncated`; captures only
  `POST`s to completion endpoints; merges Claude Code hook events with the
  exchange behind them (model, tokens, cost).
- Store: atomic, self-healing object writes; atomic ref/HEAD writes;
  unrestorable names skipped at snapshot time; executable bit stored;
  `.gitignore` semantics in git work trees; stat cache (5,000 files, one
  edit: 21 ms).
- One line-provenance engine behind `why`, `trace`, `lens`, `impact` and
  the MCP `causari_why`; every event carries an evidence class
  (`declared`, `correlated`, `observed`) that every consumer prints.
- `re watch` records nothing when the tree is unchanged.
- `re show` prints model, tokens, cost, prompt, reasoning and evidence;
  `--json`.

### Prove

- `re audit --seal` issues a Crovia Seal (`crovia.seal.v1`) over the audit
  JSON, bound to the audited commit and the method version, hash-chained
  with the proxy's completion seals under one issuer key per repository.
  `re seal verify FILE` checks it offline; so does the static page
  causari.dev/verify (no network request, no third-party code). A seal
  proves the numbers were not altered after the run, not that they are
  true, and the verifier says so.
- `re proof` retired (exit 2 with the replacement named); `re audit
  --seal` and `re seal verify` take its place.
- `re proxy --pnx` witnesses the agent's traffic and writes a signed run
  sheet per session (`crovia.pnx.v1`: winnowing fingerprints, sparse
  Merkle map, Ed25519). `re pnx prove` shows offline that no asset from a
  given set appeared in that traffic; `re pnx verify`, `sheet`, `list`.
  Proofs verify under the Python reference `tacet-pnx` and vice versa; the
  Action verifies a proof handed to it (`pnx-proof`, `pnx-assets`,
  `pnx-fail-on-present`).
- Action inputs `seal` and `seal-key`: the audit seal as an artifact, with
  a persistent issuer identity when a key is passed.
- Seal issuer: `deny_unknown_fields`, dedicated key, domain-separated
  payload, keys written `0600`, `seal_id` on exchanges.

### Report

- Weekly Survival Report at causari.dev/reports/survival/: counts per
  agent across public repositories, archive, Atom feed, `latest.json`, one
  DOI per issue on Zenodo. Method in `docs/survival-report.md`.

### Distribution and identity

- Binary `causari` with `re` as alias, both in every archive; installers,
  Homebrew tap (`croviatrust/tap`), Scoop bucket, crates.io
  (`cargo install causari`), signed SLSA attestations on every archive.
- MCP Registry entry (`io.github.croviatrust/causari`); Claude Code plugin
  (`/plugin marketplace add croviatrust/causari`); one-click Cursor link.
- `--help` grouped by task. DCO replaces the CLA. One legal entity name.
- Identity: the `∵` mark, monospace wordmark, monochrome palette;
  causari.dev rewritten with no external dependencies; `canon/canon.json`
  and `scripts/audit_surfaces.py` check every public claim in CI.

## 0.1.5

Last release before the review. Archives named `re-<tag>-<target>.*`,
single binary `re`.
