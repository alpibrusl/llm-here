# llm-here

> One tool for *"which LLM is reachable from this host, and how do I run a prompt through it?"*

`llm-here` is a single-purpose binary (plus a reusable Rust crate) that answers the question three sibling projects keep re-answering independently:

- Is `claude` / `gemini` / `cursor-agent` / `opencode` on `PATH`?
- Do I have `ANTHROPIC_API_KEY` / `OPENAI_API_KEY` / `GOOGLE_API_KEY` / `MISTRAL_API_KEY` set?
- If multiple reachable, which do I prefer?

Callers in any language talk to `llm-here` as a subprocess and read JSON on stdout. Rust callers can depend on the `llm-here-core` crate directly.

This project exists because the same detection logic had drifted across three codebases (see `docs/research/llm-here.md` in the `alpibrusl/noether` repo). Consolidating it here deletes duplicated code and eliminates a known class of drift bug.

## Status

**v0.1** — `detect` works. `run` is stubbed and returns a machine-readable error. See [Roadmap](#roadmap) below.

| Command | v0.1 | v0.2 | v0.3 |
|---|---|---|---|
| `llm-here detect` | ✅ | ✅ | ✅ |
| `llm-here run --provider <id>` (CLI providers) | stub | ✅ | ✅ |
| `llm-here run --provider <id>` (API providers) | stub | stub | ✅ |
| `llm-here run --auto` | stub | ✅ | ✅ |

## Install

```bash
# From source (while pre-release)
cargo install --git https://github.com/alpibrusl/llm-here llm-here
```

Once published:

```bash
cargo install llm-here
```

## Usage

### `detect`

```bash
llm-here detect
```

Output (JSON):

```json
{
  "schema_version": 1,
  "tool_version": "0.1.0",
  "cli_detection_skipped": false,
  "providers": [
    {
      "id": "claude-cli",
      "kind": "cli",
      "provider": "anthropic",
      "model_default": "claude-desktop",
      "binary": "/usr/local/bin/claude"
    },
    {
      "id": "anthropic-api",
      "kind": "api",
      "provider": "anthropic",
      "model_default": "claude-sonnet-4-5",
      "env": "ANTHROPIC_API_KEY"
    }
  ]
}
```

The API key value is **never** included in the output — only the env var name.

### `run` (not yet implemented)

The interface is carved out in v0.1 so callers can code against a stable error shape:

```json
{
  "schema_version": 1,
  "tool_version": "0.1.0",
  "ok": false,
  "text": null,
  "provider_used": null,
  "duration_ms": 0,
  "error": "run is not implemented in v0.1 ..."
}
```

Exit code: `1` on failure, `0` on success. Stdout is always valid JSON regardless of outcome.

## Sandbox detection

When running inside a sandbox where CLI binaries would stall (no auth state, no network access, no XDG state), set any of these env vars to skip CLI probing:

- `LLM_HERE_SKIP_CLI=1`
- `NOETHER_LLM_SKIP_CLI=1`
- `CALORON_LLM_SKIP_CLI=1`
- `AGENTSPEC_LLM_SKIP_CLI=1`

Any of them set to `1`, `true`, `yes`, or `on` triggers the skip. Three aliases exist so each caller project can keep its current convention.

## Supported providers

| id | kind | binary / env | provider | default model |
|---|---|---|---|---|
| `claude-cli` | cli | `claude` | anthropic | `claude-desktop` |
| `gemini-cli` | cli | `gemini` | google | `gemini-desktop` |
| `cursor-cli` | cli | `cursor-agent` | cursor | `cursor-desktop` |
| `opencode` | cli | `opencode` | opencode | `opencode-desktop` |
| `anthropic-api` | api | `ANTHROPIC_API_KEY` | anthropic | `claude-sonnet-4-5` |
| `openai-api` | api | `OPENAI_API_KEY` | openai | `gpt-4o` |
| `gemini-api` | api | `GOOGLE_API_KEY` | google | `gemini-1.5-pro` |
| `mistral-api` | api | `MISTRAL_API_KEY` | mistral | `mistral-large-latest` |

Registry order defines the default `--auto` fallback chain (CLIs first, then APIs).

## JSON wire format

See `SCHEMA.md` for the stable contract. Schema is semver'd independently of the binary version: additive changes (new field, new provider id) are minor bumps; removing or renaming fields is a major bump.

## Usage from other languages

### Python (agentspec, caloron-noether)

```python
import json, subprocess

def detect_providers() -> list[dict]:
    out = subprocess.check_output(["llm-here", "detect"], text=True)
    return json.loads(out)["providers"]
```

### Rust (noether-engine grid)

```rust
use llm_here_core::detect;

let report = detect();
for p in &report.providers {
    println!("{}: {:?}", p.id, p.binary);
}
```

## Roadmap

- **v0.1** ✅ — `detect` works; `run` returns a stable error shape.
- **v0.2** — `run --provider <cli-id>` dispatches via subprocess; `run --auto` chains through reachable CLI providers. Timeout capped at 25s by default (see caloron's Nix-under-30s lesson).
- **v0.3** — API providers via HTTPS (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GOOGLE_API_KEY`, `MISTRAL_API_KEY`). Full fallback chain.
- **v0.4** — feature-parity migration of noether-grid, agentspec resolver detection, caloron `_llm.py`. Cross-repo regression fixtures.

## Explicitly not in scope

- **State.** Each invocation is independent. No caching, session tokens, or conversation history.
- **Cost accounting.** Callers own their own cost ledger.
- **Streaming.** Single prompt → single completion. Streaming belongs in the caller.
- **Agent-loop semantics.** No tool use, no multi-turn orchestration.
- **Vertex AI routing.** Stays in agentspec's resolver; it's a runtime-selection concern, not a detection concern.

## License

EUPL-1.2. See `LICENSE`.
