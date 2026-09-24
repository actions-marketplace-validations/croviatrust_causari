# Changelog

Each section is the release note of the tag with the same number; the
release workflow copies it verbatim. Counts, not adjectives.

## Unreleased

### Record

- `re hook cursor`: capture from Cursor's native hooks. Merges seven
  command hooks into `.cursor/hooks.json` (`--project`, the default, also
  run by Cursor cloud agents) or `~/.cursor/hooks.json` (`--user`);
  `--dry-run` prints the merged file; idempotent, other hooks in the file
  are kept. `beforeSubmitPrompt` records the prompt, its attachments and
  the model per `conversation_id`; `preToolUse` (Shell|Write) snapshots
  the tree before the tool; `afterFileEdit` records one declared event per
  written file (agent `cursor`, evidence `cursor-hook`, model from the
  payload, prompt of the same conversation, attachments as `reads`);
  `afterShellExecution` records a command that changed the tree with the
  command as message; `afterAgentResponse` stores the answer as an
  exchange under agent `cursor`; `stop` drops the conversation's unused
  pre-states; `sessionStart` returns the experience briefing as
  `additional_context`. Files outside the repository are ignored. Every
  hook answers a JSON object, `{"permission":"allow"}` for the permission
  hook, even without a ledger. `re` must be on `PATH` for Cursor.
- Prompt records carry the runtime's `model` and `attachments` when its
  payload has them; Claude Code lines are unchanged.
- Integration matrix: Cursor is its own row (prompt and file exact, model
  yes, tokens and cost no); Windsurf and Copilot stay on MCP self-report.

### Audit

- `re audit --json` names what it measured: `repository.head` (the commit
  at HEAD, 40 hex) and `repository.origin` (the origin URL with credentials
  stripped, or `sha256:` of the path when there is no remote). Two audits
  with the same `head` measured the same tree, whatever the repository is
  called. Audit seals keep binding to the exact bytes, which now include
  this object.

### Survival Report

- One repository counts once, whatever it is called. Report #2 counted
  `All-Hands-AI/OpenHands` and `OpenHands/OpenHands` — one repository,
  renamed on GitHub — as two rows with byte-identical audits. The
  generator now drops audits that measured the same commit or are
  byte-identical, keeps the name in `.github/survival-repos.txt`, and
  lists the other under `excluded.duplicates` with the name it was
  counted under. Discovery resolves every hand-picked seed through
  `GET /repos/{owner}/{repo}` and treats the listed name and the name
  GitHub now gives as one seed, so a renamed seed is never discovered a
  second time; the seed line was corrected to `OpenHands/OpenHands`.

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
