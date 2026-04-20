# llm-here

> One tool for **"which LLM is reachable from this host, and how do I run a prompt through it?"**

[![CI](https://github.com/alpibrusl/llm-here/actions/workflows/ci.yml/badge.svg)](https://github.com/alpibrusl/llm-here/actions/workflows/ci.yml)
[![License: EUPL-1.2](https://img.shields.io/badge/license-EUPL--1.2-blue)](LICENSE)

Four subscription CLIs (`claude`, `gemini`, `cursor-agent`, `opencode`) and four API providers (Anthropic, OpenAI, Gemini, Mistral), all behind one JSON-in / JSON-out binary. Callers in any language talk to it as a subprocess; Rust callers can depend on the `llm-here-core` crate directly.

📖 **Docs:** [alpibrusl.github.io/llm-here](https://alpibrusl.github.io/llm-here/) — guides, reference, and integration examples.

## Why

Three sibling projects ([caloron-noether](https://github.com/alpibrusl/caloron-noether), [agentspec](https://github.com/alpibrusl/agentspec), [noether](https://github.com/alpibrusl/noether)) each re-implemented provider detection + dispatch and had already drifted by the time anyone noticed:

- caloron discovered the 25-second-under-Nix-30-second timeout cap months before noether-grid did.
- agentspec handles Vertex AI routing; the other two don't.
- Each project's CLI argv shapes and env-var conventions had forked in small, annoying ways.

`llm-here` is the consolidation. One implementation, one wire format, shared across every caller. See [`noether/docs/research/llm-here.md`](https://github.com/alpibrusl/noether/blob/main/docs/research/llm-here.md) for the motivating design note.

## Install

```bash
cargo install --git https://github.com/alpibrusl/llm-here llm-here
```

(Once published: `cargo install llm-here`.)

## 30-second tour

**1. See what's reachable:**

```bash
llm-here detect
```

```json
{
  "schema_version": 1,
  "tool_version": "0.4.0",
  "cli_detection_skipped": false,
  "providers": [
    {"id": "claude-cli", "kind": "cli", "provider": "anthropic",
     "model_default": "claude-desktop", "binary": "/usr/local/bin/claude"},
    {"id": "anthropic-api", "kind": "api", "provider": "anthropic",
     "model_default": "claude-sonnet-4-5", "env": "ANTHROPIC_API_KEY"}
  ]
}
```

**2. Run a prompt through a specific provider:**

```bash
echo "What's 2+2? One word." | llm-here run --provider claude-cli --timeout 25
# → {"ok": true, "text": "Four", "provider_used": "claude-cli", "duration_ms": 1834, …}
```

**3. Or let it pick:**

```bash
echo "hi" | llm-here run --auto
# CLIs first, then APIs, in the caloron-settled fallback order. First success wins.
```

Exit code `0` on success, `1` when a provider was attempted but failed, `2` on internal error. **Stdout is always valid JSON, regardless of outcome.**

## Commands

### `detect`

Probes each entry in the registry:

- CLI providers → look up the binary on `PATH`.
- API providers → check if the auth env var is set (the value is **never** included in the output — only the name).

Skip CLI probing inside sandboxes by setting any of `LLM_HERE_SKIP_CLI`, `NOETHER_LLM_SKIP_CLI`, `CALORON_LLM_SKIP_CLI`, or `AGENTSPEC_LLM_SKIP_CLI` to `1` / `true` / `yes` / `on`. Three aliases exist so each caller project can keep its current convention.

### `run`

Prompt is read from **stdin**. One flag picks the target:

| Flag | Notes |
|---|---|
| `--provider <id>` | One of the ids from `llm-here detect`. Mutually exclusive with `--auto`. |
| `--auto` | Try every reachable provider (CLIs first, then APIs) in REGISTRY order until one succeeds. |

Optional knobs:

| Flag | Default | Notes |
|---|---|---|
| `--timeout <secs>` | `25` | Wall-clock. Applies to both subprocess (CLI) and HTTP (API) calls. 25 s stays under Noether's 30 s stage kill. |
| `--model <name>` | per-provider default | For APIs: applied unconditionally. For claude/gemini/cursor CLIs: emitted as `--model <name>`. Ignored by opencode. |
| `--system-prompt <text>` | — | For claude: `--append-system-prompt <text>`. For APIs: native channel (Anthropic `system`, OpenAI/Mistral `role: system` message, Gemini `system_instruction`). Ignored by gemini/cursor/opencode CLIs. |
| `--dangerous-claude` | off | Passes `--dangerously-skip-permissions` to `claude`. Caller-owned policy — llm-here reads no ambient env for this. |

See the [command reference](https://alpibrusl.github.io/llm-here/commands/) for full details.

## Supported providers

| id | kind | binary / env | default model |
|---|---|---|---|
| `claude-cli` | cli | `claude` | `claude-desktop` |
| `gemini-cli` | cli | `gemini` | `gemini-desktop` |
| `cursor-cli` | cli | `cursor-agent` | `cursor-desktop` |
| `opencode` | cli | `opencode` | `opencode-desktop` |
| `anthropic-api` | api | `ANTHROPIC_API_KEY` | `claude-sonnet-4-5` |
| `openai-api` | api | `OPENAI_API_KEY` | `gpt-4o` |
| `gemini-api` | api | `GOOGLE_API_KEY` | `gemini-1.5-pro` |
| `mistral-api` | api | `MISTRAL_API_KEY` | `mistral-large-latest` |

Registry order defines the default `--auto` fallback chain (CLIs first, then APIs).

## Using `llm-here` from other languages

### Python (agentspec, caloron-noether)

```python
import json, subprocess

def call_llm(prompt: str, timeout: int = 30) -> str | None:
    argv = ["llm-here", "run", "--auto", "--timeout", str(timeout)]
    try:
        r = subprocess.run(argv, input=prompt, capture_output=True,
                           text=True, timeout=timeout + 5)
    except (subprocess.TimeoutExpired, FileNotFoundError, OSError):
        return None
    if r.returncode != 0:
        return None
    out = json.loads(r.stdout)
    return out["text"] if out.get("ok") else None
```

### Rust (noether-engine grid)

Depend on `llm-here-core` directly — no subprocess needed:

```toml
[dependencies]
llm-here-core = { git = "https://github.com/alpibrusl/llm-here", tag = "v0.4.0" }
```

```rust
use llm_here_core::dispatch::{run_auto_real, DispatchOptions};

let opts = DispatchOptions::default();
let report = run_auto_real("hello", &opts);
if report.ok {
    println!("{}", report.text.unwrap());
}
```

Rust consumers can also mock `CommandRunner` / `HttpClient` / `Env` traits for deterministic tests.

## What's in scope, and what isn't

**In scope.** Detection and single-shot dispatch. Subprocess for CLIs, HTTPS for APIs. Timeouts. Sandbox-skip aliases. Stable JSON wire format. A Rust crate for callers that want to skip the subprocess boundary.

**Not in scope.** These belong in the caller:

- **State.** Every invocation is independent. No conversation history, no caching, no session tokens.
- **Cost accounting.** Callers own their own cost ledger.
- **Streaming.** Single prompt → single completion.
- **Agent-loop semantics.** No tool use, no multi-turn orchestration.
- **Vertex AI routing.** Stays in [agentspec's resolver](https://github.com/alpibrusl/agentspec/blob/main/src/agentspec/resolver/vertex.py); it's a selection concern, not a detection concern.

## Status

**v0.4** — feature-complete for the three-project consolidation.

| Capability | v0.1 | v0.2 | v0.3 | v0.4 |
|---|---|---|---|---|
| `llm-here detect` | ✅ | ✅ | ✅ | ✅ |
| `llm-here run --provider <cli-id>` | stub | ✅ | ✅ | ✅ |
| `llm-here run --provider <api-id>` | stub | stub | ✅ | ✅ |
| `llm-here run --auto` (CLIs) | stub | ✅ | ✅ | ✅ |
| `llm-here run --auto` (CLIs → APIs) | stub | partial | ✅ | ✅ |
| `--model <name>` on APIs | — | — | ✅ | ✅ |
| `--model <name>` on CLIs | — | — | — | ✅ |
| `--system-prompt <text>` | — | — | — | ✅ |

See [CHANGELOG.md](CHANGELOG.md) for the full history and [the docs roadmap page](https://alpibrusl.github.io/llm-here/roadmap/) for where it's heading.

## Security

- **API key values never appear in output.** Only env var names are reported.
- **Per-call timeouts** with child-kill on expiry (no zombie processes).
- **No ambient env-var reads for tool-use permissions.** `--dangerous-claude` is per-invocation; llm-here does not silently opt in.

Report security issues to `alfonso@elumobility.com` with `[llm-here security]` in the subject. Full policy in [SECURITY.md](SECURITY.md).

## Related projects

- [**alpibrusl/noether**](https://github.com/alpibrusl/noether) — verified composition platform. First consumer; `noether-engine::llm::cli_provider` delegates here.
- [**alpibrusl/agentspec**](https://github.com/alpibrusl/agentspec) — agent manifest spec. Resolver detection delegates here.
- [**alpibrusl/caloron-noether**](https://github.com/alpibrusl/caloron-noether) — reference application for Noether. `_llm.call_llm` dispatches here.

## License

EUPL-1.2. See [LICENSE](LICENSE).
