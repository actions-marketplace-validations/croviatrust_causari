# Roadmap

Decisions taken on 2026-09-20 from [`docs/review-2026-09-20/`](docs/review-2026-09-20/).
Thesis in [`MANIFESTO.md`](MANIFESTO.md). Phases are sequential; a phase is
closed when every exit criterion holds on `main` with green CI.

Status legend: `[ ]` open · `[~]` in progress · `[x]` done.

## Phase 0 — stop doing harm

Nothing is promoted before this phase closes.

- [x] Leaderboard → measurements only: no rank, no colour class, no verdict
      sentence, no third-party badge generator; per-commit cap and sample
      floor; methodology page with known false positives; opt-out list; the
      public "Audit: owner/repo" issue bot disabled; data branch appended,
      never force-pushed.
- [x] `re audit --summary` and the Action: no 🟢/🟡/🔴, no "healthy".
- [x] `re proof`: `deny_unknown_fields`, dedicated key, domain-separated
      payload, CSC-1; keys written `0600`; `seal_id` recorded on exchanges;
      `re seal issuer` honours the configured issuer and does not create keys.
- [x] README and site aligned with the code: integration matrix from the
      code (Claude Code = hooks without cost; Aider = proxy join; Codex = cost
      only; Cursor/Copilot/Windsurf = MCP self-report), real binary size, one
      version, "signed" only when signed, "10 seconds" qualified, "zero false
      attribution" removed, "first production issuer" removed, Trust Plane
      removed until purchasable.
- [x] Broken URLs: `/verify`, `/repo?r=`, favicon, sitemap, canonical,
      `llms.txt` commands.
- [x] Skills / trust ladder / brief / mesh marked **experimental**, out of the
      proof and out of the front page. Guard out of the README until it can
      gate. `report` folded into `churn --html`.

Exit: surface audit green (see Phase 3), no verdict anywhere on causari.dev,
`re proof verify` rejects a modified manifest.

## Phase 1 — foundations of the ledger

No new feature until these hold.

- [x] `re watch`: ignore Access/Open notify kinds; notify paths filtered
      through the ignore rules; "tree unchanged → skip" in `commit.rs` for
      every recorder; watch survives lock contention.
- [x] One line-provenance engine used by `why`, `trace`, `lens`, `impact`,
      MCP `causari_why`; root event handled once.
- [x] Evidence class on every event: `declared | correlated{score,
      matched, considered, exchange_id} | self_reported`, shown by every
      consumer.
- [x] Real pre-state: `re revert` records an event; hook events scope their
      diff to the declared path and report incidental changes separately;
      no cross-session prompt fallback.
- [~] Store: temp+rename for objects and refs (no fsync yet: crash-safe,
      not power-loss-safe); names and mode validated at snapshot time;
      `.gitignore` semantics; stat cache so recording is O(changed files)
      (5,000 files, one edit: 21 ms). Open: path→writer index so queries
      are not O(events × files).
- [x] Proxy: `tool_calls`, `tool_use`/`input_json_delta`, Responses API
      output parsed; hook and proxy streams merged for Claude Code by agent,
      time window and content (Claude Code sends no session id to the
      proxy); raw request/response hashes persisted; truncated exchanges
      flagged. Known limit: `tiny_http` buffers 8 KB before the first
      chunk, so the proxy does not relay tokens live.
- [x] Audit: `git blame -w -M -C`; trailer block parsed with git semantics;
      `Assisted-by:` and `copilot-swe-agent[bot]`; shallow clone refused;
      coverage statement in every output.
- [x] CLI: no panic on closed pipe; `re show` shows prompt/model/cost;
      `re audit` hands off to `re init`/`re hook`; `--json` and non-zero exit
      codes for `audit`, `churn`, `guard`.

Exit: `re watch` idle for 60 s on Linux records 0 events; `re why` and
`re trace` agree on every line of the test corpus; a 20k-file repo records a
one-file change in < 50 ms; the adversarial harness passes on Linux.

## Phase 2 — interoperate, prove, report

- [ ] `re audit` emits and reads **Agent Trace**; reads git-ai notes (kept).
- [x] The audit result **is a Seal**: `re audit --seal` writes a
      `crovia.seal.v1` with the audit JSON as subject, bound to commit hash
      and method version, hash-chained with the proxy's completion seals;
      `re seal verify` and the static, offline `causari.dev/verify` check it
      (the browser verifier is tested against the CLI from Node); `re proof`
      retired; Action inputs `seal` / `seal-key`. Shipped in 0.2.0.
- [x] **PNX witness mode**: `re proxy --pnx` produces a signed run sheet per
      session; `re pnx prove/verify/sheet/list` native in Rust, byte-identical
      to `crovia-tacet` (reference vectors in CI, proofs cross-verified with
      `tacet-pnx` in both directions, sealed delivery accepted); the GitHub
      Action verifies a PNX proof and attaches its verdict to the PR comment
      (`pnx-proof`, `pnx-assets`, `pnx-fail-on-present`). The published
      conformance vectors (`pnx_002..004`, 21 proofs) run against
      `re pnx verify` in CI as the third runner beside Python and JS. Not yet:
      commit of the run root into a TACET epoch (PNX.md §6 step 5).
- [x] **Weekly Survival Report** replaces the leaderboard: static page, card,
      Atom feed, JSON, Zenodo deposit with DOI, same pipeline as the Crovia
      Silence Report; counts and intervals, no ranks; positioned against
      arXiv 2601.16809 and GitClear (`docs/survival-report.md`). Report #1
      published with DOI 10.5281/zenodo.22863966; the series has concept DOI
      10.5281/zenodo.22863965 and every Monday's report becomes a version of
      it.
- [ ] Hook targets: Cursor `hooks.json`, Gemini CLI.
- [ ] Ledger events optionally stored in `refs/notes/causari` so provenance
      travels with `git push`.

Exit: a stranger verifies an audit seal and a PNX proof offline with the
public key alone; the first report is deposited with a DOI. Reached on
2026-09-20 with 0.2.0.

## Phase 3 — identity and distribution

- [x] Identity: `∵` mark, monospace wordmark, monochrome palette, glyph set
      (`∵`, `⊢`, `·`, `—`), OG images, audit card, badge, site restyle.
- [x] Binary `causari` with `re` as alias; `--help` grouped by area
      (measure · record · ask · move · prove · experimental), with a test
      that every subcommand is listed.
- [x] Signed SLSA attestations on every archive, non-empty release notes,
      Action verifies the checksum, Homebrew tap (`croviatrust/homebrew-tap`),
      Scoop bucket (`croviatrust/scoop-bucket`); crates.io by Trusted
      Publishing (`publish-crate.yml`, environment `crates-io`, OIDC, no
      token): 0.2.0 was published by the tag alone, and the tap and bucket
      followed the release the same hour.
- [x] DCO instead of CLA.
- [x] One legal entity name (Crovia Trust) in NOTICE, trademark line, Cargo
      authors, Action metadata and the canon; git author fixed.
- [~] MCP registry: `io.github.croviatrust/causari` 0.2.0 listed as
      `active`, published by the release workflow after the crate (GitHub
      OIDC). Claude Code plugin: the repo is a marketplace
      (`/plugin marketplace add croviatrust/causari`). Open: Cursor MCP
      directory, awesome lists.
- [x] Family canon: `canon/canon.json` + `scripts/audit_surfaces.py`
      (claims, versions, links, glyphs, installers vs release), in CI on every
      push; live site weekly.

Exit: surface audit `critical=0 high=0`; every install path verifies a
signature; the first external contributor lands a PR without ceremony.

## Open asks (things only the owner can do)

- Hugging Face write token for the Crovia dataset mirror (family item).
- Jurisdiction for the trademark line (the entity name is Crovia Trust).
- `REPORT_PUSH_TOKEN` (fine-grained PAT of an admin, this repository only,
  Contents: read and write) so the weekly workflow can push the report and
  its DOI to `main` through branch protection. Without it the run still
  measures and deposits, and leaves the tree as an artifact.
