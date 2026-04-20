# Integrating from Python

`llm-here` is a subprocess boundary — Python callers invoke it with `subprocess.run`, pipe a prompt to stdin, and parse JSON from stdout. No PyO3, no `pip install`.

## Minimal `call_llm` shim

```python
import json
import subprocess
import shutil


def call_llm(prompt: str, timeout: int = 30) -> str | None:
    """Dispatch a prompt via llm-here. Returns model text or None on failure."""
    if not shutil.which("llm-here"):
        return None

    argv = ["llm-here", "run", "--auto", "--timeout", str(timeout)]

    try:
        result = subprocess.run(
            argv,
            input=prompt,
            capture_output=True,
            text=True,
            timeout=timeout + 5,  # soft buffer above llm-here's own timeout
            check=False,
        )
    except (subprocess.TimeoutExpired, FileNotFoundError, OSError):
        return None

    # exit 1 = provider attempted but failed; exit 2 = internal error
    if result.returncode != 0:
        return None

    try:
        payload = json.loads(result.stdout)
    except json.JSONDecodeError:
        return None

    if not payload.get("ok"):
        return None
    text = payload.get("text")
    return text if isinstance(text, str) and text else None
```

## Detection-only shim

For callers (like agentspec's resolver) that only need to know which providers are reachable:

```python
import json
import shutil
import subprocess


def detect_providers() -> dict[str, bool]:
    """Map provider id → reachable. Returns {} if llm-here isn't installed."""
    if not shutil.which("llm-here"):
        return {}

    try:
        result = subprocess.run(
            ["llm-here", "detect"],
            capture_output=True,
            text=True,
            timeout=5,
            check=False,
        )
    except (subprocess.TimeoutExpired, FileNotFoundError, OSError):
        return {}

    if result.returncode != 0:
        return {}

    try:
        providers = json.loads(result.stdout).get("providers", [])
    except json.JSONDecodeError:
        return {}

    return {p["id"]: True for p in providers}
```

## Pitfalls

**Don't swallow the buffer.** `capture_output=True` is fine for prompts ≤ ~1 MB. For larger prompts, pipe into stdin via a file-like object and read stdout incrementally — but if you're doing that, [you're probably out of scope](../index.md#whats-not).

**Timeout budgeting.** `llm-here` enforces its own wall-clock timeout internally. Always give `subprocess.run` a *larger* timeout (4-5 s buffer) so Python kills the subprocess only if llm-here itself hangs, not on legitimate dispatch.

**Exit codes before JSON.** Check `result.returncode` before trusting `result.stdout`. An exit-2 (internal error) may have produced partial or no JSON.

**API key leakage.** Don't log `result.stderr` to your own error reporter without inspecting it. llm-here tries not to leak keys, but a provider's own error message could contain more than you expect.

**Reentrancy.** Each `call_llm` spawns a fresh subprocess. There's no connection pooling; APIs get a new TLS handshake every call. For hot-path usage, consider batching or moving to the [Rust crate](rust.md).

## Forwarding caller-specific env vars

If your project has its own env-var conventions (caloron, agentspec), forward them as flags:

```python
import os

def call_llm_with_caloron_dangerous_gate(prompt: str) -> str | None:
    argv = ["llm-here", "run", "--auto", "--timeout", "25"]

    # caloron's CALORON_ALLOW_DANGEROUS_CLAUDE gate, forwarded as --dangerous-claude
    if os.environ.get("CALORON_ALLOW_DANGEROUS_CLAUDE", "").lower() in ("1", "true", "yes", "on"):
        argv.append("--dangerous-claude")

    # CALORON_LLM_PROVIDER override
    if provider := os.environ.get("CALORON_LLM_PROVIDER", "").strip():
        argv.remove("--auto")
        argv.extend(["--provider", provider])

    # ... subprocess run as above
```

`CALORON_LLM_SKIP_CLI` is honoured natively by llm-here (it's one of four [sandbox aliases](../reference/env-vars.md#sandbox-signals-skip-cli-probing)) — no explicit forwarding needed.

## Testing with mocks

Tests monkeypatch `subprocess.run` and `shutil.which`:

```python
import json
from unittest.mock import MagicMock, patch


def test_call_llm_happy_path(monkeypatch):
    monkeypatch.setattr("shutil.which", lambda cmd: "/usr/bin/llm-here")
    monkeypatch.setattr(
        "subprocess.run",
        lambda *a, **k: MagicMock(
            returncode=0,
            stdout=json.dumps({"ok": True, "text": "hello"}),
        ),
    )
    assert call_llm("hi") == "hello"


def test_call_llm_falls_back_when_llm_here_absent(monkeypatch):
    monkeypatch.setattr("shutil.which", lambda cmd: None)
    assert call_llm("hi") is None
```

See [caloron-noether's `tests/test_phases.py`](https://github.com/alpibrusl/caloron-noether/blob/main/tests/test_phases.py) for the full pattern applied in production.

## See also

- [From Rust](rust.md) — linking `llm-here-core` directly for Rust callers.
- [Consumer projects](consumers.md) — how agentspec and caloron use `llm-here`.
- [Environment variables](../reference/env-vars.md) — sandbox signals, API keys.
