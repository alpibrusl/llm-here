# llm-here JSON wire schema

This document defines the **stable** JSON output shape of `llm-here`. Callers in any language depend on this. Changes follow semver rules for wire compatibility:

- **Minor bump** (`0.x.y` → `0.x+1.y`): additive. New optional field, new allowed value in an enum, new provider id.
- **Major bump** (`0.x.y` → `x+1.0.0`): breaking. Removing a field, renaming a field, tightening a value's domain.

The current schema version is **1**. Every payload includes `schema_version` so consumers can gate on it.

## Command: `llm-here detect`

Exit code: `0` on success; `2` on internal failure.

### Payload

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
    }
  ]
}
```

### Fields

| Field | Type | Notes |
|---|---|---|
| `schema_version` | integer | This document's version. Currently `1`. |
| `tool_version` | string | Semver of the `llm-here` binary that produced the payload. |
| `cli_detection_skipped` | boolean | `true` when any of the `*_SKIP_CLI` env vars is truthy. |
| `providers` | array of `DetectedProvider` | Reachable providers in fallback order. |

### `DetectedProvider`

| Field | Type | Notes |
|---|---|---|
| `id` | string (enum) | One of the ids listed below. |
| `kind` | string | `"cli"` or `"api"`. |
| `provider` | string | Human-readable provider ("anthropic", "openai", …). |
| `model_default` | string | Default model identifier. |
| `binary` | string (optional) | Absolute path to the binary. Present iff `kind == "cli"`. |
| `env` | string (optional) | Name of the env var holding the API key. Present iff `kind == "api"`. Never contains the key value itself. |

## Command: `llm-here run`

Reads prompt from stdin. Exit code: `0` on success; `1` when a provider was attempted but failed or the command has no valid target; `2` on internal failure.

### Payload

```json
{
  "schema_version": 1,
  "tool_version": "0.2.0",
  "ok": true,
  "text": "Here's a joke about Rust ...",
  "provider_used": "claude-cli",
  "duration_ms": 1834,
  "error": null
}
```

### Fields

| Field | Type | Notes |
|---|---|---|
| `schema_version` | integer | Same as above. |
| `tool_version` | string | Same as above. |
| `ok` | boolean | `true` iff a provider returned non-empty text before the timeout. |
| `text` | string \| null | Model output (trimmed). `null` iff `ok == false`. |
| `provider_used` | string \| null | Which provider actually produced the text. May be populated on failure paths too (indicates which provider was attempted last). `null` when no provider was attempted (e.g. empty prompt, missing target flag). |
| `duration_ms` | integer | Wall time spent in the dispatch. |
| `error` | string \| null | Human-readable error message. `null` iff `ok == true`. |

### Flags

| Flag | Purpose |
|---|---|
| `--provider <id>` | Dispatch to a specific provider. Mutually exclusive with `--auto`. |
| `--auto` | Try each reachable CLI provider in REGISTRY order; first success wins. |
| `--timeout <secs>` | Wall-clock timeout for the subprocess. Default `25`. |
| `--dangerous-claude` | Passes `--dangerously-skip-permissions` to `claude`. Off by default; caller-owned opt-in. |

## Provider id registry

The `id` enum values. Adding to this list is a minor version bump; removing is major.

| id | kind |
|---|---|
| `claude-cli` | cli |
| `gemini-cli` | cli |
| `cursor-cli` | cli |
| `opencode` | cli |
| `anthropic-api` | api |
| `openai-api` | api |
| `gemini-api` | api |
| `mistral-api` | api |

## Compatibility guidance for callers

1. **Check `schema_version`** before parsing the rest of the payload. If you don't recognise it, emit a warning and fall back to text output (or abort, your choice).
2. **Treat `id` as opaque** unless you know about a specific value. Never crash on an unknown `id`.
3. **Do not assume `providers` is sorted alphabetically.** Order is fallback order.
4. **`binary` and `env` are mutually exclusive** per provider. Don't assume both or neither.
5. **The API key value is never in the payload.** Only the env var name.
