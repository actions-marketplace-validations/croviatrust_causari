use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use colored::Colorize;
use serde_json::{Value, json};
use std::io::Read;

use crate::capture::{
    Exchange, PromptRecord, append_jsonl, claim_exchange, count_contained, last_prompt,
    load_unclaimed_exchanges_since, now_ms, overlap_is_significant, prompts_path,
    significant_lines,
};
use crate::cli::{HookArgs, HookEventArgs};
use crate::object::{Event, Snapshot};
use crate::repo::Repo;
use crate::snapshot::{flatten_tree, snapshot_workspace};
use crate::store::Store;

/// `re hook claude-code` — native capture where hooks exist.
///
/// Claude Code exposes lifecycle hooks (UserPromptSubmit, PostToolUse) that
/// hand us the *real* prompt and the *real* tool call — no inference needed.
/// This command wires them up in the project's `.claude/settings.json`:
///
/// - UserPromptSubmit → `re hook-event user-prompt` (stores the prompt)
/// - PostToolUse (Edit|Write|MultiEdit|NotebookEdit) → `re hook-event post-tool`
///   (records a full Causari event: snapshot, prompt, tool, file)
///
/// Where hooks don't exist (Cursor, custom agents), `re proxy` + `re watch`
/// cover the same ground via content correlation.
pub fn run(args: HookArgs) -> Result<()> {
    match args.target.as_str() {
        "claude-code" => install_claude_code(),
        other => Err(anyhow!(
            "unknown hook target '{}' (supported: claude-code)",
            other
        )),
    }
}

const PROMPT_HOOK_CMD: &str = "re hook-event user-prompt";
const PRE_TOOL_HOOK_CMD: &str = "re hook-event pre-tool";
const TOOL_HOOK_CMD: &str = "re hook-event post-tool";
const SESSION_HOOK_CMD: &str = "re hook-event session-start";
const TOOL_MATCHER: &str = "Edit|Write|MultiEdit|NotebookEdit";
/// Max entries per section injected at session start — keep the context lean.
const SESSION_BRIEF_LIMIT: usize = 3;

fn install_claude_code() -> Result<()> {
    let repo = Repo::discover()?;
    let dir = repo.root.join(".claude");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("settings.json");

    let mut root: Value = if path.exists() {
        let raw = std::fs::read_to_string(&path)?;
        serde_json::from_str(&raw).with_context(|| format!("parsing {}", path.display()))?
    } else {
        json!({})
    };

    let hooks = root
        .as_object_mut()
        .ok_or_else(|| anyhow!("settings.json root is not an object"))?
        .entry("hooks")
        .or_insert_with(|| json!({}));

    ensure_hook(hooks, "UserPromptSubmit", None, PROMPT_HOOK_CMD)?;
    ensure_hook(hooks, "PreToolUse", Some(TOOL_MATCHER), PRE_TOOL_HOOK_CMD)?;
    ensure_hook(hooks, "PostToolUse", Some(TOOL_MATCHER), TOOL_HOOK_CMD)?;
    ensure_hook(hooks, "SessionStart", None, SESSION_HOOK_CMD)?;

    std::fs::write(&path, serde_json::to_string_pretty(&root)?)?;

    println!(
        "{} Claude Code hooks installed in {}",
        "causari:".green().bold(),
        path.display().to_string().cyan()
    );
    println!("  UserPromptSubmit → captures every prompt");
    println!(
        "  PreToolUse ({}) → snapshots the tree the agent is about to change",
        TOOL_MATCHER
    );
    println!(
        "  PostToolUse ({}) → records the edit as a Causari event, diffed against that snapshot",
        TOOL_MATCHER
    );
    println!("  SessionStart → injects verified experience into every new session");
    println!();
    println!(
        "  {} restart Claude Code (or run /hooks) to load them.",
        "note:".yellow()
    );
    Ok(())
}

/// Idempotently add our hook entry for `kind` unless already present.
fn ensure_hook(hooks: &mut Value, kind: &str, matcher: Option<&str>, command: &str) -> Result<()> {
    let entries = hooks
        .as_object_mut()
        .ok_or_else(|| anyhow!("'hooks' is not an object"))?
        .entry(kind)
        .or_insert_with(|| json!([]));
    let arr = entries
        .as_array_mut()
        .ok_or_else(|| anyhow!("'hooks.{}' is not an array", kind))?;
    let already = arr.iter().any(|e| {
        serde_json::to_string(e)
            .unwrap_or_default()
            .contains(command)
    });
    if already {
        return Ok(());
    }
    let mut entry = json!({
        "hooks": [{ "type": "command", "command": command }]
    });
    if let Some(m) = matcher {
        entry["matcher"] = json!(m);
    }
    arr.push(entry);
    Ok(())
}

// ---------------------------------------------------------------------------
// `re hook-event` — the hidden command the hooks actually invoke
// ---------------------------------------------------------------------------

/// Invoked by the agent runtime with a JSON payload on stdin.
/// Must NEVER fail loudly: a non-zero exit or stderr noise would degrade the
/// agent session. Errors are swallowed by design.
pub fn run_event(args: HookEventArgs) -> Result<()> {
    let _ = run_event_inner(&args.kind);
    Ok(())
}

fn run_event_inner(kind: &str) -> Result<()> {
    let repo = Repo::discover()?;
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    let v: Value = serde_json::from_str(&input)?;
    let session_id = v
        .get("session_id")
        .and_then(|s| s.as_str())
        .map(String::from);

    match kind {
        "user-prompt" => {
            let prompt = v
                .get("prompt")
                .and_then(|p| p.as_str())
                .unwrap_or_default()
                .to_string();
            if prompt.is_empty() {
                return Ok(());
            }
            append_jsonl(
                &prompts_path(&repo),
                &PromptRecord {
                    ts_ms: now_ms(),
                    session_id,
                    prompt,
                },
            )
        }
        "pre-tool" => record_pre_state(&repo, session_id.as_deref()),
        "post-tool" => record_tool_event(&repo, &v, session_id.as_deref()),
        // SessionStart: whatever we print on stdout is added to the agent's
        // context. Inject the trust-ranked experience briefing so every new
        // session — regardless of which model is behind it — starts with the
        // lessons this repository has already paid for. Silent when there is
        // no experience yet: zero noise on fresh repos. Never bumps recall
        // counters (trust is earned by explicit use, not by injection).
        "session-start" => {
            if let Some(md) =
                crate::commands::brief::render(&repo, &[], SESSION_BRIEF_LIMIT, false)?
            {
                print!("{md}");
            }
            Ok(())
        }
        other => Err(anyhow!("unknown hook-event kind '{}'", other)),
    }
}

/// A pre-state captured by `PreToolUse`, waiting for its `PostToolUse`.
#[derive(serde::Serialize, serde::Deserialize)]
struct PendingPre {
    ts_ms: u64,
    #[serde(default)]
    session_id: Option<String>,
    snapshot_id: String,
}

fn pending_pre_path(repo: &Repo) -> std::path::PathBuf {
    repo.dir.join("capture").join("pending-pre.jsonl")
}

/// A pre-state older than this is stale: the tool call it belonged to never
/// produced a PostToolUse (denied, crashed, interrupted).
const PENDING_PRE_MAX_AGE_MS: u64 = 10 * 60 * 1000;

/// `PreToolUse`: snapshot the tree *before* the agent edits it.
///
/// This is what makes hook attribution exact. Without it the event's
/// pre-state is the previous event's post-state, and every change made in
/// between — a human edit, a checkout, a formatter — lands in the agent's
/// diff and gets that agent's prompt as its cause (review finding: "zero
/// false attribution" was false in the interleaved case).
fn record_pre_state(repo: &Repo, session_id: Option<&str>) -> Result<()> {
    let store = Store::new(repo);
    let _lock = repo.lock()?;
    let tree = snapshot_workspace(repo)?;
    let snapshot_id = store.write_snapshot(&Snapshot {
        tree,
        created_at: Utc::now().to_rfc3339(),
    })?;
    append_jsonl(
        &pending_pre_path(repo),
        &PendingPre {
            ts_ms: now_ms(),
            session_id: session_id.map(String::from),
            snapshot_id,
        },
    )
}

/// The most recent fresh pre-state for this session, if any; consumed on read.
fn take_pending_pre(repo: &Repo, session_id: Option<&str>) -> Option<String> {
    let path = pending_pre_path(repo);
    let raw = std::fs::read_to_string(&path).ok()?;
    let now = now_ms();
    let mut keep: Vec<PendingPre> = Vec::new();
    let mut found: Option<String> = None;
    for line in raw.lines().rev() {
        let Ok(p) = serde_json::from_str::<PendingPre>(line) else {
            continue;
        };
        if now.saturating_sub(p.ts_ms) > PENDING_PRE_MAX_AGE_MS {
            continue;
        }
        if found.is_none() && p.session_id.as_deref() == session_id {
            found = Some(p.snapshot_id);
            continue;
        }
        keep.push(p);
    }
    keep.reverse();
    let body: String = keep
        .iter()
        .filter_map(|p| serde_json::to_string(p).ok())
        .map(|l| l + "\n")
        .collect();
    let _ = crate::keys::write_atomic(&path, body.as_bytes());
    found
}

/// Record a full Causari event from a Claude Code PostToolUse payload.
fn record_tool_event(repo: &Repo, v: &Value, session_id: Option<&str>) -> Result<()> {
    let store = Store::new(repo);
    let tool = v
        .get("tool_name")
        .and_then(|t| t.as_str())
        .unwrap_or("unknown")
        .to_string();
    let input = v.get("tool_input");
    // Edit/Write/MultiEdit use `file_path`; NotebookEdit uses `notebook_path`.
    let file = input
        .and_then(|i| i.get("file_path").or_else(|| i.get("notebook_path")))
        .and_then(|f| f.as_str())
        .map(String::from);

    let _lock = repo.lock()?;
    let parent_id = crate::commit::resolve_parent(repo, None)?;
    // Prefer the pre-state captured by PreToolUse moments ago; fall back to
    // the previous event's post-state when the hook is not installed.
    let pre_snapshot_id = match take_pending_pre(repo, session_id) {
        Some(id) => id,
        None => crate::commit::resolve_pre_snapshot(repo, &store, &parent_id)?,
    };
    let post_tree = snapshot_workspace(repo)?;

    // Skip no-op tool calls (nothing actually changed on disk).
    if crate::commit::tree_unchanged(&store, &pre_snapshot_id, &post_tree)? {
        return Ok(());
    }
    let post_snapshot_id = store.write_snapshot(&Snapshot {
        tree: post_tree,
        created_at: Utc::now().to_rfc3339(),
    })?;

    // The prompt must come from *this* session. Borrowing another session's
    // prompt would attribute an edit to a task it had nothing to do with;
    // when the runtime gave no session id, any prompt is the best we have.
    let prompt = match session_id {
        Some(_) => last_prompt(repo, session_id)?,
        None => last_prompt(repo, None)?,
    }
    .map(|p| p.prompt);

    let rel_file = file.as_deref().map(|f| {
        std::path::Path::new(f)
            .strip_prefix(&repo.root)
            .map(|r| r.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| f.replace('\\', "/"))
    });
    let message = match &rel_file {
        Some(f) => format!("{} {}", tool, f),
        None => tool.clone(),
    };

    // The hook knows what was written; only the proxy knows which model
    // wrote it and what it cost. Merge the two when the evidence is
    // unambiguous, otherwise leave the event as declared.
    let exchange = rel_file.as_deref().and_then(|rel| {
        let since = now_ms().saturating_sub(HOOK_MERGE_WINDOW_MS);
        let exchanges = load_unclaimed_exchanges_since(repo, since).ok()?;
        if exchanges.is_empty() {
            return None;
        }
        let added = inserted_lines_of(&store, &pre_snapshot_id, &post_snapshot_id, rel).ok()?;
        matching_exchange(&exchanges, rel, &added)
    });

    let event = Event {
        schema: "causari.event.v0.2".to_string(),
        parent: parent_id,
        agent: Some("claude-code".to_string()),
        model: exchange.as_ref().and_then(|e| e.model.clone()),
        tool: Some(tool),
        message: Some(message),
        prompt,
        reasoning: None,
        reads: Vec::new(),
        writes: rel_file.into_iter().collect(),
        tokens_in: exchange.as_ref().and_then(|e| e.tokens_in),
        tokens_out: exchange.as_ref().and_then(|e| e.tokens_out),
        cost_usd: exchange.as_ref().and_then(|e| e.cost_usd),
        pre_snapshot: pre_snapshot_id,
        post_snapshot: post_snapshot_id,
        exit_code: None,
        created_at: Utc::now().to_rfc3339(),
        evidence: Some(crate::object::Evidence::declared("claude-code-hook")),
    };
    let id = crate::commit::commit_event(repo, &store, &event, None)?;
    if let Some(e) = &exchange {
        // Its tokens and dollars now belong to this event; `re watch` must
        // not attribute them a second time.
        claim_exchange(repo, e, &id)?;
    }
    Ok(())
}

/// How far back a proxy exchange may lie to be the completion behind a
/// hook event. Claude Code writes the file within seconds of the model's
/// answer; two minutes covers slow tool approval without reaching into
/// earlier turns.
const HOOK_MERGE_WINDOW_MS: u64 = 120 * 1000;

/// The proxy exchange behind this tool call, when exactly one qualifies.
///
/// Neither stream carries the other's key, so the join is by evidence: an
/// unclaimed exchange from a Claude client (Claude Code's User-Agent starts
/// with `claude-cli`) whose completion contains the lines that just landed
/// in the declared file — the same overlap bar as `correlate`, or at least
/// one significant line together with the file's own name. Zero or several
/// candidates → `None`: a wrong model or cost is worse than none.
fn matching_exchange(exchanges: &[Exchange], rel_file: &str, added: &[String]) -> Option<Exchange> {
    let considered = significant_lines(added);
    if considered.is_empty() {
        return None;
    }
    let basename = std::path::Path::new(rel_file)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|n| !n.is_empty())?;
    let mut hits = exchanges.iter().filter(|e| {
        let from_claude = e
            .agent
            .as_deref()
            .map(|a| a.to_ascii_lowercase().contains("claude"))
            .unwrap_or(false);
        if !from_claude {
            return false;
        }
        let matched = count_contained(&considered, &e.response_text);
        overlap_is_significant(matched, considered.len())
            || (matched >= 1 && e.response_text.contains(&basename))
    });
    let only = hits.next()?;
    if hits.next().is_some() {
        return None;
    }
    Some(only.clone())
}

/// Lines inserted into one file between two snapshots. Scoped to the
/// declared path on purpose: whatever else changed in the tree is not
/// evidence about this tool call.
fn inserted_lines_of(
    store: &Store,
    pre_snapshot_id: &str,
    post_snapshot_id: &str,
    rel_file: &str,
) -> Result<Vec<String>> {
    use similar::{ChangeTag, TextDiff};
    let path = std::path::PathBuf::from(rel_file);
    let pre = flatten_tree(store, &store.read_snapshot(pre_snapshot_id)?.tree)?;
    let post = flatten_tree(store, &store.read_snapshot(post_snapshot_id)?.tree)?;
    let Some(post_id) = post.get(&path) else {
        return Ok(Vec::new());
    };
    let post_text = String::from_utf8(store.read_blob(post_id)?).unwrap_or_default();
    let pre_text = match pre.get(&path) {
        Some(id) if id == post_id => return Ok(Vec::new()),
        Some(id) => String::from_utf8(store.read_blob(id)?).unwrap_or_default(),
        None => String::new(),
    };
    Ok(TextDiff::from_lines(&pre_text, &post_text)
        .iter_all_changes()
        .filter(|c| c.tag() == ChangeTag::Insert)
        .map(|c| c.value().trim_end_matches('\n').to_string())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::{exchanges_path, load_unclaimed_exchanges_since};

    const CODE: &str = "def refresh_token(user):\n    token = issue_token(user, scope=\"session\")\n    return rotate_every(token, hours=24)\n";

    fn exchange(id: &str, agent: &str, text: &str) -> Exchange {
        Exchange {
            id: Some(id.to_string()),
            ts_ms: now_ms(),
            agent: Some(agent.to_string()),
            model: Some("claude-sonnet-4-20250514".to_string()),
            prompt: Some("add token refresh".to_string()),
            response_text: text.to_string(),
            tokens_in: Some(900),
            tokens_out: Some(80),
            cost_usd: Some(0.0039),
            request_sha256: None,
            response_sha256: None,
            seal_id: None,
            truncated: false,
        }
    }

    /// What the proxy stores for a Claude Code `Write`: the tool_use input's
    /// string leaves — the path and the file content.
    fn claude_write_completion(path: &str) -> String {
        format!("I'll add the helper.\n{path}\n{CODE}")
    }

    /// Simulate Claude Code's PreToolUse → file write → PostToolUse.
    fn run_hooks(repo: &Repo, session: &str, rel: &str, content: &str) {
        record_pre_state(repo, Some(session)).unwrap();
        let abs = repo.root.join(rel);
        std::fs::create_dir_all(abs.parent().unwrap()).unwrap();
        std::fs::write(&abs, content).unwrap();
        let payload = json!({
            "session_id": session,
            "hook_event_name": "PostToolUse",
            "tool_name": "Write",
            "tool_input": {"file_path": abs.to_string_lossy(), "content": content},
            "tool_response": {"filePath": abs.to_string_lossy(), "success": true}
        });
        record_tool_event(repo, &payload, Some(session)).unwrap();
    }

    fn head_event(repo: &Repo) -> Event {
        let id = repo.head_event().unwrap().expect("an event was recorded");
        Store::new(repo).read_event(&id).unwrap()
    }

    #[test]
    fn hook_event_inherits_model_and_cost_from_the_one_matching_exchange() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = Repo::init(tmp.path()).unwrap();
        let abs = repo.root.join("src/auth.py");
        let ex = exchange(
            "ex-1",
            "claude-cli/1.0.83 (external, cli)",
            &claude_write_completion(&abs.to_string_lossy()),
        );
        append_jsonl(&exchanges_path(&repo), &ex).unwrap();
        // Noise the join must ignore: another client, unrelated content.
        append_jsonl(
            &exchanges_path(&repo),
            &exchange(
                "ex-2",
                "codex_cli_rs/0.40",
                &claude_write_completion("src/auth.py"),
            ),
        )
        .unwrap();
        append_jsonl(
            &exchanges_path(&repo),
            &exchange(
                "ex-3",
                "claude-cli/1.0.83",
                "Nothing to do with auth.py here.",
            ),
        )
        .unwrap();

        run_hooks(&repo, "sess-1", "src/auth.py", CODE);

        let ev = head_event(&repo);
        assert_eq!(ev.agent.as_deref(), Some("claude-code"));
        assert_eq!(ev.model.as_deref(), Some("claude-sonnet-4-20250514"));
        assert_eq!((ev.tokens_in, ev.tokens_out), (Some(900), Some(80)));
        assert_eq!(ev.cost_usd, Some(0.0039));
        assert_eq!(ev.writes, vec!["src/auth.py".to_string()]);
        // Still declared: the file and prompt come from the hook, not a guess.
        assert_eq!(
            ev.evidence,
            Some(crate::object::Evidence::declared("claude-code-hook"))
        );
        // The exchange is spent; `re watch` will not attribute it again.
        let left: Vec<String> = load_unclaimed_exchanges_since(&repo, 0)
            .unwrap()
            .into_iter()
            .filter_map(|e| e.id)
            .collect();
        assert_eq!(left, vec!["ex-2".to_string(), "ex-3".to_string()]);
    }

    #[test]
    fn ambiguous_or_absent_exchanges_leave_the_event_declared_only() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = Repo::init(tmp.path()).unwrap();

        // No exchange at all.
        run_hooks(&repo, "sess-1", "src/a.py", CODE);
        let ev = head_event(&repo);
        assert_eq!(ev.model, None);
        assert_eq!(ev.cost_usd, None);

        // Two Claude exchanges both containing the code (a retry): ambiguous.
        let text = claude_write_completion("src/b.py");
        append_jsonl(
            &exchanges_path(&repo),
            &exchange("ex-a", "claude-cli/1.0", &text),
        )
        .unwrap();
        append_jsonl(
            &exchanges_path(&repo),
            &exchange("ex-b", "claude-cli/1.0", &text),
        )
        .unwrap();
        run_hooks(&repo, "sess-1", "src/b.py", CODE);
        let ev = head_event(&repo);
        assert_eq!(ev.writes, vec!["src/b.py".to_string()]);
        assert_eq!(ev.model, None);
        assert_eq!(ev.tokens_in, None);
        assert_eq!(load_unclaimed_exchanges_since(&repo, 0).unwrap().len(), 2);
    }

    #[test]
    fn matching_requires_claude_agent_and_content_overlap() {
        let added: Vec<String> = CODE.lines().map(String::from).collect();
        let good = exchange(
            "g",
            "claude-cli/1.0",
            &claude_write_completion("src/auth.py"),
        );
        assert_eq!(
            matching_exchange(std::slice::from_ref(&good), "src/auth.py", &added)
                .and_then(|e| e.id)
                .as_deref(),
            Some("g")
        );
        // Not from a Claude client.
        let other = exchange("o", "aider/0.86", &claude_write_completion("src/auth.py"));
        assert!(matching_exchange(&[other], "src/auth.py", &added).is_none());
        // From Claude, but the completion does not contain what was written.
        let unrelated = exchange("u", "claude-cli/1.0", "Sure, renaming the variable.");
        assert!(matching_exchange(&[unrelated], "src/auth.py", &added).is_none());
        // One line of many, but the file's own name is in the completion:
        // enough (a small Edit inside a big file).
        let one_line = vec![
            "    return rotate_every(token, hours=24)".to_string(),
            "x = 1".to_string(),
        ];
        let edit = exchange(
            "e",
            "claude-cli/1.0",
            "Editing auth.py:\n    return rotate_every(token, hours=24)",
        );
        assert!(matching_exchange(std::slice::from_ref(&edit), "src/auth.py", &one_line).is_some());
        // Same line, but a different file: the name does not match and one
        // line out of five is below the overlap bar.
        let many = vec![
            "    return rotate_every(token, hours=24)".to_string(),
            "alpha = compute_alpha(input)".to_string(),
            "beta = compute_beta(input)".to_string(),
            "gamma = compute_gamma(input)".to_string(),
            "delta = compute_delta(input)".to_string(),
        ];
        assert!(matching_exchange(&[edit], "src/other.py", &many).is_none());
        // Nothing significant was inserted: nothing to match on.
        assert!(matching_exchange(&[good], "src/auth.py", &["}".to_string()]).is_none());
    }
}
