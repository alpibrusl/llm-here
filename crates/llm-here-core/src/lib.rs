//! Detection and dispatch logic for LLM CLIs and API providers.
//!
//! This crate is the library half of `llm-here`. The binary crate `llm-here`
//! is a thin wrapper that exposes these APIs on stdout as JSON; Rust callers
//! can depend on this crate directly instead of spawning a subprocess.
//!
//! See `SCHEMA.md` in the repo root for the stable JSON wire format — the
//! serde output of [`DetectReport`] is what gets semver'd, not the Rust API.

pub mod detect;
pub mod dispatch;
pub mod env;
pub mod providers;
pub mod report;

pub use detect::detect;
pub use dispatch::{
    run_auto, run_auto_real, run_cli_provider, run_cli_provider_real, CommandRunner,
    DispatchOptions, DispatchOutcome, DispatchRequest, RealCommandRunner,
};
pub use providers::{Provider, ProviderId, ProviderKind};
pub use report::{DetectReport, DetectedProvider, RunReport};
