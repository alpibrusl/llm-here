# Security

`llm-here` is intentionally narrow — it detects and dispatches, nothing more — so the security surface is correspondingly small. This document lists what the tool is and isn't responsible for, and how to report issues.

## Responsibility boundary

### llm-here IS responsible for

- **Not leaking API keys in detect output.** `detect` reports env var *names* (e.g. `ANTHROPIC_API_KEY`), never values. Regression tests enforce this.
- **Honouring sandbox skip signals.** `LLM_HERE_SKIP_CLI`, `NOETHER_LLM_SKIP_CLI`, `CALORON_LLM_SKIP_CLI`, `AGENTSPEC_LLM_SKIP_CLI`. When set, CLI probing is skipped. Used by callers that run inside Nix executors or sandboxes where binary calls stall.
- **Bounded timeouts on CLI invocations** (v0.2+). Capped at 25 seconds by default to stay under the Noether 30-second stage kill and to prevent process leaks.
- **Exit code discipline.** Machine-readable: `0` success, `1` invoked-but-failed, `2` internal failure. Callers can gate on exit code without parsing JSON.

### llm-here is NOT responsible for

- **API key provisioning or rotation.** Callers manage the lifecycle of their own credentials.
- **Network TLS verification beyond `rustls` defaults.** We use `reqwest` with `rustls` (v0.3+) and trust the system root store. Consumers with pinned-cert requirements should layer that above `llm-here`.
- **Sandboxing the dispatch target.** Launching a CLI binary inherits the caller's privileges. If you need isolation around the CLI call, use `noether-sandbox` or `bubblewrap` around the `llm-here run` invocation itself.
- **Prompt content safety.** Prompts go to the model verbatim. If you need filtering, do it upstream.

## Reporting vulnerabilities

For security issues that should not be public:

- Email: **alfonso@elumobility.com** with `[llm-here security]` in the subject.
- Expect an initial acknowledgement within 7 days.
- Coordinated disclosure preferred; CVE requests welcome.

For non-sensitive bugs (crashes, spec drift, wrong detection logic), open a GitHub issue normally.

## Supported versions

While pre-1.0, only the latest released minor receives security fixes. The stability contract firms up at v1.0 — track `SCHEMA.md` for wire-format compatibility.

## Threat model (short)

Callers drive `llm-here` either as a subprocess (Python, shell) or as a library dependency (Rust via `llm-here-core`). Trust assumptions:

- The caller has already decided `llm-here` is trusted — we're not defending against a malicious host invoking a malicious local binary.
- The environment the caller provides (`PATH`, env vars) is trusted by the caller. `llm-here` reads them; it doesn't validate that they point at legitimate binaries.
- Output is structured JSON; callers parse it. We don't defend against malicious consumers of our output.

What we *do* actively defend against:

- Accidental API key leakage in output (never include values).
- Hang / resource-leak on CLI subprocess calls (timeouts, v0.2+).
- Partial writes mid-serialisation confusing parsers (output is buffered and flushed atomically).
