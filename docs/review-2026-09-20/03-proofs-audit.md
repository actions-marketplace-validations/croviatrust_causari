# Cryptography, proofs, audit and the survival metric

## 1. The survival metric

### Detection (`audit.rs:120-326`), per commit, metadata only, first match wins

| Signal | Confidence | Class |
|---|---|---|
| `refs/notes/ai` note with `schema_version` (git-ai) | 1.0 | VERIFIED |
| Trailer key `Drafted-With`, `Executed-By`, `AI-Model`, `AI-Agent`, `AI-Tool` (any non-empty value) | 1.0 | VERIFIED |
| Trailer key `AI-Session-ID`, `AI-Provenance`, `AI-Generated`, `AI-Assisted` (any value) | 1.0 | VERIFIED, agent `ai` |
| `Co-Authored-By` starting with `claude` / containing `noreply@anthropic.com`, or *containing* `copilot`/`cursor`/`aider`/`codex`/`chatgpt`/`gemini`/`openhands`/`devin`/`jules` | 1.0 | VERIFIED |
| Author identity (`aider`, `noreply@anthropic.com`, `devin-ai`, `openhands`, `cursoragent`, `google-labs-jules`) | 0.95 | VERIFIED |
| Message `(aider)` / `Generated with Claude Code` | 0.7 / 0.8 | PROBABLE |

Thresholds VERIFIED ≥ 0.9, PROBABLE ≥ 0.5. Merges excluded.

**Confirmed false positives (VERIFIED, 1.0)**, pre-PR #52: `Co-Authored-By:
Devin Smith` → `devin`; `AI-Assisted: no` → `ai`; `Executed-By: CI pipeline`
in prose → `ci` (scan is over all message lines, not the trailer block);
`contains("aider")` matches "raider", `contains("cursor")` "precursor";
`starts_with("claude")` every co-author named Claude. PR #52 fixed the
name-collision agents and negative disclosures; the trailer-block and
substring issues remain.

**Structural false negatives**: Copilot inline, Cursor Tab, Windsurf,
Continue, Zed AI, JetBrains AI leave no git trace; Claude Code's trailer is
disableable; `copilot-swe-agent[bot]` author not detected; no
`Assisted-by:` (kernel/Fedora/LLVM/OTel); granularity is the commit, never
the hunk. The denominator is "lines from AI-tagged commits", not "AI-written
lines".

### Survival (`audit.rs:405-694`)

Introduced = `git log --no-merges --numstat` added lines per attributed
commit, text files only, hard-coded generated-path exclusions. Surviving =
`git blame --line-porcelain HEAD` per tracked file (parallel), lines blamed to
the AI commit, clamped to introduced. Rate = Σsurviving / Σintroduced
(line-weighted mean).

Blame runs with **no `-w`, `-M`, `-C`, `--ignore-revs-file`**. Confirmed:
re-indenting an AI line in a human commit kills it (1.0 → 0.75; `-w` keeps
it). Mass reformat = every touched AI line dead; moving a function = death +
new introduction; conflict resolutions dropped; shallow clones not detected
(`fetch-depth: 1` silently wrong).

### Statistical soundness

**Outlier dominance, confirmed.** crewAI: `cursor` 15 commits / 3,238,232
introduced / 77,874 surviving → repo at 4.7 %, rendered by `repo.js:50` as
"High churn: most AI-written lines did not survive". Source: **one** commit
`a237ebab` "adopt directory-based docs versioning", 15,791 files, 3.2 M lines
of copied `docs/v1.14.7/**/*.mdx`. No minimum sample, no per-commit cap, no
median, no CI, no age normalisation; `croviatrust/causari` appears with 1
commit at 96.9 %; ranking is by rate descending. Survival ≠ quality.

### "10 seconds, any repo"

True for small/medium repos (anthropic-sdk-python 5.9 s incl. clone; aider
11.0 s). Not for large: one blame per tracked file + full clone;
`leaderboard.yml` allows `timeout 3600` per repo and a 300-minute job;
`audit-request.yml` warns about the 45-minute budget; committed
`survival-data.json` covered 13 of 30 repos.

## 2. Seal and Proof

### Crovia Seal (`seal.rs`) — conformant

CSC-1 serializer (`seal.rs:37-121`), payload `"CROVIA-SEAL-v1" || 0x0A ||
CSC1(S \ {signature, witnesses})`, Ed25519 over the payload, chain
`prev_seal_hash = sha256(P(prev))`, base32 nonce. The 5 vendored vectors are
byte-identical to `crovia-seal/conformance/vectors/v1/`. **All 10 valid
reference vectors accept, all 5 invalid reject.** Seals emitted by
`re proxy --seal` verify under the Python reference. Not a fork.

Divergences (verifier more permissive than the reference, unsigned region,
confirmed with mutated `seal_001`): uppercase `sig_hex` accepted;
`payload_hash_alg: "md5"` accepted (never checked, `seal.rs:520-528`); extra
key inside `signature` accepted (fail-closed only at depth 1,
`seal.rs:488-492`); witness with `alg: rsa` and no `id` accepted; nonce ≥ 16
instead of 26; non-ms timestamps; uppercase `issuer.pubkey.key_hex` (inside
payload → verifies here, fails in reference); `anchor`/`checks` unvalidated.
None forgeable; all make two verifiers disagree.

Key management: seed hex in `.causari/keys/seal-issuer.key` via `fs::write`,
**`-rw-r--r--`** confirmed. No passphrase, rotation, revocation, key id;
issuer URN identical for every user; `re seal issuer` prints the hard-coded
URN regardless of `--seal-issuer`. `chain_tail` re-reads the whole log per
emit: O(n²).

**The real gap**: the exchange record carries **no `seal_id`** and only the
parsed prompt/`response_text`; raw bytes are not persisted. No seal can be
matched to a completion, event or line; `re why` cannot cite a seal.
`generator.id` = request model (upstream's resolved model discarded),
`version` always null, `modality` always `"text"`.

### Causari Proof (`proof.rs`) — weaker construction

Schema `causari.proof.v0.1`; manifest = repo dir name, `generated_at`, HEAD
id, counts, agents/models, `files_touched`, skill trust counts,
`ledger_digest = BLAKE3(sorted event ids)`. Signed with `object::canonical_json`
(byte-order keys, floats allowed) and **no domain separator**, using **the
skill-signing key** (`proof.rs:139`).

**Fail-open on unknown fields, confirmed**: `ProofManifest` lacks
`deny_unknown_fields`; `"injected_claim": "SOC2 certified"` → `ok signature
valid`. Semantics: a self-attestation of integrity, not provenance;
`skills.proven` depends on unsigned `stats.uses` bumped by `re brief`.
`badge_markdown` links to `causari.dev/verify?k=…` (404).

## 3. Audit / guard / churn / report / brief

- `audit --summary`: 🟢/🟡/🔴 at 70 %/40 %, no sample guard.
- `guard` (`guard.rs:311-472`): four substring rules; `db` ⊂ `feedback`,
  `token` ⊂ `tokenizer`, `spec` ⊂ `inspect`, `config` ⊂ `tsconfig`; git
  fallback names human authors next to 🔴; `guard.yml` `|| true`, never gates.
- `churn`: HEAD chain only (inconsistent with `proof.rs:84` `walk_all`); root
  event with an agent claims the whole pre-existing tree as AI-introduced
  (`churn.rs:249-255`); `wasted_cost = cost × (1 − survival)` per event;
  "AI Waste Score" = `1 − survival`; the README dollars are a linear
  extrapolation from a static price table.
- `report`: churn as self-contained HTML; claim justified.
- `brief`: "Signed and verified by Causari" where verified = files exist;
  bumps `uses` on every CLI run.

## 4. The leaderboard

30 third-party repos, weekly Monday 05:17 UTC, publishes per-agent stats,
rank, colour class, ▲/▼ trend, verdict sentence, badge URL, by **force-push**
to `leaderboard-data`. `audit-request.yml` lets **anyone** trigger a public
audit of **any** repo via an issue. Against the Crovia canon ("record, do not
judge"; `crovia-seal/SPEC.md §1.2` "MUST NOT encode verdicts on quality"):
ranks, colours and "high churn" are grades about named third parties and
named vendors (Cursor 2.4 % in crewAI), published without consent, with a
metric dominated by artefacts, with no removal path, no publication history.

## 5. Bugs

Crypto: 1 `proof.rs:43` no `deny_unknown_fields`; 2 `proof.rs:139-141`
shared key, no domain, non-CSC-1; 3 `seal.rs:263`, `skill.rs:197` keys 0644;
4 `seal.rs:520-528, 543-560, 601, 666, 670` permissive verifier; 5
`skill.rs:149-163` + `brief.rs:99-101` unsigned `uses`; 6 `proxy.rs:297-330`
no `seal_id`, raw bytes discarded; 7 `seal.rs:213-228` quadratic chain tail;
8 `commands/seal.rs:101` ignores `--seal-issuer`.
Audit: 9 `audit.rs:157-160` negative values (fixed by #52); 10
`audit.rs:150-156` prose trailers; 11 `audit.rs:179-241` name collisions
(partly fixed by #52); 12 `audit.rs:641` blame without `-w -M -C`; 13 no
`copilot-swe-agent[bot]`; 14 no cap / floor; 15 no shallow detection; 16
`commands/audit.rs:67` temp dir by pid.
Guard/churn: 17 substring rules; 18 root-event claim; 19 HEAD-only; 20
`leaderboard.yml:96` force-push.

## 6. Ranked

1. Hunk-level, robust survival before publishing another figure: `-w -M -C`,
   per-commit cap or medians/percentiles + CI, sample floor, age normalisation.
2. Stop publishing verdicts about third parties: counts only, opt-in/opt-out,
   removal path, no public audit bot.
3. Link seals to content: persist raw hashes, `seal_id` on exchanges and
   events, `re why` cites a seal.
4. Rebuild `re proof` on the seal machinery: CSC-1, domain prefix,
   `deny_unknown_fields`, dedicated key.
5. Key hygiene: 0600, key ids, rotation, per-user issuer URNs.
6. Trailer detector on actual git trailers and known identities.
7. Seal verifier at full reference parity; whole conformance suite in CI.
8. Sign the trust ladder or drop ★ from the proof.
9. Explicit coverage in every output; refuse shallow clones.
10. Guard: path globs, no human names in 🔴 tables, or out of the README.
