//! Integration tests for `detect_with_env` using a fake environment.

use std::collections::HashMap;
use std::path::PathBuf;

use llm_here_core::detect::detect_with_env;
use llm_here_core::env::Env;

#[derive(Default)]
struct FakeEnv {
    vars: HashMap<String, String>,
    path_binaries: HashMap<String, PathBuf>,
}

impl FakeEnv {
    fn with_env(mut self, key: &str, value: &str) -> Self {
        self.vars.insert(key.to_string(), value.to_string());
        self
    }

    fn with_binary(mut self, name: &str, path: &str) -> Self {
        self.path_binaries
            .insert(name.to_string(), PathBuf::from(path));
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

#[test]
fn empty_env_detects_nothing() {
    let env = FakeEnv::default();
    let report = detect_with_env(&env);
    assert!(report.providers.is_empty());
    assert!(!report.cli_detection_skipped);
    assert_eq!(report.schema_version, 1);
}

#[test]
fn detects_claude_cli_when_on_path() {
    let env = FakeEnv::default().with_binary("claude", "/usr/local/bin/claude");
    let report = detect_with_env(&env);
    assert_eq!(report.providers.len(), 1);
    let p = &report.providers[0];
    assert_eq!(p.id, "claude-cli");
    assert_eq!(p.binary.as_deref(), Some("/usr/local/bin/claude"));
    assert_eq!(p.env, None);
}

#[test]
fn detects_anthropic_api_when_key_set() {
    let env = FakeEnv::default().with_env("ANTHROPIC_API_KEY", "sk-xxx");
    let report = detect_with_env(&env);
    assert_eq!(report.providers.len(), 1);
    let p = &report.providers[0];
    assert_eq!(p.id, "anthropic-api");
    assert_eq!(p.env.as_deref(), Some("ANTHROPIC_API_KEY"));
    assert_eq!(p.binary, None);
}

#[test]
fn api_key_value_is_never_serialised() {
    let env = FakeEnv::default().with_env("ANTHROPIC_API_KEY", "sk-leaky-secret-do-not-leak");
    let report = detect_with_env(&env);
    let json = serde_json::to_string(&report).unwrap();
    assert!(
        !json.contains("sk-leaky-secret-do-not-leak"),
        "detect output must never contain the API key value itself"
    );
}

#[test]
fn empty_env_var_does_not_count_as_set() {
    let env = FakeEnv::default().with_env("OPENAI_API_KEY", "");
    let report = detect_with_env(&env);
    assert!(report.providers.is_empty());
}

#[test]
fn skip_cli_env_skips_all_cli_providers() {
    let env = FakeEnv::default()
        .with_binary("claude", "/usr/local/bin/claude")
        .with_binary("gemini", "/usr/local/bin/gemini")
        .with_env("ANTHROPIC_API_KEY", "sk-xxx")
        .with_env("LLM_HERE_SKIP_CLI", "1");

    let report = detect_with_env(&env);

    assert!(report.cli_detection_skipped);
    assert_eq!(report.providers.len(), 1);
    assert_eq!(report.providers[0].id, "anthropic-api");
}

#[test]
fn caloron_skip_cli_env_is_honoured() {
    let env = FakeEnv::default()
        .with_binary("claude", "/usr/local/bin/claude")
        .with_env("CALORON_LLM_SKIP_CLI", "true");

    let report = detect_with_env(&env);

    assert!(report.cli_detection_skipped);
    assert!(report.providers.is_empty());
}

#[test]
fn agentspec_skip_cli_env_is_honoured() {
    let env = FakeEnv::default()
        .with_binary("claude", "/usr/local/bin/claude")
        .with_env("AGENTSPEC_LLM_SKIP_CLI", "yes");

    let report = detect_with_env(&env);
    assert!(report.cli_detection_skipped);
    assert!(report.providers.is_empty());
}

#[test]
fn skip_cli_with_falsy_value_does_not_skip() {
    let env = FakeEnv::default()
        .with_binary("claude", "/usr/local/bin/claude")
        .with_env("LLM_HERE_SKIP_CLI", "0");

    let report = detect_with_env(&env);
    assert!(!report.cli_detection_skipped);
    assert_eq!(report.providers.len(), 1);
    assert_eq!(report.providers[0].id, "claude-cli");
}

#[test]
fn all_providers_detected_in_fallback_order() {
    // Make every provider reachable and assert the output order matches
    // the REGISTRY declaration order (which defines the `--auto` fallback).
    let env = FakeEnv::default()
        .with_binary("claude", "/usr/local/bin/claude")
        .with_binary("gemini", "/usr/local/bin/gemini")
        .with_binary("cursor-agent", "/usr/local/bin/cursor-agent")
        .with_binary("opencode", "/usr/local/bin/opencode")
        .with_env("ANTHROPIC_API_KEY", "sk-a")
        .with_env("OPENAI_API_KEY", "sk-o")
        .with_env("GOOGLE_API_KEY", "sk-g")
        .with_env("MISTRAL_API_KEY", "sk-m");

    let report = detect_with_env(&env);

    let ids: Vec<&str> = report.providers.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(
        ids,
        vec![
            "claude-cli",
            "gemini-cli",
            "cursor-cli",
            "opencode",
            "anthropic-api",
            "openai-api",
            "gemini-api",
            "mistral-api",
        ]
    );
}

#[test]
fn schema_version_and_tool_version_are_populated() {
    let env = FakeEnv::default();
    let report = detect_with_env(&env);
    assert_eq!(report.schema_version, 1);
    assert!(!report.tool_version.is_empty());
    // tool_version should parse as a simple semver-ish string
    assert!(report.tool_version.chars().next().unwrap().is_ascii_digit());
}
