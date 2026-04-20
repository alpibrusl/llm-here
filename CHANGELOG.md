# Changelog

All notable changes to `llm-here` are documented here. Format based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); wire-format changes follow the semver rules in `SCHEMA.md`.

## [0.3.0] - 2026-04-20

### Added

- HTTPS dispatch to four API providers: `anthropic-api`, `openai-api`, `gemini-api`, `mistral-api`. Each has a stable JSON request shape derived from its public API docs.
- `--model <name>` CLI flag to override the default model per API dispatch. Ignored by CLI providers. No ambient env is read; callers own the policy.
- `run --auto` now walks APIs after CLIs in REGISTRY order — full fallback chain.
- `HttpClient` trait + `RealHttpClient` (built on `reqwest::blocking` with `rustls-tls`) in `llm-here-core::api`. Downstream Rust crates can mock the HTTP boundary the same way `CommandRunner` lets them mock subprocess dispatch.
- `DispatchOptions.model: Option<String>` field threaded through `run_auto`, `run_cli_provider`, and `run_api_provider`.
- 15 new API-dispatch tests via a `FakeHttpClient` — verified per-provider URL, auth header, model, request-body shape, non-2xx handling, timeout, connect errors, non-JSON body, unexpected JSON shape, and API-key non-leakage in error paths.

### Changed

- `run_auto` signature adds a third generic parameter `H: HttpClient` so callers can provide an injected HTTP client. Existing Rust consumers must update call sites; `run_auto_real` wires real implementations for convenience.
- `run_cli_provider` error message for API-id input no longer references "v0.3"; instead points callers to `run_api_provider`.
- `DispatchOptions::default()` now includes `model: None`.

### Fixed

- `RealHttpClient` serialises request bodies explicitly via `serde_json::to_vec` instead of `reqwest::RequestBuilder::json`, avoiding a duplicate-content-type interaction that caused Mistral to reject bodies as JSON strings instead of objects.

### Wire format

No breaking changes. `schema_version` stays at `1`. Response shapes are unchanged; `provider_used` remains populated on failure paths, now including API branches.

### Security

- API key values are never included in output payloads, detect output, or error messages. Regression test `anthropic_api_key_never_appears_in_error_message` enforces this.
- Request timeouts apply at the HTTP boundary via `reqwest`'s per-call timeout.

## [0.2.0] - 2026-04-20

### Added

- `llm-here run --provider <cli-id>` dispatches via subprocess with wall-clock timeout (default 25 s).
- `llm-here run --auto` walks reachable CLI providers in REGISTRY order, returning the first success. Short-circuits if any of `LLM_HERE_SKIP_CLI` / `NOETHER_LLM_SKIP_CLI` / `CALORON_LLM_SKIP_CLI` / `AGENTSPEC_LLM_SKIP_CLI` is truthy.
- `--dangerous-claude` flag to pass `--dangerously-skip-permissions` to `claude`. Caller-owned policy — no ambient env gate.
- `llm-here-core::dispatch` module with `CommandRunner` trait and `RealCommandRunner` impl. Callers can mock for tests.
- Prompt is read from stdin; argv templates derived from each CLI's `-p` invocation.
- 20 new dispatch tests via a `FakeCommandRunner` and `FakeEnv` — 31 total tests.

### Changed

- `llm-here run` now reads stdin instead of returning an "unimplemented" stub.
- `llm-here-core` description updated to mention dispatch.

### Wire format

No breaking changes. `RunReport.provider_used` is now populated on failure paths for the CLI branches; previously always `null` in the stub. This is a strict widening — existing v0.1 consumers that tolerate nullable values stay compatible. `schema_version` stays at 1.

### Notes

- API providers (`anthropic-api`, `openai-api`, `gemini-api`, `mistral-api`) still return a typed v0.3 stub error from `run --provider`. `run --auto` skips them entirely.
- Timeout enforcement uses `wait-timeout = 0.2` and always kills the child on expiry to prevent zombie processes.

## [0.1.0] - 2026-04-20

### Added

- Initial scaffold.
- `llm-here detect` reports reachable providers (4 CLIs on PATH + 4 APIs with env keys set) as JSON.
- `llm-here-core` library crate: `detect()`, `Env` trait, `REGISTRY`, wire-format types.
- `SCHEMA.md` defining the stable JSON wire contract.
- `SECURITY.md` stating the responsibility boundary.
- CI: fmt + clippy (-D warnings) + test + release-build smoke.
- EUPL-1.2 license.
