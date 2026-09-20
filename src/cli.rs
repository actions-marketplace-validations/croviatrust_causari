use clap::{Args, Parser, Subcommand};

/// Top-level help, grouped by what the user is trying to do. clap lists
/// subcommands flat; this template replaces that list, so every visible
/// subcommand must appear here (a test enforces it).
pub const HELP_TEMPLATE: &str = "\
{before-help}{about-with-newline}
{usage-heading} {usage}

Measure:
  audit     AI-tagged code still alive at HEAD of any git repository (git metadata only)
  churn     Survival of AI-attributed lines in the ledger, per agent; --json
  report    HTML view of churn

Record (the ledger, in .causari/):
  init      Start a ledger in this repository
  record    Record one agent action (flags or JSON on stdin)
  watch     Record every file change as an event (passive recorder)
  hook      Install agent-side hooks (`re hook claude-code`)
  proxy     Local LLM proxy: prompt, completion, tokens, cost per exchange
  mcp       Run as an MCP server (causari_record / recall / why)

Ask (queries over the ledger):
  log       Recent events
  show      One event: prompt, model, tokens, cost, evidence
  why       The event behind a line: `re why path/to/file:42`
  trace     Everything that led to a line, transitively
  impact    Everything that flowed from an event
  lens      A file annotated with per-line provenance
  diff      What one event changed (or a range)
  find      Search prompts, messages and tools

Move (sessions and time):
  revert    Put the workspace back to before an event
  bisect    Find the event that broke a command
  fork      Start a new session from here
  sessions  List sessions
  switch    Switch to a session and sync the workspace

Prove (offline-verifiable receipts):
  seal      Issue, list and verify Crovia Seals
  proof     Deprecated: use `re audit --seal` and `re seal verify`

Experimental:
  skill     Distill and verify signed units of past work
  brief     Markdown briefing of past work for a model's context
  guard     Substring rules over recent changes; --fail-on to gate

Options:
{options}{after-help}";

#[derive(Parser, Debug)]
#[command(
    name = "causari",
    bin_name = "re",
    version,
    about = "AI-written code has no author. It has causes. Causari proves them.",
    long_about = "Causari measures how many lines from AI-tagged commits are still alive in a \
                  git repository (`re audit`, any repo, no setup), and records the prompt, \
                  model and files behind every agent edit into a local, append-only ledger \
                  you can query like git. `causari` and `re` are the same program.",
    help_template = HELP_TEMPLATE
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Initialize a new Causari repository in the current directory
    Init,

    /// Record an agent action (reads a JSON event from stdin or flags)
    Record(RecordArgs),

    /// Show the history of recorded agent actions
    Log(LogArgs),

    /// Show details of a specific event
    Show(ShowArgs),

    /// Revert the workspace to the state before a given event
    Revert(RevertArgs),

    /// Show the diff introduced by an event (or between two events)
    Diff(DiffArgs),

    /// Explain who/what created a specific line: `re why path/to/file:42`
    Why(WhyArgs),

    /// Auto-record every filesystem change as a Causari event (passive recorder)
    Watch(WatchArgs),

    /// Binary-search the event that broke a test command
    Bisect(BisectArgs),

    /// Create a new session branch and switch HEAD to it (multiverse fork)
    Fork(ForkArgs),

    /// List all session branches (the tips of the event DAG)
    Sessions,

    /// Switch HEAD to an existing session and sync the workspace to its tip
    Switch(SwitchArgs),

    /// Show the FULL causal cone of a line: every event that contributed,
    /// transitively, via the files it read or wrote.
    Trace(TraceArgs),

    /// Search events by free text in prompt, message, reasoning or tool.
    Find(FindArgs),

    /// Show the DOWNSTREAM causal cone of an event (what flowed from it).
    Impact(ImpactArgs),

    /// Render a file with per-line provenance annotations.
    Lens(LensArgs),

    /// Distill, inspect and verify signed skills (the experience layer)
    Skill(SkillArgs),

    /// Emit a portable Markdown briefing of verified experience for a task —
    /// paste it into any model's context (CLAUDE.md, AGENTS.md, .cursorrules)
    Brief(BriefArgs),

    /// Generate or verify a signed AI-provenance proof (trustless, offline)
    Proof(ProofArgs),

    /// Run Causari as an MCP server (Claude Code, Cursor, Cline, Windsurf, …)
    Mcp(McpArgs),

    /// Scan recent events for risky patterns (watchdog)
    Guard(GuardArgs),

    /// Survival of AI-attributed lines in the ledger, per agent (a count, not a grade)
    Churn(ChurnArgs),

    /// How much AI-tagged code is still alive at HEAD of any git repository (git metadata only)
    Audit(AuditArgs),

    /// HTML view of `re churn` for sharing
    Report(ReportArgs),

    /// Run a local LLM capture proxy (OpenAI/Anthropic compatible).
    /// Every prompt, completion, token and dollar flows through Causari.
    Proxy(ProxyArgs),

    /// Issue, list and verify Crovia Seals (draft-crovia-seal-01 receipts)
    Seal(SealArgs),

    /// Install agent-side capture hooks (e.g. `re hook claude-code`)
    Hook(HookArgs),

    /// Internal: invoked by agent hooks with a JSON payload on stdin
    #[command(hide = true)]
    HookEvent(HookEventArgs),
}

#[derive(Args, Debug)]
pub struct RecordArgs {
    /// Short description of the action
    #[arg(short, long)]
    pub message: Option<String>,

    /// Tool used by the agent (e.g. "edit", "shell", "write_file")
    #[arg(short, long)]
    pub tool: Option<String>,

    /// Agent identifier (e.g. "claude-3.5-sonnet", "gpt-4o")
    #[arg(short, long)]
    pub agent: Option<String>,

    /// Read full event JSON from stdin instead of using flags
    #[arg(long)]
    pub stdin: bool,

    /// Record onto a named session instead of HEAD (one session per agent
    /// enables safe concurrent recording). Created on first use.
    #[arg(short = 's', long)]
    pub session: Option<String>,
}

#[derive(Args, Debug)]
pub struct LogArgs {
    /// Maximum number of events to display
    #[arg(short = 'n', long, default_value_t = 20)]
    pub limit: usize,

    /// Show one line per event
    #[arg(long)]
    pub oneline: bool,

    /// Show the full DAG: events from ALL sessions, with tip and fork markers
    #[arg(long)]
    pub all: bool,
}

#[derive(Args, Debug)]
pub struct ShowArgs {
    /// Event id (full or short prefix)
    pub id: String,

    /// Print the event as JSON (every recorded field, including prompt,
    /// model, tokens, cost and evidence class)
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct RevertArgs {
    /// Event id to revert TO (workspace will look like it did before this event)
    pub id: String,

    /// Skip confirmation prompt
    #[arg(long)]
    pub yes: bool,

    /// Validate the full restore and show file counts without changing files
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args, Debug)]
pub struct DiffArgs {
    /// Event id (or RANGE like a..b)
    pub spec: String,
}

#[derive(Args, Debug)]
pub struct WhyArgs {
    /// Location to explain, in the form `path/to/file:line`
    pub spec: String,
}

#[derive(Args, Debug)]
pub struct WatchArgs {
    /// Tag every auto-recorded event with this agent identifier
    #[arg(short, long)]
    pub agent: Option<String>,

    /// Tag every auto-recorded event with this model identifier
    #[arg(short, long)]
    pub model: Option<String>,

    /// Debounce window in milliseconds (default: 800)
    #[arg(long)]
    pub debounce: Option<u64>,

    /// Correlation window in seconds: how far back to search captured LLM
    /// exchanges when attributing a file change (default: 300)
    #[arg(long)]
    pub window: Option<u64>,

    /// Record onto a named session instead of HEAD (one session per agent
    /// enables safe concurrent recording). Created on first use.
    #[arg(short = 's', long)]
    pub session: Option<String>,
}

#[derive(Args, Debug)]
pub struct BisectArgs {
    /// Known-good event id
    #[arg(long)]
    pub good: String,

    /// Known-bad event id
    #[arg(long)]
    pub bad: String,

    /// Shell command whose success defines "good"
    #[arg(long)]
    pub test: String,
}

#[derive(Args, Debug)]
pub struct ForkArgs {
    /// New branch name
    pub name: String,

    /// Event id to fork from (default: HEAD)
    #[arg(long)]
    pub from: Option<String>,
}

#[derive(Args, Debug)]
pub struct SwitchArgs {
    /// Session name to switch to
    pub name: String,

    /// Keep the working tree as-is (only move HEAD)
    #[arg(long)]
    pub no_sync: bool,
}

#[derive(Args, Debug)]
pub struct TraceArgs {
    /// Location whose causal cone you want, in the form `path/to/file:line`
    pub spec: String,
}

#[derive(Args, Debug)]
pub struct FindArgs {
    /// Free-text query (matched against prompt, message, reasoning, tool)
    pub query: String,

    /// Maximum number of results to display
    #[arg(short = 'n', long)]
    pub limit: Option<usize>,
}

#[derive(Args, Debug)]
pub struct ImpactArgs {
    /// Event id whose downstream cone you want to inspect
    pub event: String,
}

#[derive(Args, Debug)]
pub struct LensArgs {
    /// Path to the file you want annotated with per-line provenance
    pub file: String,
}

#[derive(Args, Debug)]
pub struct ChurnArgs {
    /// Emit a Markdown summary (for CI / PR comments)
    #[arg(long)]
    pub summary: bool,

    /// Emit the analysis as JSON (per-agent and overall counts)
    #[arg(long, conflicts_with = "summary")]
    pub json: bool,

    /// Exit 1 when the overall survival rate of AI-attributed lines is
    /// below this percentage (0-100). Without it the command never fails
    /// on the numbers; exit 3 means there is nothing to measure yet.
    #[arg(long, value_name = "PERCENT")]
    pub fail_below: Option<f64>,
}

#[derive(Args, Debug)]
pub struct ReportArgs {
    /// Output file path (default: causari-report.html)
    #[arg(short, long)]
    pub output: Option<String>,

    /// Open the report in the default browser after writing it
    #[arg(long)]
    pub open: bool,
}

#[derive(Args, Debug)]
pub struct BriefArgs {
    /// Free-text task description matched against recorded experience.
    /// Empty = brief the most trusted experience in this repository.
    pub query: Vec<String>,

    /// Maximum entries per section
    #[arg(short = 'n', long, default_value_t = 5)]
    pub limit: usize,
}

#[derive(Args, Debug)]
pub struct AuditArgs {
    /// What to audit: a local path, a git URL, or a GitHub `owner/repo`
    /// shorthand (cloned to a temp dir). Defaults to the current directory.
    pub target: Option<String>,

    /// Emit a Markdown summary to stdout (for CI / PR comments)
    #[arg(long)]
    pub summary: bool,

    /// Write a shields-style SVG badge (causari-badge.svg) for your README
    #[arg(long)]
    pub badge: bool,

    /// Write a self-contained SVG survival card (causari-survival.svg)
    #[arg(long)]
    pub card: bool,

    /// Emit machine-readable JSON to stdout instead of terminal tables
    #[arg(long)]
    pub json: bool,

    /// Save this audit snapshot for trend comparison next time
    #[arg(long)]
    pub save: bool,

    /// Measure a shallow clone anyway (history is truncated; the report
    /// carries coverage.shallow = true). Prefer `git fetch --unshallow`.
    #[arg(long)]
    pub allow_shallow: bool,
}

#[derive(Args, Debug)]
pub struct GuardArgs {
    /// Number of recent events to scan (default: 20)
    #[arg(short = 'n', long)]
    pub limit: Option<usize>,

    /// Generate an SVG badge at .causari/guard-badge.svg
    #[arg(long)]
    pub badge: bool,

    /// Emit Markdown summary to stdout (for CI / PR comments)
    #[arg(long)]
    pub summary: bool,

    /// Emit findings as JSON
    #[arg(long, conflicts_with_all = ["summary", "badge"])]
    pub json: bool,

    /// Exit 1 when at least one finding of this severity or higher exists
    /// (`alert` or `warning`). Without it the exit code only reports errors.
    #[arg(long, value_name = "SEVERITY", value_parser = ["alert", "warning"])]
    pub fail_on: Option<String>,
}

#[derive(Args, Debug)]
pub struct ProxyArgs {
    /// Port to listen on (default: 4242)
    #[arg(short, long)]
    pub port: Option<u16>,

    /// Upstream base URL for OpenAI-style requests (default: https://api.openai.com)
    #[arg(long)]
    pub openai_upstream: Option<String>,

    /// Upstream base URL for Anthropic-style requests (default: https://api.anthropic.com)
    #[arg(long)]
    pub anthropic_upstream: Option<String>,

    /// Emit a Crovia Seal (draft-crovia-seal-01) for every completion:
    /// an Ed25519-signed, hash-chained, offline-verifiable receipt in
    /// .causari/seal/seals.jsonl
    #[arg(long)]
    pub seal: bool,

    /// Issuer id embedded in emitted seals
    /// (default: urn:crovia:seal-issuer:causari:<first 12 hex of your pubkey>)
    #[arg(long)]
    pub seal_issuer: Option<String>,
}

#[derive(Args, Debug)]
pub struct SealArgs {
    #[command(subcommand)]
    pub command: SealCommand,
}

#[derive(Subcommand, Debug)]
pub enum SealCommand {
    /// Verify every seal in this repo's chain (signatures + hash links)
    Verify {
        /// Verify a single external seal file instead of the repo chain
        file: Option<std::path::PathBuf>,
    },

    /// List the seals issued by this repository
    List {
        /// Maximum number of seals to display
        #[arg(short = 'n', long, default_value_t = 20)]
        limit: usize,
    },

    /// Show this repo's seal issuer identity (public key + chain state)
    Issuer,
}

#[derive(Args, Debug)]
pub struct HookArgs {
    /// Agent runtime to hook into (supported: claude-code)
    pub target: String,
}

#[derive(Args, Debug)]
pub struct HookEventArgs {
    /// Hook kind: user-prompt | pre-tool | post-tool | session-start
    pub kind: String,
}

#[derive(Args, Debug)]
pub struct SkillArgs {
    #[command(subcommand)]
    pub command: SkillCommand,
}

#[derive(Subcommand, Debug)]
pub enum SkillCommand {
    /// Distill new skills from the event ledger (idempotent)
    Distill,

    /// List all skills with their trust level
    List,

    /// Show one skill in full detail
    Show {
        /// Skill id (full or prefix, min 4 chars)
        id: String,
    },

    /// Verify the Ed25519 signature of one skill, or of all skills
    Verify {
        /// Skill id (omit to verify every skill)
        id: Option<String>,
    },

    /// Export a signed skill as a portable JSON bundle
    Export {
        /// Skill id (full or prefix)
        id: String,
        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<std::path::PathBuf>,
    },

    /// Import a signed skill bundle (signer must be local or trusted)
    Import {
        /// Path to a .json bundle or envelope
        file: std::path::PathBuf,
    },

    /// Sync skills from a shared team directory (Dropbox, git, NFS — no server)
    Pull {
        /// Directory containing .json skill bundles
        dir: std::path::PathBuf,
    },

    /// Manage trusted org signing keys for cross-repo skill mesh
    Trust {
        #[command(subcommand)]
        command: SkillTrustCommand,
    },
}

#[derive(Subcommand, Debug)]
pub enum SkillTrustCommand {
    /// Show this repo's public key (share with teammates)
    Pubkey,

    /// Register a teammate's or org's Ed25519 public key
    Add {
        /// Short label (e.g. "security-team")
        label: String,
        /// 64-char hex pubkey or path to a .pub file
        key: String,
    },

    /// List trusted signing keys
    List,

    /// Remove a trusted key by label
    Remove { label: String },
}

#[derive(Args, Debug)]
pub struct ProofArgs {
    #[command(subcommand)]
    pub command: ProofCommand,
}

#[derive(Subcommand, Debug)]
pub enum ProofCommand {
    /// Generate a signed proof + embeddable badge for this repo
    Generate {
        /// Proof JSON output path (default: causari-proof.json)
        #[arg(short, long)]
        output: Option<std::path::PathBuf>,

        /// Also write a self-contained SVG badge (default: causari-proof.svg)
        #[arg(long)]
        badge: Option<std::path::PathBuf>,

        /// Skip writing the SVG badge
        #[arg(long)]
        no_badge: bool,
    },

    /// Verify a proof's signature offline; optionally check it against this repo
    Verify {
        /// Path to a proof JSON (default: causari-proof.json)
        file: Option<std::path::PathBuf>,

        /// Also confirm the proof still matches the current ledger
        #[arg(long)]
        against_repo: bool,
    },
}

#[derive(Args, Debug)]
pub struct McpArgs {
    /// Print the JSON snippet to register Causari in Claude/Cursor/Cline,
    /// then exit. Without this flag, Causari runs as an MCP server on stdio.
    #[arg(long)]
    pub install: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    /// The grouped help replaces clap's own subcommand list, so a new
    /// subcommand that is not added to `HELP_TEMPLATE` would be invisible.
    #[test]
    fn every_visible_subcommand_is_in_the_grouped_help() {
        let cmd = Cli::command();
        let listed: Vec<&str> = HELP_TEMPLATE
            .lines()
            .filter(|l| l.starts_with("  ") && !l.starts_with("   "))
            .filter_map(|l| l.split_whitespace().next())
            .collect();
        for sub in cmd.get_subcommands().filter(|c| !c.is_hide_set()) {
            assert!(
                listed.contains(&sub.get_name()),
                "subcommand `{}` is missing from HELP_TEMPLATE",
                sub.get_name()
            );
        }
        for name in &listed {
            assert!(
                cmd.find_subcommand(name).is_some(),
                "HELP_TEMPLATE lists `{name}`, which is not a subcommand"
            );
        }
    }

    #[test]
    fn cli_definition_is_consistent() {
        Cli::command().debug_assert();
    }
}
