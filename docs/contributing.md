# Contributing

Thanks for the interest. `llm-here` is small and will stay small — most of the value is in the stability of the JSON wire format and the per-provider argv/body templates. Patches in those areas are especially welcome.

## Development loop

```bash
git clone https://github.com/alpibrusl/llm-here
cd llm-here

cargo build
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
```

All four of those must pass before a PR can merge. CI enforces them on every push.

### Local smoke

```bash
cargo build --release
./target/release/llm-here detect
echo "hi" | ./target/release/llm-here run --provider opencode --timeout 30
```

The second command requires opencode (or any other CLI) to be installed. For API providers, set the relevant env var.

## Adding a provider

1. Add an entry to `crates/llm-here-core/src/providers.rs::REGISTRY`.
2. For CLI providers, extend `build_argv` in `crates/llm-here-core/src/dispatch.rs`.
3. For API providers, add a `build_<provider>` function in `crates/llm-here-core/src/api.rs`, extend `run_api_provider`'s match, and extend `extract_text`.
4. Add tests in `crates/llm-here-core/tests/dispatch.rs` (CLI argv) or `tests/api.rs` (API request shape + response parsing).
5. Update [`docs/reference/providers.md`](reference/providers.md) and [`SCHEMA.md`](https://github.com/alpibrusl/llm-here/blob/main/SCHEMA.md#provider-id-registry).
6. Add a minor-version entry to `CHANGELOG.md`. Adding a provider is **additive** → minor bump.

## Changing the wire format

Additive changes (new optional field, new enum value, new provider id) are minor bumps.

**Breaking changes are rare.** They require:

1. A major version bump (`0.x.y` → `0.x+1.0` pre-1.0; `x.y.z` → `(x+1).0.0` post-1.0).
2. Updating `SCHEMA_VERSION` in `crates/llm-here-core/src/report.rs`.
3. An entry in the `CHANGELOG.md` "Wire format" section spelling out the break.
4. Coordination with the three consumer projects (noether, agentspec, caloron-noether) so they can bump their pinned versions.

## Testing conventions

- **Fakes over mocks.** `Env`, `CommandRunner`, `HttpClient` are trait-abstracted; tests inject local fake impls. No live subprocess spawns, no network calls, no real env mutation.
- **Test per behaviour, not per line.** Each test documents one observable property of the interface — argv order, env-var-name non-leakage, fallback semantics. See [`tests/dispatch.rs`](https://github.com/alpibrusl/llm-here/blob/main/crates/llm-here-core/tests/dispatch.rs) for the pattern.
- **Regression tests for API-key non-leakage.** Any change that touches error formatting for API providers needs a test asserting the key value doesn't appear in the `error` field. The current regression lives in `tests/api.rs::anthropic_api_key_never_appears_in_error_message`.

## Commit messages and PRs

- Small, focused commits. The commit history is the primary design record.
- One logical change per commit. If you're tempted to squash, consider separate commits instead.
- PR descriptions should include a "Test plan" section and a "Verification" section with `cargo test`, `cargo fmt --check`, and `cargo clippy --all-targets -- -D warnings` results.

## Reporting issues

File at [alpibrusl/llm-here/issues](https://github.com/alpibrusl/llm-here/issues). Useful context to include:

- Output of `llm-here --version` and `llm-here detect`.
- For dispatch issues: the full `RunReport` JSON (api keys are safe — llm-here doesn't include them).
- For consumer-project issues: which consumer (noether / agentspec / caloron), what migration state (pre-migration, mid-migration, post-migration).

## Security disclosures

**Do not file security issues publicly.** Email `alfonso@elumobility.com` with `[llm-here security]` in the subject. See [Security](security.md) for the full policy.

## Code of conduct

Be kind. Disagree with ideas, not people. Hat tip to the noether and agentspec projects for the same posture.
