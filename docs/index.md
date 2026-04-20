# llm-here

> One tool for **"which LLM is reachable from this host, and how do I run a prompt through it?"**

Four subscription CLIs (`claude`, `gemini`, `cursor-agent`, `opencode`) and four API providers (Anthropic, OpenAI, Gemini, Mistral), all behind one JSON-in / JSON-out binary. Callers in any language talk to it as a subprocess; Rust callers can depend on the [`llm-here-core`](integration/rust.md) crate directly.

## Why this exists

Three sibling projects — [caloron-noether](https://github.com/alpibrusl/caloron-noether), [agentspec](https://github.com/alpibrusl/agentspec), [noether](https://github.com/alpibrusl/noether) — each re-implemented provider detection + dispatch and had already drifted by the time anyone noticed. The motivating design note (`noether/docs/research/llm-here.md`) called out the pattern:

- caloron discovered the 25-second-under-Nix-30 timeout cap months before noether-grid did.
- agentspec handles Vertex AI routing; the other two don't.
- CLI argv shapes and env-var conventions had forked in small, annoying ways across repos.

`llm-here` is the consolidation. One implementation, one wire format, shared across every caller.

## The 30-second tour

```bash
# 1. What's reachable?
llm-here detect

# 2. Run a prompt through a specific provider:
echo "What's 2+2? One word." | llm-here run --provider claude-cli --timeout 25

# 3. Or let it pick the best-available:
echo "hi" | llm-here run --auto
```

Output is always valid JSON on stdout. Exit code `0` on success, `1` when a provider was attempted but failed, `2` on internal error.

See [Getting started](getting-started.md) for a longer walkthrough.

## What's in scope

- **Detection.** PATH lookups for CLIs, env-var checks for APIs.
- **Single-shot dispatch.** Subprocess for CLIs, HTTPS for APIs. Bounded timeouts, child-kill on expiry.
- **Fallback chain.** `--auto` walks every reachable provider in registry order until one succeeds.
- **Sandbox-aware.** `LLM_HERE_SKIP_CLI` / `NOETHER_LLM_SKIP_CLI` / `CALORON_LLM_SKIP_CLI` / `AGENTSPEC_LLM_SKIP_CLI` all short-circuit CLI probing.
- **Stable JSON wire format.** Semver'd independently of the binary version.

## What's not

These belong in the caller:

- **State.** Every invocation is independent. No conversation history, no caching, no session tokens.
- **Cost accounting.** Callers own their own cost ledger.
- **Streaming.** Single prompt → single completion.
- **Agent-loop semantics.** No tool use, no multi-turn orchestration.
- **Vertex AI routing.** Stays in agentspec's resolver — it's a runtime-selection concern, not a detection concern.

## Ecosystem

| Project | Role |
|---|---|
| [noether](https://github.com/alpibrusl/noether) | First Rust consumer. `noether-engine::llm::cli_provider` delegates to `llm-here-core`. |
| [agentspec](https://github.com/alpibrusl/agentspec) | Uses `llm-here detect` via subprocess for runtime detection. |
| [caloron-noether](https://github.com/alpibrusl/caloron-noether) | Uses `llm-here run --auto` as primary dispatch with local fallback. |

## Next steps

- [**Getting started**](getting-started.md) — install and run your first dispatch.
- [**Commands**](commands/detect.md) — full reference for `detect` and `run`.
- [**Integration**](integration/python.md) — subprocess (Python) or crate (Rust).
- [**JSON wire format**](reference/schema.md) — the stable contract callers build against.
