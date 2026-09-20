//! `re audit` end to end: the JSON contract and the shallow-clone guard,
//! exercised against real git repositories built in a temp dir.
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

/// Git with a fixed identity and no commit signing, so the user's global
/// signing setup cannot slow down or block the synthetic repositories.
fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "commit.gpgsign=false", "-c", "tag.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "Tarik")
        .env("GIT_AUTHOR_EMAIL", "tarik@example.com")
        .env("GIT_COMMITTER_NAME", "Tarik")
        .env("GIT_COMMITTER_EMAIL", "tarik@example.com")
        .status()
        .expect("git must be installed");
    assert!(status.success(), "git {args:?} failed");
}

fn re(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_re"))
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap()
}

fn json(out: &Output) -> serde_json::Value {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("valid JSON on stdout")
}

/// One human commit, then one Claude-tagged commit adding two lines.
fn repo_with_history() -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path();
    git(dir, &["init", "-q", "-b", "main"]);
    fs::write(dir.join("main.py"), "print('hello')\n").unwrap();
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", "initial scaffold"]);
    fs::write(
        dir.join("auth.py"),
        "def refresh(user):\n    return rotate(user)\n",
    )
    .unwrap();
    git(dir, &["add", "."]);
    git(
        dir,
        &[
            "commit",
            "-q",
            "-m",
            "add token refresh\n\nCo-Authored-By: Claude <noreply@anthropic.com>",
        ],
    );
    temp
}

#[test]
fn json_report_carries_compat_fields_and_method_v2_extras() {
    let temp = repo_with_history();
    let v = json(&re(temp.path(), &["audit", "--json"]));

    assert_eq!(v["total_commits"], 2);
    assert_eq!(v["verified"]["commits"], 1);
    assert_eq!(v["verified"]["introduced"], 2);
    assert_eq!(v["verified"]["surviving"], 2);
    assert_eq!(v["verified"]["survival_rate"], 1.0);
    assert_eq!(v["probable"]["commits"], 0);
    assert_eq!(v["by_agent"]["claude-code"]["introduced"], 2);

    for key in [
        "median_survival",
        "capped_survival_rate",
        "largest_commit_share",
    ] {
        assert!(v["verified"].get(key).is_some(), "verified.{key} missing");
        assert!(
            v["by_agent"]["claude-code"].get(key).is_some(),
            "by_agent.claude-code.{key} missing"
        );
    }
    assert_eq!(v["coverage"]["method"], "v2");
    assert_eq!(
        v["coverage"]["blame_flags"],
        serde_json::json!(["-w", "-M", "-C"])
    );
    assert_eq!(v["coverage"]["shallow"], false);
    assert_eq!(v["coverage"]["sample_floor"], 5);
    assert_eq!(v["coverage"]["small_sample"], true);
    assert_eq!(v["method"], "v2");
}

#[test]
fn human_readable_outputs_name_the_method_version_and_no_verdict() {
    let temp = repo_with_history();
    let summary = re(temp.path(), &["audit", "--summary"]);
    assert!(summary.status.success());
    let text = String::from_utf8_lossy(&summary.stdout).into_owned();
    let sub = text
        .lines()
        .find(|l| l.starts_with("<sub>"))
        .expect("summary ends with a <sub> footer");
    assert!(sub.contains("Method v2"), "{sub}");
    assert!(text.contains("capped") && text.contains("median"), "{text}");

    let terminal = re(temp.path(), &["audit"]);
    assert!(terminal.status.success());
    let text = String::from_utf8_lossy(&terminal.stdout).into_owned() + &text;
    assert!(text.contains("method v2"), "{text}");
    // Hard rule of the project: audit output measures, it does not grade.
    for verdict in ["healthy", "churn", "waste", "🟢", "🟡", "🔴"] {
        assert!(
            !text.to_lowercase().contains(verdict),
            "verdict word {verdict:?} in audit output"
        );
    }
}

#[test]
fn shallow_clone_is_refused_unless_allowed() {
    let origin = repo_with_history();
    let clones = tempfile::tempdir().unwrap();
    let clone = clones.path().join("shallow");
    let url = format!("file://{}", origin.path().display());
    git(
        clones.path(),
        &["clone", "-q", "--depth", "1", &url, clone.to_str().unwrap()],
    );

    let refused = re(&clone, &["audit", "--json"]);
    assert!(!refused.status.success(), "a shallow clone must be refused");
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(stderr.contains("shallow"), "{stderr}");
    assert!(stderr.contains("git fetch --unshallow"), "{stderr}");
    assert!(stderr.contains("fetch-depth: 0"), "{stderr}");
    assert!(stderr.contains("--allow-shallow"), "{stderr}");
    assert!(refused.stdout.is_empty(), "no partial report on refusal");

    let allowed = re(&clone, &["audit", "--json", "--allow-shallow"]);
    let stderr = String::from_utf8_lossy(&allowed.stderr);
    assert!(
        stderr.contains("warning") && stderr.contains("shallow"),
        "{stderr}"
    );
    let v = json(&allowed);
    assert_eq!(v["coverage"]["shallow"], true);
    assert_eq!(v["coverage"]["method"], "v2");
}
