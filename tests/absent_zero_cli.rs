//! Absence is not zero, and metadata is not authorship, on the surfaces a
//! reader meets without the JSON: the badge, the card, and the Markdown
//! summary the GitHub Action posts. The CLI text and the JSON already have
//! tests; a regression on these three would still reach READMEs and PRs.
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

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
    let out = Command::new(env!("CARGO_BIN_EXE_re"))
        .args(args)
        .current_dir(dir)
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "re {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

fn commit_file(dir: &Path, name: &str, body: &str, message: &str) {
    fs::write(dir.join(name), body).unwrap();
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", message]);
}

const CLAUDE: &str = "\n\nCo-Authored-By: Claude <noreply@anthropic.com>";

fn new_repo() -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    git(temp.path(), &["init", "-q", "-b", "main"]);
    commit_file(temp.path(), "hand.py", "hand = 1\n", "hand");
    temp
}

/// A tagged commit whose only file is excluded from the measurement: a rate
/// with no lines behind it.
fn repo_absent() -> tempfile::TempDir {
    let temp = new_repo();
    commit_file(
        temp.path(),
        "Cargo.lock",
        "# lock\n",
        &format!("lock{CLAUDE}"),
    );
    temp
}

/// One tagged line, later deleted by an untagged commit: a measured zero.
fn repo_zero() -> tempfile::TempDir {
    let temp = new_repo();
    commit_file(
        temp.path(),
        "doomed.py",
        "doomed = 1\n",
        &format!("doomed{CLAUDE}"),
    );
    git(temp.path(), &["rm", "-q", "doomed.py"]);
    git(temp.path(), &["commit", "-q", "-m", "remove doomed"]);
    temp
}

/// No tagged commit at all.
fn repo_none() -> tempfile::TempDir {
    new_repo()
}

/// Text nodes of an SVG, joined with `|`.
fn svg_text(path: &Path) -> String {
    let svg = fs::read_to_string(path).unwrap();
    let mut out = Vec::new();
    let mut rest = svg.as_str();
    while let Some(start) = rest.find("<text") {
        let after = &rest[start..];
        let Some(open_end) = after.find('>') else {
            break;
        };
        let body = &after[open_end + 1..];
        let Some(close) = body.find("</text>") else {
            break;
        };
        out.push(body[..close].trim().to_string());
        rest = &body[close + 7..];
    }
    out.join("|")
}

struct Surfaces {
    badge: String,
    card: String,
    summary: String,
}

fn surfaces(dir: &Path) -> Surfaces {
    re(dir, &["audit", "--badge", "--card"]);
    let summary = String::from_utf8(re(dir, &["audit", "--summary"]).stdout).unwrap();
    Surfaces {
        badge: svg_text(&dir.join("causari-badge.svg")),
        card: svg_text(&dir.join("causari-survival.svg")),
        summary,
    }
}

#[test]
fn absent_rate_is_absent_on_badge_card_and_summary() {
    let temp = repo_absent();
    let s = surfaces(temp.path());
    assert!(s.badge.contains("absent"), "{}", s.badge);
    assert!(
        !s.badge.contains('%'),
        "an absent rate has no percent: {}",
        s.badge
    );
    assert!(
        s.card.contains("the rate is absent, not zero"),
        "{}",
        s.card
    );
    assert!(!s.card.contains("0.0%"), "{}", s.card);
    assert!(
        s.summary.contains("AI-tagged (metadata matched): absent"),
        "{}",
        s.summary
    );
    assert!(
        !s.summary.contains("metadata matched): 0.0%") && !s.summary.contains("| 0.0% |"),
        "absent printed as zero:\n{}",
        s.summary
    );
    assert!(
        s.summary
            .contains("`absent` means no introduced lines are included"),
        "{}",
        s.summary
    );
}

#[test]
fn measured_zero_is_zero_and_never_called_absent() {
    let temp = repo_zero();
    let s = surfaces(temp.path());
    assert!(s.badge.contains("0.0%"), "{}", s.badge);
    assert!(!s.badge.contains("absent"), "{}", s.badge);
    assert!(s.card.contains("0.0% still at HEAD"), "{}", s.card);
    assert!(s.card.contains("0 of 1 lines"), "{}", s.card);
    assert!(!s.card.contains("absent"), "{}", s.card);
    assert!(
        s.summary.contains("AI-tagged (metadata matched): 0.0%"),
        "{}",
        s.summary
    );
    assert!(
        s.summary.contains("0 of 1 lines still at HEAD"),
        "{}",
        s.summary
    );
    assert!(
        !s.summary.contains("metadata matched): absent"),
        "{}",
        s.summary
    );
    // A zero over one commit is a count; the summary says so.
    assert!(
        s.summary
            .contains("Small sample: 1 AI-tagged commit (floor 5)"),
        "{}",
        s.summary
    );
    assert!(
        s.summary.contains("Age-matched gap: unavailable"),
        "{}",
        s.summary
    );
    assert!(
        s.summary.contains("This is not a measured gap of zero"),
        "{}",
        s.summary
    );
}

#[test]
fn no_tagged_commit_is_not_a_clean_bill() {
    let temp = repo_none();
    let s = surfaces(temp.path());
    assert!(s.badge.contains("absent"), "{}", s.badge);
    assert!(s.card.contains("no AI-tagged commits"), "{}", s.card);
    assert!(
        s.card.contains("nothing to measure from git metadata"),
        "{}",
        s.card
    );
    assert!(!s.card.contains("100"), "{}", s.card);
    assert!(
        s.summary.contains(
            "No metadata-matched commits. That is not a finding that the repository contains no AI-written lines."
        ),
        "{}",
        s.summary
    );
    assert!(
        !s.summary.contains("metadata matched): 0.0%"),
        "{}",
        s.summary
    );
    assert!(
        !s.summary.to_lowercase().contains("human-written"),
        "{}",
        s.summary
    );
}

#[test]
fn a_trailer_anyone_can_type_counts_as_matched_and_the_summary_says_so() {
    // Formally valid data, wrong conclusion if read as proof: a human adds the
    // Claude trailer by hand. Causari counts the commit as metadata matched,
    // names the field `verified`, and the summary says what that word means.
    let temp = new_repo();
    commit_file(
        temp.path(),
        "typed.py",
        "typed = 1\ntyped_again = 2\n",
        &format!("typed by a person{CLAUDE}"),
    );
    let v: serde_json::Value =
        serde_json::from_slice(&re(temp.path(), &["audit", "--json"]).stdout).unwrap();
    assert_eq!(v["verified"]["commits"], 1);
    assert_eq!(v["verified"]["introduced"], 2);
    assert_eq!(v["by_agent"]["claude-code"]["commits"], 1);

    let s = surfaces(temp.path());
    assert!(
        s.summary
            .contains("Metadata matched does not prove who wrote each line"),
        "{}",
        s.summary
    );
    assert!(
        s.summary.contains(
            "The JSON field is still `verified`: metadata matched, not authorship proved"
        ),
        "{}",
        s.summary
    );
    assert!(
        s.summary.contains("it is not a quality score"),
        "{}",
        s.summary
    );
    assert!(s.card.contains("a count, not a grade"), "{}", s.card);
    for word in ["healthy", "grade:", "score:", "proven", "🟢", "🔴"] {
        assert!(
            !s.summary.contains(word),
            "verdict word {word:?} in summary:\n{}",
            s.summary
        );
        assert!(
            !s.card.contains(word),
            "verdict word {word:?} on card: {}",
            s.card
        );
        assert!(
            !s.badge.contains(word),
            "verdict word {word:?} on badge: {}",
            s.badge
        );
    }
}
