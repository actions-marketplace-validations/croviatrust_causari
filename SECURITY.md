# Security

## Reporting

Write to **security@croviatrust.com**. Say what you found, how to reproduce
it and which version (`re --version`). You will get an acknowledgement
within three working days and a fix or a stated decision within thirty. We
credit reporters in the release note unless asked not to. There is no bug
bounty.

Supported: the latest release on the `0.2` line. Older releases are not
patched; upgrade with `curl -fsSL https://causari.dev/install.sh | sh`.

## What the tool stores, and where

Everything Causari writes lives under `.causari/` in the repository it was
run in (gitignored by `re init`) and never leaves the machine. Nothing is
sent to causari.dev or anywhere else; the only network calls are the ones
`re proxy` forwards on your behalf, the `git clone` of `re audit
<owner/repo>`, and the one-time download of `install.sh`.

| Stored | By | Where | In clear? |
|---|---|---|---|
| Prompts (last user message) and completions (assistant text and tool-call arguments) | `re proxy` | `.causari/capture/exchanges.jsonl` | yes, after redaction |
| Prompts, with model and attachment paths | `re hook claude-code`, `re hook cursor` | `.causari/capture/prompts.jsonl` | yes, after redaction |
| Agent answers (Cursor `afterAgentResponse`) | `re hook cursor` | `.causari/capture/exchanges.jsonl` | yes, after redaction |
| Shell commands that changed the tree (command line only, never the output) | `re hook cursor` | event objects, `.causari/index/events.jsonl` | yes, after redaction |
| Event text: `message`, `prompt`, `reasoning` | every recorder (`re record`, MCP `causari_record`, hooks, `re watch`) | `.causari/objects/`, `.causari/index/events.jsonl` | yes, after redaction |
| Workspace snapshots: every file not ignored | hooks, MCP, `re record`, `re watch` | `.causari/objects/` (content-addressed blobs) | yes |
| SHA-256 of request and response bytes; token counts; cost | `re proxy` | `exchanges.jsonl`, seals | hashes only |
| Request-body fingerprints (PNX witness) | `re proxy --pnx` | `.causari/pnx/<run>/fingerprints.log` (mode 0600) | digests only |
| Ed25519 signing keys | `re proxy --seal`, `re audit --seal`, PNX, skills | `.causari/keys/*.key` (mode 0600, fsync) | seed in hex |

Not stored, by construction: request headers (`Authorization`, `x-api-key`
and cookies are forwarded upstream by `re proxy` and never written), full
request or response bodies (only their hashes), reasoning/image/audio
content of completions, shell output, `.env` and `.env.*` files.

## Redaction

Before any of the clear-text fields above is written, credentials in
recognisable formats are replaced by `[redacted:<kind>]` and the record
carries `redactions: <n>`:

`sk-…` API keys (OpenAI, Anthropic and others), Stripe `sk_live_`/`sk_test_`/`rk_live_`,
GitHub `ghp_`/`gho_`/`ghu_`/`ghs_`/`ghr_`/`github_pat_`, GitLab `glpat-`,
Slack `xox[abprs]-`, Hugging Face `hf_`, npm `npm_`, PyPI `pypi-`, AWS access
key ids `AKIA…`, Google `AIza…`, `Bearer <token>` values, JWTs, and PEM
private-key blocks (`-----BEGIN … PRIVATE KEY-----` through the matching
`END`).

This catches the common accident — a key pasted into a prompt, a token in a
`curl` command, an agent echoing a credential back. It is not a classifier:
a bare password, a home-grown token or a secret split across lines passes
through and is stored as typed. Snapshots are not scanned: a credential
inside a tracked source file is captured like any other bytes (snapshots
honour `.gitignore`; `.env*` is always excluded). The
redaction changes the stored text only; seals over `re proxy` traffic commit
to the hashes of the wire bytes, which are unchanged.

## Permissions and durability

Keys and the PNX fingerprint log are created `0600`. Everything else under
`.causari/` is created with the process umask; on a shared machine set
`umask 077` or `chmod -R go-rwx .causari`. Objects and refs are written to
a temporary file and renamed; keys are fsynced before the rename; JSONL
captures are appended without fsync, so a power loss can lose the last
lines of a capture but cannot corrupt an object.

## Retention and deletion

Nothing expires on its own. `.causari/` is yours: delete
`.causari/capture/` to drop every prompt and completion, `.causari/` to
drop everything including the keys (seals issued so far stay verifiable by
whoever holds them; new seals start a new chain). Snapshots are
content-addressed, so a file that was captured once stays in
`.causari/objects/` until the directory is removed.

## `re proxy` is a loopback service

It listens on `127.0.0.1` only and does not authenticate clients. Any
process on the same machine can send requests through it (with its own
credentials — the proxy adds none) and those exchanges are recorded. Do
not expose the port.

## Audit seals and what they prove

A `crovia.seal.v1` audit seal proves that the named issuer key signed these
exact audit bytes for this commit with this method version, and that they
were not altered since. It does not prove the numbers are true (rerun `re
audit` on the commit), nor who controls the key. The seal issuer key is
per repository, generated on first use; anyone with read access to
`.causari/keys/seal-issuer.key` can issue seals in its name.

## Threat model

[`docs/threat-model.md`](docs/threat-model.md) states what Causari defends
against, what it does not, and the assumptions behind each claim.
