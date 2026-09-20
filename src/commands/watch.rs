use anyhow::{Context, Result};
use chrono::Utc;
use colored::Colorize;
use notify::event::{EventKind, ModifyKind};
use notify::{RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc::channel};
use std::time::{Duration, Instant};

use crate::capture::{claim_exchange, correlate, load_unclaimed_exchanges_since, now_ms};
use crate::cli::WatchArgs;
use crate::commit::{commit_event, resolve_parent, tree_unchanged};
use crate::object::{Event, Evidence, Snapshot};
use crate::repo::Repo;
use crate::snapshot::{added_lines_between, is_ignored, snapshot_workspace};
use crate::store::Store;

/// `re watch` turns Causari into a passive recorder.
///
/// It watches the working tree and, every time a debounce window elapses with
/// any change, it snapshots the workspace and writes a new event. The intent
/// is that you launch this in a side terminal, then let *any* agent (Cursor,
/// Claude Code, Cline, Aider, a script, you) operate on the repo — Causari
/// records the timeline for free, with no integration required.
///
/// When `re proxy` runs alongside, watch performs the **causal join**: the
/// lines inserted by each change are searched inside the LLM completions the
/// proxy captured moments before. A match attributes the change to the real
/// prompt, model, token usage and cost — provenance without cooperation.
///
/// This is the bridge between Causari and the rest of the ecosystem.
pub fn run(args: WatchArgs) -> Result<()> {
    let repo = Repo::discover()?;
    let store = Store::new(&repo);
    let debounce_ms = args.debounce.unwrap_or(800);

    println!(
        "{} watching {} (debounce {}ms). Press Ctrl-C to stop.",
        "causari:".green().bold(),
        repo.root.display(),
        debounce_ms
    );
    if let Some(a) = &args.agent {
        println!("  agent tag: {}", a.cyan());
    }
    if let Some(s) = &args.session {
        println!("  session:   {}", s.cyan());
    }

    // Baseline snapshot at startup. Without it, the first recorded change
    // would use a pre-state captured AFTER the change already happened
    // (pre == post, empty diff, nothing to correlate).
    let baseline_snapshot_id = {
        let tree_id = snapshot_workspace(&repo)?;
        store.write_snapshot(&Snapshot {
            tree: tree_id,
            created_at: Utc::now().to_rfc3339(),
        })?
    };

    let stop = Arc::new(AtomicBool::new(false));
    {
        let stop = Arc::clone(&stop);
        ctrlc::set_handler(move || {
            stop.store(true, Ordering::SeqCst);
        })
        .context("installing ctrl-c handler")?;
    }

    // Raw notify events, debounced here rather than by a helper crate, so
    // the event *kind* is still visible: on Linux inotify reports every
    // open/close, and snapshotting opens every file. Feeding those back in
    // as "changes" is the loop that fabricated history on idle repos.
    let (tx, rx) = channel();
    let mut watcher = notify::recommended_watcher(tx).context("creating filesystem watcher")?;
    watcher
        .watch(&repo.root, RecursiveMode::Recursive)
        .context("starting watcher")?;

    let debounce = Duration::from_millis(debounce_ms);
    let mut pending: HashSet<PathBuf> = HashSet::new();
    let mut last_change: Option<Instant> = None;

    while !stop.load(Ordering::SeqCst) {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(Ok(event)) => {
                if !is_content_change(&event.kind) {
                    continue;
                }
                for p in event.paths {
                    if is_relevant(&repo, &p) {
                        pending.insert(p);
                        last_change = Some(Instant::now());
                    }
                }
            }
            Ok(Err(err)) => {
                eprintln!("{} watcher error: {}", "warn:".yellow(), err);
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(e) => {
                eprintln!("{} channel closed: {:?}", "error:".red(), e);
                break;
            }
        }

        let quiet = last_change
            .map(|t| t.elapsed() >= debounce)
            .unwrap_or(false);
        if quiet && !pending.is_empty() {
            let touched = std::mem::take(&mut pending);
            last_change = None;
            // A failed record (lock held by another recorder for too long,
            // a transient I/O error) must not stop the watcher: report it
            // and keep watching; the next window snapshots the same tree.
            if let Err(e) = record_change(&repo, &store, &args, &touched, &baseline_snapshot_id) {
                eprintln!("{} not recorded: {:#}", "warn:".yellow(), e);
            }
        }
    }

    println!("\n{} stopped.", "causari:".green().bold());
    Ok(())
}

/// Only events that can change file *content* or the set of files count.
/// Access (open/close/read) and metadata-only changes (chmod, utime) never
/// alter a snapshot and must not trigger one.
fn is_content_change(kind: &EventKind) -> bool {
    match kind {
        EventKind::Create(_) | EventKind::Remove(_) | EventKind::Any => true,
        EventKind::Modify(m) => !matches!(m, ModifyKind::Metadata(_)),
        EventKind::Access(_) | EventKind::Other => false,
    }
}

/// A path is relevant when it is inside the workspace, outside the ledger,
/// and not excluded from snapshots by the ignore rules. Build outputs
/// (`target/`, `node_modules/`) are the loudest sources of irrelevant events.
fn is_relevant(repo: &Repo, p: &Path) -> bool {
    if is_internal(repo, p) {
        return false;
    }
    match p.strip_prefix(&repo.root) {
        Ok(rel) => !rel.as_os_str().is_empty() && !is_ignored(rel),
        Err(_) => false,
    }
}

fn is_internal(repo: &Repo, p: &std::path::Path) -> bool {
    p.starts_with(&repo.dir) || p.components().any(|c| c.as_os_str() == ".causari")
}

fn record_change(
    repo: &Repo,
    store: &Store,
    args: &WatchArgs,
    touched: &HashSet<PathBuf>,
    baseline_snapshot_id: &str,
) -> Result<()> {
    // Same logic as `re record`, simplified. The first event's pre-state is
    // the baseline captured at watcher startup. The lock serializes us with
    // any other recorder (hooks, MCP, a second watcher) for the whole
    // read-parent → snapshot → commit section.
    let _lock = repo.lock()?;
    let session = args.session.as_deref();
    let parent_id = resolve_parent(repo, session)?;
    let pre_snapshot_id = match &parent_id {
        Some(pid) => store.read_event(pid)?.post_snapshot,
        None => baseline_snapshot_id.to_string(),
    };
    let post_tree = snapshot_workspace(repo)?;
    // Nothing changed at content level (editor temp files, touches, a
    // formatter that produced identical bytes): no event.
    if tree_unchanged(store, &pre_snapshot_id, &post_tree)? {
        return Ok(());
    }
    let post_snapshot_id = store.write_snapshot(&Snapshot {
        tree: post_tree,
        created_at: Utc::now().to_rfc3339(),
    })?;

    // Causal join with the capture layer (see capture.rs).
    let window_secs = args.window.unwrap_or(300);
    let since = now_ms().saturating_sub(window_secs * 1000);
    let mut correlation = None;
    if let Ok(exchanges) = load_unclaimed_exchanges_since(repo, since) {
        if !exchanges.is_empty() {
            let added = added_lines_between(store, &pre_snapshot_id, &post_snapshot_id, 400)?;
            correlation = correlate(&added, &exchanges);
        }
    }

    let mut writes: Vec<String> = touched
        .iter()
        .filter_map(|p| {
            p.strip_prefix(&repo.root)
                .ok()
                .map(|r| r.to_string_lossy().replace('\\', "/").to_string())
        })
        .collect();
    writes.sort();
    writes.dedup();

    let (prompt, model, tokens_in, tokens_out, cost_usd, corr_agent) = match &correlation {
        Some(c) => (
            c.exchange.prompt.clone(),
            c.exchange.model.clone(),
            c.exchange.tokens_in,
            c.exchange.tokens_out,
            c.exchange.cost_usd,
            c.exchange.agent.clone(),
        ),
        None => (None, None, None, None, None, None),
    };

    let event = Event {
        schema: "causari.event.v0.2".to_string(),
        parent: parent_id,
        agent: args.agent.clone().or(corr_agent),
        model: args.model.clone().or(model),
        tool: Some("watch".to_string()),
        message: Some(format!("{} file(s) changed", writes.len())),
        prompt,
        reasoning: None,
        reads: Vec::new(),
        writes: writes.clone(),
        tokens_in,
        tokens_out,
        cost_usd,
        pre_snapshot: pre_snapshot_id,
        post_snapshot: post_snapshot_id,
        exit_code: None,
        created_at: Utc::now().to_rfc3339(),
        evidence: Some(match &correlation {
            Some(c) => Evidence::Correlated {
                exchange_id: c.exchange.id.clone(),
                matched: c.matched,
                considered: c.considered,
            },
            None => Evidence::observed("watch"),
        }),
    };
    let id = commit_event(repo, store, &event, session)?;
    if let Some(c) = &correlation {
        // Tokens and cost of this exchange now belong to `id`; later windows
        // must not re-attribute them.
        claim_exchange(repo, &c.exchange, &id)?;
    }

    let preview = writes
        .iter()
        .take(3)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    let extra = if writes.len() > 3 {
        format!(" (+{} more)", writes.len() - 3)
    } else {
        String::new()
    };
    println!(
        "  {} {}  {}{}",
        "•".green(),
        (&id[..10]).bright_black(),
        preview,
        extra.bright_black()
    );
    if let Some(c) = &correlation {
        let prompt_preview = c
            .exchange
            .prompt
            .as_deref()
            .map(|p| {
                let first = p.lines().next().unwrap_or("");
                let mut s: String = first.chars().take(70).collect();
                if first.chars().count() > 70 {
                    s.push('…');
                }
                s
            })
            .unwrap_or_else(|| "(no prompt)".to_string());
        println!(
            "    {} \"{}\"  {} {}",
            "↳ intent:".cyan(),
            prompt_preview.italic(),
            c.exchange
                .model
                .as_deref()
                .unwrap_or("unknown-model")
                .bright_black(),
            format!(
                "(confidence {:.0}%, {}/{} lines)",
                c.score * 100.0,
                c.matched,
                c.considered
            )
            .bright_black()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{AccessKind, CreateKind, DataChange, MetadataKind, RemoveKind};

    #[test]
    fn access_and_metadata_events_never_trigger_a_snapshot() {
        assert!(!is_content_change(&EventKind::Access(AccessKind::Open(
            notify::event::AccessMode::Read
        ))));
        assert!(!is_content_change(&EventKind::Access(AccessKind::Close(
            notify::event::AccessMode::Read
        ))));
        assert!(!is_content_change(&EventKind::Modify(
            ModifyKind::Metadata(MetadataKind::Any)
        )));
        assert!(!is_content_change(&EventKind::Other));
    }

    #[test]
    fn content_events_do() {
        assert!(is_content_change(&EventKind::Create(CreateKind::File)));
        assert!(is_content_change(&EventKind::Remove(RemoveKind::File)));
        assert!(is_content_change(&EventKind::Modify(ModifyKind::Data(
            DataChange::Content
        ))));
        assert!(is_content_change(&EventKind::Modify(ModifyKind::Name(
            notify::event::RenameMode::Any
        ))));
        assert!(is_content_change(&EventKind::Any));
    }

    #[test]
    fn ignored_ledger_and_build_paths_are_not_relevant() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = Repo::init(tmp.path()).unwrap();
        let root = &repo.root;
        assert!(!is_relevant(&repo, &root.join(".causari/objects/ab/cd")));
        assert!(!is_relevant(&repo, &root.join(".git/index")));
        assert!(!is_relevant(&repo, &root.join("node_modules/x/1.js")));
        assert!(!is_relevant(&repo, &root.join("target/debug/o1.o")));
        assert!(!is_relevant(&repo, &root.join(".env.local")));
        assert!(!is_relevant(&repo, root), "the root itself is not a change");
        assert!(!is_relevant(&repo, Path::new("/somewhere/else/a.rs")));
        assert!(is_relevant(&repo, &root.join("src/main.rs")));
    }
}
