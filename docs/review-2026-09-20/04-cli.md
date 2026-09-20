# The CLI as a product

Method: read every command; built `target/release/re` (1 m 14 s cold, 5.3 MB
stripped+LTO); ran every `--help`, all `re audit` modes, `re init`, the five
demo scripts, and a manual walkthrough against a throwaway repo (28 commits,
218 files, with `node_modules`).

## 1. Command inventory

| Command | Needs ledger | Maturity | Note |
|---|---|---|---|
| `init` | creates | works | fails on a bare `.causari/` left by `audit --save` |
| `record` | yes | thin | zero flags records "(no message)"; model/prompt/reads/writes/cost only via `--stdin` JSON |
| `log` | yes | works | never shows prompt/model/cost; no `--json` |
| `show` | yes | partial | omits prompt, model, reasoning, reads, writes, tokens, cost |
| `revert` | yes | works | causal preview + dry-run verified |
| `diff` | yes | works | range = `a.post..b.post`, undocumented |
| `why` | yes | works | handles root event |
| `watch` | yes | broken on real repos | phantom events (see 02-capture) |
| `bisect` | yes | works | re-tests `bad` already validated |
| `fork` | yes | works | |
| `sessions` | yes | works | |
| `switch` | yes | sharp | silently deletes files not in target tip |
| `trace` | yes | partial | rejects root-event lines `why` attributes |
| `find` | yes | works | substring-count scoring |
| `impact` | yes | works | current session only |
| `lens` | yes | works | panics on `\| head` |
| `skill *` | yes | over-claims | verified ≡ files exist |
| `brief` | yes | works | footer recommends invalid `re why <file>` |
| `proof generate/verify` | yes | works | fail-open on unknown fields (see 03) |
| `mcp` | at call | inconsistent | `causari_why` disagrees with CLI on root lines |
| `guard` | no | crude, no gate | substring rules, always exit 0 |
| `churn` | yes | works | counts `human` as an AI agent in headline |
| `audit` | no | works | 0.15 s on 28 commits; `--save` side-effect bug |
| `report` | yes | works | self-contained HTML |
| `proxy` | yes | works | verified with `mock-llm.py` |
| `seal verify/list/issuer` | yes | works | `issuer` silently creates a keypair |
| `hook claude-code` | yes | works | idempotent; bare `re` in committed file |
| `hook-event` (hidden) | yes | works | `--help` omits `session-start` |

## 2. First-run experience

`curl | sh` live; release v0.1.5 has 5 targets + `SHA256SUMS.txt`. Not on
crates.io; `cargo install --git` needs Rust ≥ 1.85 and fails opaquely on
older toolchains. `re audit` on 28 commits: 0.15 s. The right first command,
but on most repos the result is `none detected` with no hint of which signals
were looked for and no next step; it never mentions `re init` or `re hook`,
and `re init`'s next steps never mention `re audit`. The two entry points do
not know about each other.

Binary name `re`: two letters, generic, undiscoverable, baked PATH-relative
into a committed `.claude/settings.json`. Size claims 800 KB / 2 MB vs 5.3 MB
real (2.1–2.4 MB compressed).

## 3. Coherence: three products

1. Survival audit (`audit`, `guard` fallback, leaderboard, Action).
2. Causal ledger (everything else).
3. Receipts (`seal`, `proxy --seal`, `proof`), with two unrelated Ed25519
   identities per repo.

Taglines disagree across README, `banner.rs`, `cli.rs`, `Cargo.toml`.
`why`/`trace`/`lens`/`impact` are four views of one graph with three
line-introduction implementations that disagree. `churn` vs `audit` measure
the same metric from two sources with different thresholds (20/40 % waste vs
40/70 % survival) and words. `report` = `churn` to HTML. `find`/`brief`/
`causari_recall` = three rankers; only `brief` bumps counters. `guard` calls a
human git author "agent".

Merge candidates: `report` → `churn --html`; `impact`/`trace`/`lens` under
`why`; `sessions`+`switch`+`fork` → `session`; `seal` behind `proxy --seal`.

## 4. Output quality

Colours respect TTY and `NO_COLOR`; palette inconsistent. JSON only for
`audit`. Exit codes: `guard`/`churn`/`audit` always 0; only `proof verify`,
`seal verify`, `skill verify` can gate. Errors mostly good; weak on non-git
dir, missing proof file, missing source file. `re --help` is a flat 27-item
list. Vocabulary drift: event/action/change/changeset, session/branch/
timeline, exchange/capture, skill/experience/lesson, intent = prompt,
agent = model; three badge commands write to three default paths; "1 commits".

## 5. Release / install

Targets: linux gnu x86_64/aarch64, macOS x86_64/aarch64, windows x86_64. No
musl. `SHA256SUMS.txt` **not signed** although `install.sh:13` says "signed";
no cosign/attestation/SLSA. `install.sh` verifies sha256, uses `mktemp`+trap;
resolves `latest` via unauthenticated GitHub API (rate-limited). `action.yml`
downloads the binary with **no checksum verification**; fallback `cargo
install --git` takes minutes. `guard.yml` builds from source per PR. No
Homebrew, no crates.io, no binstall metadata, no cargo-dist. Pre-push hook
claims to mirror lint "exactly" but runs tests lint does not.

## 6. Bugs (reproduced)

1. `re audit --save` creates a bare `.causari/` → `re init` "already exists",
   `re log`/`churn`/`guard` fail reading HEAD; not gitignored.
2. `re watch` phantom events on a 218-file tree (4–5 in 3 s, empty diffs,
   `writes` listing `.git`, `node_modules/*`, empty path).
3. Root-event line: `why` attributes, `trace` and MCP `causari_why` refuse;
   `demo-mcp.sh` shows it.
4. `re lens README.md | head -8` → broken-pipe panic (all `println!`).
5. `re seal issuer` writes a keypair.
6. `scripts/demo-record.sh` uses flags (`--reads`, `--writes`, `re trace
   <file>`) the binary rejects; the landing-page GIF shows a different CLI.
7. `re brief` footer recommends invalid syntax.
8. `re guard` 4 alerts → exit 0; summary says "failing".
9. `re switch` deletes unrecorded files without confirmation.
10. `hook-event --help` omits `session-start`.

## 7. Ranked

1. One line-provenance engine.
2. Watch no-op filtering and ignore-aware `writes`.
3. `re audit` hands off to the ledger; `re init` mentions `audit`/`hook`.
4. Rename the trust ladder or its evidence.
5. Decide what the product is.
6. Rename the binary (or ship `causari` alongside `re`).
7. `--json` and meaningful exit codes everywhere.
8. Supply chain: sign or stop saying signed; verify in the Action; publish.
9. `re show` shows the event.
10. Fix the small credibility leaks (each minutes; each hit in ten minutes).
