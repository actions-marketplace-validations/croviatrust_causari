/// `re audit` — retroactive survival count for commits whose git metadata
/// matched an AI-detection rule. Matching metadata is not proof a model
/// wrote the line. The JSON field stays `verified`.
///
/// Works on any git repository without a Causari ledger. Counts how many
/// of those lines `git blame` still attributes to the commit at HEAD.
use anyhow::{Context, Result, bail};
use colored::Colorize;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::audit::{
    AgeBucket, AuditOptions, Baseline, CAP_CEILING_LINES, CommitRow, EvidenceClass,
    IGNORE_REVS_FILE, METHOD_VERSION, ShallowCloneRefused, SurvivalReport, SurvivalStat,
    audit_repo,
};
use crate::audit_seal::{self, AuditBinding};
use crate::cli::AuditArgs;
use crate::exit::exit_with;
use crate::repo::Repo;
use crate::seal::SealIssuer;

const DEFAULT_SEAL_FILE: &str = "audit.seal.json";

/// Human label for the audit class whose JSON field is still `verified`.
const LABEL_TAGGED: &str = "AI-tagged (metadata matched)";
const LABEL_BY_AGENT: &str = "By agent (metadata matched only)";
const CARD_EMPTY: &str = "no AI-tagged commits";
const SUMMARY_FIELD_NOTE: &str =
    "The JSON field is still `verified`: metadata matched, not authorship proved.";

/// Best-effort temp-clone guard: removes the checkout when the audit is done.
struct TempClone(PathBuf);

impl Drop for TempClone {
    fn drop(&mut self) {
        // Git object files are read-only on Windows; clear attributes first.
        let _ = clear_readonly(&self.0);
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn clear_readonly(dir: &Path) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let mut perms = entry.metadata()?.permissions();
        if perms.readonly() {
            #[allow(clippy::permissions_set_readonly_false)]
            perms.set_readonly(false);
            std::fs::set_permissions(&path, perms)?;
        }
        if path.is_dir() {
            clear_readonly(&path)?;
        }
    }
    Ok(())
}

/// Resolve the audit target: local path (default `.`), git URL, or GitHub
/// `owner/repo` shorthand. Remote targets are cloned into a temp directory
/// that is removed when the audit finishes.
fn resolve_target(target: Option<&str>) -> Result<(PathBuf, Option<TempClone>)> {
    let Some(raw) = target else {
        let cwd = std::env::current_dir().context("cannot determine current directory")?;
        return Ok((cwd, None));
    };

    let as_path = Path::new(raw);
    if as_path.exists() {
        return Ok((as_path.to_path_buf(), None));
    }

    let url =
        if raw.starts_with("http://") || raw.starts_with("https://") || raw.starts_with("git@") {
            raw.to_string()
        } else if raw.split('/').count() == 2 && !raw.contains(char::is_whitespace) {
            // GitHub shorthand: owner/repo
            format!("https://github.com/{raw}")
        } else {
            bail!("'{raw}' is neither an existing path, a git URL, nor an owner/repo shorthand");
        };

    let dest = std::env::temp_dir().join(format!("causari-audit-{}", std::process::id()));
    if dest.exists() {
        // A crashed previous run can leave a stale checkout behind.
        let _ = clear_readonly(&dest);
        std::fs::remove_dir_all(&dest)
            .with_context(|| format!("cannot clear stale clone dir {}", dest.display()))?;
    }
    eprintln!("cloning {url} ...");
    let status = Command::new("git")
        .args(["clone", "--quiet", "--single-branch", &url])
        .arg(&dest)
        .status()
        .context("failed to run git clone")?;
    if !status.success() {
        bail!("git clone failed for {url}");
    }
    // `git clone` does not fetch notes; git-ai authorship logs live under
    // refs/notes/ai. Best effort: most repositories simply do not have it.
    let _ = Command::new("git")
        .args(["fetch", "--quiet", "origin", "+refs/notes/ai:refs/notes/ai"])
        .current_dir(&dest)
        .stderr(std::process::Stdio::null())
        .status();
    Ok((dest.clone(), Some(TempClone(dest))))
}

/// The machine-readable report: every class and agent carries the sums, the
/// line-weighted rate and the robust figures; `coverage` says how it was
/// measured and `repository` names what was measured — the commit at HEAD
/// and the origin label (credentials stripped, or a digest of the path when
/// there is no remote). The same `repository.head` is the same commit, not
/// a promise that a later audit matches: classification also reads
/// `refs/notes/ai`, which is not part of the commit, plus the method and
/// the blame flags in `coverage`. A repository that has moved has a
/// different head.
fn report_json(dir: &Path, report: &SurvivalReport) -> Result<serde_json::Value> {
    let mut value = serde_json::to_value(report)?;
    value["method"] = serde_json::json!(METHOD_VERSION);
    let roots = crate::audit::root_commits(dir)?;
    let mut repo_obj = serde_json::json!({
        "head": crate::audit::head_commit(dir)?,
        "origin": audit_seal::repo_label(dir),
        "root_commits": roots,
    });
    if let Some(root) = roots.first() {
        repo_obj["root_commit"] = serde_json::json!(root);
    }
    value["repository"] = repo_obj;
    Ok(value)
}

/// The exact bytes `--json` prints: pretty JSON and one newline. An audit
/// seal commits to these bytes, so they are produced in one place.
fn audit_json_bytes(dir: &Path, report: &SurvivalReport) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(&report_json(dir, report)?)?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// The repository whose seal issuer signs an audit. A local audit signs
/// with the audited repository's own identity, so audit seals and exchange
/// seals of one project form one chain; `.causari/` is created there on
/// first use, as `re init` would. A temp clone has no identity of its own:
/// the current directory's repository signs.
fn issuer_repo(dir: &Path, is_temp_clone: bool) -> Result<Repo> {
    if !is_temp_clone {
        if let Ok(repo) = Repo::discover_from(dir) {
            return Ok(repo);
        }
    }
    if let Ok(repo) = Repo::discover() {
        return Ok(repo);
    }
    if is_temp_clone {
        bail!(
            "--seal needs an issuer identity: run `re init` in the directory that should sign \
             (its .causari/keys/seal-issuer.key and seal chain are used), then audit again"
        );
    }
    let repo = Repo::init(dir)?;
    let _ = repo.ensure_gitignored();
    eprintln!(
        "created {} for the seal issuer key and chain (gitignored)",
        repo.dir.display()
    );
    Ok(repo)
}

/// Issue the seal over `audit_json` and write the bundle to `out`.
fn seal_audit(
    dir: &Path,
    is_temp_clone: bool,
    args: &AuditArgs,
    report: &SurvivalReport,
    audit_json: &[u8],
) -> Result<(PathBuf, serde_json::Value)> {
    let binding = AuditBinding {
        commit: crate::audit::head_commit(dir)?,
        method: report.coverage.method.to_string(),
        allow_shallow: args.allow_shallow,
        shallow: report.coverage.shallow,
        repo: audit_seal::repo_label(dir),
        notes_oid: Some(
            report
                .coverage
                .notes_oid
                .clone()
                .unwrap_or_else(|| "none".to_string()),
        ),
        ignore_revs_sha256: Some(
            report
                .coverage
                .ignore_revs_sha256
                .clone()
                .unwrap_or_else(|| "none".to_string()),
        ),
    };
    let repo = issuer_repo(dir, is_temp_clone)?;
    let mut issuer = SealIssuer::load_or_create(&repo, None)?;
    let bundle = audit_seal::issue(&mut issuer, audit_json, &binding)?;
    let out = args
        .output
        .clone()
        .unwrap_or_else(|| PathBuf::from(DEFAULT_SEAL_FILE));
    let mut text = serde_json::to_string_pretty(&bundle)?;
    text.push('\n');
    std::fs::write(&out, text).with_context(|| format!("writing {}", out.display()))?;
    Ok((out, bundle))
}

pub fn run(args: AuditArgs) -> Result<()> {
    if args.diff.len() == 2 {
        return run_diff(&args.diff[0], &args.diff[1], args.json, args.summary);
    }

    let (dir, tmp_clone) = resolve_target(args.target.as_deref())?;
    let opts = AuditOptions {
        allow_shallow: args.allow_shallow,
    };
    let report = audit_repo(&dir, &opts).map_err(|e| {
        if e.is::<ShallowCloneRefused>() {
            exit_with(2, e)
        } else {
            e.context("audit failed")
        }
    })?;

    let audit_json = if args.json || args.seal {
        Some(audit_json_bytes(&dir, &report)?)
    } else {
        None
    };
    // Seal before printing: a failed issuance must not leave a report on
    // stdout that looks sealed.
    let sealed = if args.seal {
        let bytes = audit_json.as_deref().unwrap_or_default();
        Some(seal_audit(
            &dir,
            tmp_clone.is_some(),
            &args,
            &report,
            bytes,
        )?)
    } else {
        None
    };

    if args.json {
        let mut stdout = std::io::stdout().lock();
        stdout.write_all(audit_json.as_deref().unwrap_or_default())?;
        stdout.flush()?;
        if let Some((out, bundle)) = &sealed {
            // stdout carries exactly the sealed bytes; the notice goes elsewhere.
            eprintln!(
                "seal {} (sequence {}) written to {}",
                bundle["seal"]["seal_id"].as_str().unwrap_or("?"),
                bundle["seal"]["chain"]["sequence"],
                out.display()
            );
        }
        return Ok(());
    }

    let head = crate::audit::head_commit(&dir).ok();
    if args.summary {
        print_summary(&report, head.as_deref());
    } else {
        print_terminal(&report, head.as_deref());
        // The ledger is a different store. Say so once, only for a working
        // tree that does not already have one, so `re report` is not the
        // next step after this audit.
        if tmp_clone.is_none() && !dir.join(".causari").is_dir() {
            println!();
            println!(
                "  {} this audit did not use a ledger. `re report` reads a local ledger and fails in this directory.",
                "ledger:".bright_black()
            );
            println!("         `re init` creates that ledger. It does not export the audit above.");
        }
    }

    if let Some((out, bundle)) = &sealed {
        let seal = &bundle["seal"];
        println!();
        if args.summary {
            // The summary is Markdown for a PR comment or job summary; the
            // seal note is one paragraph of it.
            println!(
                "Sealed: `{}` (crovia.seal.v1) over this audit of `{}`, written to `{}`. \
                 Verify offline with `re seal verify {}` or at https://causari.dev/verify — \
                 the seal proves these exact numbers were signed for this commit, not that they are true.",
                seal["seal_id"].as_str().unwrap_or("?"),
                seal["generator"]["params"]["commit"]
                    .as_str()
                    .map(|c| &c[..c.len().min(12)])
                    .unwrap_or("?"),
                out.display(),
                out.display()
            );
        } else {
            println!(
                "{} seal {} written to {}",
                "✓".green().bold(),
                seal["seal_id"].as_str().unwrap_or("?").cyan(),
                out.display()
            );
            println!(
                "  issuer   {}  (sequence {})",
                seal["issuer"]["id"].as_str().unwrap_or("?"),
                seal["chain"]["sequence"]
            );
            println!(
                "  commit   {}  method {}",
                seal["generator"]["params"]["commit"]
                    .as_str()
                    .unwrap_or("?"),
                seal["generator"]["params"]["method"]
                    .as_str()
                    .unwrap_or("?")
            );
            println!(
                "  verify   {} — or drop the file on https://causari.dev/verify",
                format!("re seal verify {}", out.display()).cyan()
            );
        }
    }

    if args.badge {
        let svg = generate_badge(&report);
        let path = Path::new("causari-badge.svg");
        std::fs::write(path, svg).with_context(|| format!("writing {}", path.display()))?;
        println!(
            "{} badge written to {} — embed it in your README:",
            "✓".green().bold(),
            path.display()
        );
        println!("    ![AI survival](./causari-badge.svg)");
    }

    if args.card {
        let svg = generate_svg_card(&report);
        let path = Path::new("causari-survival.svg");
        std::fs::write(path, svg).with_context(|| format!("writing {}", path.display()))?;
        println!(
            "{} survival card written to {}",
            "✓".green().bold(),
            path.display()
        );
    }

    if args.save {
        let mut snapshot = report_json(&dir, &report)?;
        snapshot["timestamp"] = serde_json::json!(chrono::Utc::now().to_rfc3339());
        let path = Path::new(".causari/survival-snapshots.jsonl");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .with_context(|| format!("opening {}", path.display()))?;
        writeln!(file, "{}", serde_json::to_string(&snapshot)?)
            .with_context(|| format!("writing {}", path.display()))?;
        println!(
            "{} snapshot saved to {}",
            "✓".green().bold(),
            path.display()
        );
    }

    Ok(())
}

struct LoadedReceipt {
    audit: serde_json::Value,
    sealed: bool,
    seal_id: Option<String>,
    issuer_id: Option<String>,
    pubkey_hex: Option<String>,
}

/// Load a receipt from a plain `re audit --json` file or from a signed
/// `re audit --seal` bundle (which is verified first).
fn load_receipt(path: &Path) -> Result<LoadedReceipt> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read receipt {}", path.display()))?;
    let val: serde_json::Value = serde_json::from_str(&raw)
        .with_context(|| format!("{} is not valid JSON", path.display()))?;
    if audit_seal::is_bundle(&val) {
        let verified = audit_seal::verify_bundle(&val)
            .with_context(|| format!("invalid audit seal bundle {}", path.display()))?;
        Ok(LoadedReceipt {
            audit: verified.audit,
            sealed: true,
            seal_id: Some(verified.seal_id),
            issuer_id: Some(verified.issuer_id),
            pubkey_hex: Some(verified.pubkey_hex),
        })
    } else {
        Ok(LoadedReceipt {
            audit: val,
            sealed: false,
            seal_id: None,
            issuer_id: None,
            pubkey_hex: None,
        })
    }
}

fn extract_root_commits(receipt: &serde_json::Value) -> Vec<String> {
    if let Some(arr) = receipt["repository"]["root_commits"].as_array() {
        let mut out: Vec<String> = arr
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
        out.sort();
        out.dedup();
        if !out.is_empty() {
            return out;
        }
    }
    if let Some(r) = receipt["repository"]["root_commit"].as_str() {
        return vec![r.to_string()];
    }
    Vec::new()
}

/// Deterministic Evidence Diff between two receipts (`re audit --diff A B`).
///
/// Enforces the single-axis rule:
/// - **Repository diff** (`repository_diff`): same `method`, `blame_flags`,
///   and `numstat_flags`, different `HEAD` on a repository that shares at
///   least one root commit.
/// - **Method diff** (`method_diff`): same `HEAD` and same external metadata,
///   different `method` or blame/numstat flags.
/// - **Metadata diff** (`metadata_diff`): same `HEAD` and same `method`/flags,
///   different `notes_oid` (`refs/notes/ai`) or `ignore_revs_sha256`.
/// - **Identical inputs** (`identical_inputs`): same `HEAD`, `method`, flags,
///   and metadata.
/// - **Refused** (exit 2): more than one axis varies at once, root commit
///   sets are disjoint (unrelated repositories), or either receipt is shallow.
fn run_diff(before_path: &Path, after_path: &Path, as_json: bool, summary: bool) -> Result<()> {
    let before_loaded = load_receipt(before_path).map_err(|e| exit_with(2, e))?;
    let after_loaded = load_receipt(after_path).map_err(|e| exit_with(2, e))?;
    let before = &before_loaded.audit;
    let after = &after_loaded.audit;

    let b_head = before["repository"]["head"].as_str().ok_or_else(|| {
        exit_with(
            2,
            anyhow::anyhow!("{} misses repository.head", before_path.display()),
        )
    })?;
    let a_head = after["repository"]["head"].as_str().ok_or_else(|| {
        exit_with(
            2,
            anyhow::anyhow!("{} misses repository.head", after_path.display()),
        )
    })?;
    let b_method = before["coverage"]["method"]
        .as_str()
        .or_else(|| before["method"].as_str())
        .ok_or_else(|| {
            exit_with(
                2,
                anyhow::anyhow!("{} misses coverage.method", before_path.display()),
            )
        })?;
    let a_method = after["coverage"]["method"]
        .as_str()
        .or_else(|| after["method"].as_str())
        .ok_or_else(|| {
            exit_with(
                2,
                anyhow::anyhow!("{} misses coverage.method", after_path.display()),
            )
        })?;

    if before["coverage"]["shallow"].as_bool() == Some(true)
        || after["coverage"]["shallow"].as_bool() == Some(true)
    {
        return Err(exit_with(
            2,
            anyhow::anyhow!(
                "refusing to diff shallow receipts: truncated history makes commit sets incomparable"
            ),
        ));
    }

    let b_roots = extract_root_commits(before);
    let a_roots = extract_root_commits(after);
    let shared_roots: Vec<String> = b_roots
        .iter()
        .filter(|r| a_roots.contains(r))
        .cloned()
        .collect();
    if !b_roots.is_empty() && !a_roots.is_empty() && shared_roots.is_empty() {
        return Err(exit_with(
            2,
            anyhow::anyhow!(
                "refusing to diff unrelated repositories: root_commits {:?} and {:?} share no common root",
                b_roots,
                a_roots
            ),
        ));
    }

    let b_flags = &before["coverage"]["blame_flags"];
    let a_flags = &after["coverage"]["blame_flags"];
    let b_nflags = &before["coverage"]["numstat_flags"];
    let a_nflags = &after["coverage"]["numstat_flags"];
    let nflags_changed = if b_method == "v5" && a_method == "v5" {
        b_nflags != a_nflags
    } else {
        !b_nflags.is_null() && !a_nflags.is_null() && b_nflags != a_nflags
    };
    let method_or_flags_changed = b_method != a_method || b_flags != a_flags || nflags_changed;

    let b_notes = &before["coverage"]["notes_oid"];
    let a_notes = &after["coverage"]["notes_oid"];
    let b_ir_sha = &before["coverage"]["ignore_revs_sha256"];
    let a_ir_sha = &after["coverage"]["ignore_revs_sha256"];
    let b_ir_flag = before["coverage"]["ignore_revs_file"].as_bool();
    let a_ir_flag = after["coverage"]["ignore_revs_file"].as_bool();
    let notes_present_both = before["coverage"].get("notes_oid").is_some()
        && after["coverage"].get("notes_oid").is_some();
    let ir_sha_present_both = before["coverage"].get("ignore_revs_sha256").is_some()
        && after["coverage"].get("ignore_revs_sha256").is_some();
    let metadata_changed = (notes_present_both && b_notes != a_notes)
        || (ir_sha_present_both && b_ir_sha != a_ir_sha)
        || (b_ir_flag != a_ir_flag);
    let metadata_fully_determined = notes_present_both
        && (ir_sha_present_both || (b_ir_flag == Some(false) && a_ir_flag == Some(false)));
    let metadata_status = if metadata_changed {
        "changed"
    } else if metadata_fully_determined {
        "unchanged"
    } else {
        "indeterminate"
    };

    if b_head != a_head && method_or_flags_changed {
        return Err(exit_with(
            2,
            anyhow::anyhow!(
                "refusing to diff across two axes at once: HEAD changed ({} -> {}) AND method/flags changed ({b_method} -> {a_method}). \
                 Re-audit both commits under the same method for a repository diff, or audit the same commit under both methods for a method diff.",
                &b_head[..b_head.len().min(12)],
                &a_head[..a_head.len().min(12)]
            ),
        ));
    }
    if method_or_flags_changed && metadata_changed {
        return Err(exit_with(
            2,
            anyhow::anyhow!(
                "refusing to diff across two axes at once: method/flags changed ({b_method} -> {a_method}) AND external metadata (notes_oid / ignore_revs) changed on the same commit."
            ),
        ));
    }

    let kind = if b_head != a_head {
        "repository_diff"
    } else if method_or_flags_changed {
        if metadata_status == "unchanged" {
            "method_diff"
        } else {
            "indeterminate_method_diff"
        }
    } else if metadata_changed {
        "metadata_diff"
    } else if metadata_status == "indeterminate" {
        "indeterminate_metadata"
    } else {
        "identical_inputs"
    };

    let provenance_status = match (before_loaded.sealed, after_loaded.sealed) {
        (true, true) => {
            if before_loaded.pubkey_hex == after_loaded.pubkey_hex {
                "sealed_same_issuer"
            } else {
                "sealed_different_issuers"
            }
        }
        (true, false) | (false, true) => "partially_sealed",
        (false, false) => "unsealed",
    };

    let b_commits: Vec<CommitRow> = before
        .get("commits")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|e| {
            exit_with(
                2,
                anyhow::anyhow!("invalid commits[] in {}: {e}", before_path.display()),
            )
        })?
        .unwrap_or_default();
    let a_commits: Vec<CommitRow> = after
        .get("commits")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|e| {
            exit_with(
                2,
                anyhow::anyhow!("invalid commits[] in {}: {e}", after_path.display()),
            )
        })?
        .unwrap_or_default();
    let has_commit_rows = before.get("commits").is_some() && after.get("commits").is_some();

    let mut b_map: std::collections::BTreeMap<&str, &CommitRow> = std::collections::BTreeMap::new();
    for c in &b_commits {
        b_map.insert(c.hash.as_str(), c);
    }
    let mut a_map: std::collections::BTreeMap<&str, &CommitRow> = std::collections::BTreeMap::new();
    for c in &a_commits {
        a_map.insert(c.hash.as_str(), c);
    }

    let commit_set_relation = if b_head == a_head {
        "same_head"
    } else if !has_commit_rows {
        "unknown"
    } else {
        let b_subset_of_a = !b_map.is_empty() && b_map.keys().all(|k| a_map.contains_key(k));
        let a_subset_of_b = !a_map.is_empty() && a_map.keys().all(|k| b_map.contains_key(k));
        match (b_subset_of_a, a_subset_of_b) {
            (true, false) => "subset_forward",
            (false, true) => "subset_reverse",
            (true, true) => "same_commit_set",
            (false, false) => "diverged",
        }
    };

    let mut added_verified = Vec::new();
    let mut removed_verified = Vec::new();
    let mut survival_changed = Vec::new();
    let mut class_changed = Vec::new();

    for c in &a_commits {
        match b_map.get(c.hash.as_str()) {
            None => {
                if c.class == EvidenceClass::Verified {
                    added_verified.push(serde_json::json!({
                        "hash": c.hash,
                        "agent": c.agent,
                        "introduced": c.introduced,
                        "surviving": c.surviving,
                    }));
                }
            }
            Some(prev) => {
                if prev.class != c.class || prev.agent != c.agent {
                    class_changed.push(serde_json::json!({
                        "hash": c.hash,
                        "before_class": prev.class,
                        "after_class": c.class,
                        "before_agent": prev.agent,
                        "after_agent": c.agent,
                        "introduced": c.introduced,
                        "surviving": c.surviving,
                    }));
                }
                if (prev.class == EvidenceClass::Verified || c.class == EvidenceClass::Verified)
                    && prev.surviving != c.surviving
                {
                    survival_changed.push(serde_json::json!({
                        "hash": c.hash,
                        "agent": c.agent.as_ref().or(prev.agent.as_ref()),
                        "introduced": c.introduced,
                        "before_surviving": prev.surviving,
                        "after_surviving": c.surviving,
                        "delta_surviving": c.surviving as i64 - prev.surviving as i64,
                    }));
                }
            }
        }
    }
    for prev in &b_commits {
        if !a_map.contains_key(prev.hash.as_str()) && prev.class == EvidenceClass::Verified {
            removed_verified.push(serde_json::json!({
                "hash": prev.hash,
                "agent": prev.agent,
                "introduced": prev.introduced,
                "before_surviving": prev.surviving,
            }));
        }
    }

    let b_v = &before["verified"];
    let a_v = &after["verified"];
    let b_rate = b_v["survival_rate"].as_f64();
    let a_rate = a_v["survival_rate"].as_f64();
    let delta_rate = match (b_rate, a_rate) {
        (Some(br), Some(ar)) => Some(ar - br),
        _ => None,
    };

    let diff_value = serde_json::json!({
        "schema": "causari.audit_diff.v1",
        "kind": kind,
        "metadata_status": metadata_status,
        "commit_set_relation": commit_set_relation,
        "shared_root_commits": shared_roots,
        "provenance": {
            "status": provenance_status,
            "before_sealed": before_loaded.sealed,
            "after_sealed": after_loaded.sealed,
            "before_signature_valid": before_loaded.sealed,
            "after_signature_valid": after_loaded.sealed,
            "before_issuer_id": before_loaded.issuer_id,
            "after_issuer_id": after_loaded.issuer_id,
            "before_pubkey_hex": before_loaded.pubkey_hex,
            "after_pubkey_hex": after_loaded.pubkey_hex,
            "before_seal_id": before_loaded.seal_id,
            "after_seal_id": after_loaded.seal_id,
            "issuer_trust": "unverified_offline",
        },
        "before": {
            "head": b_head,
            "method": b_method,
            "root_commit": b_roots.first(),
            "root_commits": b_roots,
            "sealed": before_loaded.sealed,
            "notes_oid": b_notes,
            "ignore_revs_sha256": b_ir_sha,
            "verified": {
                "commits": b_v["commits"],
                "introduced": b_v["introduced"],
                "surviving": b_v["surviving"],
                "survival_rate": b_rate,
            }
        },
        "after": {
            "head": a_head,
            "method": a_method,
            "root_commit": a_roots.first(),
            "root_commits": a_roots,
            "sealed": after_loaded.sealed,
            "notes_oid": a_notes,
            "ignore_revs_sha256": a_ir_sha,
            "verified": {
                "commits": a_v["commits"],
                "introduced": a_v["introduced"],
                "surviving": a_v["surviving"],
                "survival_rate": a_rate,
            }
        },
        "delta": {
            "verified_commits": a_v["commits"].as_i64().unwrap_or(0) - b_v["commits"].as_i64().unwrap_or(0),
            "verified_introduced": a_v["introduced"].as_i64().unwrap_or(0) - b_v["introduced"].as_i64().unwrap_or(0),
            "verified_surviving": a_v["surviving"].as_i64().unwrap_or(0) - b_v["surviving"].as_i64().unwrap_or(0),
            "survival_rate": delta_rate,
        },
        "commit_sets_available": has_commit_rows,
        "added_verified_commits": added_verified,
        "removed_verified_commits": removed_verified,
        "survival_changed_commits": survival_changed,
        "class_changed_commits": class_changed,
    });

    if as_json {
        println!("{}", serde_json::to_string_pretty(&diff_value)?);
        return Ok(());
    }

    let rate_delta_str = match delta_rate {
        Some(d) => format!("{:+.1} percentage points", d * 100.0),
        None => "absent on at least one side".to_string(),
    };
    if summary {
        println!("## ∵ causari · evidence diff ({kind})");
        println!();
        println!(
            "- **Provenance**: `{provenance_status}` (issuer trust: `unverified_offline`) · **Metadata status**: `{metadata_status}` · **Commit-set relation**: `{commit_set_relation}`"
        );
        println!(
            "- **Before**: `{}` (method `{}`, {} AI-tagged commits, {}/{} lines, {})",
            &b_head[..b_head.len().min(12)],
            b_method,
            b_v["commits"],
            b_v["surviving"],
            b_v["introduced"],
            pct(b_rate)
        );
        println!(
            "- **After**: `{}` (method `{}`, {} AI-tagged commits, {}/{} lines, {})",
            &a_head[..a_head.len().min(12)],
            a_method,
            a_v["commits"],
            a_v["surviving"],
            a_v["introduced"],
            pct(a_rate)
        );
        println!(
            "- **Delta**: {:+} commits, {:+} introduced, {:+} surviving ({rate_delta_str})",
            diff_value["delta"]["verified_commits"]
                .as_i64()
                .unwrap_or(0),
            diff_value["delta"]["verified_introduced"]
                .as_i64()
                .unwrap_or(0),
            diff_value["delta"]["verified_surviving"]
                .as_i64()
                .unwrap_or(0),
        );
        if has_commit_rows {
            println!(
                "- **Commit sets** (set difference B \\ A and A \\ B): {} in after only, {} in before only, {} with survival change, {} reclassified.",
                added_verified.len(),
                removed_verified.len(),
                survival_changed.len(),
                class_changed.len(),
            );
        }
        return Ok(());
    }

    println!(
        "{} ({} · {} · {})",
        "∵ causari · evidence diff".bold(),
        kind.cyan(),
        provenance_status,
        commit_set_relation
    );
    println!(
        "  before  {}  method {}  ({}/{} lines, {})",
        &b_head[..b_head.len().min(12)],
        b_method,
        b_v["surviving"],
        b_v["introduced"],
        pct(b_rate)
    );
    println!(
        "  after   {}  method {}  ({}/{} lines, {})",
        &a_head[..a_head.len().min(12)],
        a_method,
        a_v["surviving"],
        a_v["introduced"],
        pct(a_rate)
    );
    println!(
        "  delta   {:+} commits · {:+} introduced · {:+} surviving ({rate_delta_str})",
        diff_value["delta"]["verified_commits"]
            .as_i64()
            .unwrap_or(0),
        diff_value["delta"]["verified_introduced"]
            .as_i64()
            .unwrap_or(0),
        diff_value["delta"]["verified_surviving"]
            .as_i64()
            .unwrap_or(0),
    );
    if has_commit_rows {
        println!(
            "  sets    {} in after only · {} in before only · {} survival changed · {} reclassified",
            added_verified.len(),
            removed_verified.len(),
            survival_changed.len(),
            class_changed.len(),
        );
    }
    Ok(())
}

/// Under the first percentage: what the rate is, and what a missing tag is not.
fn print_survival_reading(report: &SurvivalReport) {
    if report.verified.commits == 0 {
        println!(
            "  No metadata-matched commits. That is not a finding that the repository \
             contains no AI-written lines. UNKNOWN and untagged are commits with no such \
             signal; they are not a finding that a human wrote them."
        );
        return;
    }
    if report.verified.introduced == 0 {
        println!("  {}", absent_rate_sentence());
        return;
    }
    println!("  {}", survival_reading(report));
    if report.verified.survival_rate() == Some(0.0) {
        println!("  {}", measured_zero_sentence());
    }
}

fn absent_rate_sentence() -> &'static str {
    "Rate absent: no introduced lines are included in this measurement (generated paths, \
     lockfiles and binaries are excluded). That is not a finding that the commits introduced \
     no lines. The JSON value null is not a measured zero."
}

fn measured_zero_sentence() -> &'static str {
    "Measured zero: lines were introduced and none survive at HEAD. This is 0.0%, not an absent rate."
}

fn survival_reading(report: &SurvivalReport) -> String {
    format!(
        "Of the lines introduced by metadata-matched commits, this percentage is the share \
         `git blame {}` still attributes to those commits. It is not the share of the \
         repository and it is not a quality score. Metadata matched does not prove who \
         wrote each line. UNKNOWN and untagged are commits with no such signal; they are \
         not a finding that a human wrote them.",
        report.coverage.blame_flags.join(" ")
    )
}

/// `gap` is tagged line-weighted survival minus untagged line-weighted survival,
/// in rate units. Printed in percentage points, with the direction named.
fn age_matched_sentence(b: &Baseline, floor: u64) -> String {
    match &b.age_matched {
        Some(m) => format!(
            "Age-matched gap: {:+.1} percentage points (AI-tagged {} minus untagged {} in the matched age windows). \
             Positive means the metadata-matched lines have the higher line-weighted survival in those windows, \
             over {} matched age window{} holding {:.0}% of the AI-tagged lines. \
             Lines outside those windows are not in this gap.",
            m.gap * 100.0,
            pct(Some(m.tagged_rate)),
            pct(Some(m.untagged_rate)),
            m.buckets_used,
            plural(m.buckets_used as u64),
            m.tagged_lines_covered * 100.0,
        ),
        None => {
            let tagged_introduced: u64 =
                b.by_age.iter().map(|bucket| bucket.tagged.introduced).sum();
            if tagged_introduced == 0 {
                "Age-matched gap: unavailable. No AI-tagged lines are included in the measurement, \
                 so there is no age comparison. This is not a finding that those commits introduced \
                 no lines, and it is not a measured gap of zero."
                    .into()
            } else {
                format!(
                    "Age-matched gap: unavailable. No age window holds at least {floor} commits \
                     of both kinds with introduced lines, so there is no comparison across \
                     matched age windows. This is not a measured gap of zero."
                )
            }
        }
    }
}

fn below_floor_sentence(commits: u64, floor: u64) -> String {
    format!(
        "below floor: {commits} commit{} (floor {floor}); not comparable. \
         Meeting the floor is not a reliability guarantee.",
        plural(commits),
    )
}

fn print_export_footer(report: &SurvivalReport, head: Option<&str>, markdown: bool) {
    let flags = report.coverage.blame_flags.join(" ");
    let head_text = match head {
        Some(h) => format!("commit `{h}`"),
        None => "the commit in `repository.head`".to_string(),
    };
    if markdown {
        println!();
        println!(
            "Read the age-matched gap as the comparison. An agent line marked below floor is not \
             comparable. Meeting the floor is not a reliability guarantee."
        );
        println!();
        println!(
            "Export: `re audit <target> --json` (the counts and {head_text}), `--summary` for this \
             Markdown, `--seal` to sign those JSON bytes for that commit and method {}. \
             `--badge` and `--card` write SVGs. `re report` is the local ledger, not this audit.",
            report.coverage.method
        );
        println!();
        println!(
            "The export stores the counts, {head_text}, method {}, blame `{flags}`, and the coverage flags. \
             It does not store `refs/notes/ai`. That ref is not part of the commit; adding or removing \
             it can change the counts while the stored SHA stays the same. A repository that has moved \
             has a different commit, and this export does not reproduce it.",
            report.coverage.method
        );
        return;
    }
    println!();
    println!("{}", "Reading and export".bright_black().bold());
    println!(
        "  · The age-matched gap above is the comparison. An agent row marked below floor is not comparable."
    );
    println!("    Meeting the floor is not a reliability guarantee.");
    println!(
        "  · Export: `re audit <target> --json` (counts and {head_text}), `--summary` for Markdown,"
    );
    println!(
        "    `--seal` signs those JSON bytes for that commit and method {}. `--badge` and `--card` write SVGs.",
        report.coverage.method
    );
    println!("    `re report` reads the local ledger. It does not export this audit.");
    println!(
        "  · Stored in `--json` and in `--seal`: the counts, {head_text}, method {}, blame `{flags}`,",
        report.coverage.method
    );
    println!(
        "    plus coverage flags (shallow, ignore-revs, sample floor). `refs/notes/ai` is not stored."
    );
    println!(
        "    That ref is not part of the commit. Adding or removing it can change the counts while the SHA stays."
    );
    println!(
        "    A repository that has moved has a different commit, and this export does not reproduce it."
    );
}

fn print_terminal(report: &SurvivalReport, head: Option<&str>) {
    println!("{}", "∵ causari · AI code survival".bold());
    println!(
        "{}",
        "───────────────────────────────────────────────────".bright_black()
    );
    println!(
        "  {} commits analyzed (git metadata only, no setup required)",
        report.total_commits
    );
    println!();

    print_class(LABEL_TAGGED, &report.verified, false);
    print_survival_reading(report);
    println!(
        "  {}",
        age_matched_sentence(&report.baseline, report.coverage.sample_floor)
    );
    if let Some(sentence) = dominance_sentence(&report.verified) {
        println!("  {sentence}");
    }
    print_class("Probable AI-assisted", &report.probable, true);

    if !report.by_agent.is_empty() {
        println!("{}", LABEL_BY_AGENT.bold());
        println!(
            "  {:20} {:>7} {:>10} {:>9} {:>8} {:>8} {:>8}",
            "agent", "commits", "introduced", "survived", "line-wt", "capped", "median"
        );
        for (agent, stat) in &report.by_agent {
            println!(
                "  {:20} {:>7} {:>10} {:>9} {:>8} {:>8} {:>8}",
                agent.cyan(),
                stat.commits,
                stat.introduced,
                stat.surviving,
                pct(stat.survival_rate()),
                pct(stat.capped_survival_rate()),
                pct(stat.median_survival()),
            );
            if let Some(sentence) = dominance_sentence(stat) {
                println!("    {sentence}");
            }
            if stat.commits < report.coverage.sample_floor {
                println!(
                    "    {}",
                    below_floor_sentence(stat.commits, report.coverage.sample_floor)
                );
            }
        }
    }

    print_baseline(&report.baseline, report.coverage.sample_floor);

    println!();
    println!("{}", "Confidence notes".bright_black().bold());
    println!(
        "  · JSON field `verified` = metadata matched (trailers, bot author, …), not authorship proved"
    );
    println!("  · PROBABLE = weak heuristic; may include human-assisted commits");
    println!("  · UNKNOWN commits are excluded from headline numbers; they form");
    println!("    the untagged baseline (human, inline-completed and untagged-agent code alike)");
    println!(
        "  · absent = no introduced lines included in the measurement (generated paths, lockfiles \
         and binaries are excluded), JSON null. Not a measured zero, and not a claim that the \
         commits introduced nothing. 0.0% = included lines of which none survive. An age-matched \
         gap marked unavailable is not a gap of zero."
    );
    println!("  · line-wt = Σ surviving / Σ introduced; capped = same, with each commit");
    println!(
        "    weighing at most min(p95 of per-commit introduced lines, {} lines);",
        CAP_CEILING_LINES
    );
    println!("    median = median of per-commit rates");
    if report.coverage.small_sample {
        println!(
            "  · Small sample: {} verified commit{} (floor {}). Read the figures as counts.",
            report.verified.commits,
            plural(report.verified.commits),
            report.coverage.sample_floor
        );
    }
    println!("  · Only lines from AI-tagged commits are measured; inline completions");
    println!("    (Copilot, Cursor Tab, …) leave no git trace and are invisible here");
    println!(
        "  · blame {}{}",
        report.coverage.blame_flags.join(" "),
        if report.coverage.ignore_revs_file {
            format!(" --ignore-revs-file={IGNORE_REVS_FILE}")
        } else {
            String::new()
        }
    );
    if report.coverage.shallow {
        println!("  · Shallow clone: history is truncated, the figures above are partial");
    }
    println!(
        "  · A measurement, not a grade: method {} at https://causari.dev/method",
        report.coverage.method
    );
    print_export_footer(report, head, false);
}

fn print_summary(report: &SurvivalReport, head: Option<&str>) {
    let v = &report.verified;

    // A measurement, not a grade: no colour, no verdict. The reader judges.
    println!("## ∵ causari · AI code survival");
    println!();
    println!(
        "{} commits analyzed (git metadata only, retroactive, no setup).",
        report.total_commits
    );
    println!();
    if report.coverage.shallow {
        println!(
            "_Shallow clone: history is truncated and these figures are partial. \
             Use `fetch-depth: 0` or `git fetch --unshallow` for a full measurement._"
        );
        println!();
    }

    if v.commits > 0 {
        println!("{}", tagged_summary_bold(v));
        println!();
        if v.introduced == 0 {
            println!("{}", absent_rate_sentence());
        } else {
            println!("{}", survival_reading(report));
            if v.survival_rate() == Some(0.0) {
                println!();
                println!("{}", measured_zero_sentence());
            }
        }
        println!();
        println!(
            "{}",
            age_matched_sentence(&report.baseline, report.coverage.sample_floor)
        );
        if let Some(sentence) = dominance_sentence(v) {
            println!();
            println!("_{sentence}._");
        }
        if report.coverage.small_sample {
            println!();
            println!(
                "_Small sample: {} AI-tagged commit{} (floor {}). Read the figures as counts, not rates. Meeting the floor is not a reliability guarantee._",
                v.commits,
                plural(v.commits),
                report.coverage.sample_floor
            );
        }
        println!();
        summary_baseline(&report.baseline);
    } else {
        println!(
            "No metadata-matched commits. That is not a finding that the repository contains no AI-written lines."
        );
        println!();
        println!(
            "{}",
            age_matched_sentence(&report.baseline, report.coverage.sample_floor)
        );
        println!();
    }
    if report.probable.commits > 0 {
        println!(
            "Probable AI-assisted: {} commits, {} introduced, {} survived ({} line-weighted · {} capped · median {}).",
            report.probable.commits,
            report.probable.introduced,
            report.probable.surviving,
            pct(report.probable.survival_rate()),
            pct(report.probable.capped_survival_rate()),
            pct(report.probable.median_survival()),
        );
        println!();
    }

    if !report.by_agent.is_empty() {
        println!("| Agent | Commits | Introduced | Survived | Line-weighted | Capped | Median |");
        println!("|---|---:|---:|---:|---:|---:|---:|");
        for (agent, stat) in &report.by_agent {
            println!(
                "| {} | {} | {} | {} | {} | {} | {} |",
                agent,
                stat.commits,
                stat.introduced,
                stat.surviving,
                pct(stat.survival_rate()),
                pct(stat.capped_survival_rate()),
                pct(stat.median_survival()),
            );
        }
        let dominated: Vec<String> = report
            .by_agent
            .iter()
            .filter_map(|(agent, stat)| dominance_sentence(stat).map(|s| format!("{agent}: {s}")))
            .collect();
        let below: Vec<String> = report
            .by_agent
            .iter()
            .filter(|(_, stat)| stat.commits < report.coverage.sample_floor)
            .map(|(agent, stat)| {
                format!(
                    "{agent}: {}",
                    below_floor_sentence(stat.commits, report.coverage.sample_floor)
                )
            })
            .collect();
        if !dominated.is_empty() || !below.is_empty() {
            println!();
            for line in dominated {
                println!("_{line}._  ");
            }
            for line in below {
                let line = line.trim_end_matches('.');
                println!("_{line}._  ");
            }
        }
        println!();
    }

    println!(
        "<sub>PROBABLE = heuristic. \
         Counts lines from AI-tagged commits still attributed to them by `git blame {}`; \
         inline completions leave no git trace and are not measured. \
         Capped: each commit weighs at most min(p95 of per-commit introduced lines, {} lines); \
         median: median of per-commit rates. \
         Untagged = commits with no AI signal (human, inline-completed and untagged-agent code alike); \
         age = commit date to HEAD date. \
         `absent` means no introduced lines are included in the measurement (JSON null), not a measured zero and not a claim that the commits introduced nothing. \
         An unavailable age-matched gap is not a gap of zero. \
         Method {}: [causari.dev/method](https://causari.dev/method). \
         `--json` and `--seal` record `coverage.notes_oid` (`refs/notes/ai` OID), not the raw note bodies. \
         {}</sub>",
        report.coverage.blame_flags.join(" "),
        CAP_CEILING_LINES,
        report.coverage.method,
        SUMMARY_FIELD_NOTE,
    );
    print_export_footer(report, head, true);
}

/// The bold line of `re audit --summary`. Metadata matched, not authorship.
fn tagged_summary_bold(v: &SurvivalStat) -> String {
    format!(
        "**AI-tagged (metadata matched): {}** line-weighted ({} of {} lines still at HEAD, {} commit{}) · {} capped · median {}",
        pct(v.survival_rate()),
        v.surviving,
        v.introduced,
        v.commits,
        plural(v.commits),
        pct(v.capped_survival_rate()),
        pct(v.median_survival()),
    )
}

/// The Markdown counterpart of [`print_baseline`]: one paragraph, one
/// table when there is something to put side by side.
fn summary_baseline(b: &Baseline) {
    if b.untagged.commits == 0 {
        println!(
            "Every commit carries an AI tag; there is no untagged code in this repository to compare with."
        );
        println!();
        return;
    }
    println!(
        "Same repository, untagged lines: {} of {} still at HEAD ({} line-weighted, {} commit{}).",
        b.untagged.surviving,
        b.untagged.introduced,
        pct(b.untagged.survival_rate()),
        b.untagged.commits,
        plural(b.untagged.commits),
    );
    let rows: Vec<&AgeBucket> = b
        .by_age
        .iter()
        .filter(|r| r.tagged.commits + r.untagged.commits > 0)
        .collect();
    if rows.len() > 1 {
        println!();
        println!("| Line age | AI-tagged | Untagged |");
        println!("|---|---:|---:|");
        for r in rows {
            println!(
                "| {} | {} | {} |",
                r.label(),
                cohort_cell(&r.tagged),
                cohort_cell(&r.untagged)
            );
        }
    }
    if let Some(o) = &b.oldest_surviving
        && o.commits_before > 0
    {
        println!();
        println!(
            "_The oldest line still at HEAD dates {}; {} commit{} ({} AI-tagged) are older and nothing from before that date survives, tagged or not._",
            o.date,
            o.commits_before,
            plural(o.commits_before),
            o.tagged_commits_before,
        );
    }
    println!();
}

/// The same repository's untagged lines, by age, next to the AI-tagged ones.
fn print_baseline(b: &Baseline, floor: u64) {
    println!();
    println!(
        "{}",
        "Baseline: untagged lines of the same repository".bold()
    );
    if b.untagged.commits == 0 {
        println!("  every commit carries an AI tag; there is no untagged code to compare with");
    } else {
        println!(
            "  untagged: {} commits, {} introduced, {} survived · {} line-weighted · {} median",
            b.untagged.commits,
            b.untagged.introduced,
            b.untagged.surviving,
            pct(b.untagged.survival_rate()),
            pct(b.untagged.median_survival()),
        );
    }
    let rows: Vec<&AgeBucket> = b
        .by_age
        .iter()
        .filter(|r| r.tagged.commits + r.untagged.commits > 0)
        .collect();
    if !rows.is_empty() {
        println!(
            "  {:>10} {:>24} {:>24}",
            "line age", "AI-tagged", "untagged"
        );
        for r in rows {
            println!(
                "  {:>10} {:>24} {:>24}{}",
                r.label(),
                cohort_cell(&r.tagged),
                cohort_cell(&r.untagged),
                if r.comparable() {
                    ""
                } else {
                    "   (below floor on one side)"
                }
            );
        }
    }
    println!("  {}", age_matched_sentence(b, floor));
    if let Some(o) = &b.oldest_surviving
        && o.commits_before > 0
    {
        println!(
            "  oldest line still at HEAD dates {} ({} days); {} commit{} ({} lines, {} AI-tagged commit{} \
             with {} lines) are older: nothing from before that date survives, tagged or not",
            o.date,
            o.age_days,
            o.commits_before,
            plural(o.commits_before),
            o.introduced_before,
            o.tagged_commits_before,
            plural(o.tagged_commits_before),
            o.tagged_introduced_before,
        );
    }
}

fn cohort_cell(stat: &SurvivalStat) -> String {
    if stat.commits == 0 {
        return "—".into();
    }
    format!(
        "{} ({} commit{})",
        pct(stat.survival_rate()),
        stat.commits,
        plural(stat.commits)
    )
}

fn print_class(label: &str, stat: &SurvivalStat, dominance: bool) {
    if stat.commits == 0 {
        println!("{}: {}", label.bold(), "none detected".bright_black());
        return;
    }
    println!(
        "{}: {} commits, {} introduced, {} survived",
        label.bold(),
        stat.commits,
        stat.introduced,
        stat.surviving,
    );
    println!(
        "  survival {} line-weighted · {} capped · median {}",
        pct(stat.survival_rate()),
        pct(stat.capped_survival_rate()),
        pct(stat.median_survival()),
    );
    if dominance {
        if let Some(sentence) = dominance_sentence(stat) {
            println!("  {sentence}");
        }
    }
}

/// One plain sentence when a single commit holds at least half of a row's
/// introduced lines: the row then measures that commit, and the reader
/// should know before comparing it with anything.
fn dominance_sentence(stat: &SurvivalStat) -> Option<String> {
    if !stat.dominated_by_one_commit() {
        return None;
    }
    let share = stat.largest_commit_share()?;
    Some(format!(
        "one commit accounts for {:.0}% of introduced lines; this row measures that commit",
        share * 100.0
    ))
}

/// `None` is an absent rate: no introduced lines are included in the measurement,
/// JSON null. `Some(0.0)` is a measured zero and stays `0.0%`.
fn pct(rate: Option<f64>) -> String {
    match rate {
        Some(r) => format!("{:.1}%", r * 100.0),
        None => "absent".into(),
    }
}

fn plural(n: u64) -> &'static str {
    if n == 1 { "" } else { "s" }
}

/// The identity palette. Numbers never carry colour: a badge or a card reports
/// a measurement, it does not grade it, so every value renders in graphite.
const INK: &str = "#0b0d10";
const PAPER: &str = "#f5f4ef";
const GRAPHITE: &str = "#3b4252";
const MIST: &str = "#9aa3ad";
const MONO: &str = "ui-monospace,'JetBrains Mono','SF Mono','Cascadia Mono',Menlo,Consolas,'DejaVu Sans Mono',monospace";

/// The ∵ mark as three discs, so it renders identically in every viewer and
/// never depends on a font carrying U+2235.
fn mark_svg(x: f32, y: f32, size: f32, fill: &str) -> String {
    let s = size / 100.0;
    let r = 14.5 * s;
    [(28.0, 34.0), (72.0, 34.0), (50.0, 72.0)]
        .iter()
        .map(|(cx, cy): &(f32, f32)| {
            format!(
                r#"<circle cx="{:.2}" cy="{:.2}" r="{:.2}" fill="{fill}"/>"#,
                x + cx * s,
                y + cy * s,
                r
            )
        })
        .collect::<Vec<_>>()
        .join("")
}

/// Shields-style flat badge: `∵ AI survival | NN.N%`.
fn generate_badge(report: &SurvivalReport) -> String {
    let v = &report.verified;
    let value = match v.survival_rate() {
        None => "absent".to_string(),
        Some(r) => format!("{:.1}%", r * 100.0),
    };
    let label = "AI survival";
    let label_w: u32 = 102;
    let value_w: u32 = 16 + value.len() as u32 * 7;
    let total_w = label_w + value_w;
    let mark = mark_svg(5.0, 4.0, 12.0, PAPER);
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{total_w}" height="20" role="img" aria-label="∵ {label}: {value}">
  <clipPath id="r"><rect width="{total_w}" height="20" rx="3" fill="#fff"/></clipPath>
  <g clip-path="url(#r)">
    <rect width="{label_w}" height="20" fill="{INK}"/>
    <rect x="{label_w}" width="{value_w}" height="20" fill="{GRAPHITE}"/>
  </g>
  {mark}
  <g fill="{PAPER}" font-family="{MONO}" font-size="11">
    <text x="22" y="14">{label}</text>
    <text x="{vx}" y="14" text-anchor="middle">{value}</text>
  </g>
</svg>"##,
        vx = label_w + value_w / 2,
    )
}

fn generate_svg_card(report: &SurvivalReport) -> String {
    let v = &report.verified;
    let (headline, detail) = if v.commits == 0 {
        (
            CARD_EMPTY.to_string(),
            "nothing to measure from git metadata".to_string(),
        )
    } else {
        match v.survival_rate() {
            None => (
                "absent".to_string(),
                format!(
                    "{} commits have no lines included in the measurement; the rate is absent, not zero",
                    v.commits
                ),
            ),
            Some(r) => (
                format!("{:.1}% still at HEAD", r * 100.0),
                format!(
                    "{} of {} lines, {} commits",
                    v.surviving, v.introduced, v.commits
                ),
            ),
        }
    };
    let sample_note = if v.commits > 0 && v.commits < 5 {
        " · small sample"
    } else {
        ""
    };
    let mark = mark_svg(36.0, 30.0, 28.0, PAPER);
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="560" height="240" viewBox="0 0 560 240" role="img" aria-label="∵ causari · AI code survival: {headline}">
  <rect width="560" height="240" rx="12" fill="{INK}"/>
  {mark}
  <text x="74" y="52" fill="{PAPER}" font-family="{MONO}" font-size="16" font-weight="500">causari <tspan fill="{MIST}">· AI code survival</tspan></text>
  <text x="36" y="118" fill="{PAPER}" font-family="{MONO}" font-size="30" font-weight="500">{headline}</text>
  <text x="36" y="148" fill="{MIST}" font-family="{MONO}" font-size="13">{detail}{sample_note}</text>
  <text x="36" y="172" fill="{MIST}" font-family="{MONO}" font-size="13">probable AI-assisted: {probable} commits, excluded from the number above</text>
  <line x1="36" y1="192" x2="524" y2="192" stroke="{GRAPHITE}" stroke-width="1"/>
  <text x="36" y="214" fill="{MIST}" font-family="{MONO}" font-size="11">git metadata only · a count, not a grade · re audit · method {method} · causari.dev/method</text>
</svg>"##,
        probable = report.probable.commits,
        method = report.coverage.method,
    )
}

#[cfg(test)]
mod wording_tests {
    use super::*;

    #[test]
    fn labels_say_metadata_matched_and_keep_the_json_field_name() {
        assert_eq!(LABEL_TAGGED, "AI-tagged (metadata matched)");
        assert_eq!(LABEL_BY_AGENT, "By agent (metadata matched only)");
        assert_eq!(CARD_EMPTY, "no AI-tagged commits");
        assert!(SUMMARY_FIELD_NOTE.contains("`verified`"));
        assert!(SUMMARY_FIELD_NOTE.contains("metadata matched"));
        assert!(SUMMARY_FIELD_NOTE.contains("not authorship proved"));
        let mut stat = SurvivalStat::default();
        stat.record(10, 4);
        let line = tagged_summary_bold(&stat);
        assert!(line.starts_with("**AI-tagged (metadata matched):"));
        assert!(!line.contains("Verified"));
        assert!(!line.to_lowercase().contains("proven"));
    }

    #[test]
    fn absent_rate_is_not_a_measured_zero_and_the_gap_names_its_reason() {
        assert_eq!(pct(None), "absent");
        assert_eq!(pct(Some(0.0)), "0.0%");

        let no_lines = Baseline::default();
        let no_lines_text = age_matched_sentence(&no_lines, 5);
        assert!(no_lines_text.contains("No AI-tagged lines are included in the measurement"));
        assert!(no_lines_text.contains("not a finding that those commits introduced"));
        assert!(no_lines_text.contains("not a measured gap of zero"));
        assert!(!no_lines_text.contains("at least 5"));

        let mut below_floor = Baseline::default();
        below_floor.by_age.push(AgeBucket {
            tagged: {
                let mut stat = SurvivalStat::default();
                stat.record(4, 0);
                stat
            },
            ..Default::default()
        });
        let below = age_matched_sentence(&below_floor, 5);
        assert!(below.contains("at least 5 commits"));
        assert!(below.contains("not a measured gap of zero"));
        assert!(!below.contains("No AI-tagged lines are included in the measurement"));

        let mut measured = below_floor.clone();
        measured.age_matched = Some(crate::audit::AgeMatched {
            tagged_rate: 0.0,
            untagged_rate: 0.0,
            gap: 0.0,
            buckets_used: 1,
            tagged_lines_covered: 1.0,
        });
        let zero_gap = age_matched_sentence(&measured, 5);
        assert!(zero_gap.starts_with("Age-matched gap: +0.0 percentage points"));
        assert!(!zero_gap.contains("unavailable"));
    }
}
