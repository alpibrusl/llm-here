# Consumer projects

Three sibling projects use `llm-here` today. Each migrated from its own detection/dispatch code to a shared `llm-here` boundary.

## Noether

**Role:** first Rust consumer. Grid-workers dispatch LLM completions to subscription CLIs; the production path runs inside Nix stages.

**Migration:** [noether#54](https://github.com/alpibrusl/noether/pull/54), merged 2026-04-20.

**How it uses `llm-here`:** `crates/noether-engine/src/llm/cli_provider.rs` is a ~50-LOC adapter over `llm-here-core`:

```rust
use llm_here_core::dispatch::{run_cli_provider, DispatchOptions, RealCommandRunner};
use llm_here_core::providers::ProviderId;

impl LlmProvider for CliProvider {
    fn complete(&self, messages: &[Message], config: &LlmConfig) -> Result<String, LlmError> {
        // ... compose messages into a single prompt, split out system prompt ...
        let opts = DispatchOptions {
            timeout: Duration::from_secs(self.config.timeout_secs),
            dangerous_claude: self.config.dangerous_claude
                && self.spec.id == ProviderId::ClaudeCli,
            model: model_override(&config.model, self.spec.default_model),
            system_prompt: if has_native_system_flag { system } else { None },
        };
        let report = run_cli_provider(self.spec.id, &prompt, &opts, &RealCommandRunner);
        // ... map to LlmError on failure ...
    }
}
```

**What stayed in Noether:** multi-message `Message`/`Role` composition, `NOETHER_LLM_SKIP_CLI` belt-and-braces check, the `LlmProvider` trait (noether-specific). What moved: PATH lookup, argv building, subprocess spawn, timeout enforcement, error translation.

**Net diff:** −33 LOC. Dispatch logic now lives in one place across the stack.

## Agentspec

**Role:** runtime resolver for AI agent manifests. Decides which CLI or API to provision based on `model.preferred` and reachable providers on the host.

**Migration:** [agentspec#29](https://github.com/alpibrusl/agentspec/pull/29).

**How it uses `llm-here`:** `src/agentspec/resolver/resolver.py:_detect_runtimes()` delegates the four subscription CLIs it shares with llm-here (`claude-code`, `gemini-cli`, `cursor-cli`, `opencode`) and falls back to `shutil.which` for everything else:

```python
_LLM_HERE_CLI_IDS = {
    "claude-code": "claude-cli",
    "gemini-cli": "gemini-cli",
    "cursor-cli": "cursor-cli",
    "opencode": "opencode",
}


def _detect_runtimes() -> dict[str, bool]:
    available = {
        name: shutil.which(binary) is not None
        for name, binary in RUNTIME_BINARIES.items()
    }
    llm_here_results = _query_llm_here_detect()
    if llm_here_results is not None:
        available.update(llm_here_results)
    return available
```

**What stayed in Agentspec:** Vertex AI routing (`resolver/vertex.py`), `PROVIDER_MAP` (manifest provider-prefix → runtime), codex-cli, goose, aider, ollama, test-echo detection. These are either agentspec-specific selection concerns or runtimes llm-here doesn't yet cover.

**Soft dependency:** when `llm-here` isn't on PATH, the whole chain reverts to `shutil.which`. Existing behaviour is strictly preserved; new behaviour only activates when both the binary and llm-here are installed.

## Caloron-noether

**Role:** reference application for Noether. Runs sprint-driven autonomous agents; `call_llm()` is the LLM gateway used by PO (product-owner) phases.

**Migration:** [caloron-noether#22](https://github.com/alpibrusl/caloron-noether/pull/22).

**How it uses `llm-here`:** `stages/phases/_llm.py:call_llm()` tries `llm-here` first, falls back to the in-tree provider chain:

```python
def call_llm(prompt: str, timeout: int = 120) -> str | None:
    out = _call_via_llm_here(prompt, timeout)
    if out is not None:
        return out
    # ... fallback: in-tree provider chain ...
```

The llm-here path forwards caloron's env-var gates as flags:

- `CALORON_LLM_PROVIDER=claude-cli` → `--provider claude-cli`
- `CALORON_ALLOW_DANGEROUS_CLAUDE=1` → `--dangerous-claude`
- `CALORON_LLM_SKIP_CLI` → honoured natively by llm-here as an alias

**Why soft dep, not hard dep:** caloron is "a reference app for Noether" — keeping the deploy-minimal story simple matters. Phase 1 (this PR) runs alongside the in-tree chain with zero behavioural risk. Phase 2 (delete the fallback, hit the ~30-LOC target from the original issue) lands once llm-here is ubiquitous in caloron deployments.

## The meta-tracker

[noether#46](https://github.com/alpibrusl/noether/issues/46) tracks the cross-repo consolidation. It links to all three migration PRs and will close when the last one merges.

## Integration patterns worth copying

Three conventions emerged across the three migrations:

1. **Soft-dep with graceful fallback.** Unless you're willing to make llm-here a hard install requirement, always detect its presence via `shutil.which("llm-here")` (Python) or `CommandRunner` trait-probe (Rust) and fall back cleanly.
2. **Translate caller-specific env vars into flags at the boundary.** Don't ask `llm-here` to learn your env var names — forward them as flags. Keeps `llm-here` stateless and gives callers precise control.
3. **Mock at the transport boundary.** `FakeCommandRunner` / `FakeHttpClient` (Rust) or monkeypatched `subprocess.run` (Python). Tests stay fast and deterministic, and the same pattern works across all three codebases.

## See also

- [From Python](python.md) — generic Python integration.
- [From Rust](rust.md) — generic Rust integration.
