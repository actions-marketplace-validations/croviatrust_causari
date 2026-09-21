//! `re hook cursor` end to end, through the binary: install the hooks in a
//! fresh repository, feed the payloads Cursor would send, ask the ledger.
use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Output, Stdio},
};

use serde_json::{Value, json};

fn re(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_re"))
        .args(args)
        .current_dir(root)
        .env("NO_COLOR", "1")
        .env_remove("CURSOR_PROJECT_DIR")
        .output()
        .unwrap()
}

fn ok(root: &Path, args: &[&str]) -> String {
    let output = re(root, args);
    assert!(
        output.status.success(),
        "re {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

/// `re hook-event cursor:<event>` with `payload` on stdin, as Cursor runs it.
fn hook_event(root: &Path, event: &str, payload: &Value) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_re"))
        .args(["hook-event", &format!("cursor:{event}")])
        .current_dir(root)
        .env("NO_COLOR", "1")
        .env_remove("CURSOR_PROJECT_DIR")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(payload.to_string().as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "hook-event cursor:{event} exited non-zero"
    );
    assert!(
        output.stderr.is_empty(),
        "hook-event cursor:{event} wrote to stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "hook-event cursor:{event} did not answer JSON ({e}): {:?}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

fn common(root: &Path, conversation: &str, event: &str) -> Value {
    json!({
        "conversation_id": conversation,
        "generation_id": "gen-01",
        "model": "claude-opus-4-7-thinking-max",
        "model_id": "claude-opus-4-7",
        "model_params": [{ "id": "thinking", "value": "true" }],
        "hook_event_name": event,
        "cursor_version": "1.7.2",
        "workspace_roots": [root.to_string_lossy()],
        "user_email": null,
        "transcript_path": null
    })
}

fn with(mut base: Value, extra: Value) -> Value {
    base.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    base
}

fn expected_hooks_json() -> Value {
    json!({
        "version": 1,
        "hooks": {
            "sessionStart": [{ "command": "re hook-event cursor:sessionStart" }],
            "beforeSubmitPrompt": [{ "command": "re hook-event cursor:beforeSubmitPrompt" }],
            "preToolUse": [{ "command": "re hook-event cursor:preToolUse", "matcher": "Shell|Write" }],
            "afterFileEdit": [{ "command": "re hook-event cursor:afterFileEdit" }],
            "afterShellExecution": [{ "command": "re hook-event cursor:afterShellExecution" }],
            "afterAgentResponse": [{ "command": "re hook-event cursor:afterAgentResponse" }],
            "stop": [{ "command": "re hook-event cursor:stop" }]
        }
    })
}

fn fixture() -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    // A git work tree, so `re init` writes .gitignore. Not fatal without git.
    let _ = Command::new("git")
        .args(["init", "-q"])
        .current_dir(temp.path())
        .output();
    ok(temp.path(), &["init"]);
    temp
}

const PROMPT: &str = "Add a health-check endpoint\nreturning build sha and uptime";
const CODE: &str =
    "export function health() {\n  return { sha: SHA, uptime: process.uptime() };\n}\n";

#[test]
fn install_is_exact_idempotent_and_dry_run_writes_nothing() {
    let temp = fixture();
    let root = temp.path();
    let path = root.join(".cursor/hooks.json");

    let dry = ok(root, &["hook", "cursor", "--dry-run"]);
    assert_eq!(
        serde_json::from_str::<Value>(&dry).unwrap(),
        expected_hooks_json()
    );
    assert!(!path.exists(), "--dry-run must not write");

    let out = ok(root, &["hook", "cursor", "--project"]);
    assert!(out.contains("Cursor hooks installed"), "{out}");
    let first = fs::read_to_string(&path).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&first).unwrap(),
        expected_hooks_json()
    );
    assert!(first.ends_with('\n'));

    ok(root, &["hook", "cursor"]);
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        first,
        "second install changed the file"
    );

    // The ledger stays out of git; the hooks file is meant to be committed.
    if root.join(".git").exists() {
        let gitignore = fs::read_to_string(root.join(".gitignore")).unwrap();
        assert!(gitignore.lines().any(|l| l.trim() == ".causari/"));
        assert!(!gitignore.contains(".cursor"));
    }
}

#[test]
fn prompt_then_edit_is_answered_by_why_and_show() {
    let temp = fixture();
    let root = temp.path();
    ok(root, &["hook", "cursor"]);

    let submit = with(
        common(root, "conv-42", "beforeSubmitPrompt"),
        json!({ "prompt": PROMPT, "attachments": [] }),
    );
    assert_eq!(
        hook_event(root, "beforeSubmitPrompt", &submit),
        json!({ "continue": true })
    );

    let pre = with(
        common(root, "conv-42", "preToolUse"),
        json!({ "tool_name": "Write", "tool_input": { "file_path": "src/health.ts" }, "tool_use_id": "tu-1", "cwd": root.to_string_lossy() }),
    );
    assert_eq!(
        hook_event(root, "preToolUse", &pre),
        json!({ "permission": "allow" })
    );

    let file = root.join("src/health.ts");
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, CODE).unwrap();
    let edit = with(
        common(root, "conv-42", "afterFileEdit"),
        json!({ "file_path": file.to_string_lossy(), "edits": [{ "old_string": "", "new_string": CODE }] }),
    );
    assert_eq!(hook_event(root, "afterFileEdit", &edit), json!({}));

    let id = fs::read_to_string(root.join(".causari/refs/sessions/main"))
        .unwrap()
        .trim()
        .to_string();
    let shown: Value = serde_json::from_str(&ok(root, &["show", &id, "--json"])).unwrap();
    assert_eq!(shown["agent"], json!("cursor"));
    assert_eq!(shown["model"], json!("claude-opus-4-7"));
    assert_eq!(shown["prompt"], json!(PROMPT));
    assert_eq!(shown["tool"], json!("Write"));
    assert_eq!(shown["writes"], json!(["src/health.ts"]));
    assert_eq!(
        shown["evidence"],
        json!({ "class": "declared", "source": "cursor-hook" })
    );
    assert!(shown.get("tokens_in").is_none());

    let why = ok(root, &["why", "src/health.ts:2"]);
    assert!(why.contains("agent:     cursor"), "{why}");
    assert!(why.contains("model:     claude-opus-4-7"), "{why}");
    assert!(why.contains("Add a health-check endpoint"), "{why}");
    assert!(why.contains("returning build sha and uptime"), "{why}");
    assert!(why.contains("declared by cursor-hook"), "{why}");

    // The rest of the turn: answer, stop. Both answer `{}`; stop leaves no
    // pending pre-state behind.
    let answer = with(
        common(root, "conv-42", "afterAgentResponse"),
        json!({ "text": "Added src/health.ts with the endpoint." }),
    );
    assert_eq!(hook_event(root, "afterAgentResponse", &answer), json!({}));
    let stop = with(
        common(root, "conv-42", "stop"),
        json!({ "status": "completed", "loop_count": 0 }),
    );
    assert_eq!(hook_event(root, "stop", &stop), json!({}));
    let pending = root.join(".causari/capture/pending-pre.jsonl");
    assert!(!pending.exists() || fs::read_to_string(&pending).unwrap().trim().is_empty());
}

#[test]
fn edits_outside_the_repository_are_ignored_silently() {
    let temp = fixture();
    let root = temp.path();
    let elsewhere = tempfile::tempdir().unwrap();
    let file = elsewhere.path().join("notes.md");
    fs::write(&file, "# notes\n").unwrap();
    let edit = with(
        common(root, "conv-1", "afterFileEdit"),
        json!({ "file_path": file.to_string_lossy(), "edits": [{ "old_string": "", "new_string": "# notes\n" }] }),
    );
    assert_eq!(hook_event(root, "afterFileEdit", &edit), json!({}));
    assert!(!root.join(".causari/refs/sessions/main").exists());
}

#[test]
fn hooks_answer_valid_json_even_without_a_ledger() {
    // Cursor treats a preToolUse answer that is not valid JSON as a block.
    let bare = tempfile::tempdir().unwrap();
    let pre = with(
        common(bare.path(), "conv-1", "preToolUse"),
        json!({ "tool_name": "Shell", "tool_input": { "command": "ls" } }),
    );
    assert_eq!(
        hook_event(bare.path(), "preToolUse", &pre),
        json!({ "permission": "allow" })
    );
    let submit = with(
        common(bare.path(), "conv-1", "beforeSubmitPrompt"),
        json!({ "prompt": "hello" }),
    );
    assert_eq!(
        hook_event(bare.path(), "beforeSubmitPrompt", &submit),
        json!({ "continue": true })
    );
    assert_eq!(
        hook_event(bare.path(), "afterFileEdit", &json!({})),
        json!({})
    );
}

#[test]
fn user_level_install_merges_into_the_home_hooks_file() {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join(".cursor/hooks.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        &path,
        r#"{ "version": 1, "hooks": { "afterFileEdit": [{ "command": "./hooks/format.sh" }] } }"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_re"))
        .args(["hook", "cursor", "--user"])
        .current_dir(home.path())
        .env("NO_COLOR", "1")
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let merged: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        merged["hooks"]["afterFileEdit"],
        json!([
            { "command": "./hooks/format.sh" },
            { "command": "re hook-event cursor:afterFileEdit" }
        ])
    );
    assert_eq!(
        merged["hooks"]["stop"],
        expected_hooks_json()["hooks"]["stop"]
    );
}

#[test]
fn unknown_target_names_both_supported_runtimes() {
    let temp = fixture();
    let output = re(temp.path(), &["hook", "windsurf"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("claude-code, cursor"));
    let output = re(temp.path(), &["hook", "claude-code", "--user"]);
    assert!(!output.status.success());
}
