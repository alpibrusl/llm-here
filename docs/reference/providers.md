# Providers

The registry is the single source of truth for which providers llm-here knows about. It's defined in [`crates/llm-here-core/src/providers.rs`](https://github.com/alpibrusl/llm-here/blob/main/crates/llm-here-core/src/providers.rs) and intentionally hard-coded — callers can reason about the full set of possible outputs without runtime surprises.

## The registry

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

The order matters: it's the fallback order used by `llm-here run --auto`.

## Per-CLI invocation

Argv templates are derived from each CLI's documented `-p` / prompt invocation. Verified in caloron-noether's field-tested `stages/phases/_llm.py` and noether-engine's `cli_provider.rs`.

| id | argv (with a prompt `P` and optional flags) |
|---|---|
| `claude-cli` | `claude [--dangerously-skip-permissions] [--append-system-prompt SYS] [--model M] -p P` |
| `gemini-cli` | `gemini -y [--model M] -p P` |
| `cursor-cli` | `cursor-agent [--model M] -p P --output-format text` |
| `opencode` | `opencode run P` |

- `--dangerously-skip-permissions` is gated by [`--dangerous-claude`](../commands/run.md). No ambient env read.
- `--append-system-prompt` is sent only when `--system-prompt` is passed to `llm-here`.
- `--model` is skipped when the value matches `model_default` (no-op), and silently ignored by opencode (no such flag upstream).
- Gemini/cursor/opencode have no system-prompt flag; system prompts for those should be inlined into the main prompt by the caller.

## Per-API invocation

All APIs use `POST` with `Content-Type: application/json` and `reqwest`'s per-request timeout. The response path varies; errors are translated into a typed `error` string in the [run report](../commands/run.md).

### `anthropic-api`

```
POST https://api.anthropic.com/v1/messages
x-api-key: $ANTHROPIC_API_KEY
anthropic-version: 2023-06-01
```

```json
{
  "model": "claude-sonnet-4-5",
  "max_tokens": 4096,
  "system": "<system prompt if provided>",
  "messages": [{"role": "user", "content": "<prompt>"}]
}
```

Text is extracted from `content[0].text`.

### `openai-api`

```
POST https://api.openai.com/v1/chat/completions
Authorization: Bearer $OPENAI_API_KEY
```

```json
{
  "model": "gpt-4o",
  "messages": [
    {"role": "system", "content": "<system prompt if provided>"},
    {"role": "user", "content": "<prompt>"}
  ]
}
```

Text is extracted from `choices[0].message.content`.

### `gemini-api`

```
POST https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent?key=$GOOGLE_API_KEY
```

```json
{
  "system_instruction": {"parts": [{"text": "<system prompt if provided>"}]},
  "contents": [{"parts": [{"text": "<prompt>"}]}]
}
```

Text is extracted from `candidates[0].content.parts[0].text`.

### `mistral-api`

```
POST https://api.mistral.ai/v1/chat/completions
Authorization: Bearer $MISTRAL_API_KEY
```

Same body shape as OpenAI. Text is extracted from `choices[0].message.content`.

## Proposing a new provider

File an issue at [alpibrusl/llm-here/issues](https://github.com/alpibrusl/llm-here/issues) with:

- Provider name and category (CLI or API).
- Binary name (for CLIs) or auth env var (for APIs).
- Argv template (for CLIs) or request/response shape (for APIs).
- At least one sibling project that needs it.

**Providers we've explicitly considered and deferred:**

- `codex-cli` (OpenAI's CLI) — fits the model, not yet added.
- `ollama` — local HTTP server, different semantics from subscription CLIs. Probably belongs in a separate tool.
- `aider` — coding agent with its own tool-use loop. Out of scope per the not-in-scope list in the [home page](../index.md).
- `goose` — MCP-native agent, manages its own provider config. Same reasoning as aider.

## Wire format

Provider ids are a closed set — they're part of the [JSON wire format](schema.md) and semver'd accordingly. Adding a provider is a **minor** bump; removing or renaming is a **major** bump.

## See also

- [JSON wire format](schema.md)
- [Environment variables](env-vars.md)
- [Commands: `run`](../commands/run.md)
