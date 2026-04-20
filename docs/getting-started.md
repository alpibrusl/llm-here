# Getting started

## Install

Pre-release (current):

```bash
cargo install --git https://github.com/alpibrusl/llm-here llm-here
```

Once on crates.io (tracked as a follow-up):

```bash
cargo install llm-here
```

Verify:

```bash
llm-here --version
# → llm-here 0.4.0
```

## First detect

```bash
llm-here detect
```

You'll get a JSON report listing every provider llm-here knows about that's currently reachable on this host:

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
      "id": "mistral-api",
      "kind": "api",
      "provider": "mistral",
      "model_default": "mistral-large-latest",
      "env": "MISTRAL_API_KEY"
    }
  ]
}
```

!!! note "Keys are never in the output"
    For API providers, `llm-here detect` reports the env var name (`MISTRAL_API_KEY`), never the key value. Regression-tested since v0.1.

See the [providers reference](reference/providers.md) for the full registry and [`detect` command reference](commands/detect.md) for all options.

## First run

Prompts are read from **stdin**:

```bash
echo "What's 2+2? Answer in one word." | llm-here run --provider claude-cli --timeout 30
```

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

Or let `llm-here` pick the first-available:

```bash
echo "hi" | llm-here run --auto
```

`--auto` walks the fallback chain (CLIs first, then APIs) and returns the first success. If everything fails, you get a typed error:

```json
{
  "schema_version": 1,
  "tool_version": "0.4.0",
  "ok": false,
  "text": null,
  "provider_used": null,
  "duration_ms": 7,
  "error": "no providers reachable on this host"
}
```

Exit codes:

| Code | Meaning |
|---|---|
| `0` | Provider returned non-empty text within the timeout. |
| `1` | A provider was attempted but failed (wrong key, non-2xx response, CLI error, timeout, …). Still valid JSON on stdout. |
| `2` | Internal error (serialisation, IO). |

## Overriding the model

For API providers (and claude/gemini/cursor CLIs), pass `--model`:

```bash
echo "hi" | llm-here run --provider anthropic-api --model claude-opus-4-1
```

Opencode has no `--model` flag on the upstream CLI and silently ignores the option. See the [run command reference](commands/run.md) for the full flag list.

## What next

- [**Full command reference**](commands/run.md) — every flag, every exit code, every error shape.
- [**Providers**](reference/providers.md) — the registry table and how to propose additions.
- [**JSON wire format**](reference/schema.md) — the stable contract for callers.
- [**From Python**](integration/python.md) / [**From Rust**](integration/rust.md) — integrate `llm-here` in your own code.
