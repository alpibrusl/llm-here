# Changelog

All notable changes to `llm-here` are documented here. Format based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); wire-format changes follow the semver rules in `SCHEMA.md`.

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
