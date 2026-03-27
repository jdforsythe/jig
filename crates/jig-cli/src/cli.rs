use clap::{Args, Parser, Subcommand};

/// jig — Intentional Context Utilization for Claude Code
#[derive(Parser, Debug)]
#[command(
    name = "jig",
    version,
    about = "Assembles and launches Claude Code sessions from layered YAML configs",
    long_about = None,
)]
pub struct Cli {
    /// Template name to use (bypasses TUI)
    #[arg(short = 't', long)]
    pub template: Option<String>,

    /// Persona name to use
    #[arg(short = 'p', long)]
    pub persona: Option<String>,

    /// Use last session config (bypasses TUI)
    #[arg(long)]
    pub last: bool,

    /// Re-stage most recent session and pass --resume to claude
    #[arg(long)]
    pub resume: bool,

    /// Relaunch a specific session by UUID
    #[arg(long)]
    pub session: Option<String>,

    /// Headless launch with default template (bypasses TUI)
    #[arg(long)]
    pub go: bool,

    /// Assemble but don't fork; print resolved command
    #[arg(long)]
    pub dry_run: bool,

    /// Output machine-readable JSON (with --dry-run: JSON output)
    #[arg(long, global = true)]
    pub json: bool,

    /// Auto-approve only cached hooks (not new external hooks)
    #[arg(long, global = true)]
    pub yes: bool,

    /// Non-interactive mode (auto-deny new approvals)
    #[arg(long, global = true)]
    pub non_interactive: bool,

    /// Verbosity: -v (info), -vv (debug), -vvv (trace)
    #[arg(short = 'v', long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Manage templates
    #[command(alias = "t")]
    Template(TemplateArgs),

    /// Manage personas
    #[command(alias = "pe")]
    Persona(PersonaArgs),

    /// Manage skills
    #[command(alias = "sk")]
    Skill(SkillArgs),

    /// Sync skill sources
    Sync(SyncArgs),

    /// Initialize jig in the current directory
    Init,

    /// Import a template or skill from a URL
    Import(ImportArgs),

    /// Diagnose and repair jig state
    Doctor(DoctorArgs),

    /// View session history
    History(HistoryArgs),

    /// Diff resolved config against another config file
    Diff(DiffArgs),

    /// Generate shell completions
    Completions(CompletionsArgs),

    /// Read or modify config values
    Config(ConfigArgs),
}

#[derive(Args, Debug)]
pub struct TemplateArgs {
    #[command(subcommand)]
    pub subcommand: TemplateSubcommand,
}

#[derive(Subcommand, Debug)]
pub enum TemplateSubcommand {
    /// List available templates
    List,
    /// Show a template's resolved config
    Show { name: String },
}

#[derive(Args, Debug)]
pub struct PersonaArgs {
    #[command(subcommand)]
    pub subcommand: PersonaSubcommand,
}

#[derive(Subcommand, Debug)]
pub enum PersonaSubcommand {
    /// List available personas
    List,
    /// Show a persona's config
    Show { name: String },
}

#[derive(Args, Debug)]
pub struct SkillArgs {
    #[command(subcommand)]
    pub subcommand: SkillSubcommand,
}

#[derive(Subcommand, Debug)]
pub enum SkillSubcommand {
    /// List installed skills
    List,
}

#[derive(Args, Debug)]
pub struct SyncArgs {
    /// Only fail if sources are out of date (for CI)
    #[arg(long)]
    pub frozen: bool,
}

#[derive(Args, Debug)]
pub struct ImportArgs {
    /// URL to import from
    pub url: String,

    /// Target scope
    #[arg(long, default_value = "project")]
    pub scope: String,
}

#[derive(Args, Debug)]
pub struct DoctorArgs {
    /// Run full security audit (config validation, file permissions)
    #[arg(long)]
    pub audit: bool,
}

#[derive(Args, Debug)]
pub struct HistoryArgs {
    /// Limit number of entries shown
    #[arg(long, default_value = "20")]
    pub limit: usize,
}

#[derive(Args, Debug)]
pub struct DiffArgs {
    /// Config file to compare against
    pub config: std::path::PathBuf,
}

#[derive(Args, Debug)]
pub struct CompletionsArgs {
    /// Shell to generate completions for
    pub shell: clap_complete::Shell,
}

#[derive(Args, Debug)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub subcommand: ConfigSubcommand,
}

#[derive(Subcommand, Debug)]
pub enum ConfigSubcommand {
    /// Show the resolved config (optionally with --explain for provenance)
    Show {
        #[arg(long)]
        explain: bool,
    },
    /// Set a scalar config value (dotted path notation)
    Set {
        key: String,
        value: String,
        #[arg(long, default_value = "local")]
        scope: String,
    },
    /// Add a value to an array field
    Add {
        key: String,
        value: String,
        #[arg(long, default_value = "local")]
        scope: String,
    },
    /// Remove a value from an array field
    Remove {
        key: String,
        value: String,
        #[arg(long, default_value = "local")]
        scope: String,
    },
}
