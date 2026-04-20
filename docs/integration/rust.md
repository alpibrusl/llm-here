# Integrating from Rust

Rust callers have two options:

1. **Subprocess.** Same JSON contract as [Python callers](python.md). Useful when you want strict isolation.
2. **Crate dependency.** Link `llm-here-core` directly — same detection + dispatch, no subprocess overhead, typed errors, mock-friendly.

Most Rust consumers pick option 2. Noether-engine did.

## Add the dependency

While pre-release, pin to a tag via git:

```toml
[dependencies]
llm-here-core = { git = "https://github.com/alpibrusl/llm-here", tag = "v0.4.0" }
```

Once published to crates.io, switch to a version spec:

```toml
llm-here-core = "0.4"
```

!!! note "reqwest brings rustls + tokio"
    `llm-here-core` depends on `reqwest` (blocking + rustls-tls) for API dispatch. If your crate already uses reqwest, it's a no-cost addition; if not, it's about ~1.5 minutes of cold compile time. Feature-gate if that's a problem.

## Detection

```rust
use llm_here_core::detect;

let report = detect();
println!("Schema version: {}", report.schema_version);
for provider in &report.providers {
    println!(
        "  {} ({}): default model = {}",
        provider.id, provider.provider, provider.model_default
    );
}
```

`detect()` reads the real environment. For deterministic tests, use `detect_with_env(&fake_env)` — see [testing](#testing).

## Dispatch

### Single provider

```rust
use llm_here_core::dispatch::{run_cli_provider_real, DispatchOptions};
use llm_here_core::providers::ProviderId;

let opts = DispatchOptions::default();  // 25 s timeout, no dangerous-claude, default model
let report = run_cli_provider_real(ProviderId::ClaudeCli, "What's 2+2?", &opts);

if report.ok {
    println!("{}", report.text.unwrap_or_default());
} else {
    eprintln!("dispatch failed: {}", report.error.unwrap_or_default());
}
```

For API providers:

```rust
use llm_here_core::api::run_api_provider_real;

let report = run_api_provider_real(ProviderId::AnthropicApi, "hi", &opts);
```

### Auto-chain

```rust
use llm_here_core::dispatch::run_auto_real;

let report = run_auto_real("hello", &opts);
// Tries every reachable CLI provider (unless *_SKIP_CLI is set),
// then every API provider whose auth env var is set. Returns on
// first success.
```

### Customising the dispatch

```rust
use std::time::Duration;
use llm_here_core::dispatch::DispatchOptions;

let opts = DispatchOptions {
    timeout: Duration::from_secs(60),
    dangerous_claude: true,  // caller-owned opt-in
    model: Some("claude-opus-4-1".into()),
    system_prompt: Some("Be terse.".into()),
};
```

- `timeout`: wall-clock; enforced via `wait-timeout` for CLIs and `reqwest` for APIs.
- `dangerous_claude`: applies to `ProviderId::ClaudeCli` only. Ignored elsewhere.
- `model`: forwarded to APIs (always) and to claude/gemini/cursor CLIs (as `--model <name>`). Ignored by opencode.
- `system_prompt`: routed through each provider's native channel. Ignored by gemini/cursor/opencode CLIs.

## Testing

The three boundaries — env, subprocess, HTTP — are all trait-abstracted:

| Trait | Default impl | Used in |
|---|---|---|
| `Env` | `RealEnv` (reads `std::env::var`, `which::which`) | detection |
| `CommandRunner` | `RealCommandRunner` (spawns via `std::process::Command`) | CLI dispatch |
| `HttpClient` | `RealHttpClient` (reqwest::blocking + rustls) | API dispatch |

Tests inject fakes:

```rust
use llm_here_core::{
    dispatch::{run_auto, CommandRunner, DispatchOptions, DispatchOutcome, DispatchRequest},
    env::Env,
    api::{HttpClient, HttpOutcome, HttpRequest},
};
use std::path::PathBuf;

struct FakeEnv { /* ... */ }
impl Env for FakeEnv {
    fn var(&self, key: &str) -> Option<String> { /* ... */ }
    fn which(&self, binary: &str) -> Option<PathBuf> { /* ... */ }
}

struct FakeRunner;
impl CommandRunner for FakeRunner {
    fn run(&self, _req: DispatchRequest) -> DispatchOutcome {
        DispatchOutcome::Success { stdout: "canned response".into() }
    }
}

struct FakeHttp;
impl HttpClient for FakeHttp {
    fn post_json(&self, _req: HttpRequest) -> HttpOutcome {
        HttpOutcome::Response {
            status: 200,
            body: r#"{"content":[{"text":"canned"}]}"#.into(),
        }
    }
}

#[test]
fn auto_picks_first_reachable_cli() {
    let env: FakeEnv = /* reachable claude only */;
    let report = run_auto("hi", &DispatchOptions::default(), &env, &FakeRunner, &FakeHttp);
    assert!(report.ok);
    assert_eq!(report.provider_used.as_deref(), Some("claude-cli"));
}
```

This is the exact pattern `noether-engine::llm::cli_provider` uses in its test suite.

## Real-world examples

- [**noether-engine** `crates/noether-engine/src/llm/cli_provider.rs`](https://github.com/alpibrusl/noether/blob/main/crates/noether-engine/src/llm/cli_provider.rs) — maps `noether`'s `Message` / `Role` into `llm-here-core`'s single-prompt dispatch.
- [**llm-here-core** tests](https://github.com/alpibrusl/llm-here/blob/main/crates/llm-here-core/tests/) — `dispatch.rs` and `api.rs` use the same fake traits against the core directly.

## See also

- [From Python](python.md) — subprocess-based integration for non-Rust callers.
- [Consumer projects](consumers.md) — how each sibling project wires up.
- [JSON wire format](../reference/schema.md) — the structs that serialise into the subprocess output.
