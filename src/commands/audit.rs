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

use crate::audit::{SurvivalReport, SurvivalStat, audit_repo};
use crate::cli::AuditArgs;

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

pub fn run(args: AuditArgs) -> Result<()> {
    let (dir, _tmp) = resolve_target(args.target.as_deref())?;
    let report = audit_repo(&dir).context("audit failed")?;

    if args.json {
        serde_json::to_writer_pretty(
            std::io::stdout(),
            &serde_json::json!({
                "total_commits": report.total_commits,
                "verified": {
                    "commits": report.verified.commits,
                    "introduced": report.verified.introduced,
                    "surviving": report.verified.surviving,
                    "survival_rate": report.verified.survival_rate(),
                },
                "probable": {
                    "commits": report.probable.commits,
                    "introduced": report.probable.introduced,
                    "surviving": report.probable.surviving,
                    "survival_rate": report.probable.survival_rate(),
                },
                "by_agent": report.by_agent,
            }),
        )?;
        println!();
        return Ok(());
    }

    if args.summary {
        print_summary(&report);
    } else {
        print_terminal(&report);
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
        let snapshot = serde_json::json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "total_commits": report.total_commits,
            "verified": {
                "commits": report.verified.commits,
                "introduced": report.verified.introduced,
                "surviving": report.verified.surviving,
                "survival_rate": report.verified.survival_rate(),
            },
            "probable": {
                "commits": report.probable.commits,
                "introduced": report.probable.introduced,
                "surviving": report.probable.surviving,
                "survival_rate": report.probable.survival_rate(),
            },
            "by_agent": report.by_agent,
        });
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
        for (agent, stat) in &report.by_agent {
            println!(
                "  {:20} {:>6} lines, {:>6} survived ({:>5.1}%)",
                agent.cyan(),
                stat.introduced,
                stat.surviving,
                stat.survival_rate().unwrap_or(0.0) * 100.0
            );
        }
    }

    println!();
    println!("{}", "Confidence notes".bright_black().bold());
    println!("  · VERIFIED = explicit metadata (trailers, bot author, etc.)");
    println!("  · PROBABLE = weak heuristic; may include human-assisted commits");
    println!("  · UNKNOWN commits are excluded from headline numbers");
    println!("  · Only lines from AI-tagged commits are measured; inline completions");
    println!("    (Copilot, Cursor Tab, …) leave no git trace and are invisible here");
    println!("  · A measurement, not a grade: method at https://causari.dev/method");
}

fn print_summary(report: &SurvivalReport) {
    let v = &report.verified;
    let rate = v.survival_rate();

    // A measurement, not a grade: no colour, no verdict. The reader judges.
    println!("## ∵ causari · AI code survival");
    println!();
    println!(
        "{} commits analyzed (git metadata only, retroactive, no setup).",
        report.total_commits
    );
    println!();

    if v.commits > 0 {
        println!(
            "**Verified AI survival: {:.1}%** ({} of {} lines still at HEAD, {} commit{})",
            rate.unwrap_or(0.0) * 100.0,
            v.surviving,
            v.introduced,
            v.commits,
            if v.commits == 1 { "" } else { "s" }
        );
        if v.commits < 5 {
            println!();
            println!(
                "_Small sample: {} AI-tagged commit{}. A single commit can dominate this figure; read it as a count, not a rate._",
                v.commits,
                if v.commits == 1 { "" } else { "s" }
            );
        }
        println!();
    }
    if report.probable.commits > 0 {
        println!(
            "Probable AI-assisted: {} commits, {} introduced, {} survived ({:.1}%).",
            report.probable.commits,
            report.probable.introduced,
            report.probable.surviving,
            report.probable.survival_rate().unwrap_or(0.0) * 100.0
        );
        println!();
    }

    if !report.by_agent.is_empty() {
        println!("| Agent | Introduced | Survived | Survival |");
        println!("|---|---:|---:|---:|");
        for (agent, stat) in &report.by_agent {
            println!(
                "| {} | {} | {} | {:.1}% |",
                agent,
                stat.introduced,
                stat.surviving,
                stat.survival_rate().unwrap_or(0.0) * 100.0
            );
        }
        println!();
    }

    println!(
        "<sub>VERIFIED = explicit commit metadata; PROBABLE = heuristic. \
         Counts lines from AI-tagged commits still attributed to them by `git blame`; \
         inline completions leave no git trace and are not measured. \
         Method: [causari.dev/method](https://causari.dev/method) · reproduce: `re audit`</sub>"
    );
}

fn print_class(label: &str, stat: &SurvivalStat) {
    if stat.commits == 0 {
        println!("{}: {}", label.bold(), "none detected".bright_black());
        return;
    }
    let pct = stat.survival_rate().unwrap_or(0.0) * 100.0;
    println!(
        "{}: {} commits, {} introduced, {} survived ({:.1}%)",
        label.bold(),
        stat.commits,
        stat.introduced,
        stat.surviving,
        pct
    );
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
            format!("{} of {} lines, {} commits", v.surviving, v.introduced, v.commits),
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
  <text x="36" y="214" fill="{MIST}" font-family="{MONO}" font-size="11">git metadata only · a count, not a grade · re audit · causari.dev/method</text>
</svg>"##,
        probable = report.probable.commits,
    )
}
