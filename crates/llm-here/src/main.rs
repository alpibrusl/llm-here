//! `llm-here` CLI binary.
//!
//! Thin wrapper around [`llm_here_core`]: parses argv, invokes the
//! appropriate core function, prints its output as JSON on stdout, and
//! exits. All observable behaviour is the JSON on stdout; human-oriented
//! messages go to stderr.

use std::io::Write;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use llm_here_core::report::{RunReport, SCHEMA_VERSION};

#[derive(Debug, Parser)]
#[command(
    name = "llm-here",
    version,
    about = "Detect and dispatch LLM CLIs / API providers reachable from this host.",
    long_about = "llm-here answers \"which LLM is reachable from this host, and how do I run \
                  a prompt through it?\" Output is JSON on stdout; human-oriented messages \
                  go to stderr. See SCHEMA.md for the stable wire format."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// List reachable providers (CLIs on PATH + APIs with env keys set).
    Detect,
    /// Run a prompt through a provider. Not yet implemented in v0.1.
    Run(RunArgs),
}

#[derive(Debug, clap::Args)]
struct RunArgs {
    /// Provider id (e.g. claude-cli). Mutually exclusive with --auto.
    #[arg(long, conflicts_with = "auto")]
    provider: Option<String>,

    /// Try each reachable provider in fallback order.
    #[arg(long, conflicts_with = "provider")]
    auto: bool,

    /// Wall-clock timeout in seconds.
    #[arg(long, default_value_t = 25)]
    timeout: u32,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    match cli.command {
        Command::Detect => {
            let report = llm_here_core::detect();
            if let Err(e) = serde_json::to_writer_pretty(&mut out, &report) {
                eprintln!("llm-here: failed to serialise detect report: {e}");
                return ExitCode::from(2);
            }
            let _ = writeln!(&mut out);
            ExitCode::SUCCESS
        }
        Command::Run(args) => emit_run_unimplemented(&mut out, args),
    }
}

fn emit_run_unimplemented<W: Write>(out: &mut W, args: RunArgs) -> ExitCode {
    // v0.1 carves out the JSON shape without the transport. Callers can
    // code against a stable failure mode today; v0.2 wires subprocess
    // dispatch for CLIs, v0.3 adds API transport.
    let target = if args.auto {
        "auto".to_string()
    } else {
        args.provider.unwrap_or_else(|| "(unspecified)".to_string())
    };
    let report = RunReport {
        schema_version: SCHEMA_VERSION,
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        ok: false,
        text: None,
        provider_used: None,
        duration_ms: 0,
        error: Some(format!(
            "run is not implemented in v0.1 (target: {target}). Use `llm-here detect` \
             to list providers; dispatch is tracked in https://github.com/alpibrusl/llm-here \
             milestones."
        )),
    };
    if serde_json::to_writer_pretty(&mut *out, &report).is_err() {
        eprintln!("llm-here: failed to serialise run report");
        return ExitCode::from(2);
    }
    let _ = writeln!(out);
    // Non-zero exit so shell-script callers see the failure; JSON body is
    // still machine-readable for programmatic callers.
    ExitCode::from(1)
}
