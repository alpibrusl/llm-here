# `detect`

Probes each provider in the registry and reports which ones are reachable on this host.

## Usage

```bash
llm-here detect
```

No flags, no stdin. Always exits `0` (or `2` on an internal serialisation error).

## What it probes

| Kind | Check |
|---|---|
| CLI | `which <binary>` — does the binary resolve on `PATH`? |
| API | Is the auth env var set to a non-empty value? |

**CLI probing is skipped** when any of these env vars is truthy (`1`, `true`, `yes`, `on`):

- `LLM_HERE_SKIP_CLI`
- `NOETHER_LLM_SKIP_CLI`
- `CALORON_LLM_SKIP_CLI`
- `AGENTSPEC_LLM_SKIP_CLI`

Three caller-specific aliases exist so each consumer project can keep its existing convention. See [environment variables](../reference/env-vars.md) for the full list.

## Output

```json
{
  "schema_version": 1,
  "tool_version": "0.4.0",
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

| Field | Type | Notes |
|---|---|---|
| `schema_version` | int | Currently `1`. Bumped on breaking changes. |
| `tool_version` | string | Semver of the `llm-here` binary. |
| `cli_detection_skipped` | bool | `true` when any `*_SKIP_CLI` env is set. |
| `providers[]` | list | Reachable providers in fallback order. |
| `providers[].id` | string | Stable provider id. See [providers](../reference/providers.md). |
| `providers[].kind` | string | `"cli"` or `"api"`. |
| `providers[].provider` | string | Human name (`anthropic`, `openai`, `google`, `mistral`, `cursor`, `opencode`). |
| `providers[].model_default` | string | Default model used when `--model` is not specified. |
| `providers[].binary` | string? | Absolute binary path. Present iff `kind == "cli"`. |
| `providers[].env` | string? | Auth env var name. Present iff `kind == "api"`. **Never the value.** |

## Examples

**Nothing installed, no keys set:**

```json
{"schema_version": 1, "tool_version": "0.4.0",
 "cli_detection_skipped": false, "providers": []}
```

**Sandboxed with API key:**

```bash
LLM_HERE_SKIP_CLI=1 llm-here detect
```

```json
{
  "schema_version": 1,
  "tool_version": "0.4.0",
  "cli_detection_skipped": true,
  "providers": [
    {"id": "anthropic-api", "kind": "api", "provider": "anthropic",
     "model_default": "claude-sonnet-4-5", "env": "ANTHROPIC_API_KEY"}
  ]
}
```

## Common uses

- **Readiness gates.** CI jobs check that at least one provider is reachable before running an LLM-dependent stage.
- **Deployment diagnostics.** When a stage fails with "no providers reachable", run `llm-here detect` on the host to see what's actually present.
- **Caller dispatch.** Libraries like agentspec use `detect` to decide which runtime to provision for a manifest.

See also: [`run`](run.md), [providers reference](../reference/providers.md).
