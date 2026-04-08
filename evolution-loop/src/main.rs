mod state;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "evolution-loop", about = "Chess engine evolution orchestrator")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start a new evolution session
    Start {
        /// Baseline version tag (e.g. v1.2)
        #[arg(long)]
        baseline_version: String,

        /// Optional ideas checklist file path
        #[arg(long)]
        ideas_file: Option<PathBuf>,

        /// Output directory for session artifacts
        #[arg(long, default_value = "tasks/evolution-runs")]
        output_dir: PathBuf,

        /// Maximum number of iterations
        #[arg(long, default_value_t = 10)]
        max_iterations: u32,

        /// Maximum consecutive infra failures before stopping
        #[arg(long, default_value_t = 3)]
        max_infra_failures: u32,

        /// Per-phase timeout in seconds
        #[arg(long, default_value_t = 1800)]
        phase_timeout_secs: u64,

        /// Stream Claude phase output to stdout in addition to log files
        #[arg(long)]
        verbose: bool,
    },
    /// Resume an interrupted evolution session
    Resume {
        /// Path to the session directory (containing session.env)
        #[arg(long)]
        session: PathBuf,

        /// Resume from a specific phase (propose/implement/validate/benchmark/decide)
        #[arg(long, value_parser = parse_phase)]
        from: Option<String>,

        /// Per-phase timeout in seconds
        #[arg(long, default_value_t = 1800)]
        phase_timeout_secs: u64,

        /// Stream Claude phase output to stdout in addition to log files
        #[arg(long)]
        verbose: bool,
    },
}

fn parse_phase(s: &str) -> Result<String, String> {
    match s {
        "propose" | "implement" | "validate" | "benchmark" | "decide" => Ok(s.to_string()),
        _ => Err(format!(
            "invalid phase '{}': must be one of propose, implement, validate, benchmark, decide",
            s
        )),
    }
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Start {
            baseline_version,
            ideas_file,
            output_dir,
            max_iterations,
            max_infra_failures,
            phase_timeout_secs,
            verbose,
        } => {
            println!(
                "Starting evolution session: baseline={}, output_dir={}, max_iterations={}, \
                 max_infra_failures={}, phase_timeout_secs={}, verbose={}, ideas_file={:?}",
                baseline_version,
                output_dir.display(),
                max_iterations,
                max_infra_failures,
                phase_timeout_secs,
                verbose,
                ideas_file
            );
        }
        Commands::Resume {
            session,
            from,
            phase_timeout_secs,
            verbose,
        } => {
            println!(
                "Resuming evolution session: session={}, from={:?}, phase_timeout_secs={}, verbose={}",
                session.display(),
                from,
                phase_timeout_secs,
                verbose
            );
        }
    }
}
