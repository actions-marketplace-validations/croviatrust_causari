# Public surface and market position

## 1. Claims per surface, and inconsistencies

| Surface | Self-description |
|---|---|
| README h3, GitHub description | "How much of your AI-written code actually survives? … one command, any git repo, zero setup." |
| README logo alt | "intent-addressable code" |
| `site/index.html` title/OG | "Trace intent. Debug causality." / "The first content-addressable ledger for AI agents" |
| `Cargo.toml` | "bidirectional causal version control for AI-agent actions" |
| `CONTRIBUTING.md:62` | "we are *not* a VCS" |
| `action.yml` | "Causari Survival Audit" |
| v0.1.0 release notes (live) | "Licensed under BSL 1.1" |
| Show HN / Dev.to (July 2026) | "Git tells you what changed. Causari tells you why." |

Inconsistencies: versions (Cargo 0.1.5; site hero and JSON-LD 0.1.1; README
example v0.1.0; bug template 0.1.0); binary size (800 KB / 2 MB / 3 MB);
install flags (`-fsSL` vs `-sSf`; `iwr` vs `irm`); `llms.txt` says
`re mcp serve` exposing why/trace/impact/find/skills (real: `re mcp`, three
tools); `guard` described two different ways; "15+ commands" vs 27; license
flip BSL → Apache inside 0.1.x with v0.1.0 notes unchanged; entity name
Croviatrust / Crovia Trust / Crovia; emails `hello@`, `security@`,
`bot@causari.dev`; `site/README.md` says Pages, `wrangler.jsonc` is Workers;
`www` unresponsive.

Broken URLs: `/repo?r=` hijacked by `_redirects` → GitHub (kills the "claim
your profile" flow); `/verify` **404** (target of the README proof badge);
`repo.html` → `/assets/favicon.svg` 404; sitemap lists only `/`; no
canonical.

Marketing vs reality: "Real commands. Real output. No mockups." with 8-hex
ids vs real 10-hex and a `re why` layout the binary does not produce; "10
seconds" vs `timeout 3600`; "prebuilt binary (seconds)" vs cargo-install
fallback on non-x86_64-linux; "No telemetry" while loading Google Fonts;
"signed SHA256SUMS" unsigned; Sigstore/Homebrew/Scoop/nix promised in v0.1.0
notes, none exist; CLA bot claimed, none exists; separate stale
`causari-guard-action` repo; `src/audit.rs` does not parse `Assisted-by:`;
roadmap promises an "Agent Provenance Protocol" while Agent Trace exists and
is never mentioned; the comparison table benchmarks against git / LangSmith /
IDE checkpoints, none of the actual competitors.

## 2. Live status (2026-09-20)

Site 200 on Cloudflare; HSTS preload, CSP, XFO DENY, nosniff: good baseline.
JSON-LD `SoftwareApplication` stale version, no `offers`. `llms.txt` served
with wrong URLs. Leaderboard live data 2026-09-14, weekly cron green.

crates.io: not published. GitHub: 7★, 0 forks, 0 watchers, 1 issue ever, 51
PRs (15 Dependabot, 1 Cloudflare, rest maintainer), no external human
contributor; owner is a **User** account, 3 followers, 14 repos. Releases
v0.1.0…v0.1.5 + rolling `v1`; bodies empty since v0.1.1; ~100 downloads
total, macOS arm64 ≈ 2. Marketplace: listed. Search: only first-party
surfaces; "intent-addressable code" has zero independent use. HN 3 points,
Dev.to 1 reaction, awesome-mcp-servers PR unmerged since 08-30, MCP registry
0 results, no Homebrew, no Product Hunt, no social handles.

## 3. Messaging

The survival hook is the strongest asset, undermined by: the site still
leading with the ledger story; visibly fragile methodology in public
(crewAI); peer-reviewed prior art (arXiv 2601.16809) finding the opposite
narrative, uncited. "Intent-addressable code" is not understandable to a
stranger. Three buyers addressed at once (developer, eng manager, CISO);
only the first is served; the paid plane is in the future tense everywhere.

Honest one-sentence pitch a stranger would repeat: "a Rust CLI that reads
your git history to tell you what percentage of AI-written code is still
alive at HEAD, per agent, and can optionally record every Claude Code prompt
and edit so `re why file:line` shows which prompt produced a line."

## 4. Competitive landscape

See `00-synthesis.md`. Additional near-tier: agentdiff (43★, ed25519-signed
line attribution, Agent Trace format, 7 agents' hooks), AgentNote (15★,
`refs/notes/agentnote`), Posthook (Go, SQLite + notes), Origin CLI (`origin
blame/why`, hosted governance dashboard: the business-model twin), Claude
Code `/rewind` checkpoints (overlaps revert/bisect/fork), GitButler (21.7k★,
complementary), jj (31.7k★, `jj-ai` emits git-ai notes; integration target),
Semgrep/Snyk/SLSA (complementary; Proof could be an in-toto attestation).

## 5. Legal, licensing, community

Apache-2.0 + a CLA granting relicensing "under any license, including
commercial, proprietary" for a zero-contributor project with no CLA bot:
pure friction; DCO suffices unless a proprietary core is planned. BSL → Apache
flip undocumented in releases. Trademark claimed with an inconsistent owner
entity and no legal form/jurisdiction. The leaderboard names 30 repos weekly
with value-laden wording ("waste"), no opt-out; low legal risk, real
reputational risk to Causari itself. `curl | sh` against an unsigned sums
file described as signed.

## 6. Channels not yet used

crates.io; Homebrew tap (cargo-dist automates Homebrew, Scoop, MSI, release
notes); winget/nix/npx shim; Marketplace badge + arm64 runners; official MCP
registry (`server.json`); Claude Code plugin marketplace, Cursor MCP
directory, Cline/Windsurf marketplaces, Smithery, Glama; awesome-rust,
awesome-cli-apps, awesome-claude-code; Agent Trace ecosystem (reader/writer +
listing PR); HN relaunch on the data, not the tool; Product Hunt; methodology
blog posts; technical report / arXiv; shields.io endpoint from the report
data; social handles; newsletters (Pragmatic Engineer, TLDR, Changelog,
Console.dev).

## 7. Ranked

1. One front door: the survival measurement.
2. The leaderboard's credibility is the whole business and it is leaking.
3. Adopt the ecosystem's metadata (`Assisted-by:`, Agent Trace); don't invent.
4. Competitors are funded and git-native; lean into retroactive, verifiable.
5. Fix broken public URLs before any launch.
6. Ship the Rust-CLI distribution basics.
7. Stop making claims the surfaces don't back.
8. Drop or justify the CLA.
9. Name the buyer for the paid plane or remove it from the free pitch.
10. Position against the evidence (arXiv, GitClear, GitHub), not around it.
