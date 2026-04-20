//! API dispatch tests using a FakeHttpClient + FakeEnv.
//!
//! Exercises the provider-specific request shapes, response parsing,
//! HTTP error paths, and the integration between `run_api_provider`
//! and the env layer. No network calls.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use llm_here_core::api::{run_api_provider, HttpClient, HttpOutcome, HttpRequest};
use llm_here_core::dispatch::DispatchOptions;
use llm_here_core::env::Env;
use llm_here_core::providers::ProviderId;
use serde_json::{json, Value};

// ─── Fakes ───────────────────────────────────────────────────────────────

#[derive(Default)]
struct FakeEnv {
    vars: HashMap<String, String>,
}

impl FakeEnv {
    fn with_env(mut self, key: &str, value: &str) -> Self {
        self.vars.insert(key.to_string(), value.to_string());
        self
    }
}

impl Env for FakeEnv {
    fn var(&self, key: &str) -> Option<String> {
        self.vars.get(key).cloned().filter(|v| !v.is_empty())
    }
    fn which(&self, _binary: &str) -> Option<PathBuf> {
        None
    }
}

/// Fake HTTP client: matches on URL prefix, returns a canned outcome.
/// Records every request so tests can assert on URL, headers, and body.
#[derive(Default)]
struct FakeHttp {
    rules: Vec<(String, HttpOutcome)>,
    requests: RefCell<Vec<HttpRequest>>,
}

impl FakeHttp {
    fn new() -> Self {
        Self::default()
    }
    fn on_url_prefix(mut self, prefix: &str, outcome: HttpOutcome) -> Self {
        self.rules.push((prefix.to_string(), outcome));
        self
    }
    fn requests(&self) -> Vec<HttpRequest> {
        self.requests.borrow().clone()
    }
}

impl HttpClient for FakeHttp {
    fn post_json(&self, req: HttpRequest) -> HttpOutcome {
        self.requests.borrow_mut().push(req.clone());
        for (prefix, outcome) in &self.rules {
            if req.url.starts_with(prefix) {
                return outcome.clone();
            }
        }
        HttpOutcome::Other {
            message: format!("no FakeHttp rule matched {}", req.url),
        }
    }
}

fn opts() -> DispatchOptions {
    DispatchOptions {
        timeout: Duration::from_secs(25),
        dangerous_claude: false,
        model: None,
        system_prompt: None,
    }
}

fn header(req: &HttpRequest, name: &str) -> Option<String> {
    req.headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.clone())
}

// ─── Missing-key guard ───────────────────────────────────────────────────

#[test]
fn api_dispatch_errors_when_env_var_unset() {
    let env = FakeEnv::default();
    let http = FakeHttp::new();
    let report = run_api_provider(ProviderId::AnthropicApi, "hi", &opts(), &env, &http);
    assert!(!report.ok);
    let err = report.error.as_deref().unwrap();
    assert!(err.contains("ANTHROPIC_API_KEY"));
    assert!(err.contains("not set"));
    // No HTTP call should have been attempted.
    assert_eq!(http.requests().len(), 0);
}

#[test]
fn api_dispatch_errors_when_env_var_empty_string() {
    let env = FakeEnv::default().with_env("OPENAI_API_KEY", "");
    let http = FakeHttp::new();
    let report = run_api_provider(ProviderId::OpenaiApi, "hi", &opts(), &env, &http);
    assert!(!report.ok);
    assert!(report.error.as_deref().unwrap().contains("not set"));
    assert_eq!(http.requests().len(), 0);
}

#[test]
fn api_dispatch_rejects_cli_ids() {
    let env = FakeEnv::default();
    let http = FakeHttp::new();
    let report = run_api_provider(ProviderId::ClaudeCli, "hi", &opts(), &env, &http);
    assert!(!report.ok);
    let err = report.error.as_deref().unwrap();
    assert!(err.contains("not an API provider"));
    assert_eq!(http.requests().len(), 0);
}

// ─── Anthropic ──────────────────────────────────────────────────────────

#[test]
fn anthropic_request_shape_and_success() {
    let env = FakeEnv::default().with_env("ANTHROPIC_API_KEY", "sk-anthropic-xxx");
    let http = FakeHttp::new().on_url_prefix(
        "https://api.anthropic.com",
        HttpOutcome::Response {
            status: 200,
            body: json!({
                "content": [{"type": "text", "text": "  hello from claude api\n"}]
            })
            .to_string(),
        },
    );
    let report = run_api_provider(ProviderId::AnthropicApi, "greet me", &opts(), &env, &http);
    assert!(report.ok);
    assert_eq!(report.text.as_deref(), Some("hello from claude api"));
    assert_eq!(report.provider_used.as_deref(), Some("anthropic-api"));

    let req = http.requests().into_iter().next().unwrap();
    assert_eq!(req.url, "https://api.anthropic.com/v1/messages");
    assert_eq!(
        header(&req, "x-api-key").as_deref(),
        Some("sk-anthropic-xxx")
    );
    assert_eq!(
        header(&req, "anthropic-version").as_deref(),
        Some("2023-06-01")
    );
    // Default model when --model not set.
    assert_eq!(
        req.body.get("model").and_then(Value::as_str),
        Some("claude-sonnet-4-5")
    );
    assert_eq!(
        req.body.get("max_tokens").and_then(Value::as_u64),
        Some(4096)
    );
    assert_eq!(
        req.body
            .pointer("/messages/0/content")
            .and_then(Value::as_str),
        Some("greet me")
    );
}

#[test]
fn anthropic_model_override_via_opts() {
    let env = FakeEnv::default().with_env("ANTHROPIC_API_KEY", "sk-x");
    let http = FakeHttp::new().on_url_prefix(
        "https://api.anthropic.com",
        HttpOutcome::Response {
            status: 200,
            body: json!({"content": [{"text": "ok"}]}).to_string(),
        },
    );
    let opts = DispatchOptions {
        model: Some("claude-opus-4-1".into()),
        ..opts()
    };
    let _ = run_api_provider(ProviderId::AnthropicApi, "q", &opts, &env, &http);
    let req = http.requests().into_iter().next().unwrap();
    assert_eq!(
        req.body.get("model").and_then(Value::as_str),
        Some("claude-opus-4-1")
    );
}

#[test]
fn anthropic_api_key_never_appears_in_error_message() {
    let env = FakeEnv::default().with_env("ANTHROPIC_API_KEY", "sk-LEAKY-SECRET-DO-NOT-LEAK");
    let http = FakeHttp::new().on_url_prefix(
        "https://api.anthropic.com",
        HttpOutcome::Response {
            status: 401,
            body: "{\"error\": \"unauthorized\"}".into(),
        },
    );
    let report = run_api_provider(ProviderId::AnthropicApi, "q", &opts(), &env, &http);
    assert!(!report.ok);
    let err = report.error.as_deref().unwrap();
    assert!(err.contains("401"));
    assert!(
        !err.contains("sk-LEAKY-SECRET-DO-NOT-LEAK"),
        "error message leaked the API key: {err}"
    );
}

// ─── OpenAI ─────────────────────────────────────────────────────────────

#[test]
fn openai_request_shape_and_success() {
    let env = FakeEnv::default().with_env("OPENAI_API_KEY", "sk-openai-xxx");
    let http = FakeHttp::new().on_url_prefix(
        "https://api.openai.com",
        HttpOutcome::Response {
            status: 200,
            body: json!({
                "choices": [{"message": {"role": "assistant", "content": "hello from openai"}}]
            })
            .to_string(),
        },
    );
    let report = run_api_provider(ProviderId::OpenaiApi, "greet", &opts(), &env, &http);
    assert!(report.ok);
    assert_eq!(report.text.as_deref(), Some("hello from openai"));

    let req = http.requests().into_iter().next().unwrap();
    assert_eq!(req.url, "https://api.openai.com/v1/chat/completions");
    assert_eq!(
        header(&req, "authorization").as_deref(),
        Some("Bearer sk-openai-xxx")
    );
    assert_eq!(
        req.body.get("model").and_then(Value::as_str),
        Some("gpt-4o")
    );
    // OpenAI payloads don't include max_tokens by default (callers can
    // override later; unset means the server's default applies).
    assert!(req.body.get("max_tokens").is_none());
}

// ─── Gemini ─────────────────────────────────────────────────────────────

#[test]
fn gemini_request_shape_and_success() {
    let env = FakeEnv::default().with_env("GOOGLE_API_KEY", "google-xxx");
    let http = FakeHttp::new().on_url_prefix(
        "https://generativelanguage.googleapis.com",
        HttpOutcome::Response {
            status: 200,
            body: json!({
                "candidates": [{
                    "content": {"parts": [{"text": "hello from gemini"}]}
                }]
            })
            .to_string(),
        },
    );
    let report = run_api_provider(ProviderId::GeminiApi, "greet", &opts(), &env, &http);
    assert!(report.ok);
    assert_eq!(report.text.as_deref(), Some("hello from gemini"));

    let req = http.requests().into_iter().next().unwrap();
    assert!(req.url.starts_with(
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-pro:generateContent"
    ));
    assert!(req.url.contains("key=google-xxx"));
    assert_eq!(
        req.body
            .pointer("/contents/0/parts/0/text")
            .and_then(Value::as_str),
        Some("greet")
    );
}

// ─── Mistral ────────────────────────────────────────────────────────────

#[test]
fn mistral_request_shape_and_success() {
    let env = FakeEnv::default().with_env("MISTRAL_API_KEY", "mistral-xxx");
    let http = FakeHttp::new().on_url_prefix(
        "https://api.mistral.ai",
        HttpOutcome::Response {
            status: 200,
            body: json!({
                "choices": [{"message": {"content": "hello from mistral"}}]
            })
            .to_string(),
        },
    );
    let report = run_api_provider(ProviderId::MistralApi, "greet", &opts(), &env, &http);
    assert!(report.ok);
    assert_eq!(report.text.as_deref(), Some("hello from mistral"));

    let req = http.requests().into_iter().next().unwrap();
    assert_eq!(req.url, "https://api.mistral.ai/v1/chat/completions");
    assert_eq!(
        header(&req, "authorization").as_deref(),
        Some("Bearer mistral-xxx")
    );
    assert_eq!(
        req.body.get("model").and_then(Value::as_str),
        Some("mistral-large-latest")
    );
}

// ─── Error paths ────────────────────────────────────────────────────────

#[test]
fn api_dispatch_surfaces_non_2xx_body_tail() {
    let env = FakeEnv::default().with_env("OPENAI_API_KEY", "sk-x");
    let http = FakeHttp::new().on_url_prefix(
        "https://api.openai.com",
        HttpOutcome::Response {
            status: 500,
            body: "internal server error: upstream deadline exceeded".into(),
        },
    );
    let report = run_api_provider(ProviderId::OpenaiApi, "q", &opts(), &env, &http);
    assert!(!report.ok);
    let err = report.error.as_deref().unwrap();
    assert!(err.contains("HTTP 500"));
    assert!(err.contains("upstream deadline exceeded"));
}

#[test]
fn api_dispatch_handles_timeout() {
    let env = FakeEnv::default().with_env("MISTRAL_API_KEY", "mistral-x");
    let http = FakeHttp::new().on_url_prefix("https://api.mistral.ai", HttpOutcome::Timeout);
    let report = run_api_provider(ProviderId::MistralApi, "q", &opts(), &env, &http);
    assert!(!report.ok);
    assert!(report.error.as_deref().unwrap().contains("timed out"));
}

#[test]
fn api_dispatch_handles_connect_error() {
    let env = FakeEnv::default().with_env("ANTHROPIC_API_KEY", "sk-x");
    let http = FakeHttp::new().on_url_prefix(
        "https://api.anthropic.com",
        HttpOutcome::ConnectError {
            message: "dns resolution failed".into(),
        },
    );
    let report = run_api_provider(ProviderId::AnthropicApi, "q", &opts(), &env, &http);
    assert!(!report.ok);
    let err = report.error.as_deref().unwrap();
    assert!(err.contains("connect error"));
    assert!(err.contains("dns resolution failed"));
}

#[test]
fn api_dispatch_handles_non_json_response_body() {
    let env = FakeEnv::default().with_env("ANTHROPIC_API_KEY", "sk-x");
    let http = FakeHttp::new().on_url_prefix(
        "https://api.anthropic.com",
        HttpOutcome::Response {
            status: 200,
            body: "<html><body>Nginx error page</body></html>".into(),
        },
    );
    let report = run_api_provider(ProviderId::AnthropicApi, "q", &opts(), &env, &http);
    assert!(!report.ok);
    assert!(report.error.as_deref().unwrap().contains("parse error"));
}

#[test]
fn api_dispatch_handles_unexpected_json_shape() {
    let env = FakeEnv::default().with_env("OPENAI_API_KEY", "sk-x");
    let http = FakeHttp::new().on_url_prefix(
        "https://api.openai.com",
        HttpOutcome::Response {
            status: 200,
            // Valid JSON but missing choices[0].message.content.
            body: json!({"choices": []}).to_string(),
        },
    );
    let report = run_api_provider(ProviderId::OpenaiApi, "q", &opts(), &env, &http);
    assert!(!report.ok);
    assert!(report.error.as_deref().unwrap().contains("parse error"));
}

// ─── System prompt plumbing ─────────────────────────────────────────────

#[test]
fn anthropic_system_prompt_goes_in_top_level_field() {
    let env = FakeEnv::default().with_env("ANTHROPIC_API_KEY", "sk-x");
    let http = FakeHttp::new().on_url_prefix(
        "https://api.anthropic.com",
        HttpOutcome::Response {
            status: 200,
            body: json!({"content": [{"text": "ok"}]}).to_string(),
        },
    );
    let opts = DispatchOptions {
        system_prompt: Some("be terse".into()),
        ..opts()
    };
    let _ = run_api_provider(ProviderId::AnthropicApi, "hi", &opts, &env, &http);
    let req = http.requests().into_iter().next().unwrap();
    // Anthropic's API: top-level `system` field, not a message role.
    assert_eq!(
        req.body.get("system").and_then(Value::as_str),
        Some("be terse")
    );
    // The user message stays as-is.
    assert_eq!(
        req.body
            .pointer("/messages/0/content")
            .and_then(Value::as_str),
        Some("hi")
    );
}

#[test]
fn openai_system_prompt_prepends_system_role_message() {
    let env = FakeEnv::default().with_env("OPENAI_API_KEY", "sk-x");
    let http = FakeHttp::new().on_url_prefix(
        "https://api.openai.com",
        HttpOutcome::Response {
            status: 200,
            body: json!({"choices": [{"message": {"content": "ok"}}]}).to_string(),
        },
    );
    let opts = DispatchOptions {
        system_prompt: Some("be terse".into()),
        ..opts()
    };
    let _ = run_api_provider(ProviderId::OpenaiApi, "hi", &opts, &env, &http);
    let req = http.requests().into_iter().next().unwrap();
    // Messages array: [{role: "system", content: sys}, {role: "user", content: prompt}]
    assert_eq!(
        req.body.pointer("/messages/0/role").and_then(Value::as_str),
        Some("system")
    );
    assert_eq!(
        req.body
            .pointer("/messages/0/content")
            .and_then(Value::as_str),
        Some("be terse")
    );
    assert_eq!(
        req.body.pointer("/messages/1/role").and_then(Value::as_str),
        Some("user")
    );
    assert_eq!(
        req.body
            .pointer("/messages/1/content")
            .and_then(Value::as_str),
        Some("hi")
    );
}

#[test]
fn mistral_system_prompt_prepends_system_role_message() {
    // Mistral uses the OpenAI-compatible shape; same behaviour.
    let env = FakeEnv::default().with_env("MISTRAL_API_KEY", "mistral-x");
    let http = FakeHttp::new().on_url_prefix(
        "https://api.mistral.ai",
        HttpOutcome::Response {
            status: 200,
            body: json!({"choices": [{"message": {"content": "ok"}}]}).to_string(),
        },
    );
    let opts = DispatchOptions {
        system_prompt: Some("system-text".into()),
        ..opts()
    };
    let _ = run_api_provider(ProviderId::MistralApi, "q", &opts, &env, &http);
    let req = http.requests().into_iter().next().unwrap();
    assert_eq!(
        req.body.pointer("/messages/0/role").and_then(Value::as_str),
        Some("system")
    );
    assert_eq!(
        req.body
            .pointer("/messages")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn gemini_system_prompt_goes_in_system_instruction() {
    let env = FakeEnv::default().with_env("GOOGLE_API_KEY", "google-x");
    let http = FakeHttp::new().on_url_prefix(
        "https://generativelanguage.googleapis.com",
        HttpOutcome::Response {
            status: 200,
            body: json!({
                "candidates": [{"content": {"parts": [{"text": "ok"}]}}]
            })
            .to_string(),
        },
    );
    let opts = DispatchOptions {
        system_prompt: Some("be terse".into()),
        ..opts()
    };
    let _ = run_api_provider(ProviderId::GeminiApi, "hi", &opts, &env, &http);
    let req = http.requests().into_iter().next().unwrap();
    assert_eq!(
        req.body
            .pointer("/system_instruction/parts/0/text")
            .and_then(Value::as_str),
        Some("be terse")
    );
}

#[test]
fn api_dispatch_handles_empty_text() {
    let env = FakeEnv::default().with_env("ANTHROPIC_API_KEY", "sk-x");
    let http = FakeHttp::new().on_url_prefix(
        "https://api.anthropic.com",
        HttpOutcome::Response {
            status: 200,
            body: json!({"content": [{"text": "   \n"}]}).to_string(),
        },
    );
    let report = run_api_provider(ProviderId::AnthropicApi, "q", &opts(), &env, &http);
    assert!(!report.ok);
    assert!(report.error.as_deref().unwrap().contains("empty"));
}
