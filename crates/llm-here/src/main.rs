//! `llm-here` CLI binary.
//!
//! Thin wrapper around [`llm_here_core`]: parses argv, invokes the
//! appropriate core function, prints its output as JSON on stdout, and
//! exits. All observable behaviour is the JSON on stdout; human-oriented
//! messages go to stderr.

use std::io::{self, Read, Write};
use std::process::ExitCode;
use std::time::Duration;

use clap::{Parser, Subcommand};
use llm_here_core::dispatch::{DispatchOptions, run_auto_real, run_cli_provider_real};
use llm_here_core::providers::ProviderId;
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
    /// Run a prompt through a provider. Prompt is read from stdin.
    ///
    /// API providers are not yet implemented; `--provider <api-id>` returns
    /// a typed error in the JSON response. `--auto` skips API providers and
    /// only tries reachable CLI providers until v0.3.
    Run(RunArgs),
}

#[derive(Debug, clap::Args)]
struct RunArgs {
    /// Provider id (e.g. claude-cli). Mutually exclusive with --auto.
    #[arg(long, conflicts_with = "auto")]
    provider: Option<String>,

    /// Try each reachable CLI provider in fallback order.
    #[arg(long, conflicts_with = "provider")]
    auto: bool,

    /// Wall-clock timeout in seconds. Default 25 s — stays under
    /// Noether's 30 s stage kill and caloron's sandbox stall window.
    #[arg(long, default_value_t = 25)]
    timeout: u32,

    /// Pass `--dangerously-skip-permissions` to `claude`. Caller-owned
    /// policy: llm-here reads no ambient env for this — the caller
    /// decides per invocation.
    #[arg(long = "dangerous-claude")]
    dangerous_claude: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    match cli.command {
        Command::Detect => {
            let report = llm_here_core::detect();
            write_json(&mut out, &report)
        }
        Command::Run(args) => run(&mut out, args),
    }
}

fn run<W: Write>(out: &mut W, args: RunArgs) -> ExitCode {
    if !args.auto && args.provider.is_none() {
        let report = stub_error_report(
            "missing target: pass either --provider <id> or --auto".into(),
            None,
        );
        return emit_report(out, &report);
    }

    let prompt = match read_stdin() {
        Ok(p) if !p.trim().is_empty() => p,
        Ok(_) => {
            let report = stub_error_report(
                "prompt is empty — llm-here run reads the prompt from stdin".into(),
                None,
            );
            return emit_report(out, &report);
        }
        Err(e) => {
            let report = stub_error_report(format!("failed to read stdin: {e}"), None);
            return emit_report(out, &report);
        }
    };

    let opts = DispatchOptions {
        timeout: Duration::from_secs(args.timeout as u64),
        dangerous_claude: args.dangerous_claude,
    };

    let report = if args.auto {
        run_auto_real(&prompt, &opts)
    } else {
        let id_str = args.provider.as_deref().unwrap_or_default();
        match ProviderId::parse(id_str) {
            Some(id) => run_cli_provider_real(id, &prompt, &opts),
            None => stub_error_report(
                format!("unknown provider id: {id_str}. See `llm-here detect` for valid ids."),
                Some(id_str.to_string()),
            ),
        }
    };

    emit_report(out, &report)
}

fn read_stdin() -> io::Result<String> {
    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf)?;
    Ok(buf)
}

fn stub_error_report(error: String, provider_used: Option<String>) -> RunReport {
    RunReport {
        schema_version: SCHEMA_VERSION,
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        ok: false,
        text: None,
        provider_used,
        duration_ms: 0,
        error: Some(error),
    }
}

fn emit_report<W: Write>(out: &mut W, report: &RunReport) -> ExitCode {
    if serde_json::to_writer_pretty(&mut *out, report).is_err() {
        eprintln!("llm-here: failed to serialise run report");
        return ExitCode::from(2);
    }
    let _ = writeln!(out);
    if report.ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn write_json<W: Write, T: serde::Serialize>(out: &mut W, report: &T) -> ExitCode {
    if let Err(e) = serde_json::to_writer_pretty(&mut *out, report) {
        eprintln!("llm-here: failed to serialise report: {e}");
        return ExitCode::from(2);
    }
    let _ = writeln!(out);
    ExitCode::SUCCESS
}
