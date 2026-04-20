//! Environment abstraction.
//!
//! Detection reads the filesystem (PATH lookups) and env vars. Both are
//! abstracted behind a trait so tests can inject deterministic state
//! without mutating the real process environment.

use std::path::PathBuf;

pub trait Env {
    fn var(&self, key: &str) -> Option<String>;
    fn which(&self, binary: &str) -> Option<PathBuf>;
}

/// Real environment: reads `std::env::var` and resolves binaries via
/// the `which` crate.
pub struct RealEnv;

impl Env for RealEnv {
    fn var(&self, key: &str) -> Option<String> {
        std::env::var(key).ok().filter(|v| !v.is_empty())
    }

    fn which(&self, binary: &str) -> Option<PathBuf> {
        which::which(binary).ok()
    }
}

/// Env vars that signal "skip CLI providers, we're in a sandbox".
///
/// Three names are accepted so each caller (noether, caloron, agentspec)
/// can keep using its existing convention without having to learn a new
/// one. Any of them set to a truthy value triggers the skip.
pub const SKIP_CLI_ENV_VARS: &[&str] = &[
    "LLM_HERE_SKIP_CLI",
    "NOETHER_LLM_SKIP_CLI",
    "CALORON_LLM_SKIP_CLI",
    "AGENTSPEC_LLM_SKIP_CLI",
];

pub fn is_truthy(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

pub fn should_skip_cli<E: Env + ?Sized>(env: &E) -> bool {
    SKIP_CLI_ENV_VARS
        .iter()
        .any(|k| env.var(k).as_deref().is_some_and(is_truthy))
}
