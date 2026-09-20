# Synthesis

## What Causari is today, stripped of marketing

A 14k-line Rust binary, 133 green tests, green CI on 5 targets, containing
**three products** with three different taglines:

1. **`re audit`**: reads the git history of any repo, recognises "AI" commits
   from trailers (`Co-Authored-By: Claude`, bot authors, aider markers) and
   measures with `git blame` how many of those lines are still alive at HEAD.
   Zero setup, fast on medium repos (aider: 11 s). The hook.
2. **The causal ledger** (`init/record/watch/proxy/hook/mcp/why/trace/impact/
   lens/bisect/fork/revert/skill/brief/churn/report`): content-addressed events
   (BLAKE3) with pre/post tree snapshots, sessions as refs, prompt and model
   inline. The actual thesis of the project.
3. **Cryptographic receipts** (`proxy --seal`, `seal verify`, `proof`):
   issuance of `crovia.seal.v1` and a signed proof of the ledger.

Traction on 2026-09-20: 7 stars, 0 forks, 0 external contributors, 1 issue
ever, not on crates.io, ~100 downloads nearly all from the project's own CI,
Show HN at 3 points. The site still sells the 2025 tagline ("Trace intent.
Debug causality.") while README and GitHub sell survival.

## What is genuinely good

- The atom **event = prompt + pre/post snapshot** is right: "what did the
  agent change" becomes a set difference, not a claim.
- `re audit` as the entry point is strong, quotable, social. No other open,
  reproducible tool measures this on any repo.
- The Seal implementation is **byte-for-byte conformant** with the Python
  reference: all 15 vectors pass and seals emitted by the proxy verify under
  `crovia_seal`. It is not a fork.
- The recent hardening (verify-on-read, CAS on refs, preflight before
  restore, bisect exit-code discipline) is well done and the tests cite the
  findings they cover.
- Site security headers, checksum-verifying installer, 5-target CI: above
  average for a project of this size.

## The hard truths, by severity

**[R]** = reproduced against the built binary.

1. **[R] `re watch` fabricates history on Linux.** Idle repo, 38 events in
   12 s, `writes` containing `.git/…` and the empty root. `Cargo.toml` asks for
   `notify 6.1` but `notify-debouncer-mini 0.7` pulls `notify 8.2`, which also
   emits open events; the snapshot opens every file and the loop never ends.
   The "tree unchanged → skip" check exists in the hook path and not in watch.
   The validation harness ran on Windows. Every derived number (`proof`,
   `churn`, `why`) is polluted on the OS where CI and servers run.
2. **The leaderboard publishes verdicts about non-consenting third parties
   with an artefact-dominated metric.** crewAI: 4.9 % over 3.3 M lines, of
   which 3.2 M from **one commit** of documentation versioning
   (`docs/v1.14.7/**`); the site renders it as "High churn: most AI-written
   lines did not survive" with a ready-made badge. At the other end
   `openai/openai-agents-python` at 99.99 % on 2 commits. No per-commit cap,
   no sample floor, no confidence interval, blame without `-w -M -C` (a
   formatter "kills" every AI line). A public bot lets anyone audit any repo
   by opening an issue. This is exactly what the Crovia canon forbids
   ("record, do not judge") and what was just retired on croviatrust.com.
   PR #52 confirms false positives (Devin Jones, `AI-Assisted: no`) were live
   for weeks at confidence 1.0.
3. **[R] `re proof verify` is fail-open.** `"injected_claim": "SOC2
   certified"` in the manifest: `ok signature valid`. The proof is signed with
   the skill key, no domain separator, with a canonicaliser different from the
   CSC-1 the same binary implements for Seals. The reproduced proof "attested
   41 events": all phantoms from item 1. **[R]** Private keys are
   `-rw-r--r--`.
4. **The "universal fallback" works with no modern agent.** The proxy does
   not extract text from `tool_calls` (OpenAI), `tool_use` (Anthropic) nor
   from the Responses API: `response_text: ""` for Claude Code, Codex, Cursor,
   Agents SDK. The content join has nothing to bind to and their costs are
   never attributed. Works with Aider and the demo scripts. Cursor and
   Windsurf, listed under the fallback in the README, have no realistic path.
5. **Attribution lies in the cases that matter.** The hook snapshot takes
   the whole tree: a human edit between two agent edits is attributed to the
   next prompt. `re revert` records nothing, so the next event owns the
   revert. The cross-session fallback assigns another session's prompt.
   "Zero false attribution" is false in the interleaved case.
6. **Seals are orphan receipts.** The stored exchange has no `seal_id` and
   the raw bytes are discarded: no seal can be linked to a completion, event
   or line. "Which model wrote this code, and can you prove it? One file and
   one public key" is not deliverable today.
7. **Skill "trust" is a counter.** `verified` = the written files still
   exist (not the content); `proven` = recalled 3 times, with `uses` outside
   the signature and bumped by `re brief` on every run. Then it lands in the
   proof.
8. **Storage: correct in the small, unsustainable in the large.** Object
   writes without temp+rename: a truncated blob blocks that content forever
   and `record` keeps "succeeding" on top. Refs written truncate-then-write:
   a mid-write crash = silently orphaned session. Every event re-reads and
   re-hashes the whole tree (no stat cache); every query is O(events ×
   files). On 20k files: `re why` 1.3 s, `re trace` 3 s at 25 events. The
   component ignore list silently drops `src/build/`. No mode/symlink in
   snapshots; names with `:` or a trailing dot make the snapshot
   unrestorable. No gc/fsck.
9. **Three implementations of "who introduced this line"** (`why`, `trace`,
   MCP) that already contradict each other on the root event; the MCP demo
   shows the bug in its own output.
10. **Incoherent public surface**: versions 0.1.1/0.1.0/0.1.5 scattered,
    binary sizes 800 KB/2 MB/3 MB (real 5.3 MB), `/verify` 404, `/repo?r=`
    hijacked by redirects, `llms.txt` with non-existent MCP commands,
    "signed SHA256SUMS" unsigned, `action.yml` without checksum
    verification, CLA bot non-existent, empty release notes since v0.1.1,
    v0.1.0 still marked BSL.

## The market, which does not exist in the repo

| | What it does | Traction | Relation to Causari |
|---|---|---|---|
| **Entire / Checkpoints** (Thomas Dohmke, ex GitHub CEO) | Agent + git hooks, prompt/transcript/tokens per commit, stored in a git branch, UI | 5.1k★, $60M at $300M (Feb 2026) | Direct competitor of the ledger. Git-native. `.causari/` is gitignored and dies on the laptop. |
| **git-ai** | Per-line AI authorship in `refs/notes/ai`, survives rebase/squash, standard v3, Teams tier | 2.8k★, Rust | Owns "attribution that survives history". Causari already reads its notes. |
| **Agent Trace** (Cursor) | Open RFC attributing code ranges to conversations/models; Cognition, Sourcegraph, Google Jules, Cloudflare, Vercel, git-ai, Cline, OpenCode | consortium | Occupies exactly the roadmap's "Agent Provenance Protocol" slot. Causari neither reads nor emits it. |
| **`Assisted-by:` trailer** | Linux kernel, Fedora, LLVM, OpenTelemetry | de-facto standard | `re audit` does not parse it. |
| **GitClear** | Churn / "maintainability gap" reports, incumbent for eng managers | established | Owns the manager narrative. |
| **GitHub** | "AI code survival rate" announced on roadmap | platform | Will make Causari's hook native. The window is now. |
| **arXiv 2601.16809 "Will It Survive?"** | 201 projects: AI code has a **16 % lower** modification hazard | academic | The "AI Waste" thesis presumes the opposite and does not cite it. |

## The thesis

Everyone above answers "who wrote this line?". Attribution: crowded, funded,
soon native in GitHub. Causari cannot win there.

The question nobody asks, and which is Crovia's DNA: **"is this claim about
AI code verifiable by someone who trusts neither you nor the tool?"** Every
number about AI in code today is a self-report.

> **AI-written code has no author. It has causes. Causari proves them.**

Causari is not another provenance tracker; it is **the proof layer above
all trackers**. It reads everyone's format (Agent Trace, git-ai, kernel
trailers, Entire checkpoints, its own hooks), measures with a public,
reproducible method, and signs the result as `crovia.seal.v1` so anyone can
verify offline. Neutral, not a contender. Same move Crovia makes on models.

Second, buried insight: **`re proxy` is already a PNX egress witness.** With
`crovia-tacet` it can emit a signed PNX run sheet per coding session: "this
Claude Code session did not exfiltrate the keys, the customer data, the
protected sources", with the 47-byte guarantee, verifiable offline. Nobody
offers this; the protocol is ours.

The family becomes one grammar: TACET proves silence, PNX proves
non-exfiltration, **Causari proves cause and survival**.
