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
fn json_report_carries_compat_fields_and_method_extras() {
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
    assert_eq!(v["coverage"]["method"], "v5");
    assert_eq!(
        v["coverage"]["blame_flags"],
        serde_json::json!(["-w", "-M", "-C"])
    );
    assert_eq!(v["coverage"]["shallow"], false);
    assert_eq!(v["coverage"]["sample_floor"], 5);
    assert_eq!(v["coverage"]["small_sample"], true);
    assert_eq!(v["coverage"]["copy_capped_lines"], 0);
    assert!(v["coverage"]["ignore_revs_sha256"].is_null());
    assert!(v["coverage"]["notes_oid"].is_null());
    assert!(v["coverage"].get("skipped_files").is_none());
    assert_eq!(v["method"], "v5");
    assert_eq!(v["by_agent"]["claude-code"]["below_floor"], true);
    assert_eq!(v["commits"].as_array().map(Vec::len), Some(2));
    assert_eq!(v["commits"][1]["class"], "verified");
    assert_eq!(v["commits"][1]["agent"], "claude-code");
    assert_eq!(v["commits"][1]["introduced"], 2);
    assert_eq!(v["commits"][1]["owned_at_head"], 2);
    assert_eq!(v["commits"][1]["surviving"], 2);

    // Method v3: the same repository's untagged lines are the baseline. The
    // scaffold commit is the one untagged commit; its line stands at HEAD.
    let b = &v["baseline"];
    assert_eq!(b["untagged"]["commits"], 1);
    assert_eq!(b["untagged"]["introduced"], 1);
    assert_eq!(b["untagged"]["surviving"], 1);
    assert_eq!(b["by_age"].as_array().map(Vec::len), Some(6));
    assert_eq!(b["by_age"][0]["from_days"], 0);
    assert_eq!(b["by_age"][0]["to_days"], 30);
    assert_eq!(b["by_age"][0]["tagged"]["commits"], 1);
    assert_eq!(b["by_age"][0]["untagged"]["commits"], 1);
    assert!(b["by_age"][5]["to_days"].is_null());
    // Two commits do not reach the floor: no age-matched figure, and the
    // oldest surviving line is the first commit with nothing before it.
    assert!(b["age_matched"].is_null());
    assert_eq!(b["oldest_surviving"]["commits_before"], 0);
    assert_eq!(b["oldest_surviving"]["age_days"], 0);

    // The report names what it measured: the commit at HEAD, and the origin
    // label (a digest of the path here, since this repo has no remote).
    let head = v["repository"]["head"].as_str().expect("repository.head");
    assert_eq!(head.len(), 40);
    assert!(head.chars().all(|c| c.is_ascii_hexdigit()));
    let origin = v["repository"]["origin"]
        .as_str()
        .expect("repository.origin");
    assert!(
        origin.starts_with("sha256:"),
        "origin without remote: {origin}"
    );
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
    assert!(sub.contains("Method v5"), "{sub}");
    assert!(text.contains("untagged lines"), "{text}");
    assert!(text.contains("capped") && text.contains("median"), "{text}");

    let terminal = re(temp.path(), &["audit"]);
    assert!(terminal.status.success());
    let text = String::from_utf8_lossy(&terminal.stdout).into_owned() + &text;
    assert!(text.contains("method v5"), "{text}");
    assert!(text.contains("Baseline: untagged lines"), "{text}");
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
    let raw_origin = origin.path().to_string_lossy().replace('\\', "/");
    let url = format!("file:///{}", raw_origin.trim_start_matches('/'));
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
    assert_eq!(v["coverage"]["method"], "v5");
}

fn commit_file(dir: &Path, name: &str, body: &str, message: &str) {
    fs::write(dir.join(name), body).unwrap();
    git(dir, &["add", name]);
    git(dir, &["commit", "-q", "-m", message]);
}

/// Five untagged commits and five metadata-matched commits in one age
/// window, plus one Cursor commit so one agent row is under the floor.
fn repo_with_age_match_and_small_agent() -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path();
    git(dir, &["init", "-q", "-b", "main"]);
    for i in 0..5 {
        commit_file(
            dir,
            &format!("hand{i}.py"),
            &format!("hand = {i}\n"),
            &format!("hand {i}"),
        );
    }
    for i in 0..5 {
        commit_file(
            dir,
            &format!("ai{i}.py"),
            &format!("ai = {i}\n"),
            &format!("ai {i}\n\nCo-Authored-By: Claude <noreply@anthropic.com>"),
        );
    }
    commit_file(
        dir,
        "cursor.py",
        "cursor = 1\n",
        "cursor\n\nCo-Authored-By: Cursor Agent <cursoragent@cursor.com>",
    );
    temp
}

#[test]
fn audit_reading_export_and_unavailable_age_match() {
    let temp = repo_with_history();
    let terminal = re(temp.path(), &["audit"]);
    assert!(terminal.status.success());
    let text = String::from_utf8_lossy(&terminal.stdout);
    let survival = text.find("survival ").expect("survival line");
    let meaning = text
        .find("not the share of the repository")
        .expect("meaning");
    let gap = text.find("Age-matched gap: unavailable").expect("gap");
    let probable = text.find("Probable AI-assisted").expect("probable");
    assert!(
        survival < meaning && meaning < gap && gap < probable,
        "reading order:\n{text}"
    );
    let line_after_survival = text[survival..].lines().nth(1).unwrap_or("");
    assert!(
        line_after_survival.contains("not the share of the repository"),
        "meaning must be the next line after the percentage, got {line_after_survival:?}"
    );
    assert!(
        text.contains("does not prove who wrote each line"),
        "{text}"
    );
    assert!(
        text.contains("not a finding that a human wrote them"),
        "{text}"
    );
    assert!(
        text.contains("below floor: 1 commit (floor 5); not comparable"),
        "{text}"
    );
    assert!(
        text.contains("Meeting the floor is not a reliability guarantee"),
        "{text}"
    );
    assert!(text.contains("--json"), "{text}");
    assert!(text.contains("--summary"), "{text}");
    assert!(text.contains("--seal"), "{text}");
    assert!(text.contains("refs/notes/ai"), "{text}");
    assert!(text.contains("has moved"), "{text}");
    assert!(
        text.contains("`re report` reads the local ledger"),
        "{text}"
    );

    let summary = re(temp.path(), &["audit", "--summary"]);
    assert!(summary.status.success());
    let summary_text = String::from_utf8_lossy(&summary.stdout);
    let bold = summary_text
        .find("AI-tagged (metadata matched)")
        .expect("summary headline");
    let summary_gap = summary_text
        .find("Age-matched gap: unavailable")
        .expect("summary gap");
    assert!(bold < summary_gap, "{summary_text}");
    assert!(
        summary_text.contains("claude-code: below floor:"),
        "{summary_text}"
    );

    let exported = json(&re(temp.path(), &["audit", "--json"]));
    let head = exported["repository"]["head"]
        .as_str()
        .expect("repository.head");
    assert_eq!(head.len(), 40);
    assert!(
        text.contains(head),
        "terminal names the measured commit:\n{text}"
    );
    assert_eq!(exported["coverage"]["method"], "v5");
    assert_eq!(
        exported["coverage"]["blame_flags"],
        serde_json::json!(["-w", "-M", "-C"])
    );
    assert!(exported["baseline"]["age_matched"].is_null());
    assert!(text.contains("not a measured gap of zero"), "{text}");
    assert!(
        text.contains("at least 5 commits of both kinds with introduced lines"),
        "{text}"
    );
    assert!(
        !text.contains("No AI-tagged lines are included in the measurement"),
        "a repo with tagged lines must not use the empty-introduction sentence:\n{text}"
    );
}

#[test]
fn age_matched_gap_states_units_and_direction() {
    let temp = repo_with_age_match_and_small_agent();
    let terminal = re(temp.path(), &["audit"]);
    assert!(terminal.status.success());
    let text = String::from_utf8_lossy(&terminal.stdout);
    assert!(
        text.contains(
            "Age-matched gap: +0.0 percentage points (AI-tagged 100.0% minus untagged 100.0% in the matched age windows)"
        ),
        "{text}"
    );
    assert!(
        !text.contains("same age") && !text.contains("of the same age"),
        "windows are buckets, not identical timestamps:\n{text}"
    );
    assert!(
        text.contains(
            "Positive means the metadata-matched lines have the higher line-weighted survival"
        ),
        "{text}"
    );
    assert!(text.contains("below floor: 1 commit (floor 5)"), "{text}");
    // The five-commit agent is at the floor. The warning belongs to the
    // one-commit row. Meeting the floor is not described as reliability.
    let cursor = text.find("cursor").expect("cursor row");
    let claude = text.find("claude-code").expect("claude row");
    let cursor_warning = text[cursor..].find("below floor").expect("cursor floor");
    let claude_section_end = if claude < cursor { cursor } else { text.len() };
    let claude_slice = if claude < cursor {
        &text[claude..claude_section_end]
    } else {
        &text[claude..]
    };
    assert!(
        !claude_slice.contains("below floor"),
        "claude row is at the floor and must not be marked below it:\n{claude_slice}"
    );
    assert!(
        cursor_warning < 400,
        "cursor warning follows the cursor row"
    );

    let v = json(&re(temp.path(), &["audit", "--json"]));
    let gap = v["baseline"]["age_matched"]["gap"].as_f64().expect("gap");
    assert!(gap.abs() < 1e-9, "{gap}");
    assert_eq!(v["by_agent"]["claude-code"]["commits"], 5);
    assert_eq!(v["by_agent"]["cursor"]["commits"], 1);
}

#[test]
fn same_commit_changes_class_when_the_git_ai_note_changes() {
    let temp = repo_with_history();
    let dir = temp.path();
    let before = json(&re(dir, &["audit", "--json"]));
    let head = before["repository"]["head"].as_str().unwrap().to_string();
    assert_eq!(before["verified"]["commits"], 1);

    let schema_only = dir.join("schema-only.txt");
    fs::write(
        &schema_only,
        "auth.py\n---\n{\"schema_version\":\"authorship/3.0.0\",\"prompts\":{}}\n",
    )
    .unwrap();
    git(
        dir,
        &[
            "notes",
            "--ref=ai",
            "add",
            "-f",
            "-F",
            schema_only.to_str().unwrap(),
            "HEAD",
        ],
    );
    let mid = json(&re(dir, &["audit", "--json"]));
    assert_eq!(mid["repository"]["head"], head);
    assert_eq!(
        mid["verified"]["commits"], 1,
        "a schema-only note must not add a verified commit under v4"
    );

    let named = dir.join("named.txt");
    fs::write(
        &named,
        concat!(
            "auth.py\n  p 1\n---\n",
            "{\"schema_version\":\"authorship/3.0.0\",",
            "\"prompts\":{\"p\":{\"agent_id\":{\"tool\":\"mock_ai\"}}}}\n"
        ),
    )
    .unwrap();
    // HEAD is the Claude-tagged commit. A named tool in the note outranks
    // the trailer, so the agent changes while the commit SHA does not.
    git(
        dir,
        &[
            "notes",
            "--ref=ai",
            "add",
            "-f",
            "-F",
            named.to_str().unwrap(),
            "HEAD",
        ],
    );
    let after = json(&re(dir, &["audit", "--json"]));
    assert_eq!(after["repository"]["head"], head);
    assert!(
        after["by_agent"].get("mock_ai").is_some(),
        "named tool on an unchanged commit changes the agent: {after}"
    );
    // The agent key is the classification result. The note body is not stored.
    let raw = serde_json::to_string(&after).unwrap();
    assert!(!raw.contains("schema_version"), "{raw}");
    assert!(!raw.contains("authorship/3.0.0"), "{raw}");
    assert!(after.get("notes").is_none());
    assert!(after["repository"].get("notes").is_none());
}

#[test]
fn report_outside_a_ledger_points_at_remote_audit() {
    let temp = tempfile::tempdir().unwrap();
    let out = re(temp.path(), &["report", "--open"]);
    assert!(!out.status.success(), "report outside a ledger must fail");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("local ledger"), "{err}");
    assert!(err.contains("re init"), "{err}");
    assert!(err.contains("re audit owner/repo"), "{err}");
    assert!(
        !err.contains("web audit"),
        "no unimplemented web flow: {err}"
    );
    assert!(err.contains("uses the local ledger"), "{err}");
}

const HEAD_TS: i64 = 1_700_000_000;
const DAY: i64 = 86_400;

fn git_at(dir: &Path, args: &[&str], unix: i64) {
    let when = unix.to_string();
    let status = Command::new("git")
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "tag.gpgsign=false",
            "-c",
            "core.fsmonitor=false",
        ])
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "Tarik")
        .env("GIT_AUTHOR_EMAIL", "tarik@example.com")
        .env("GIT_COMMITTER_NAME", "Tarik")
        .env("GIT_COMMITTER_EMAIL", "tarik@example.com")
        .env("GIT_AUTHOR_DATE", &when)
        .env("GIT_COMMITTER_DATE", &when)
        .status()
        .expect("git must be installed");
    assert!(status.success(), "git {args:?} failed");
}

fn commit_dated(dir: &Path, name: &str, body: &str, message: &str, unix: i64) {
    fs::write(dir.join(name), body).unwrap();
    git_at(dir, &["add", "--", name], unix);
    git_at(dir, &["commit", "-q", "-m", message], unix);
}

/// Five untagged and five Claude commits in the 0–30 day window. `positive`
/// then deletes one line from each untagged file; otherwise from each tagged
/// file. The deletion is a later untagged commit with no introduced lines.
fn repo_signed_gap(positive: bool) -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path();
    let young = HEAD_TS - 10 * DAY;
    git_at(dir, &["init", "-q", "-b", "main"], young);
    for i in 0..5 {
        commit_dated(
            dir,
            &format!("hand{i}.py"),
            "h0\nh1\n",
            &format!("hand {i}"),
            young,
        );
        commit_dated(
            dir,
            &format!("ai{i}.py"),
            "a0\na1\n",
            &format!("ai {i}\n\nCo-Authored-By: Claude <noreply@anthropic.com>"),
            young,
        );
    }
    for i in 0..5 {
        if positive {
            fs::write(dir.join(format!("hand{i}.py")), "h0\n").unwrap();
        } else {
            fs::write(dir.join(format!("ai{i}.py")), "a0\n").unwrap();
        }
    }
    git_at(dir, &["add", "."], HEAD_TS);
    git_at(
        dir,
        &[
            "commit",
            "-q",
            "-m",
            if positive {
                "trim untagged"
            } else {
                "trim tagged"
            },
        ],
        HEAD_TS,
    );
    temp
}

struct ExpectedGap {
    sentence: &'static str,
    gap: f64,
    tagged: f64,
    untagged: f64,
    covered: f64,
}

fn assert_gap_pair(text: &str, summary: &str, v: &serde_json::Value, expected: ExpectedGap) {
    assert!(text.contains(expected.sentence), "terminal:\n{text}");
    assert!(summary.contains(expected.sentence), "summary:\n{summary}");
    assert!(
        !text.contains("same age"),
        "terminal must not claim identical ages:\n{text}"
    );
    let m = &v["baseline"]["age_matched"];
    let got_gap = m["gap"].as_f64().expect("gap");
    let got_tagged = m["tagged_rate"].as_f64().expect("tagged_rate");
    let got_untagged = m["untagged_rate"].as_f64().expect("untagged_rate");
    let got_covered = m["tagged_lines_covered"].as_f64().expect("covered");
    assert!(
        (got_gap - expected.gap).abs() < 1e-9,
        "gap {got_gap} != {}",
        expected.gap
    );
    assert!(
        (got_tagged - expected.tagged).abs() < 1e-9,
        "tagged {got_tagged}"
    );
    assert!(
        (got_untagged - expected.untagged).abs() < 1e-9,
        "untagged {got_untagged}"
    );
    assert!(
        (got_covered - expected.covered).abs() < 1e-9,
        "covered {got_covered}"
    );
    assert_eq!(m["buckets_used"], 1);
    // JSON keeps the rate. The sentence converts it to percentage points.
    assert!((got_gap * 100.0 - expected.gap * 100.0).abs() < 1e-9);
    assert!(v.get("notes").is_none());
}

#[test]
fn age_matched_gap_sign_and_percentage_points_match_json() {
    let positive_sentence = "Age-matched gap: +50.0 percentage points (AI-tagged 100.0% minus untagged 50.0% in the matched age windows)";
    let pos = repo_signed_gap(true);
    let pos_out = re(pos.path(), &["audit"]);
    assert!(
        pos_out.status.success(),
        "{}",
        String::from_utf8_lossy(&pos_out.stderr)
    );
    let pos_sum = re(pos.path(), &["audit", "--summary"]);
    assert!(
        pos_sum.status.success(),
        "{}",
        String::from_utf8_lossy(&pos_sum.stderr)
    );
    let pos_text = String::from_utf8_lossy(&pos_out.stdout).into_owned();
    let pos_summary = String::from_utf8_lossy(&pos_sum.stdout).into_owned();
    let pos_json = json(&re(pos.path(), &["audit", "--json"]));
    assert_gap_pair(
        &pos_text,
        &pos_summary,
        &pos_json,
        ExpectedGap {
            sentence: positive_sentence,
            gap: 0.5,
            tagged: 1.0,
            untagged: 0.5,
            covered: 1.0,
        },
    );
    assert!(
        pos_text.contains("holding 100% of the AI-tagged lines"),
        "{pos_text}"
    );
    assert!((pos_json["verified"]["survival_rate"].as_f64().unwrap() - 1.0).abs() < 1e-9);

    let negative_sentence = "Age-matched gap: -50.0 percentage points (AI-tagged 50.0% minus untagged 100.0% in the matched age windows)";
    let neg = repo_signed_gap(false);
    let neg_out = re(neg.path(), &["audit"]);
    assert!(
        neg_out.status.success(),
        "{}",
        String::from_utf8_lossy(&neg_out.stderr)
    );
    let neg_sum = re(neg.path(), &["audit", "--summary"]);
    assert!(
        neg_sum.status.success(),
        "{}",
        String::from_utf8_lossy(&neg_sum.stderr)
    );
    let neg_text = String::from_utf8_lossy(&neg_out.stdout).into_owned();
    let neg_summary = String::from_utf8_lossy(&neg_sum.stdout).into_owned();
    let neg_json = json(&re(neg.path(), &["audit", "--json"]));
    assert_gap_pair(
        &neg_text,
        &neg_summary,
        &neg_json,
        ExpectedGap {
            sentence: negative_sentence,
            gap: -0.5,
            tagged: 0.5,
            untagged: 1.0,
            covered: 1.0,
        },
    );
    assert!(
        neg_text.contains("holding 100% of the AI-tagged lines"),
        "{neg_text}"
    );
    assert!((neg_json["verified"]["survival_rate"].as_f64().unwrap() - 0.5).abs() < 1e-9);
}

/// Tagged lines in a 90–180 day window have no untagged counterpart, so they
/// stay out of the gap. The 0–30 day window is the positive case above.
#[test]
fn age_matched_gap_covers_only_comparable_windows() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path();
    let old = HEAD_TS - 100 * DAY;
    let young = HEAD_TS - 10 * DAY;
    git_at(dir, &["init", "-q", "-b", "main"], old);
    for i in 0..5 {
        commit_dated(
            dir,
            &format!("old{i}.py"),
            "o0\no1\n",
            &format!("old {i}\n\nCo-Authored-By: Claude <noreply@anthropic.com>"),
            old,
        );
    }
    for i in 0..5 {
        commit_dated(
            dir,
            &format!("hand{i}.py"),
            "h0\nh1\n",
            &format!("hand {i}"),
            young,
        );
        commit_dated(
            dir,
            &format!("ai{i}.py"),
            "a0\na1\n",
            &format!("young {i}\n\nCo-Authored-By: Claude <noreply@anthropic.com>"),
            young,
        );
    }
    for i in 0..5 {
        fs::remove_file(dir.join(format!("old{i}.py"))).unwrap();
        fs::write(dir.join(format!("hand{i}.py")), "h0\n").unwrap();
    }
    git_at(dir, &["add", "-A"], HEAD_TS);
    git_at(
        dir,
        &["commit", "-q", "-m", "drop old and trim young untagged"],
        HEAD_TS,
    );

    let out = re(dir, &["audit"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let summary = String::from_utf8_lossy(&re(dir, &["audit", "--summary"]).stdout).into_owned();
    let v = json(&re(dir, &["audit", "--json"]));
    let sentence = "Age-matched gap: +50.0 percentage points (AI-tagged 100.0% minus untagged 50.0% in the matched age windows)";
    assert_gap_pair(
        &text,
        &summary,
        &v,
        ExpectedGap {
            sentence,
            gap: 0.5,
            tagged: 1.0,
            untagged: 0.5,
            covered: 0.5,
        },
    );
    assert!(
        text.contains("holding 50% of the AI-tagged lines"),
        "{text}"
    );
    assert!(
        text.contains("Lines outside those windows are not in this gap."),
        "{text}"
    );
    // Headline survival counts the deleted older lines. The gap does not.
    let headline = v["verified"]["survival_rate"].as_f64().unwrap();
    assert!((headline - 0.5).abs() < 1e-9, "headline {headline}");
    assert!(
        (v["baseline"]["age_matched"]["tagged_rate"]
            .as_f64()
            .unwrap()
            - headline)
            .abs()
            > 0.4,
        "windowed tagged rate must differ from the headline when coverage is partial"
    );
    assert_eq!(v["verified"]["introduced"], 20);
    assert_eq!(v["verified"]["surviving"], 10);
}

#[test]
fn ledger_commands_share_the_discover_error() {
    let temp = tempfile::tempdir().unwrap();
    let cmds: &[&[&str]] = &[
        &["log"],
        &["why", "file.py:1"],
        &["churn"],
        &["record", "-m", "note"],
        &["seal", "list"],
        &["report"],
        &["show", "abcdef"],
        &["diff", "abc"],
    ];
    for args in cmds {
        let out = re(temp.path(), args);
        assert!(
            !out.status.success(),
            "{args:?} should fail without a ledger"
        );
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(
            err.contains("uses the local ledger"),
            "{args:?} must use the shared discover error:\n{err}"
        );
        assert!(err.contains("`re init` creates it"), "{args:?}: {err}");
        assert!(err.contains("re audit owner/repo"), "{args:?}: {err}");
        assert!(
            err.contains("does not audit a remote repository"),
            "{args:?}: {err}"
        );
    }

    let audit = re(temp.path(), &["audit"]);
    let err = String::from_utf8_lossy(&audit.stderr);
    assert!(
        !err.contains("local ledger"),
        "audit does not require the ledger:\n{err}"
    );
}

/// Metadata-matched commit that only adds a lockfile: introduced lines stay 0.
fn repo_with_absent_tagged_rate() -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path();
    git(dir, &["init", "-q", "-b", "main"]);
    commit_file(dir, "hand.py", "hand = 1\n", "hand");
    commit_file(
        dir,
        "Cargo.lock",
        "# lock\n",
        "lock\n\nCo-Authored-By: Claude <noreply@anthropic.com>",
    );
    temp
}

/// One tagged line, then an untagged commit deletes it.
fn repo_with_measured_zero() -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path();
    git(dir, &["init", "-q", "-b", "main"]);
    commit_file(dir, "hand.py", "hand = 1\n", "hand");
    commit_file(
        dir,
        "doomed.py",
        "doomed = 1\n",
        "doomed\n\nCo-Authored-By: Claude <noreply@anthropic.com>",
    );
    git(dir, &["rm", "-q", "doomed.py"]);
    git(dir, &["commit", "-q", "-m", "remove doomed"]);
    temp
}

#[test]
fn absent_rate_measured_zero_and_unavailable_age_gap_stay_distinct() {
    let absent = repo_with_absent_tagged_rate();
    let absent_json = json(&re(absent.path(), &["audit", "--json"]));
    assert_eq!(absent_json["verified"]["commits"], 1);
    assert_eq!(absent_json["verified"]["introduced"], 0);
    assert_eq!(absent_json["verified"]["surviving"], 0);
    assert!(absent_json["verified"]["survival_rate"].is_null());
    assert!(absent_json["baseline"]["age_matched"].is_null());
    assert_eq!(absent_json["baseline"]["untagged"]["introduced"], 1);
    assert_eq!(absent_json["coverage"]["method"], "v5");

    let absent_text = String::from_utf8_lossy(&re(absent.path(), &["audit"]).stdout).into_owned();
    assert!(absent_text.contains("Rate absent:"), "{absent_text}");
    assert!(
        absent_text.contains("The JSON value null is not a measured zero."),
        "{absent_text}"
    );
    assert!(
        absent_text.contains("no introduced lines are included in this measurement"),
        "{absent_text}"
    );
    assert!(
        absent_text.contains("not a finding that the commits introduced"),
        "{absent_text}"
    );
    assert!(
        absent_text.contains("No AI-tagged lines are included in the measurement"),
        "{absent_text}"
    );
    assert!(
        !absent_text.contains("these metadata-matched commits introduced no lines"),
        "{absent_text}"
    );
    assert!(
        absent_text.contains("survival absent line-weighted"),
        "{absent_text}"
    );
    assert!(
        !absent_text.contains("survival 0.0%"),
        "an absent rate must not be printed as zero:\n{absent_text}"
    );
    assert!(!absent_text.contains("at least 5 commits"), "{absent_text}");

    let zero = repo_with_measured_zero();
    let zero_json = json(&re(zero.path(), &["audit", "--json"]));
    assert_eq!(zero_json["verified"]["commits"], 1);
    assert_eq!(zero_json["verified"]["introduced"], 1);
    assert_eq!(zero_json["verified"]["surviving"], 0);
    assert_eq!(zero_json["verified"]["survival_rate"], 0.0);
    assert!(zero_json["baseline"]["age_matched"].is_null());
    assert_eq!(zero_json["coverage"]["method"], "v5");

    let zero_text = String::from_utf8_lossy(&re(zero.path(), &["audit"]).stdout).into_owned();
    assert!(zero_text.contains("0.0%"), "{zero_text}");
    assert!(zero_text.contains("Measured zero:"), "{zero_text}");
    assert!(
        zero_text.contains("at least 5 commits of both kinds with introduced lines"),
        "{zero_text}"
    );
    assert!(
        zero_text.contains("not a measured gap of zero"),
        "{zero_text}"
    );
    assert!(
        !zero_text.contains("Rate absent:"),
        "a measured zero must not be called absent:\n{zero_text}"
    );
    assert!(
        !zero_text.contains("No AI-tagged lines are included in the measurement"),
        "{zero_text}"
    );
}

#[test]
fn audit_diff_separates_repository_evolution_and_method_upgrades_and_refuses_two_axes() {
    let temp = repo_with_history();
    let dir = temp.path();

    let r1_bytes = re(dir, &["audit", "--json"]).stdout;
    let r1_path = dir.join("r1.json");
    fs::write(&r1_path, &r1_bytes).unwrap();

    // Trim one AI line and add a new AI commit -> repository diff under v5.
    fs::write(dir.join("auth.py"), "def refresh(user):\n").unwrap();
    git(dir, &["add", "auth.py"]);
    git(dir, &["commit", "-q", "-m", "trim auth"]);
    commit_file(
        dir,
        "extra.py",
        "x = 1\ny = 2\nz = 3\n",
        "add extra\n\nCo-Authored-By: Claude <noreply@anthropic.com>",
    );

    let r2_bytes = re(dir, &["audit", "--json"]).stdout;
    let r2_path = dir.join("r2.json");
    fs::write(&r2_path, &r2_bytes).unwrap();

    let diff_out = re(
        dir,
        &[
            "audit",
            "--diff",
            r1_path.to_str().unwrap(),
            r2_path.to_str().unwrap(),
            "--json",
        ],
    );
    let d = json(&diff_out);
    assert_eq!(d["schema"], "causari.audit_diff.v1");
    assert_eq!(d["kind"], "repository_diff");
    assert_eq!(d["commit_set_relation"], "subset_forward");
    assert_eq!(d["provenance"]["status"], "unsealed");
    assert_eq!(d["provenance"]["before_signature_valid"], false);
    assert_eq!(d["provenance"]["after_signature_valid"], false);
    assert_eq!(d["provenance"]["issuer_trust"], "unverified_offline");
    assert_eq!(d["delta"]["verified_commits"], 1);
    assert_eq!(d["delta"]["verified_introduced"], 3);
    assert_eq!(d["delta"]["verified_surviving"], 2);
    assert_eq!(
        d["added_verified_commits"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(
        d["survival_changed_commits"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(d["survival_changed_commits"][0]["delta_surviving"], -1);

    // Reverse diff: after -> before reports commit_set_relation == "subset_reverse".
    let rev_diff = json(&re(
        dir,
        &[
            "audit",
            "--diff",
            r2_path.to_str().unwrap(),
            r1_path.to_str().unwrap(),
            "--json",
        ],
    ));
    assert_eq!(rev_diff["kind"], "repository_diff");
    assert_eq!(rev_diff["commit_set_relation"], "subset_reverse");

    // Build an authentic Method v4 receipt for the exact same HEAD, matching the
    // exact v4 baseline JSON schema (no `commits`, no `repository.root_commits` or
    // `root_commit`, and no `coverage.notes_oid`, `ignore_revs_sha256`,
    // `numstat_flags`, or `copy_capped_lines`).
    let r2_val: serde_json::Value = serde_json::from_slice(&r2_bytes).unwrap();
    let r2_v4 = serde_json::json!({
        "baseline": {
            "age_matched": r2_val["baseline"]["age_matched"],
            "by_age": r2_val["baseline"]["by_age"],
            "untagged": r2_val["baseline"]["untagged"],
        },
        "by_agent": r2_val["by_agent"],
        "coverage": {
            "blame_flags": ["-w", "-M", "-C"],
            "ignore_revs_file": false,
            "method": "v4",
            "sample_floor": 5,
            "shallow": false,
            "small_sample": true,
        },
        "method": "v4",
        "probable": r2_val["probable"],
        "repository": {
            "head": r2_val["repository"]["head"],
            "origin": r2_val["repository"]["origin"],
        },
        "unknown_commits": r2_val["unknown_commits"],
        "verified": r2_val["verified"],
    });
    assert!(r2_v4["coverage"].get("notes_oid").is_none());
    assert!(r2_v4["coverage"].get("ignore_revs_sha256").is_none());
    assert!(r2_v4.get("commits").is_none());
    let r2_v4_path = dir.join("r2_v4.json");
    fs::write(&r2_v4_path, serde_json::to_vec_pretty(&r2_v4).unwrap()).unwrap();

    // Authentic v4 vs v5 on the same HEAD: because v4 did not record `coverage.notes_oid`,
    // `re audit --diff` must NEVER claim a pure `method_diff`; it reports
    // `kind = "indeterminate_method_diff"` and `metadata_status = "indeterminate"`.
    let mdiff_v4_v5 = json(&re(
        dir,
        &[
            "audit",
            "--diff",
            r2_v4_path.to_str().unwrap(),
            r2_path.to_str().unwrap(),
            "--json",
        ],
    ));
    assert_eq!(mdiff_v4_v5["kind"], "indeterminate_method_diff");
    assert_eq!(mdiff_v4_v5["metadata_status"], "indeterminate");
    assert_eq!(mdiff_v4_v5["commit_set_relation"], "same_head");
    assert_eq!(mdiff_v4_v5["commit_sets_available"], false);

    // Pure `method_diff` is reported ONLY when both receipts explicitly record
    // external metadata (`notes_oid` and `ignore_revs_sha256`) and prove it
    // stayed unchanged while method/flags varied.
    let mut r2_alt_flags = r2_val.clone();
    r2_alt_flags["coverage"]["blame_flags"] = serde_json::json!(["-w", "-M"]);
    let r2_alt_flags_path = dir.join("r2_alt_flags.json");
    fs::write(
        &r2_alt_flags_path,
        serde_json::to_vec_pretty(&r2_alt_flags).unwrap(),
    )
    .unwrap();
    let pure_mdiff = json(&re(
        dir,
        &[
            "audit",
            "--diff",
            r2_alt_flags_path.to_str().unwrap(),
            r2_path.to_str().unwrap(),
            "--json",
        ],
    ));
    assert_eq!(pure_mdiff["kind"], "method_diff");
    assert_eq!(pure_mdiff["metadata_status"], "unchanged");
    assert_eq!(pure_mdiff["commit_set_relation"], "same_head");

    // Same HEAD and same method, git-ai note added -> metadata_diff.
    let note_payload =
        r#"{"schema_version":"git-ai/3.0.0","prompts":{"p1":{"agent_id":{"tool":"cursor"}}}}"#;
    git(
        dir,
        &["notes", "--ref=ai", "add", "-m", note_payload, "HEAD"],
    );
    let r2_noted_bytes = re(dir, &["audit", "--json"]).stdout;
    let r2_noted_path = dir.join("r2_noted.json");
    fs::write(&r2_noted_path, &r2_noted_bytes).unwrap();
    git(dir, &["update-ref", "-d", "refs/notes/ai"]);

    let meta_diff = json(&re(
        dir,
        &[
            "audit",
            "--diff",
            r2_path.to_str().unwrap(),
            r2_noted_path.to_str().unwrap(),
            "--json",
        ],
    ));
    assert_eq!(meta_diff["kind"], "metadata_diff");
    assert_eq!(meta_diff["commit_set_relation"], "same_head");
    assert!(meta_diff["before"]["notes_oid"].is_null());
    assert!(meta_diff["after"]["notes_oid"].is_string());

    // Sealed provenance: same issuer, partially sealed, and different issuers.
    let s1_path = dir.join("s1.seal.json");
    assert!(
        re(dir, &["audit", "--seal", "-o", s1_path.to_str().unwrap()])
            .status
            .success()
    );
    let partial_diff = json(&re(
        dir,
        &[
            "audit",
            "--diff",
            s1_path.to_str().unwrap(),
            r2_path.to_str().unwrap(),
            "--json",
        ],
    ));
    assert_eq!(partial_diff["provenance"]["status"], "partially_sealed");
    assert_eq!(partial_diff["provenance"]["before_signature_valid"], true);
    assert_eq!(partial_diff["provenance"]["after_signature_valid"], false);

    let s2_path = dir.join("s2.seal.json");
    assert!(
        re(dir, &["audit", "--seal", "-o", s2_path.to_str().unwrap()])
            .status
            .success()
    );
    let same_issuer_diff = json(&re(
        dir,
        &[
            "audit",
            "--diff",
            s1_path.to_str().unwrap(),
            s2_path.to_str().unwrap(),
            "--json",
        ],
    ));
    assert_eq!(same_issuer_diff["kind"], "identical_inputs");
    assert_eq!(
        same_issuer_diff["provenance"]["status"],
        "sealed_same_issuer"
    );
    assert_eq!(
        same_issuer_diff["provenance"]["before_signature_valid"],
        true
    );
    assert_eq!(
        same_issuer_diff["provenance"]["after_signature_valid"],
        true
    );
    assert_eq!(
        same_issuer_diff["provenance"]["issuer_trust"],
        "unverified_offline"
    );

    // Clone to a second directory (which mints a different issuer key) and seal there.
    let clone_tmp = tempfile::tempdir().unwrap();
    let clone_dir = clone_tmp.path().join("clone");
    git(
        clone_tmp.path(),
        &[
            "clone",
            "-q",
            dir.to_str().unwrap(),
            clone_dir.to_str().unwrap(),
        ],
    );
    let s_other_path = clone_dir.join("other.seal.json");
    assert!(
        re(
            &clone_dir,
            &["audit", "--seal", "-o", s_other_path.to_str().unwrap()]
        )
        .status
        .success()
    );
    let diff_issuer = json(&re(
        dir,
        &[
            "audit",
            "--diff",
            s1_path.to_str().unwrap(),
            s_other_path.to_str().unwrap(),
            "--json",
        ],
    ));
    assert_eq!(
        diff_issuer["provenance"]["status"],
        "sealed_different_issuers"
    );
    assert_ne!(
        diff_issuer["provenance"]["before_pubkey_hex"],
        diff_issuer["provenance"]["after_pubkey_hex"]
    );

    // Multi-root merge (--allow-unrelated-histories): shares original root -> allowed.
    git(dir, &["checkout", "-q", "--orphan", "second_root"]);
    git(dir, &["rm", "-rf", "-q", "."]);
    commit_file(
        dir,
        "orphan.py",
        "o = 1\n",
        "orphan root\n\nCo-Authored-By: Claude <noreply@anthropic.com>",
    );
    git(dir, &["checkout", "-q", "main"]);
    git(
        dir,
        &[
            "merge",
            "-q",
            "--allow-unrelated-histories",
            "-m",
            "merge second root",
            "second_root",
        ],
    );
    let r_multi_path = dir.join("r_multi.json");
    fs::write(&r_multi_path, re(dir, &["audit", "--json"]).stdout).unwrap();
    let multi_diff = json(&re(
        dir,
        &[
            "audit",
            "--diff",
            r2_path.to_str().unwrap(),
            r_multi_path.to_str().unwrap(),
            "--json",
        ],
    ));
    assert_eq!(multi_diff["kind"], "repository_diff");
    assert_eq!(multi_diff["commit_set_relation"], "subset_forward");
    assert_eq!(
        multi_diff["after"]["root_commits"].as_array().map(Vec::len),
        Some(2)
    );
    assert_eq!(
        multi_diff["shared_root_commits"].as_array().map(Vec::len),
        Some(1)
    );

    // Different HEAD AND different method -> refused with exit code 2.
    let refused = re(
        dir,
        &[
            "audit",
            "--diff",
            r1_path.to_str().unwrap(),
            r2_v4_path.to_str().unwrap(),
        ],
    );
    assert_eq!(refused.status.code(), Some(2));
    let err = String::from_utf8_lossy(&refused.stderr);
    assert!(err.contains("two axes"), "{err}");

    // Same HEAD, different flags/method AND different metadata (`notes_oid`) -> refused with exit code 2.
    let refused_meta = re(
        dir,
        &[
            "audit",
            "--diff",
            r2_alt_flags_path.to_str().unwrap(),
            r2_noted_path.to_str().unwrap(),
        ],
    );
    assert_eq!(refused_meta.status.code(), Some(2));
    let err_meta = String::from_utf8_lossy(&refused_meta.stderr);
    assert!(err_meta.contains("two axes"), "{err_meta}");

    // Authentic v4 (`ignore_revs_file: false`) vs v5 where `ignore_revs_file: true` ->
    // observable metadata changed AND method changed -> refused with exit code 2.
    let mut r2_ir = r2_val.clone();
    r2_ir["coverage"]["ignore_revs_file"] = serde_json::json!(true);
    r2_ir["coverage"]["ignore_revs_sha256"] = serde_json::json!(
        "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
    );
    let r2_ir_path = dir.join("r2_ir.json");
    fs::write(&r2_ir_path, serde_json::to_vec_pretty(&r2_ir).unwrap()).unwrap();
    let refused_v4_ir = re(
        dir,
        &[
            "audit",
            "--diff",
            r2_v4_path.to_str().unwrap(),
            r2_ir_path.to_str().unwrap(),
        ],
    );
    assert_eq!(refused_v4_ir.status.code(), Some(2));
    let err_v4_ir = String::from_utf8_lossy(&refused_v4_ir.stderr);
    assert!(err_v4_ir.contains("two axes"), "{err_v4_ir}");
}
