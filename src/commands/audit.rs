/// `re audit` — retroactive Group-0 AI-code survival audit.
///
/// Works on any git repository without a Causari ledger. Reads git history,
/// classifies AI-authored commits by metadata, then counts how many of those
/// lines survived to HEAD.
use anyhow::{Context, Result, bail};
use colored::Colorize;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::audit::{
    AuditOptions, CAP_CEILING_LINES, IGNORE_REVS_FILE, METHOD_VERSION, ShallowCloneRefused,
    SurvivalReport, SurvivalStat, audit_repo,
};
use crate::audit_seal::{self, AuditBinding};
use crate::cli::AuditArgs;
use crate::exit::exit_with;
use crate::repo::Repo;
use crate::seal::SealIssuer;

const DEFAULT_SEAL_FILE: &str = "audit.seal.json";

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
/// measured.
fn report_json(report: &SurvivalReport) -> Result<serde_json::Value> {
    let mut value = serde_json::to_value(report)?;
    value["method"] = serde_json::json!(METHOD_VERSION);
    Ok(value)
}

/// The exact bytes `--json` prints: pretty JSON and one newline. An audit
/// seal commits to these bytes, so they are produced in one place.
fn audit_json_bytes(report: &SurvivalReport) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(&report_json(report)?)?;
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
    let (dir, tmp) = resolve_target(args.target.as_deref())?;
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
        Some(audit_json_bytes(&report)?)
    } else {
        None
    };
    // Seal before printing: a failed issuance must not leave a report on
    // stdout that looks sealed.
    let sealed = if args.seal {
        let bytes = audit_json.as_deref().unwrap_or_default();
        Some(seal_audit(&dir, tmp.is_some(), &args, &report, bytes)?)
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

    if args.summary {
        print_summary(&report);
    } else {
        print_terminal(&report);
    }

    if let Some((out, bundle)) = &sealed {
        let seal = &bundle["seal"];
        println!();
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
        let mut snapshot = report_json(&report)?;
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

fn print_terminal(report: &SurvivalReport) {
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

    print_class("Verified AI-authored", &report.verified);
    print_class("Probable AI-assisted", &report.probable);

    if !report.by_agent.is_empty() {
        println!("{}", "By agent (verified only)".bold());
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
        }
    }

    println!();
    println!("{}", "Confidence notes".bright_black().bold());
    println!("  · VERIFIED = explicit metadata (trailers, bot author, etc.)");
    println!("  · PROBABLE = weak heuristic; may include human-assisted commits");
    println!("  · UNKNOWN commits are excluded from headline numbers");
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
}

fn print_summary(report: &SurvivalReport) {
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
        println!(
            "**Verified AI survival: {}** line-weighted ({} of {} lines still at HEAD, {} commit{}) · {} capped · median {}",
            pct(v.survival_rate()),
            v.surviving,
            v.introduced,
            v.commits,
            plural(v.commits),
            pct(v.capped_survival_rate()),
            pct(v.median_survival()),
        );
        if let Some(sentence) = dominance_sentence(v) {
            println!();
            println!("_{sentence}._");
        }
        if report.coverage.small_sample {
            println!();
            println!(
                "_Small sample: {} AI-tagged commit{} (floor {}). Read the figures as counts, not rates._",
                v.commits,
                plural(v.commits),
                report.coverage.sample_floor
            );
        }
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
        if !dominated.is_empty() {
            println!();
            for line in dominated {
                println!("_{line}._  ");
            }
        }
        println!();
    }

    println!(
        "<sub>VERIFIED = explicit commit metadata; PROBABLE = heuristic. \
         Counts lines from AI-tagged commits still attributed to them by `git blame {}`; \
         inline completions leave no git trace and are not measured. \
         Capped: each commit weighs at most min(p95 of per-commit introduced lines, {} lines); \
         median: median of per-commit rates. \
         Method {}: [causari.dev/method](https://causari.dev/method) · reproduce: `re audit`</sub>",
        report.coverage.blame_flags.join(" "),
        CAP_CEILING_LINES,
        report.coverage.method,
    );
}

fn print_class(label: &str, stat: &SurvivalStat) {
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
    if let Some(sentence) = dominance_sentence(stat) {
        println!("  {sentence}");
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

fn pct(rate: Option<f64>) -> String {
    match rate {
        Some(r) => format!("{:.1}%", r * 100.0),
        None => "n/a".into(),
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
        None => "n/a".to_string(),
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
    let (headline, detail) = match v.survival_rate() {
        None => (
            "no verified AI commits".to_string(),
            "nothing to measure from git metadata".to_string(),
        ),
        Some(r) => (
            format!("{:.1}% still at HEAD", r * 100.0),
            format!(
                "{} of {} lines, {} commits",
                v.surviving, v.introduced, v.commits
            ),
        ),
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
