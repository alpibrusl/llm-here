# Environment variables

`llm-here` reads a small, stable set of environment variables. API keys are read when dispatching; `*_SKIP_CLI` aliases are read at detect and dispatch time.

## API authentication

| Variable | Provider |
|---|---|
| `ANTHROPIC_API_KEY` | `anthropic-api` |
| `OPENAI_API_KEY` | `openai-api` |
| `GOOGLE_API_KEY` | `gemini-api` |
| `MISTRAL_API_KEY` | `mistral-api` |

**Empty strings don't count as set.** `GOOGLE_API_KEY=""` is equivalent to unset.

!!! warning "Key values never appear in output"
    `detect` reports only the env var name. `run` reports errors that never include the key value. This is regression-tested.

## Sandbox signals — skip CLI probing

When set to a truthy value (`1` / `true` / `yes` / `on`), any of these causes `detect` and `run --auto` to skip CLI providers entirely:

- `LLM_HERE_SKIP_CLI`
- `NOETHER_LLM_SKIP_CLI`
- `CALORON_LLM_SKIP_CLI`
- `AGENTSPEC_LLM_SKIP_CLI`

Three caller-specific aliases exist so each sibling project can keep its existing convention unchanged when it migrates to `llm-here`. **Any one** of them set to truthy triggers the skip.

### When you'd set these

- **Nix-executor stages in noether-grid.** The sandbox doesn't mount the operator's CLI auth state, so a subscription CLI stalls waiting for interactive login. The default 25 s timeout kicks in, but bypassing CLIs entirely saves the whole round trip.
- **Agentspec runners in locked-down environments.** The manifest might prefer a claude CLI but the sandbox only exposes API keys.
- **CI.** No subscription CLIs are installed; short-circuiting the PATH lookups saves milliseconds per invocation.

APIs (keyed dispatch) still run when a `*_SKIP_CLI` variable is set — this is a **CLI skip**, not an all-providers skip.

## What `llm-here` deliberately does NOT read

For everything else, flags win. `llm-here` never reads ambient environment for dispatch policy:

| Behaviour | Mechanism |
|---|---|
| Pass `--dangerously-skip-permissions` to claude | `--dangerous-claude` flag only. Not `CALORON_ALLOW_DANGEROUS_CLAUDE` or similar. |
| Override model | `--model <name>` flag only. No `LLM_HERE_MODEL` or per-provider env vars. |
| Override system prompt | `--system-prompt <text>` flag only. |
| Override timeout | `--timeout <secs>` flag only. |

This keeps dispatch **caller-owned**: when a consumer project wants to forward its own env-var gates (e.g. caloron's `CALORON_ALLOW_DANGEROUS_CLAUDE`), it reads the env var in its own code and decides whether to pass the flag, rather than having `llm-here` silently opt in.

## Examples

### Skip CLIs in a Nix stage

```bash
export NOETHER_LLM_SKIP_CLI=1
llm-here detect
# → {"cli_detection_skipped": true, "providers": [/* only APIs */]}
```

### Mix an API key + a local CLI

```bash
export ANTHROPIC_API_KEY=sk-…
llm-here detect
# → Reports both claude-cli (on PATH) AND anthropic-api (env set).
# `--auto` tries claude-cli first; if it fails, falls through to anthropic-api.
```

### Force API dispatch even when a CLI is installed

No direct env-var knob for this; use `--provider <api-id>` explicitly:

```bash
export OPENAI_API_KEY=sk-…
echo "hi" | llm-here run --provider openai-api
```

Or skip CLIs globally:

```bash
LLM_HERE_SKIP_CLI=1 echo "hi" | llm-here run --auto
```

## See also

- [Commands: `detect`](../commands/detect.md)
- [Commands: `run`](../commands/run.md)
- [Consumer projects](../integration/consumers.md) — how each caller uses these env vars.
