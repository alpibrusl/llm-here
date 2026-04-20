//! HTTPS dispatch to API providers.
//!
//! Each provider has its own request shape, auth style, and response
//! path. We keep the provider-specific glue in this module and abstract
//! the HTTP boundary behind [`HttpClient`] so tests can inject canned
//! responses without hitting the network.

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::dispatch::DispatchOptions;
use crate::env::{Env, RealEnv};
use crate::providers::{get, ProviderId, ProviderKind};
use crate::report::{RunReport, SCHEMA_VERSION};

/// Default max tokens requested from every provider that requires one.
/// Hardcoded for v0.3 — matches caloron's field-tested value. A flag
/// will land in a later release if callers ask.
const DEFAULT_MAX_TOKENS: u32 = 4096;

// ─── HTTP abstraction ────────────────────────────────────────────────────

/// One HTTPS POST. Owned so implementations can consume it freely.
#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub url: String,
    /// `(name, value)` pairs. Preserved in order; duplicates allowed
    /// (some providers want multiple Accept/Version headers together).
    pub headers: Vec<(String, String)>,
    pub body: Value,
    pub timeout: Duration,
}

/// Outcome of an HTTP call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HttpOutcome {
    /// Server responded. `status` is the HTTP status code; `body` is the
    /// raw response body as text (may or may not parse as JSON).
    Response { status: u16, body: String },
    /// Request did not complete within the timeout.
    Timeout,
    /// Transport-level failure (DNS, TLS, connection refused, etc).
    ConnectError { message: String },
    /// Catch-all for request-building / IO failures.
    Other { message: String },
}

/// Trait abstracting "POST this JSON and tell me what came back". The
/// production impl uses `reqwest::blocking`; tests use a `FakeHttpClient`
/// in `tests/api.rs`.
pub trait HttpClient {
    fn post_json(&self, req: HttpRequest) -> HttpOutcome;
}

/// Production HTTP client, built on `reqwest::blocking` with rustls and
/// a per-call timeout. Thread-safe; callers can reuse one instance across
/// dispatches.
pub struct RealHttpClient {
    inner: reqwest::blocking::Client,
}

impl RealHttpClient {
    pub fn new() -> Self {
        // A single client reuses the connection pool and TLS session
        // cache across dispatches. `reqwest::blocking::Client::new()`
        // never fails in practice; `expect` surfaces the rustls backend
        // failure loudly if it ever does.
        Self {
            inner: reqwest::blocking::Client::builder()
                .use_rustls_tls()
                .build()
                .expect("reqwest::blocking::Client failed to build with rustls"),
        }
    }
}

impl Default for RealHttpClient {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpClient for RealHttpClient {
    fn post_json(&self, req: HttpRequest) -> HttpOutcome {
        // Serialise the body ourselves and send as raw bytes. `.json()`
        // cooperates badly with our provider-supplied `content-type`
        // header (it prepends its own, and at least one provider — Mistral
        // — rejects the resulting duplicate-header payload as a JSON
        // string rather than a JSON object). Explicit is safer.
        let body_bytes = match serde_json::to_vec(&req.body) {
            Ok(b) => b,
            Err(e) => {
                return HttpOutcome::Other {
                    message: format!("failed to serialise request body: {e}"),
                };
            }
        };

        let mut builder = self
            .inner
            .post(&req.url)
            .timeout(req.timeout)
            .body(body_bytes);

        for (name, value) in &req.headers {
            builder = builder.header(name.as_str(), value.as_str());
        }

        match builder.send() {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let body = resp.text().unwrap_or_default();
                HttpOutcome::Response { status, body }
            }
            Err(e) if e.is_timeout() => HttpOutcome::Timeout,
            Err(e) if e.is_connect() || e.is_request() => HttpOutcome::ConnectError {
                message: e.to_string(),
            },
            Err(e) => HttpOutcome::Other {
                message: e.to_string(),
            },
        }
    }
}

// ─── Provider adapters ───────────────────────────────────────────────────

/// Dispatch a prompt to an API provider. Reads the required env var for
/// auth, builds the provider-specific request, posts it, and parses the
/// response. Returns a [`RunReport`] in all cases.
pub fn run_api_provider<E: Env + ?Sized, H: HttpClient>(
    id: ProviderId,
    prompt: &str,
    opts: &DispatchOptions,
    env: &E,
    http: &H,
) -> RunReport {
    let started = Instant::now();

    let p = get(id);
    if p.kind != ProviderKind::Api {
        return error_report(
            started,
            Some(id.as_str()),
            format!(
                "provider {} is not an API provider; use run_cli_provider for CLIs",
                id.as_str()
            ),
        );
    }

    let env_var = p.env.expect("API providers declare an env var");
    let api_key = match env.var(env_var) {
        Some(k) => k,
        None => {
            return error_report(
                started,
                Some(id.as_str()),
                format!("{}: env var {} is not set", id.as_str(), env_var),
            );
        }
    };

    let model = opts
        .model
        .clone()
        .unwrap_or_else(|| p.model_default.to_string());

    let system = opts.system_prompt.as_deref();
    let http_req = match id {
        ProviderId::AnthropicApi => build_anthropic(prompt, system, &api_key, &model, opts.timeout),
        ProviderId::OpenaiApi => build_openai_compat(
            "https://api.openai.com/v1/chat/completions",
            prompt,
            system,
            &api_key,
            &model,
            opts.timeout,
        ),
        ProviderId::GeminiApi => build_gemini(prompt, system, &api_key, &model, opts.timeout),
        ProviderId::MistralApi => build_openai_compat(
            "https://api.mistral.ai/v1/chat/completions",
            prompt,
            system,
            &api_key,
            &model,
            opts.timeout,
        ),
        ProviderId::ClaudeCli
        | ProviderId::GeminiCli
        | ProviderId::CursorCli
        | ProviderId::Opencode => unreachable!("caller checked kind == Api above"),
    };

    let outcome = http.post_json(http_req);
    let parsed = parse_response(id, outcome);
    outcome_to_report(started, id, parsed)
}

/// Convenience: dispatch using the real env + real HTTP client.
pub fn run_api_provider_real(id: ProviderId, prompt: &str, opts: &DispatchOptions) -> RunReport {
    run_api_provider(id, prompt, opts, &RealEnv, &RealHttpClient::new())
}

// ─── Request builders ────────────────────────────────────────────────────

fn build_anthropic(
    prompt: &str,
    system: Option<&str>,
    api_key: &str,
    model: &str,
    timeout: Duration,
) -> HttpRequest {
    // Anthropic wants the system prompt as a top-level `system` field,
    // not inline in `messages`.
    let mut body = json!({
        "model": model,
        "max_tokens": DEFAULT_MAX_TOKENS,
        "messages": [{"role": "user", "content": prompt}],
    });
    if let Some(s) = system {
        body["system"] = Value::String(s.to_string());
    }
    HttpRequest {
        url: "https://api.anthropic.com/v1/messages".to_string(),
        headers: vec![
            ("x-api-key".into(), api_key.to_string()),
            ("anthropic-version".into(), "2023-06-01".into()),
            ("content-type".into(), "application/json".into()),
        ],
        body,
        timeout,
    }
}

/// Shared builder for OpenAI-compatible chat-completions endpoints.
/// Covers `openai-api` and `mistral-api` — both accept the same payload
/// shape and `Authorization: Bearer` auth. The URL is the only difference.
fn build_openai_compat(
    url: &str,
    prompt: &str,
    system: Option<&str>,
    api_key: &str,
    model: &str,
    timeout: Duration,
) -> HttpRequest {
    // OpenAI/Mistral want the system prompt as an extra message with
    // `role: "system"` prepended to the user turn.
    let mut messages: Vec<Value> = Vec::new();
    if let Some(s) = system {
        messages.push(json!({"role": "system", "content": s}));
    }
    messages.push(json!({"role": "user", "content": prompt}));
    HttpRequest {
        url: url.to_string(),
        headers: vec![
            ("authorization".into(), format!("Bearer {api_key}")),
            ("content-type".into(), "application/json".into()),
        ],
        body: json!({
            "model": model,
            "messages": messages,
        }),
        timeout,
    }
}

fn build_gemini(
    prompt: &str,
    system: Option<&str>,
    api_key: &str,
    model: &str,
    timeout: Duration,
) -> HttpRequest {
    // Gemini puts the API key in the query string rather than a header,
    // and exposes the system prompt as `system_instruction`.
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent?key={api_key}"
    );
    let mut body = json!({
        "contents": [{"parts": [{"text": prompt}]}],
    });
    if let Some(s) = system {
        body["system_instruction"] = json!({"parts": [{"text": s}]});
    }
    HttpRequest {
        url,
        headers: vec![("content-type".into(), "application/json".into())],
        body,
        timeout,
    }
}

// ─── Response parsing ────────────────────────────────────────────────────

/// Result of parsing a raw HTTP outcome into a terminal state for the
/// report builder. Keeps parsing separate from IO so tests can exercise
/// it in isolation.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ApiResult {
    Success(String),
    Http { status: u16, message: String },
    Timeout,
    ConnectError(String),
    ParseError(String),
    Other(String),
}

fn parse_response(id: ProviderId, outcome: HttpOutcome) -> ApiResult {
    match outcome {
        HttpOutcome::Timeout => ApiResult::Timeout,
        HttpOutcome::ConnectError { message } => ApiResult::ConnectError(message),
        HttpOutcome::Other { message } => ApiResult::Other(message),
        HttpOutcome::Response { status, body } => {
            if !(200..300).contains(&status) {
                return ApiResult::Http {
                    status,
                    message: tail(&body, 400),
                };
            }
            let parsed: Value = match serde_json::from_str(&body) {
                Ok(v) => v,
                Err(e) => {
                    return ApiResult::ParseError(format!("body is not valid JSON: {e}"));
                }
            };
            match extract_text(id, &parsed) {
                Some(s) if !s.trim().is_empty() => ApiResult::Success(s.trim().to_string()),
                Some(_) => ApiResult::ParseError("provider returned empty text".into()),
                None => ApiResult::ParseError(format!(
                    "provider response did not contain the expected text path (got: {})",
                    tail(&parsed.to_string(), 200)
                )),
            }
        }
    }
}

fn extract_text(id: ProviderId, v: &Value) -> Option<String> {
    match id {
        ProviderId::AnthropicApi => v
            .get("content")?
            .get(0)?
            .get("text")?
            .as_str()
            .map(str::to_string),
        ProviderId::OpenaiApi | ProviderId::MistralApi => v
            .get("choices")?
            .get(0)?
            .get("message")?
            .get("content")?
            .as_str()
            .map(str::to_string),
        ProviderId::GeminiApi => v
            .get("candidates")?
            .get(0)?
            .get("content")?
            .get("parts")?
            .get(0)?
            .get("text")?
            .as_str()
            .map(str::to_string),
        ProviderId::ClaudeCli
        | ProviderId::GeminiCli
        | ProviderId::CursorCli
        | ProviderId::Opencode => None,
    }
}

// ─── Report translation ──────────────────────────────────────────────────

fn outcome_to_report(started: Instant, id: ProviderId, result: ApiResult) -> RunReport {
    let duration_ms = started.elapsed().as_millis() as u64;
    let id_str = id.as_str().to_string();

    let (ok, text, error) = match result {
        ApiResult::Success(s) => (true, Some(s), None),
        ApiResult::Http { status, message } => (
            false,
            None,
            Some(format!("{id_str}: HTTP {status}; body: {message}")),
        ),
        ApiResult::Timeout => (
            false,
            None,
            Some(format!("{id_str}: timed out after {duration_ms}ms")),
        ),
        ApiResult::ConnectError(m) => (false, None, Some(format!("{id_str}: connect error: {m}"))),
        ApiResult::ParseError(m) => (false, None, Some(format!("{id_str}: parse error: {m}"))),
        ApiResult::Other(m) => (false, None, Some(format!("{id_str}: {m}"))),
    };

    RunReport {
        schema_version: SCHEMA_VERSION,
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        ok,
        text,
        provider_used: Some(id_str),
        duration_ms,
        error,
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
        let safe_start = t
            .char_indices()
            .map(|(i, _)| i)
            .find(|&i| i >= start)
            .unwrap_or(t.len());
        format!("…{}", &t[safe_start..])
    }
}
