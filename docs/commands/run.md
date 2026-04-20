# `run`

Dispatch a prompt to a provider. Reads the prompt from **stdin**; emits a JSON `RunReport` on stdout.

## Usage

```bash
echo "<prompt>" | llm-here run [--provider <id> | --auto] [flags…]
```

## Target (exactly one required)

| Flag | Behaviour |
|---|---|
| `--provider <id>` | Dispatch to one specific provider. Id must be one of the values in `llm-here detect` (see [providers](../reference/providers.md)). |
| `--auto` | Try every reachable provider in REGISTRY order; return the first success. |

## Optional flags

| Flag | Default | Notes |
|---|---|---|
| `--timeout <secs>` | `25` | Wall-clock timeout. Applies to both subprocess (CLI) and HTTP (API). Defaults to 25 s to stay under Noether's 30 s stage kill. |
| `--model <name>` | per-provider default | For APIs: applied unconditionally. For claude/gemini/cursor CLIs: emitted as `--model <name>`. Ignored by opencode. |
| `--system-prompt <text>` | — | For claude: `--append-system-prompt`. For APIs: native channel per provider. Ignored by gemini/cursor/opencode CLIs. |
| `--dangerous-claude` | off | Passes `--dangerously-skip-permissions` to `claude`. Caller-owned — llm-here reads no ambient env for this. |

## Output

Always valid JSON on stdout. `ok: true` means a provider returned non-empty text within the timeout; `ok: false` means something failed.

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

### Fields

| Field | Type | Notes |
|---|---|---|
| `schema_version` | int | Currently `1`. |
| `tool_version` | string | Semver of the `llm-here` binary. |
| `ok` | bool | `true` iff provider returned non-empty text before timeout. |
| `text` | string? | Model output (trimmed). `null` when `ok == false`. |
| `provider_used` | string? | Which provider produced the text. May be populated on failure paths too (indicates the provider that was attempted last). `null` when no provider was attempted (empty prompt, missing target flag). |
| `duration_ms` | int | Wall time spent in dispatch. |
| `error` | string? | Human-readable error message. `null` when `ok == true`. API keys are **never** included in error messages. |

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Success. `ok: true` in the payload. |
| `1` | Attempted but failed (bad key, non-2xx response, timeout, CLI error, all providers exhausted in `--auto`). Stdout is still valid JSON. |
| `2` | Internal error (JSON serialisation, IO, broken pipe). |

Callers can gate on exit code without parsing JSON; parsing the body gives typed error details.

## The fallback chain (`--auto`)

Providers are tried in the order they appear in the registry:

1. `claude-cli`
2. `gemini-cli`
3. `cursor-cli`
4. `opencode`
5. `anthropic-api`
6. `openai-api`
7. `gemini-api`
8. `mistral-api`

For each candidate, llm-here checks if it's **reachable** (binary on PATH for CLIs; env var set for APIs). Unreachable providers are skipped without an attempt. First successful dispatch wins; if all reachable providers fail, the last failure's error is returned.

CLI providers are skipped entirely when any of the `*_SKIP_CLI` env vars is truthy. See [environment variables](../reference/env-vars.md) for the aliases.

## Timeout handling

- **CLI providers.** The subprocess is spawned; `wait-timeout` enforces the wall-clock limit. On expiry, the child is killed and reaped (no zombie processes). The error reads `"<provider-id>: timed out after <ms>ms"`.
- **API providers.** `reqwest`'s per-request timeout. On expiry, the HTTP connection is dropped. Error: `"<provider-id>: timed out after <ms>ms"`.

The default of 25 s is tuned for the Nix-executor stage kill; deployments outside Nix can pass a higher value.

## Examples

**Specific CLI provider with a custom model:**

```bash
echo "Summarise this in one sentence: …" \
  | llm-here run --provider claude-cli --model claude-opus-4-1 --timeout 60
```

**Auto with a system prompt:**

```bash
echo "Who wrote Hamlet?" \
  | llm-here run --auto \
      --system-prompt "Answer in exactly three words."
```

**API-only deployment (no CLIs installed):**

```bash
export OPENAI_API_KEY=sk-…
echo "hi" | llm-here run --provider openai-api
```

**Sandbox (CLI auth state not mounted, so skip CLIs entirely):**

```bash
LLM_HERE_SKIP_CLI=1 echo "hi" | llm-here run --auto
# Falls straight through to API providers.
```

## See also

- [`detect`](detect.md) — enumerate reachable providers.
- [Providers](../reference/providers.md) — registry table.
- [JSON wire format](../reference/schema.md) — semver rules for the payload shape.
- [From Python](../integration/python.md) / [From Rust](../integration/rust.md) — wrapping `run` in a caller.
