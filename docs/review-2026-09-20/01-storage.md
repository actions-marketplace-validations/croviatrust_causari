# Core data model and storage

Built with Rust 1.98 (edition 2024 requires ≥ 1.85); 130 unit + 3 integration
tests green. Experiments run against the binary in scratch directories.

## 1. Data model

Four content-addressed kinds (`object.rs:16-23`), stored as
`marker byte || payload` (`store.rs:101-114`): blob `B` (raw bytes), tree `T`
(canonical JSON `{"entries":{name:{"id","kind"}}}`), snapshot `S`
(`{"created_at","tree"}`), event `E`. `TreeEntry.kind` is a free string, no
mode, size or symlink kind.

**Ids.** BLAKE3 hex (`object.rs:160-162`). `canonical_json` (`object.rs:131-157`)
sorts keys recursively via `serde_json::Value`; it is **not** the CSC-1
canonicaliser used by `seal.rs:37`, and `tests/vectors/canonical_cases.json` is
only exercised by the seal module. `Event.cost_usd: Option<f64>` goes through
float formatting, which CSC-1 forbids.

**Blob ids are versioned by store history** (`store.rs:42-51`): v1 objects
(pre-`630e472`) live under `hash(content)`, v2 under `hash('B'||content)`;
`write_blob` returns the v1 id if such an object exists. The same bytes can
have two valid ids, so tree ids depend on which repo computes them:
content-addressable per repo, not globally.

**Event and DAG.** `Event` (`object.rs:67-127`): `schema` hardcoded
`"causari.event.v0.2"` in six places, single `parent`, agent/model/tool/
message/prompt/reasoning, declared `reads`/`writes`, tokens/cost, `exit_code`,
`created_at`, `pre_snapshot`, `post_snapshot`. One parent → a forest of chains
sharing prefixes, never a DAG; no merge node. Sessions are refs
(`refs/sessions/<name>`), fork points are recomputed by `walk_all`
(`dag.rs:67-96`), ordering by string comparison of `created_at` (`dag.rs:94`).
Intent is not a node type: prompt/reasoning are inline, never deduplicated.

**Snapshots.** `snapshot_workspace` (`snapshot.rs:42-90`) recursively reads
the root, skips any path with a component in `DEFAULT_IGNORES` or matching
`.env*` (`snapshot.rs:12-38`), skips symlinks and non-UTF-8 names, reads every
regular file fully and `write_blob`s it, writes one tree per directory
(including empty ones). `created_at` is inside the `Snapshot`, so **identical
trees produce distinct snapshot ids** (verified: 8 snapshots → 1 tree);
`pre == post` is never true at id level. The pre-state of an event is the
parent's `post_snapshot` (`commit.rs:34-45`), never re-snapshotted.

**"Bidirectional"** in code = file-name dataflow over a linear chain.
`effective_writes` = set difference of flattened pre/post trees
(`snapshot.rs:389-414`); `effective_reads` = declared reads ∪ writes
(`snapshot.rs:327-338`). `trace` (`trace.rs:52-128`) finds the line writer by
diffing pre/post text per event along HEAD's chain, then BFS through the most
recent earlier writer of each read file. `impact` (`impact.rs:124-175`) taints
forward along HEAD's chain. Other sessions are invisible; hook and watch
recorders leave `reads` empty (`hook.rs:232`, `watch.rs:181`), so for the
deterministic paths the cone reduces to "events that touched the same files".

## 2. On-disk layout

```
.causari/
  HEAD                     "ref: refs/sessions/main\n" or raw id
  config.toml              written once, never read
  guard.toml               the only config loaded (config.rs:20)
  lock                     advisory lock with owner pid
  objects/<2>/<62>         one file per object
  refs/sessions/<name>     "<event id>\n"; <name>.cas transient CAS guard
  index/events.jsonl       append-only cache, self-healing (index.rs:105-145)
  capture/exchanges.jsonl, prompts.jsonl, claims.jsonl
  seal/seals.jsonl
  keys/skill-signing.key, .pub, seal-issuer.key    plaintext hex, default umask
  skills/, guard-badge.svg, survival-snapshots.jsonl
```

Beside `.git/`, gitignored by `re init`, shares nothing with git: different
hash, JSON trees, no packs, no delta, no gc.

Atomicity: objects `exists()` then `fs::write` (`store.rs:76, 97`), no
temp+rename, no fsync. Refs `fs::write` truncate-then-write (`repo.rs:212,
214, 272`; `fork.rs:50-54`; `switch.rs:79-82`). Lock via `create_new`
(`repo.rs:340-372`), 10 s acquisition timeout, broken only if owner provably
dead after 30 s or unconditionally after 10 min. CAS on refs via `RefCasGuard`
(`repo.rs:476-531`).

## 3. Confirmed bugs

- **B1 Torn objects never healed.** `store.rs:76-93` treats "exists and first
  byte matches" as present. A truncated blob blocks every future write of that
  content and `write_blob` returns the id. Reproduced: truncate a blob →
  `re record` succeeds referencing it → `re revert --dry-run` fails integrity.
- **B2 Snapshot accepts names restore rejects.** `build_tree` validates
  nothing; `validate_restore_tree` (`snapshot.rs:141-150`) rejects trailing
  `.`/space, `:`, `\`, control chars on every platform. One such file disables
  revert/bisect/switch/fork for every snapshot containing it.
- **B3 Ignore list matches any component.** `is_ignored` (`snapshot.rs:33-38`)
  drops `src/build/gen.rs`, `packages/dist/`, a Python package called `build`,
  silently. No `.gitignore` semantics (acknowledged at `snapshot.rs:10-11`).
- **B4 `re revert` mis-attributes its own changes.** Pre = parent.post
  (`commit.rs:34-36`) and revert records nothing (`revert.rs:89-92`), so the
  next event's `effective_writes` includes the revert. Reproduced: `re why`
  answers `agent: claude` for a line the revert restored.
- **B5 `re watch` records no-op events and a self-triggered storm.**
  `record_change` (`watch.rs:112-192`) never compares trees; the comment at
  `watch.rs:136-137` claiming upstream filtering is false; `touched` paths are
  only filtered against `.causari` (`watch.rs:108-110`), so `cargo build`
  writing `target/` triggers a snapshot. Reproduced: ~3 events/s indefinitely
  on an idle repo (`writes: ["", "a.txt"]`), identical trees.
- **B6 File mode not stored.** `TreeEntry` has no mode; restore writes bytes
  only (`snapshot.rs:230`); executable bit lost. Empty directories recreated
  but never removed by `cleanup_extras` (`snapshot.rs:257`).

## 3b. Hazards not yet triggered

- Non-atomic ref write + "missing ref = new session": an empty ref makes
  `session_head` return `None`, `commit_event` sets `expected = None` and
  forks the "new" session from HEAD; the old history becomes unreachable with
  the CAS passing.
- Lock-breaking TOCTOU (`repo.rs:354-358`).
- 10 s lock timeout vs big-repo snapshot time: hook events dropped.
- HEAD `ref:` path not validated (`repo.rs:185-186`).
- Two `Snapshot` objects per root record, fresh `Snapshot` per no-op, no gc/
  prune/fsck; `resolve_id` scans buckets with `read_dir`.
- `created_at` inside hashed objects: identity is not a pure function of
  content.
- Root event special-cased in `why.rs:86-91` and `lens.rs:61-69` but not in
  `trace.rs`.

## 4. Performance (20k files, 200 dirs, 79 MB, 25 events, release, tmpfs)

| Command | Time | Why |
|---|---|---|
| `re record` (1 file changed) | 179 ms | full read+hash of 20k files; objects 82 MB for 79 MB source |
| `re revert --dry-run` | 216 ms | flatten + read every blob + every workspace file |
| `re why` (25 events) | 1.26 s | 2 × `flatten_tree` per event (`why.rs:71-72`) |
| `re lens` | 1.25 s | same |
| `re trace` | 3.0 s | 6 flattens per event |
| `re impact` | 3.4 s (debug) | 4 flattens per event |
| `re log --all`, `re find` | 3 ms | index only |

Recording is O(files × bytes) per event with no stat cache; every query is
O(events × files). No per-file history, no path→writer index. One file per
object, trees as JSON with 64-hex ids per entry, no delta, no gc. `re watch`
watches `.causari/objects/*` and `target/` too.

## 5. Design quality

Good: the event-with-pre/post-snapshot atom; sessions as refs with implicit
fork; marker byte in hash and verify-on-read; CAS-guarded refs and
"restore first, move HEAD last"; preflight before write; honest cache index.

Redundant: `Snapshot` wrapper defeats dedup and duplicates `Event.created_at`;
two canonicalisers in one binary; v1/v2 blob coexistence instead of a one-shot
migration; `commit.rs` billed as the single recorder while four recorders
duplicate snapshot/no-op logic differently.

Duplicated with git: the whole blob/tree store, refs, HEAD, lock, prefix
resolution, with none of git's engineering (index, packs, deltas, gc, fsck,
gitignore, modes, symlinks, merges). A repo tracked by both stores every
version twice.

## 6. Ranked

1. Snapshot the real pre-state or record reverts (B4).
2. Path→history index; every query is O(events × files).
3. Atomic, self-healing object/ref writes (B1, orphaned sessions).
4. Watch: no-op check in `commit.rs`, ignore-aware notify paths (B5).
5. Store mode, symlinks, validate names at capture time (B2, B6).
6. Real `.gitignore` semantics (B3).
7. Decide the relationship with git: sit on its object store or build a real one.
8. Persist computed causal edges; cross-session visibility.
9. Stat-cache the working tree.
10. Drop `created_at` from hashed `Snapshot`; one canonical encoding (CSC-1).
