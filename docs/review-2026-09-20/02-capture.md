# Capture and attribution

## 1. Capture paths

### `re proxy` (`commands/proxy.rs`, parsing in `capture.rs`)

`tiny_http` on `127.0.0.1:<port>` (default 4242), no `--host`, plain HTTP, one
thread per request. Upstream via `ureq` + rustls, 15 s connect timeout, no
read timeout; no `HTTPS_PROXY`, no system CA store. Routing: `/anthropic/*`,
`/openai/*`, bare `/v1/messages*` → Anthropic, else OpenAI; upstreams
overridable. Capture predicate: path *contains* `/chat/completions`,
`/messages` or `/responses` and status < 400 (also matches
`/v1/messages/count_tokens`, `GET /v1/responses/{id}`).

Request headers allow-listed; response headers forwarded minus hop-by-hop.
Body tee-streamed, parsed after `respond()`.

| Wire shape | Text | Tokens |
|---|---|---|
| OpenAI chat non-stream `choices[].message.content` | yes | yes |
| OpenAI chat SSE `delta.content` | yes | only with `stream_options.include_usage` |
| Anthropic messages non-stream `content[].text` | yes | yes |
| Anthropic SSE `content_block_delta.delta.text` | yes | yes |
| **OpenAI `tool_calls` arguments** | **no → `""`** | yes |
| **Anthropic `tool_use` / `input_json_delta`** | **no → `""`** | yes |
| **Responses API `output[].content[].output_text`** | **no → `""`** | yes |
| **Responses SSE `response.output_text.delta`** | **no** | — |
| reasoning/thinking, images, audio | no | — |

Probe: four exchanges (OpenAI tool call, Anthropic tool use, Responses,
SSE tool call) all stored with `"response_text":""`. Every modern coding
agent emits code inside tool-call arguments.

Prompt = last `role:"user"` text message; system prompt, earlier turns, tool
results not stored. Agent identity = raw `User-Agent`. Cost: static per-MTok
table, prefix/substring matched (`o3` mis-prices `o3-mini`; unknown models →
`None`). One JSON line per exchange in `.causari/capture/exchanges.jsonl`,
128-bit random id. `--seal` emits an Ed25519 hash-chained receipt over the raw
bytes (`proxy.rs:313-330`).

### `re watch` (`commands/watch.rs`)

`notify-debouncer-mini` (800 ms) recursive on root, baseline snapshot at
start. Per batch: filter `.causari/` only, lock, snapshot the whole tree,
inserted lines between parent post and new snapshot capped at 400
(`snapshot.rs:344-382`), unclaimed exchanges in the last 300 s, `correlate()`.
Event `tool:"watch"`, `writes` = raw notify paths, prompt/model/tokens/cost
from the winning exchange, exchange claimed after commit. Confidence is
**printed only**, never stored (`watch.rs:240-246`).

`correlate()` (`capture.rs:223-259`): trimmed inserted lines ≥ 6 chars,
`str::contains` in each completion, score = matched/considered, best exchange
wins (ties → newer), threshold 0.25. No time ordering beyond the window, no
per-line attribution, no common-line penalty.

### `re record`, `re hook`, `re mcp`, skills

`record`: manual self-report; only path (besides MCP) that sets `exit_code`,
`reads`, `reasoning`, tokens explicitly.

`hook`: only target `claude-code`; no git hooks, no Cursor/Windsurf/Codex.
Edits the shared `.claude/settings.json` adding `UserPromptSubmit`,
`PostToolUse (Edit|Write|MultiEdit|NotebookEdit)`, `SessionStart` with a bare
`re` command (PATH-dependent). `hook-event` swallows every error, exits 0.
`post-tool`: `tool_input.file_path` (NotebookEdit uses `notebook_path` →
empty `writes`, verified), whole-tree snapshot, skip if unchanged, prompt =
last for `session_id` **falling back to any session** (`hook.rs:208-210`),
`agent:"claude-code"`, no model/tokens/cost, always committed to HEAD.
`session-start` prints `re brief`. Not covered: `Bash` edits, model id,
token usage.

`mcp`: stdio only, always answers `protocolVersion: "2024-11-05"`, every
failure returned as JSON-RPC `-32601` instead of `isError` result. Tools
`causari_record`, `causari_recall` (bumps `stats.uses` on every hit shown),
`causari_why` (no traversal guard on `file`).

Skills are Causari's own JSON envelope, not agent-framework skills: signed
with a per-repo Ed25519 key in plaintext at `.causari/keys/skill-signing.key`;
`distill` groups **consecutive events with byte-identical prompt**.

## 2. Attribution model

| Path | Prompt | Model/cost | Files | Certainty |
|---|---|---|---|---|
| hook | last prompt for session (fallback: any) | none | declared path + whole-tree snapshot | declared, but the snapshot absorbs every other change |
| proxy+watch | `correlate()` winner | from exchange | notify paths + snapshot diff | heuristic, 0.25 threshold, one winner per window |
| MCP/record | as declared | as declared | as declared + snapshot | self-report |

`re why` walks only HEAD's chain and picks the first event whose snapshot
diff inserted the exact line; the event's prompt is then printed. `Event`
has no `confidence`, `evidence_class` or `capture_path` field.

**Human edits interleaved — verified false attribution.** Hook prompt,
`Write a.py`; human appends to `human.py` without hook; agent `Edit a.py`;
`re why human.py:2` → `agent: claude-code … prompt: add a health endpoint`.
The whole-tree snapshot folds every human/`Bash`/formatter/`git checkout`
change into the next agent event. README's "zero false attribution" holds
only when no hook fires afterwards.

**Concurrency.** Ledger integrity is solid (lock + CAS + one-exchange-one-
event claims), but sessions do not partition the filesystem: two watchers
each record every change on their own session; `re why` sees only HEAD's
chain. Watch exits on any `record_change` error including the 10 s lock
timeout (`watch.rs:91`).

**Trust after `db7180e`.** `recorded → verified if !failed && (exit_zero ||
survived) → proven if uses ≥ 3`. Still over-claiming: `survived` = every
written path **still exists** at tip (`skill.rs:648`), not content; hook and
watch never set `exit_code`, so hook skills become `verified` immediately;
`proven` counts recall hits (`mcp.rs:324`).

## 3. Integration reality

| Agent | Zero-friction today | Env var | Join viable |
|---|---|---|---|
| Claude Code | hooks, MCP | `ANTHROPIC_BASE_URL` | **no** (edits in `tool_use`); hook path has no cost, proxy path has cost but no join; nothing merges them |
| Cursor | MCP only | none usable; has its own `hooks.json` (`afterFileEdit`, `beforeSubmitPrompt`) with no `re hook cursor` | no |
| Codex CLI | MCP | `OPENAI_BASE_URL` | **no** (Responses text not parsed; `apply_patch` tool) |
| OpenAI Agents SDK | — | `OPENAI_BASE_URL` | no |
| Aider | — | `OPENAI_API_BASE`/`ANTHROPIC_API_BASE` (README names the wrong variable) | **yes** |
| Cline / Roo | MCP | provider base URL | only with text-embedded tool protocol |
| Copilot | MCP | none | impossible |
| Windsurf | MCP | none | impossible |
| scripts / curl / mocks | — | base URL | yes (all demos) |

The proxy is a reverse proxy requiring a base-URL rewrite, not CONNECT/MITM;
anything without a base-URL knob is out of reach regardless of TLS pinning.

## 4. Security / privacy

Prompts, completions and every event's prompt/reasoning in clear under
`.causari/`. Snapshots store every non-ignored file raw; `*.pem`, `id_rsa`,
`credentials.json`, `.npmrc`, `secrets.yaml` are snapshotted; no configurable
ignore. Watch `writes` records names of ignored/secret files before the
filter (verified `.env`, `target/out.bin`). API keys forwarded verbatim, never
persisted (good); the proxy prints the first 60 chars of every prompt. Loopback
plain HTTP, **no auth**: any local process can inject fake exchanges into the
evidence store. `re hook claude-code` writes into a committed file. Signing
keys plaintext hex with default umask. MCP `causari_why` joins `file` without
traversal guard.

## 5. Bugs

1. **Watch self-triggers on Linux (critical, verified).** `Cargo.toml`
   `notify = "6.1"` unused; `notify-debouncer-mini 0.7` pulls `notify 8.2`
   whose inotify mask includes `IN_OPEN`; debouncer-mini forwards every kind;
   snapshot opens every file → new batch → … Idle repo: 24 events / 4 s. No
   tree-unchanged check in `record_change`. RESULTS.md was produced on Windows.
2. Tool-call / Responses payloads → `response_text: ""` → no join, cost never
   claimed.
3. Hook path absorbs foreign changes (`hook.rs:192`).
4. Cross-session prompt fallback (`hook.rs:208-210`).
5. `NotebookEdit` `notebook_path` unread (`hook.rs:183-187`).
6. Watch dies on lock contention (`watch.rs:91`, `repo.rs:359-364`).
7. Client disconnect mid-stream drops the exchange (`proxy.rs:278`).
8. OpenAI streaming cost usually `None`.
9. Per-edit full-tree snapshot (`snapshot.rs:42-84`).
10. `added_lines_between` 400-line cap in path order.
11. `is_completion_path` false positives.
12. Substring join with no time ordering.
13. Confidence not persisted.
14. MCP: fixed protocol version, `-32601` for tool errors, no HTTP transport.
15. Hooks in shared settings with bare `re`, non-atomic write.
16. Dead `notify 6.1` dependency.
17. Doc mismatches (`agent: proxy-watch`, Aider env var, `hook-event` kinds).

## 6. Ranked

1. Fix and test `re watch` on Linux; filter Access/Open kinds; tree-unchanged
   check in `record_change`.
2. Parse tool calls or the join is dead on arrival.
3. Store an evidence class per event and show it everywhere.
4. Stop absorbing foreign edits into agent events (scope hook diff to the
   declared path; split declared vs incidental).
5. Merge hook and proxy streams for Claude Code by `session_id` and time.
6. Hook targets beyond Claude Code (Cursor `hooks.json`, Gemini CLI).
7. Incremental snapshots (stat cache / per-path hashing).
8. Re-ground trust: content survival, success signal from a later task.
9. Treat the ledger as evidence: configurable ignores, redaction, proxy auth.
10. Rewrite the integration matrix from the code, not the vision.
