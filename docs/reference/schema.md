# JSON wire format

`llm-here` emits JSON on stdout. Callers in any language depend on this shape; it's **semver'd independently of the binary version**.

- **Minor bump** (e.g. `0.x.y` → `0.x+1.y`): additive. New optional field, new allowed value in an enum, new provider id in the registry.
- **Major bump** (e.g. `0.x.y` → `x+1.0.0`): breaking. Removing a field, renaming a field, tightening a value's domain, changing a type.

The current `schema_version` is **1**. Every payload includes it so consumers can gate on changes.

## `DetectReport` — output of `llm-here detect`

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
    }
  ]
}
```

| Field | Type | Notes |
|---|---|---|
| `schema_version` | integer | Currently `1`. |
| `tool_version` | string | Binary semver. |
| `cli_detection_skipped` | boolean | `true` when any `*_SKIP_CLI` env var is truthy. |
| `providers` | `DetectedProvider[]` | Reachable providers in fallback order. |

### `DetectedProvider`

| Field | Type | Notes |
|---|---|---|
| `id` | string (enum) | One of the [registered provider ids](providers.md). |
| `kind` | `"cli"` \| `"api"` | |
| `provider` | string | Human-readable provider ("anthropic", "openai", …). |
| `model_default` | string | Default model when `--model` isn't passed. |
| `binary` | string? | Absolute path. Present iff `kind == "cli"`. |
| `env` | string? | Env var **name**. Present iff `kind == "api"`. Never contains the value. |

## `RunReport` — output of `llm-here run`

```json
{
  "schema_version": 1,
  "tool_version": "0.4.0",
  "ok": true,
  "text": "Four.",
  "provider_used": "claude-cli",
  "duration_ms": 1834,
  "error": null
}
```

| Field | Type | Notes |
|---|---|---|
| `schema_version` | integer | Currently `1`. |
| `tool_version` | string | Binary semver. |
| `ok` | boolean | `true` iff a provider returned non-empty text within the timeout. |
| `text` | string? | Model output (trimmed). `null` when `ok == false`. |
| `provider_used` | string? | Provider that produced the text (or was attempted last on failure). `null` when nothing was attempted. |
| `duration_ms` | integer | Wall time spent in dispatch. |
| `error` | string? | Human-readable message. `null` when `ok == true`. **API keys never appear here.** |

## Exit code contract

Exit codes are part of the wire format (they gate the JSON body consistently across callers):

| Code | Meaning |
|---|---|
| `0` | Success. `ok: true` in the body. |
| `1` | Attempted but failed, or missing target flag, or empty prompt. Body is still valid JSON with `ok: false` and a typed `error`. |
| `2` | Internal error (JSON serialisation, IO, broken pipe). Body may or may not be present. |

## Compatibility guidance for callers

1. **Check `schema_version`** before parsing the rest. If it's higher than your code supports, emit a warning and fall back to text output (or abort).
2. **Treat `id` as opaque** unless you know about a specific value. New ids can appear in minor versions; never crash on an unknown id.
3. **Don't assume `providers[]` is sorted alphabetically.** Order is fallback order (CLIs first, then APIs, in registry order).
4. **`binary` and `env` are mutually exclusive** per provider. Don't assume both or neither.
5. **The API key value is never in the payload.** Only the env var name.
6. **Exit code is authoritative**, but parsing `ok` gives the same answer with typed errors.

## Versioning in practice

The binary is on `0.x.y` during migration work; when it stabilises and the three sibling projects have all migrated, we'll cut `1.0.0` and publish to crates.io. At that point the wire format gets a stability contract analogous to noether's [`STABILITY.md`](https://github.com/alpibrusl/noether/blob/main/STABILITY.md).

Until then: any wire-format change ships with a `CHANGELOG` note that spells out whether it's additive or breaking.

## See also

- [Providers](providers.md) — registry.
- [Commands: `detect`](../commands/detect.md) / [`run`](../commands/run.md) — flag-level reference.
- [Environment variables](env-vars.md) — `*_SKIP_CLI` aliases and behaviour.
