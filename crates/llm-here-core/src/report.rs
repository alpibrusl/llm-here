//! JSON wire types. These structs define the stable output format.
//!
//! **Semver belongs to this module.** Additive changes (new field with a
//! default, new provider id in [`crate::providers`]) are minor bumps.
//! Removing or renaming a field is a major bump. See `SCHEMA.md` for the
//! full contract downstream callers rely on.

use serde::{Deserialize, Serialize};

use crate::providers::{Provider, ProviderKind};

/// Top-level output of `llm-here detect`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectReport {
    /// Schema version of this payload. Bumped on any breaking change.
    pub schema_version: u32,
    /// Version of the `llm-here` crate that produced this report.
    pub tool_version: String,
    /// Whether CLI detection was skipped because of a sandbox-signal env var.
    pub cli_detection_skipped: bool,
    /// Providers that were detected as reachable on this host.
    pub providers: Vec<DetectedProvider>,
}

/// One detected provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedProvider {
    pub id: String,
    pub kind: ProviderKind,
    pub provider: String,
    pub model_default: String,
    /// Set for CLI providers: absolute path to the binary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binary: Option<String>,
    /// Set for API providers: name of the env var holding the key.
    /// The key itself is never included.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<String>,
}

impl DetectedProvider {
    pub(crate) fn from_registry(p: &Provider, binary_path: Option<String>) -> Self {
        Self {
            id: p.id.as_str().to_string(),
            kind: p.kind,
            provider: p.provider.to_string(),
            model_default: p.model_default.to_string(),
            binary: binary_path,
            env: p.env.map(str::to_string),
        }
    }
}

/// Top-level output of `llm-here run`. Not yet implemented — this shape is
/// carved out early so callers can code against it today.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunReport {
    pub schema_version: u32,
    pub tool_version: String,
    pub ok: bool,
    pub text: Option<String>,
    pub provider_used: Option<String>,
    pub duration_ms: u64,
    pub error: Option<String>,
}

/// Wire-format schema version. Bumped on breaking changes; additive
/// changes do not bump this.
pub const SCHEMA_VERSION: u32 = 1;
