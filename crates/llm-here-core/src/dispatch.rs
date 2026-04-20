//! Subprocess dispatch for CLI providers.
//!
//! Each provider has a canonical argv template that takes the prompt as
//! a positional argument (all four supported CLIs work that way). We
//! spawn the subprocess, enforce a timeout, and surface the outcome as
//! a typed [`DispatchOutcome`] which the higher-level `run_*` functions
//! translate into a [`RunReport`].
//!
//! The subprocess call is abstracted behind [`CommandRunner`] so tests
//! can inject canned outcomes without spawning real processes.

use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::api::{run_api_provider, HttpClient, RealHttpClient};
use crate::env::{should_skip_cli, Env, RealEnv};
use crate::providers::{get, ProviderId, ProviderKind, REGISTRY};
use crate::report::{RunReport, SCHEMA_VERSION};

/// Options for a single dispatch.
#[derive(Debug, Clone)]
pub struct DispatchOptions {
    /// Wall-clock timeout for the subprocess or HTTP call. Enforced via
    /// `wait-timeout` for CLIs and `reqwest` timeout for APIs.
    pub timeout: Duration,
    /// Pass `--dangerously-skip-permissions` to `claude`. Caller-owned policy:
    /// llm-here does not read `CALORON_ALLOW_DANGEROUS_CLAUDE` or any other
    /// ambient env — the caller decides per invocation whether to enable it.
    pub dangerous_claude: bool,
    /// Model override for API providers. `None` uses each provider's
    /// `model_default` from the REGISTRY. Ignored by CLI providers (which
    /// manage their own model selection internally).
    pub model: Option<String>,
}

impl Default for DispatchOptions {
    fn default() -> Self {
        Self {
            // 25 s default — stays under Noether's 30 s stage kill and under
            // caloron's field-tested sandbox stall window.
            timeout: Duration::from_secs(25),
            dangerous_claude: false,
            model: None,
        }
    }
}

/// One dispatch invocation. Owned so implementations can freely move its
/// parts into platform-specific handles.
#[derive(Debug, Clone)]
pub struct DispatchRequest {
    pub argv: Vec<String>,
    pub timeout: Duration,
}

/// Outcome of running a subprocess. All variants are terminal — the
/// caller does not retry at this level.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DispatchOutcome {
    /// Process exited 0 and produced non-empty stdout.
    Success { stdout: String },
    /// Process exited with a non-zero status. `stdout`/`stderr` included
    /// for diagnostics; callers typically surface `stderr` as the error.
    NonZeroExit {
        code: Option<i32>,
        stdout: String,
        stderr: String,
    },
    /// Timeout fired before the process completed. The child has been
    /// killed by the runner.
    Timeout,
    /// Binary was not on PATH (or the OS reported ENOENT during spawn).
    NotFound,
    /// Catch-all for IO errors during spawn or wait.
    Other { message: String },
}

/// Trait abstracting "run this argv with this timeout, tell me what
/// happened". The real impl uses `std::process::Command`; tests use a
/// `FakeCommandRunner` in `tests/dispatch.rs`.
pub trait CommandRunner {
    fn run(&self, req: DispatchRequest) -> DispatchOutcome;
}

/// Production runner: spawns via `std::process::Command` and enforces
/// the timeout with `wait-timeout::ChildExt::wait_timeout`.
pub struct RealCommandRunner;

impl CommandRunner for RealCommandRunner {
    fn run(&self, req: DispatchRequest) -> DispatchOutcome {
        let (head, tail) = match req.argv.split_first() {
            Some(x) => x,
            None => {
                return DispatchOutcome::Other {
                    message: "empty argv".into(),
                };
            }
        };

        let mut cmd = Command::new(head);
        cmd.args(tail)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return DispatchOutcome::NotFound;
            }
            Err(e) => {
                return DispatchOutcome::Other {
                    message: format!("spawn failed: {e}"),
                };
            }
        };

        use wait_timeout::ChildExt;
        let status = match child.wait_timeout(req.timeout) {
            Ok(Some(s)) => s,
            Ok(None) => {
                // Timeout fired. Kill and harvest the child to prevent zombies.
                let _ = child.kill();
                let _ = child.wait();
                return DispatchOutcome::Timeout;
            }
            Err(e) => {
                return DispatchOutcome::Other {
                    message: format!("wait failed: {e}"),
                };
            }
        };

        // Drain stdout/stderr after the process has exited. They were
        // captured into OS-level pipes while the child ran; reading
        // them now cannot hang.
        let mut stdout = String::new();
        let mut stderr = String::new();
        if let Some(mut pipe) = child.stdout.take() {
            use std::io::Read;
            let _ = pipe.read_to_string(&mut stdout);
        }
        if let Some(mut pipe) = child.stderr.take() {
            use std::io::Read;
            let _ = pipe.read_to_string(&mut stderr);
        }

        if status.success() {
            DispatchOutcome::Success { stdout }
        } else {
            DispatchOutcome::NonZeroExit {
                code: status.code(),
                stdout,
                stderr,
            }
        }
    }
}

// Silences the unused-import lint when only `RealCommandRunner` is used
// in downstream binaries that don't touch `Write`.
#[allow(dead_code)]
fn _force_write_use() {
    let _: Option<&dyn Write> = None;
}

/// Build the argv for a CLI provider. Returns `None` for non-CLI providers.
///
/// Templates are derived from each CLI's documented `-p / --prompt`
/// invocation. Verified in caloron-noether's `stages/phases/_llm.py`
/// which has field-tested all four in real sprint loops.
pub fn build_argv(id: ProviderId, prompt: &str, opts: &DispatchOptions) -> Option<Vec<String>> {
    let p = get(id);
    if p.kind != ProviderKind::Cli {
        return None;
    }
    let binary = p.binary.expect("CLI providers declare a binary");
    let argv: Vec<String> = match id {
        ProviderId::ClaudeCli => {
            let mut v = vec![binary.to_string()];
            if opts.dangerous_claude {
                v.push("--dangerously-skip-permissions".to_string());
            }
            v.push("-p".to_string());
            v.push(prompt.to_string());
            v
        }
        ProviderId::GeminiCli => vec![
            binary.to_string(),
            "-y".to_string(),
            "-p".to_string(),
            prompt.to_string(),
        ],
        ProviderId::CursorCli => vec![
            binary.to_string(),
            "-p".to_string(),
            prompt.to_string(),
            "--output-format".to_string(),
            "text".to_string(),
        ],
        ProviderId::Opencode => vec![binary.to_string(), "run".to_string(), prompt.to_string()],
        ProviderId::AnthropicApi
        | ProviderId::OpenaiApi
        | ProviderId::GeminiApi
        | ProviderId::MistralApi => return None,
    };
    Some(argv)
}

/// Run a prompt through one specific provider.
pub fn run_cli_provider<R: CommandRunner>(
    id: ProviderId,
    prompt: &str,
    opts: &DispatchOptions,
    runner: &R,
) -> RunReport {
    let started = Instant::now();
    let argv = match build_argv(id, prompt, opts) {
        Some(a) => a,
        None => {
            return error_report(
                started,
                Some(id.as_str()),
                format!(
                    "provider {} is not a CLI provider; use run_api_provider for APIs",
                    id.as_str()
                ),
            );
        }
    };
    let outcome = runner.run(DispatchRequest {
        argv,
        timeout: opts.timeout,
    });
    outcome_to_report(started, id, outcome)
}

/// Try every reachable provider in REGISTRY order; return the first
/// success. Tries CLIs first (unless `*_SKIP_CLI` env is truthy, in
/// which case CLIs are skipped), then API providers whose auth env var
/// is set.
pub fn run_auto<E: Env + ?Sized, R: CommandRunner, H: HttpClient>(
    prompt: &str,
    opts: &DispatchOptions,
    env: &E,
    runner: &R,
    http: &H,
) -> RunReport {
    let started = Instant::now();
    let skip_cli = should_skip_cli(env);

    let mut last_error: Option<String> = None;
    let mut attempted_any = false;

    for p in REGISTRY {
        let reachable = match p.kind {
            ProviderKind::Cli => {
                if skip_cli {
                    continue;
                }
                let binary = p.binary.expect("CLI providers declare a binary");
                env.which(binary).is_some()
            }
            ProviderKind::Api => {
                let var = p.env.expect("API providers declare an env var");
                env.var(var).is_some()
            }
        };
        if !reachable {
            continue;
        }

        attempted_any = true;
        let report = match p.kind {
            ProviderKind::Cli => run_cli_provider(p.id, prompt, opts, runner),
            ProviderKind::Api => run_api_provider(p.id, prompt, opts, env, http),
        };
        if report.ok {
            return report;
        }
        last_error = report.error;
    }

    let message = if attempted_any {
        last_error.unwrap_or_else(|| "all reachable providers failed".to_string())
    } else if skip_cli {
        "all CLI providers skipped via *_SKIP_CLI env and no API keys set".to_string()
    } else {
        "no providers reachable on this host".to_string()
    };
    error_report(started, None, message)
}

/// Convenience: run against the real environment, real subprocess runner,
/// and real HTTP client. Used by the `llm-here` binary.
pub fn run_auto_real(prompt: &str, opts: &DispatchOptions) -> RunReport {
    run_auto(
        prompt,
        opts,
        &RealEnv,
        &RealCommandRunner,
        &RealHttpClient::new(),
    )
}

pub fn run_cli_provider_real(id: ProviderId, prompt: &str, opts: &DispatchOptions) -> RunReport {
    run_cli_provider(id, prompt, opts, &RealCommandRunner)
}

fn outcome_to_report(started: Instant, id: ProviderId, outcome: DispatchOutcome) -> RunReport {
    let duration_ms = started.elapsed().as_millis() as u64;
    let id_str = id.as_str().to_string();

    match outcome {
        DispatchOutcome::Success { stdout } => {
            let trimmed = stdout.trim();
            if trimmed.is_empty() {
                RunReport {
                    schema_version: SCHEMA_VERSION,
                    tool_version: env!("CARGO_PKG_VERSION").to_string(),
                    ok: false,
                    text: None,
                    provider_used: Some(id_str.clone()),
                    duration_ms,
                    error: Some(format!("{id_str}: exited 0 but produced empty stdout")),
                }
            } else {
                RunReport {
                    schema_version: SCHEMA_VERSION,
                    tool_version: env!("CARGO_PKG_VERSION").to_string(),
                    ok: true,
                    text: Some(trimmed.to_string()),
                    provider_used: Some(id_str),
                    duration_ms,
                    error: None,
                }
            }
        }
        DispatchOutcome::NonZeroExit {
            code,
            stdout: _,
            stderr,
        } => {
            let code_part = code.map(|c| c.to_string()).unwrap_or_else(|| "?".into());
            let stderr_tail = tail(&stderr, 400);
            RunReport {
                schema_version: SCHEMA_VERSION,
                tool_version: env!("CARGO_PKG_VERSION").to_string(),
                ok: false,
                text: None,
                provider_used: Some(id_str.clone()),
                duration_ms,
                error: Some(format!("{id_str}: exit {code_part}; stderr: {stderr_tail}")),
            }
        }
        DispatchOutcome::Timeout => RunReport {
            schema_version: SCHEMA_VERSION,
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
            ok: false,
            text: None,
            provider_used: Some(id_str.clone()),
            duration_ms,
            error: Some(format!("{id_str}: timed out after {duration_ms}ms")),
        },
        DispatchOutcome::NotFound => RunReport {
            schema_version: SCHEMA_VERSION,
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
            ok: false,
            text: None,
            provider_used: Some(id_str.clone()),
            duration_ms,
            error: Some(format!("{id_str}: binary not found on PATH")),
        },
        DispatchOutcome::Other { message } => RunReport {
            schema_version: SCHEMA_VERSION,
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
            ok: false,
            text: None,
            provider_used: Some(id_str.clone()),
            duration_ms,
            error: Some(format!("{id_str}: {message}")),
        },
    }
}

fn error_report(started: Instant, provider: Option<&str>, error: String) -> RunReport {
    let duration_ms = started.elapsed().as_millis() as u64;
    RunReport {
        schema_version: SCHEMA_VERSION,
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        ok: false,
        text: None,
        provider_used: provider.map(str::to_string),
        duration_ms,
        error: Some(error),
    }
}

fn tail(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.len() <= max {
        t.to_string()
    } else {
        let start = t.len() - max;
        // UTF-8 boundary-safe cut: walk forward to the next char boundary.
        let safe_start = t
            .char_indices()
            .map(|(i, _)| i)
            .find(|&i| i >= start)
            .unwrap_or(t.len());
        format!("…{}", &t[safe_start..])
    }
}
