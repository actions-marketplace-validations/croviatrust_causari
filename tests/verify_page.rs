//! The browser verifier at causari.dev/verify and the CLI must agree: a
//! bundle `re audit --seal` writes verifies on the page, and every
//! alteration the CLI rejects is rejected there too. The page scripts are
//! run under Node (WebCrypto, no DOM) by scripts/check_verify.mjs against a
//! chain of three seals issued here. Skipped, loudly, when Node is absent
//! or too old for WebCrypto Ed25519.
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
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

fn re(dir: &Path, args: &[&str]) {
    let out = Command::new(env!("CARGO_BIN_EXE_re"))
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "re {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn node() -> Option<PathBuf> {
    let out = Command::new("node").arg("--version").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let major: u32 = version
        .trim_start_matches('v')
        .split('.')
        .next()?
        .parse()
        .ok()?;
    // WebCrypto Ed25519 landed in Node 18.4; the check script exits 2 on
    // older runtimes, which would fail the assertion below for the wrong
    // reason.
    (major >= 20).then(|| PathBuf::from("node"))
}

#[test]
fn browser_verifier_agrees_with_the_cli() {
    let Some(node) = node() else {
        eprintln!("skipping: node >= 20 not found; the page check needs WebCrypto Ed25519");
        return;
    };

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

    // Three seals in one chain: the page must see gaps and fragments.
    re(dir, &["audit", "--seal", "--json"]);
    re(dir, &["audit", "--seal", "--json", "-o", "second.seal.json"]);
    re(dir, &["audit", "--seal", "--json", "-o", "third.seal.json"]);
    let bundle = dir.join("audit.seal.json");
    let chain = dir.join(".causari/seal/seals.jsonl");
    assert!(bundle.is_file() && chain.is_file());
    assert_eq!(fs::read_to_string(&chain).unwrap().lines().count(), 3);

    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/check_verify.mjs");
    let out = Command::new(node)
        .arg(&script)
        .arg(&bundle)
        .arg(&chain)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "check_verify.mjs exit {:?}\n{stdout}\n{stderr}",
        out.status.code()
    );
    assert!(stdout.contains("every case agrees with the CLI"), "{stdout}");
    assert!(!stdout.contains("FAIL"), "{stdout}");
}
