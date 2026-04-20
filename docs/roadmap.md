# Roadmap

## Shipped

### v0.1.0 — initial scaffold

- `llm-here detect` with the 8-provider registry.
- `llm-here-core::Env` trait for testable detection.
- Stable JSON wire format (`schema_version: 1`).
- EUPL-1.2 license, CI (fmt + clippy + test + release build).

### v0.2.0 — CLI dispatch

- `llm-here run --provider <cli-id>` and `llm-here run --auto` dispatch via subprocess.
- `CommandRunner` trait for deterministic tests.
- Default 25 s timeout with child-kill on expiry (via `wait-timeout`).
- `--dangerous-claude` flag for caller-owned opt-in.
- Per-CLI argv templates matching caloron's field-tested patterns.

### v0.3.0 — API dispatch

- HTTPS transport for all four API providers (Anthropic, OpenAI, Gemini, Mistral).
- `HttpClient` trait + `RealHttpClient` (reqwest + rustls).
- `--model <name>` applied to API dispatch.
- `run --auto` chains through APIs after CLIs.

### v0.4.0 — full per-provider surface

- `--system-prompt <text>` flag, routed to each provider's native channel.
- `--model <name>` propagates to CLI argv (claude/gemini/cursor).
- Feature-complete for the noether-grid migration.

## In progress

### v0.5+ — publication and stability

- **crates.io publish.** Swap the git-tag dep for a version spec. Unblocks the noether `Cargo.toml` migration from `git = "..."` to `version = "0.x"`. Small amount of packaging prep; tracked informally.
- **Consumer migration follow-ups:** caloron-noether Phase 2 (delete the in-tree fallback once llm-here is ubiquitous). Tracked in [caloron-noether#21](https://github.com/alpibrusl/caloron-noether/issues/21).
- **Cross-repo regression fixtures.** Send one prompt through each of `llm-here`, caloron's legacy `_llm.py`, and agentspec's resolver; assert identical observable behaviour for a mock provider. Catches regressions in the "we broke something during consolidation" class.

## Probable v1.0

Once the v0.5+ follow-ups settle, cut `v1.0.0` with:

- Stability contract analogous to [`noether/STABILITY.md`](https://github.com/alpibrusl/noether/blob/main/STABILITY.md).
- The JSON wire format locks to `schema_version: 1` for 1.x.
- crates.io listing for both `llm-here` (binary) and `llm-here-core` (library).
- Independent release cadence from noether's minor version track.

## Explicit non-goals

These come up periodically and remain out of scope:

| Idea | Why not |
|---|---|
| Conversation history / threading | Callers have opinions about storage; a subprocess boundary is the wrong place to impose one. |
| Streaming tokens | Single prompt → single completion is the contract. Streaming belongs in the caller that consumes the output. |
| Cost accounting | Callers own their own cost ledgers. llm-here would have to replicate provider-specific pricing lookups, which drifts. |
| Tool use / agent loops | Different semantics per provider; the subprocess boundary adds round trips the tool-use spec can't tolerate. |
| Vertex AI routing | Stays in agentspec's resolver. It's a *runtime selection* concern, not a *detection* concern. |
| Ollama integration | Local HTTP server with different semantics. A separate tool might be warranted; file an issue if you have a concrete use case. |

See the [home page](index.md#whats-not) for the current short version.

## Provider additions under consideration

| Provider | Status |
|---|---|
| `codex-cli` | Fits the model; add if caloron or agentspec ask for it. |
| `goose-cli` | MCP-native agent; semantics diverge from subscription CLIs. Probably stays out. |
| `aider-cli` | Coding agent with its own tool-use loop. Out of scope. |

File issues for others at [alpibrusl/llm-here/issues](https://github.com/alpibrusl/llm-here/issues).

## See also

- [Changelog](changelog.md) — version-by-version change log.
- [Consumer projects](integration/consumers.md) — migration status.
