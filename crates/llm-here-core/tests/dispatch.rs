//! Dispatch tests using a fake CommandRunner and fake Env.
//!
//! Real-subprocess tests live outside CI — they require binaries on PATH
//! that aren't available in a fresh GitHub Actions runner. These
//! fake-based tests verify the argv-template logic, the outcome-to-report
//! translation, and the fallback semantics deterministically.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use llm_here_core::api::{HttpClient, HttpOutcome, HttpRequest};
use llm_here_core::dispatch::{
    build_argv, run_auto, run_cli_provider, CommandRunner, DispatchOptions, DispatchOutcome,
    DispatchRequest,
};
use llm_here_core::env::Env;
use llm_here_core::providers::ProviderId;

// ─── Fake environment ────────────────────────────────────────────────────

#[derive(Default)]
struct FakeEnv {
    vars: HashMap<String, String>,
    path_binaries: HashMap<String, PathBuf>,
}

impl FakeEnv {
    fn with_binary(mut self, name: &str, path: &str) -> Self {
        self.path_binaries
            .insert(name.to_string(), PathBuf::from(path));
        self
    }
    fn with_env(mut self, key: &str, value: &str) -> Self {
        self.vars.insert(key.to_string(), value.to_string());
        self
    }
}

impl Env for FakeEnv {
    fn var(&self, key: &str) -> Option<String> {
        self.vars.get(key).cloned().filter(|v| !v.is_empty())
    }
    fn which(&self, binary: &str) -> Option<PathBuf> {
        self.path_binaries.get(binary).cloned()
    }
}

// ─── Fake command runner ─────────────────────────────────────────────────

/// Runner that matches against argv[0] and returns a canned outcome.
/// Records every call so tests can assert what was actually invoked.
#[derive(Default)]
struct FakeRunner {
    rules: HashMap<String, DispatchOutcome>,
    calls: RefCell<Vec<DispatchRequest>>,
}

impl FakeRunner {
    fn new() -> Self {
        Self::default()
    }
    fn on(mut self, binary: &str, outcome: DispatchOutcome) -> Self {
        self.rules.insert(binary.to_string(), outcome);
        self
    }
    fn calls(&self) -> Vec<DispatchRequest> {
        self.calls.borrow().clone()
    }
}

impl CommandRunner for FakeRunner {
    fn run(&self, req: DispatchRequest) -> DispatchOutcome {
        self.calls.borrow_mut().push(req.clone());
        let head = req.argv.first().cloned().unwrap_or_default();
        self.rules
            .get(&head)
            .cloned()
            .unwrap_or(DispatchOutcome::NotFound)
    }
}

/// HTTP client that always fails with ConnectError. Used in dispatch
/// tests that don't exercise API dispatch — we want API branches in
/// `run_auto` to short-circuit cleanly so the test assertions can focus
/// on CLI behaviour. API-specific behaviour is tested in `tests/api.rs`.
struct FailingHttpClient;

impl HttpClient for FailingHttpClient {
    fn post_json(&self, _req: HttpRequest) -> HttpOutcome {
        HttpOutcome::ConnectError {
            message: "test: HTTP deliberately unavailable in dispatch.rs tests".into(),
        }
    }
}

fn opts() -> DispatchOptions {
    DispatchOptions {
        timeout: Duration::from_secs(25),
        dangerous_claude: false,
        model: None,
    }
}

// ─── build_argv ──────────────────────────────────────────────────────────

#[test]
fn claude_argv_template_default() {
    let argv = build_argv(ProviderId::ClaudeCli, "hello", &opts()).unwrap();
    assert_eq!(argv, vec!["claude", "-p", "hello"]);
}

#[test]
fn claude_argv_template_dangerous() {
    let opts = DispatchOptions {
        dangerous_claude: true,
        ..opts()
    };
    let argv = build_argv(ProviderId::ClaudeCli, "hello", &opts).unwrap();
    assert_eq!(
        argv,
        vec!["claude", "--dangerously-skip-permissions", "-p", "hello"]
    );
}

#[test]
fn gemini_argv_template() {
    let argv = build_argv(ProviderId::GeminiCli, "hi", &opts()).unwrap();
    assert_eq!(argv, vec!["gemini", "-y", "-p", "hi"]);
}

#[test]
fn cursor_argv_template() {
    let argv = build_argv(ProviderId::CursorCli, "x", &opts()).unwrap();
    assert_eq!(
        argv,
        vec!["cursor-agent", "-p", "x", "--output-format", "text"]
    );
}

#[test]
fn opencode_argv_template() {
    let argv = build_argv(ProviderId::Opencode, "q", &opts()).unwrap();
    assert_eq!(argv, vec!["opencode", "run", "q"]);
}

#[test]
fn api_providers_have_no_argv() {
    assert!(build_argv(ProviderId::AnthropicApi, "p", &opts()).is_none());
    assert!(build_argv(ProviderId::OpenaiApi, "p", &opts()).is_none());
    assert!(build_argv(ProviderId::GeminiApi, "p", &opts()).is_none());
    assert!(build_argv(ProviderId::MistralApi, "p", &opts()).is_none());
}

// ─── run_cli_provider ────────────────────────────────────────────────────

#[test]
fn run_cli_provider_success() {
    let runner = FakeRunner::new().on(
        "claude",
        DispatchOutcome::Success {
            stdout: "  hello from claude\n".into(),
        },
    );
    let report = run_cli_provider(ProviderId::ClaudeCli, "hi", &opts(), &runner);
    assert!(report.ok);
    assert_eq!(report.text.as_deref(), Some("hello from claude"));
    assert_eq!(report.provider_used.as_deref(), Some("claude-cli"));
    assert!(report.error.is_none());
}

#[test]
fn run_cli_provider_empty_stdout_is_failure() {
    let runner = FakeRunner::new().on(
        "claude",
        DispatchOutcome::Success {
            stdout: "   \n".into(),
        },
    );
    let report = run_cli_provider(ProviderId::ClaudeCli, "hi", &opts(), &runner);
    assert!(!report.ok);
    assert!(report.text.is_none());
    assert!(report.error.as_deref().unwrap().contains("empty stdout"));
}

#[test]
fn run_cli_provider_nonzero_exit_reports_stderr_tail() {
    let runner = FakeRunner::new().on(
        "gemini",
        DispatchOutcome::NonZeroExit {
            code: Some(2),
            stdout: String::new(),
            stderr: "authentication failed: refresh token expired\n".into(),
        },
    );
    let report = run_cli_provider(ProviderId::GeminiCli, "q", &opts(), &runner);
    assert!(!report.ok);
    let err = report.error.as_deref().unwrap();
    assert!(err.contains("exit 2"));
    assert!(err.contains("authentication failed"));
}

#[test]
fn run_cli_provider_timeout_surfaces_typed_error() {
    let runner = FakeRunner::new().on("opencode", DispatchOutcome::Timeout);
    let report = run_cli_provider(ProviderId::Opencode, "q", &opts(), &runner);
    assert!(!report.ok);
    assert!(report.error.as_deref().unwrap().contains("timed out"));
}

#[test]
fn run_cli_provider_not_found_reports_cleanly() {
    let runner = FakeRunner::new(); // no rules ⇒ NotFound
    let report = run_cli_provider(ProviderId::ClaudeCli, "q", &opts(), &runner);
    assert!(!report.ok);
    assert!(report
        .error
        .as_deref()
        .unwrap()
        .contains("binary not found"));
}

#[test]
fn run_cli_provider_rejects_api_id() {
    let runner = FakeRunner::new();
    let report = run_cli_provider(ProviderId::AnthropicApi, "q", &opts(), &runner);
    assert!(!report.ok);
    let err = report.error.as_deref().unwrap();
    assert!(err.contains("not a CLI provider"));
    assert!(err.contains("run_api_provider"));
    // Must not have invoked the runner.
    assert_eq!(runner.calls().len(), 0);
}

// ─── run_auto ────────────────────────────────────────────────────────────

#[test]
fn run_auto_picks_first_reachable_cli_that_succeeds() {
    let env = FakeEnv::default()
        .with_binary("claude", "/usr/local/bin/claude")
        .with_binary("gemini", "/usr/local/bin/gemini");
    let runner = FakeRunner::new()
        .on(
            "claude",
            DispatchOutcome::Success {
                stdout: "claude-response".into(),
            },
        )
        .on(
            "gemini",
            DispatchOutcome::Success {
                stdout: "gemini-response".into(),
            },
        );
    let report = run_auto("hi", &opts(), &env, &runner, &FailingHttpClient);
    assert!(report.ok);
    assert_eq!(report.provider_used.as_deref(), Some("claude-cli"));
    assert_eq!(report.text.as_deref(), Some("claude-response"));
    // Only claude should have been called — first-success short-circuits.
    assert_eq!(runner.calls().len(), 1);
}

#[test]
fn run_auto_falls_through_to_next_reachable_on_failure() {
    let env = FakeEnv::default()
        .with_binary("claude", "/usr/local/bin/claude")
        .with_binary("gemini", "/usr/local/bin/gemini");
    let runner = FakeRunner::new()
        .on(
            "claude",
            DispatchOutcome::NonZeroExit {
                code: Some(1),
                stdout: String::new(),
                stderr: "claude unhappy".into(),
            },
        )
        .on(
            "gemini",
            DispatchOutcome::Success {
                stdout: "gemini won".into(),
            },
        );
    let report = run_auto("hi", &opts(), &env, &runner, &FailingHttpClient);
    assert!(report.ok);
    assert_eq!(report.provider_used.as_deref(), Some("gemini-cli"));
    assert_eq!(runner.calls().len(), 2);
}

#[test]
fn run_auto_skips_unreachable_cli_without_invoking_it() {
    let env = FakeEnv::default().with_binary("gemini", "/usr/local/bin/gemini");
    // Note: `claude` is NOT on the fake PATH, so it must not be attempted.
    let runner = FakeRunner::new().on(
        "gemini",
        DispatchOutcome::Success {
            stdout: "gemini".into(),
        },
    );
    let report = run_auto("hi", &opts(), &env, &runner, &FailingHttpClient);
    assert!(report.ok);
    assert_eq!(report.provider_used.as_deref(), Some("gemini-cli"));
    assert_eq!(runner.calls().len(), 1);
    assert_eq!(runner.calls()[0].argv[0], "gemini");
}

#[test]
fn run_auto_returns_last_error_when_all_fail() {
    let env = FakeEnv::default()
        .with_binary("claude", "/usr/local/bin/claude")
        .with_binary("gemini", "/usr/local/bin/gemini");
    let runner = FakeRunner::new().on("claude", DispatchOutcome::Timeout).on(
        "gemini",
        DispatchOutcome::NonZeroExit {
            code: Some(3),
            stdout: String::new(),
            stderr: "gemini final error".into(),
        },
    );
    let report = run_auto("hi", &opts(), &env, &runner, &FailingHttpClient);
    assert!(!report.ok);
    let err = report.error.as_deref().unwrap();
    // Last failure wins (gemini, since it's tried after claude).
    assert!(err.contains("gemini final error"));
}

#[test]
fn run_auto_returns_error_when_nothing_reachable() {
    // Empty PATH, no API keys — nothing to try on this host.
    let env = FakeEnv::default();
    let runner = FakeRunner::new();
    let report = run_auto("hi", &opts(), &env, &runner, &FailingHttpClient);
    assert!(!report.ok);
    assert!(report
        .error
        .as_deref()
        .unwrap()
        .contains("no providers reachable"));
    assert_eq!(runner.calls().len(), 0);
}

#[test]
fn run_auto_short_circuits_when_skip_cli_env_set() {
    let env = FakeEnv::default()
        .with_binary("claude", "/usr/local/bin/claude")
        .with_env("LLM_HERE_SKIP_CLI", "1");
    let runner = FakeRunner::new().on(
        "claude",
        DispatchOutcome::Success {
            stdout: "should not be called".into(),
        },
    );
    let report = run_auto("hi", &opts(), &env, &runner, &FailingHttpClient);
    assert!(!report.ok);
    let err = report.error.as_deref().unwrap();
    assert!(err.contains("skipped"));
    assert_eq!(runner.calls().len(), 0);
}

#[test]
fn run_auto_honours_caloron_skip_cli_alias() {
    let env = FakeEnv::default()
        .with_binary("claude", "/usr/local/bin/claude")
        .with_env("CALORON_LLM_SKIP_CLI", "true");
    let runner = FakeRunner::new();
    let report = run_auto("hi", &opts(), &env, &runner, &FailingHttpClient);
    assert!(!report.ok);
    assert_eq!(runner.calls().len(), 0);
}

#[test]
fn run_auto_falls_through_to_apis_after_clis_fail() {
    // As of v0.3, `run_auto` tries CLIs first (REGISTRY order), then APIs
    // whose auth env var is set. When every CLI fails and the HTTP client
    // rejects API calls, we expect all 3 API keys to be attempted in order
    // (anthropic, openai, gemini) before the final failure report.
    let env = FakeEnv::default()
        .with_binary("claude", "/usr/local/bin/claude")
        .with_env("ANTHROPIC_API_KEY", "sk-x")
        .with_env("OPENAI_API_KEY", "sk-y")
        .with_env("GOOGLE_API_KEY", "sk-z");
    let runner = FakeRunner::new().on("claude", DispatchOutcome::Timeout);

    let report = run_auto("hi", &opts(), &env, &runner, &FailingHttpClient);
    assert!(!report.ok);

    // The runner saw exactly one call (claude-cli).
    assert_eq!(runner.calls().len(), 1);
    assert_eq!(runner.calls()[0].argv[0], "claude");

    // The last error surfaced should be from an API call (FailingHttpClient
    // always returns ConnectError), not the CLI timeout — the API chain
    // ran after the CLI failed.
    let err = report.error.as_deref().unwrap();
    assert!(
        err.contains("connect error"),
        "expected API connect error in final report, got: {err}"
    );
}
