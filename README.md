<h1 align="center">∵ causari</h1>

<p align="center"><strong>AI-written code has no author. It has causes. Causari proves them.</strong></p>
<p align="center"><em>How many lines from AI-tagged commits are still alive in your repo? One command, any git repo, no setup. A count, not a grade.</em></p>

<p align="center">
  <a href="https://causari.dev"><strong>causari.dev</strong></a>
  &nbsp;·&nbsp;
  <a href="https://causari.dev/survival">Weekly measurements</a>
  &nbsp;·&nbsp;
  <a href="https://causari.dev/method">Method</a>
  &nbsp;·&nbsp;
  <a href="MANIFESTO.md">Manifesto</a>
  &nbsp;·&nbsp;
  <a href="ROADMAP.md">Roadmap</a>
  &nbsp;·&nbsp;
  <a href="https://github.com/croviatrust/causari/releases">Releases</a>
</p>

<p align="center">
  <img alt="CI" src="https://github.com/croviatrust/causari/actions/workflows/ci.yml/badge.svg?branch=main">
  <img alt="License" src="https://img.shields.io/badge/license-Apache--2.0-3b4252">
  <img alt="Platform" src="https://img.shields.io/badge/linux%20%7C%20macOS%20%7C%20windows-3b4252">
</p>

---

```bash
curl -fsSL https://causari.dev/install.sh | sh     # Linux / macOS (Windows below)

re audit                    # the repo you are in
re audit vercel/next.js     # any public repo, cloned to a temp dir and removed after
```

```console
$ re audit
∵ causari · AI code survival
───────────────────────────────────────────────────
  36 commits analyzed (git metadata only, no setup required)

Verified AI-authored: 3 commits, 1773 introduced, 1773 survived (100.0%)
Probable AI-assisted: none detected
By agent (verified only)
  cursor                 1773 lines,   1773 survived (100.0%)

Confidence notes
  · VERIFIED = explicit metadata (trailers, bot author, etc.)
  · PROBABLE = weak heuristic; may include human-assisted commits
  · UNKNOWN commits are excluded from headline numbers
  · Only lines from AI-tagged commits are measured; inline completions
    (Copilot, Cursor Tab, …) leave no git trace and are invisible here
  · A measurement, not a grade: method at https://causari.dev/method
```

Everyone argues about how much code AI writes. Nobody can check the numbers.
`re audit` reads plain git history — `Co-Authored-By` trailers, bot authors,
agent markers — finds the commits that carry machine-readable AI authorship,
and asks `git blame` how many of their lines are still at HEAD. No model, no
estimate, no survey. Anyone re-runs it and gets the same bytes.

- `--json` the exact bytes behind any published row
- `--summary` Markdown for CI; `--badge` / `--card` one-colour SVGs
- `--save` append a snapshot to track your own trend

**What it cannot see**: code from inline completions (Copilot, Cursor Tab,
Windsurf, …) leaves no git trace and counts as human. Commits without a
trailer are UNKNOWN. A formatter pass or a moved function counts as a death
under method v1. One bulk commit can dominate a line-weighted ratio; ratios
under 5 AI-tagged commits are flagged. All of this is written out at
[causari.dev/method](https://causari.dev/method), with how to contest a number.

## In CI: a count on every pull request

```yaml
# .github/workflows/causari.yml
on: pull_request
permissions: { contents: read, pull-requests: write }
jobs:
  audit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with: { fetch-depth: 0 }      # the audit reads every commit; shallow clones are wrong
      - uses: croviatrust/causari@v1
```

The [Action](https://github.com/marketplace/actions/causari-survival-audit)
downloads the prebuilt Linux binary (a few seconds), runs `re audit --summary`,
writes it to the job summary and posts one sticky comment per PR. No cloud,
no account. A [live example](https://github.com/croviatrust/causari-audit-demo/pull/1).

## The ledger: from "which commit" to "which prompt"

The audit works on any history. If you also want to know *why* a line exists
— the prompt, the model, the files the agent read — Causari records agent
actions as they happen into a local, append-only ledger (`.causari/`,
gitignored), with a snapshot of the tree before and after each one.

```bash
re init                       # create .causari/ (added to .gitignore)
re hook claude-code           # record every Claude Code prompt and edit, exactly
re proxy                      # local LLM proxy: prompts, models, tokens, cost
re watch                      # attribute file changes to captured completions

re why    src/auth.ts:42      # which recorded event introduced this line
re trace  src/auth.ts:42      # upstream: events that fed into it through reads/writes
re impact <event-id>          # downstream: what later events depended on it
re lens   src/auth.ts         # the file annotated line by line with its event
re find   "the JWT refactor"  # search prompts, messages, reasoning
re bisect --test "npm test"   # first recorded event that breaks a test
re revert <id>                # restore the pre-state, with a preview of what else you undo
re fork / re sessions / re switch / re log --all / re diff a..b
```

### What each agent actually gives you today

Claims about "any agent" are cheap. This table is derived from the code and
is kept current; if a cell is wrong, open an issue.

| Agent | Prompt + file, exact | Model, tokens, cost | How |
|---|---|---|---|
| **Claude Code** | yes, via lifecycle hooks | not yet (edits travel as `tool_use`, which the proxy does not join to files yet) | `re hook claude-code` |
| **Aider** | heuristic join, measured | yes | `OPENAI_API_BASE` / `ANTHROPIC_API_BASE` → `re proxy` + `re watch` |
| **Codex CLI**, OpenAI Agents SDK | not yet (Responses API output not parsed) | yes | `OPENAI_BASE_URL` → `re proxy` |
| **Cursor**, **Windsurf**, **Copilot** | only what the agent self-reports via MCP | no | `re mcp` |
| **Cline / Roo**, custom scripts, curl | heuristic join when the completion carries the code as text | yes | base URL → `re proxy` + `re watch` |

Two evidence classes, and every output says which one it is:

- **Declared** (hooks, MCP, `re record`): the agent stated what it did. Exact
  prompt and path. If a human edits a file between two hook events, the hook
  snapshot absorbs that edit into the next agent event — a known limit being
  fixed in [Phase 1](ROADMAP.md).
- **Correlated** (proxy + watch): the lines you inserted are searched inside
  completions captured moments before. A score, not a fact. The adversarial
  harness in [`examples/real-session/`](examples/real-session/RESULTS.md)
  gives measured numbers: 100 % on a clean write, 50 % after a formatter pass,
  wrong per-line attribution when two prompts touch one file in the same
  window.

Everything stays on your machine. `re proxy` stores prompts and completions
verbatim; snapshots store every non-ignored file (`.env*`, `node_modules`,
`target`, `dist`, `build`, `.git` and a few others are excluded by default).
Treat `.causari/` as sensitive.

## Receipts you can verify without us

**Crovia Seals.** `re proxy --seal` issues a
[crovia.seal.v1](https://github.com/croviatrust/crovia-seal) receipt for every
completion: Ed25519-signed, hash-chained, committing to SHA-256 hashes of the
exact request and response bytes (content never leaves the machine). Each
recorded exchange carries its `seal_id` and the same hashes, so a receipt can
be matched to the completion it covers. The implementation passes the
reference conformance vectors; seals verify under the Python reference
implementation and vice versa.

```bash
re proxy --seal          # issue a receipt per completion
re seal verify           # every signature, whole chain, offline
re seal issuer           # your issuer id and public key (read-only)
```

**Causari Proof.** `re proof generate` signs a summary of the ledger — event
count, agents, models, files touched, a digest over the exact set of event ids
— with a dedicated key, domain-separated, canonicalised with CSC-1. `re proof
verify` fails closed: a proof containing any field the signer did not sign does
not even parse.

A proof says *this is what the ledger contained*, signed by this key. It does
not say the ledger is complete. That distinction is on the output.

## Experimental

These commands exist, work in the demos, and are not yet held to the standard
above. They are out of the proof and out of the front page until they are.

- `re skill distill / verify / export / import / pull / trust`: signed units
  of past work; the trust ladder (recorded → verified → proven) currently
  measures file existence and recall counts, not correctness.
- `re brief`: a Markdown briefing of past work for a model's context.
- `re guard`: substring rules over recent changes; never gates a build.
- `re churn`, `re report`: survival measured over the ledger instead of git,
  with cost extrapolated from a static price table.
- `re mcp`: stdio MCP server with `causari_record`, `causari_recall`,
  `causari_why`; `re mcp --install` prints the client config.

## Install

```bash
# Linux / macOS
curl -fsSL https://causari.dev/install.sh | sh

# Windows (PowerShell)
irm https://causari.dev/install.ps1 | iex

# Homebrew (macOS, Linux)
brew install croviatrust/tap/causari

# Scoop (Windows)
scoop bucket add causari https://github.com/croviatrust/scoop-bucket && scoop install causari

# from source (Rust 1.85+)
cargo install --git https://github.com/croviatrust/causari --locked
```

One program under two names: `causari` is the binary, `re` is the short alias
every example uses. Both are in every archive and both are installed. One
static binary, about 5 MB, for Linux (x86_64, aarch64), macOS (x86_64, Apple
silicon) and Windows (x86_64), installed to `~/.local/bin` (or
`%LOCALAPPDATA%\Programs\causari`). The installer checks the archive's
SHA-256 against the `SHA256SUMS.txt` published with each release and refuses
to install on a mismatch. From v0.2.0, every archive and the sums file carry a
signed SLSA build-provenance attestation from the release workflow:

```bash
gh attestation verify causari-v0.2.0-x86_64-unknown-linux-gnu.tar.gz --repo croviatrust/causari
```

By hand:

```bash
VERSION=$(curl -fsSL https://api.github.com/repos/croviatrust/causari/releases/latest | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p')
TARGET=x86_64-unknown-linux-gnu
base="https://github.com/croviatrust/causari/releases/download/$VERSION"
curl -fsSLO "$base/causari-$VERSION-$TARGET.tar.gz" && curl -fsSLO "$base/SHA256SUMS.txt"
sha256sum --ignore-missing -c SHA256SUMS.txt && tar -xzf "causari-$VERSION-$TARGET.tar.gz" && install -m755 causari re ~/.local/bin/
```

The [Homebrew tap](https://github.com/croviatrust/homebrew-tap) and the
[Scoop bucket](https://github.com/croviatrust/scoop-bucket) render their
manifests from each release's `SHA256SUMS.txt` and re-render every six hours.
crates.io is on the [roadmap](ROADMAP.md).

Demos: `scripts/demo*.sh|ps1` (mock LLM included), `examples/real-session/`
(the adversarial harness), `scripts/recovery_lab.py` (revert/bisect stress
lab).

## How it works, in one paragraph

Every recorded event is a content-addressed object (BLAKE3) with the tree
before, the tree after, the agent, model and tool, the prompt, declared reads
and writes, tokens and cost, and a parent. Sessions are refs; forks are
implicit. `re why` finds the first event on the current chain whose
before/after diff inserted the line; `re trace` follows reads and writes
backwards from there; `re impact` forwards. Unchanged files share blobs
between snapshots. The full design, and its current limits, are in
[`docs/review-2026-09-20/`](docs/review-2026-09-20/).

## Where this is going

Causari does not compete with provenance trackers (Agent Trace, git-ai,
`Assisted-by:` trailers, Entire checkpoints); it reads them, measures with a
public method, and signs the result so a third party can verify it offline.
Next: `git blame -w -M -C` and per-commit caps in the audit; Agent Trace and
`Assisted-by:` readers; the audit result as a Seal; a PNX witness mode in
the proxy that proves what an agent session did *not* send to the model.
Phases and exit criteria: [`ROADMAP.md`](ROADMAP.md).

## Family

Causari is part of [Crovia](https://croviatrust.com), one grammar in three
tenses: **TACET** proves a model's silence about its training data, **PNX**
proves an agent's egress carried no protected bytes, **Causari** proves why a
line of code exists and whether it is still there. Same rules everywhere:
reproducible numbers, no verdicts, offline verification, limits stated first.

## License

Apache-2.0 (see `LICENSE`). "Causari" is a trademark of Crovia Trust; the
license does not grant trademark rights (see `NOTICE`). Contributing: see
`CONTRIBUTING.md`.
