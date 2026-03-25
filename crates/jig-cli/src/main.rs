mod cli;
mod approval;
mod subcommands;

use clap::Parser;
use tracing_subscriber::{EnvFilter, fmt};

use cli::{Cli, Commands};
use jig_core::{
    assembly::{AssemblyOptions, run_assembly},
    config::resolve::CliOverrides,
};

fn main() -> miette::Result<()> {
    // Step 1: Parse CLI args BEFORE any I/O
    let cli = Cli::parse();

    // Step 2: Handle --help, --version, completions before any config I/O
    if let Some(Commands::Completions(ref args)) = cli.command {
        use clap::CommandFactory;
        use clap_complete::generate;
        let mut cmd = Cli::command();
        let bin_name = cmd.get_name().to_owned();
        generate(args.shell, &mut cmd, bin_name, &mut std::io::stdout());
        return Ok(());
    }

    // Step 3: Initialize tracing based on --verbose count
    init_tracing(cli.verbose);

    // Step 4: Route subcommands
    let cwd = std::env::current_dir().map_err(|e| miette::miette!("{}", e))?;

    match &cli.command {
        Some(cmd) => {
            if let Err(e) = subcommands::dispatch(cmd, &cwd, cli.json) {
                return Err(miette::miette!("{}", e));
            }
        }
        None => {
            // No subcommand — launch path
            run_launch(cli, &cwd)?;
        }
    }

    Ok(())
}

fn run_launch(cli: Cli, cwd: &std::path::Path) -> miette::Result<()> {
    use jig_core::history::last_session;

    let _overrides = CliOverrides {
        template: cli.template.clone(),
        persona: cli.persona.clone(),
        model: None,
    };

    // Determine if TUI should open
    let use_tui = !cli.go && !cli.dry_run && !cli.last && !cli.resume && cli.template.is_none();

    let (final_template, final_persona) = if use_tui {
        #[cfg(feature = "tui")]
        {
            use jig_tui::app::run_tui;
            match run_tui().map_err(|e| miette::miette!("{}", e))? {
                Some((t, p)) => (Some(t), Some(p)),
                None => return Ok(()), // User quit without selecting
            }
        }
        #[cfg(not(feature = "tui"))]
        {
            tracing::warn!("TUI not available in headless build. Use -t <template> to specify a template.");
            (None, None)
        }
    } else if cli.last || cli.resume {
        match last_session() {
            Some(entry) => (entry.template, entry.persona),
            None => {
                eprintln!("No previous session found.");
                return Ok(());
            }
        }
    } else {
        (cli.template.clone(), cli.persona.clone())
    };

    let effective_overrides = CliOverrides {
        template: final_template.clone(),
        persona: final_persona.clone(),
        model: None,
    };

    let approval_ui = Box::new(approval::TerminalApprovalUi {
        non_interactive: cli.non_interactive,
    });

    let opts = AssemblyOptions {
        project_dir: cwd.to_owned(),
        cli_overrides: effective_overrides,
        dry_run: cli.dry_run,
        approval_ui,
        yes: cli.yes,
        non_interactive: cli.non_interactive,
    };

    let exit_code = run_assembly(opts).map_err(|e| miette::miette!("{}", e))?;

    std::process::exit(exit_code);
}

fn init_tracing(verbose: u8) {
    let level = match verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };

    let filter = EnvFilter::try_from_env("RUST_LOG")
        .unwrap_or_else(|_| EnvFilter::new(format!("jig_core={level},jig_cli={level}")));

    // All tracing output goes to stderr (stdout is for machine-readable output)
    fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
}
