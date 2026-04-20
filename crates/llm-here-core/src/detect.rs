//! `detect` implementation — builds a [`DetectReport`] by probing the
//! environment abstraction for each provider in the registry.

use crate::env::{should_skip_cli, Env, RealEnv};
use crate::providers::{ProviderKind, REGISTRY};
use crate::report::{DetectReport, DetectedProvider, SCHEMA_VERSION};

/// Detect reachable providers using the real process environment.
pub fn detect() -> DetectReport {
    detect_with_env(&RealEnv)
}

/// Detect reachable providers against a pluggable environment.
///
/// Used by the real `detect()` wrapper above and by tests.
pub fn detect_with_env<E: Env + ?Sized>(env: &E) -> DetectReport {
    let skip_cli = should_skip_cli(env);
    let mut providers = Vec::new();

    for p in REGISTRY {
        match p.kind {
            ProviderKind::Cli => {
                if skip_cli {
                    continue;
                }
                let binary = p.binary.expect("CLI providers must declare a binary");
                if let Some(path) = env.which(binary) {
                    providers.push(DetectedProvider::from_registry(
                        p,
                        Some(path.to_string_lossy().into_owned()),
                    ));
                }
            }
            ProviderKind::Api => {
                let var = p.env.expect("API providers must declare an env var");
                if env.var(var).is_some() {
                    providers.push(DetectedProvider::from_registry(p, None));
                }
            }
        }
    }

    DetectReport {
        schema_version: SCHEMA_VERSION,
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        cli_detection_skipped: skip_cli,
        providers,
    }
}
