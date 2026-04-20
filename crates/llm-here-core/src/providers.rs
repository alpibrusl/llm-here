//! Static registry of known providers.
//!
//! Adding a provider means adding an entry here and updating `SCHEMA.md` with
//! the new `id` value. The registry is intentionally hard-coded: callers
//! should be able to reason about the full set of possible outputs without
//! runtime surprises.

use serde::{Deserialize, Serialize};

/// Stable identifier for a provider. Part of the JSON wire contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderId {
    ClaudeCli,
    GeminiCli,
    CursorCli,
    Opencode,
    AnthropicApi,
    OpenaiApi,
    GeminiApi,
    MistralApi,
}

impl ProviderId {
    pub fn as_str(self) -> &'static str {
        match self {
            ProviderId::ClaudeCli => "claude-cli",
            ProviderId::GeminiCli => "gemini-cli",
            ProviderId::CursorCli => "cursor-cli",
            ProviderId::Opencode => "opencode",
            ProviderId::AnthropicApi => "anthropic-api",
            ProviderId::OpenaiApi => "openai-api",
            ProviderId::GeminiApi => "gemini-api",
            ProviderId::MistralApi => "mistral-api",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "claude-cli" => Some(ProviderId::ClaudeCli),
            "gemini-cli" => Some(ProviderId::GeminiCli),
            "cursor-cli" => Some(ProviderId::CursorCli),
            "opencode" => Some(ProviderId::Opencode),
            "anthropic-api" => Some(ProviderId::AnthropicApi),
            "openai-api" => Some(ProviderId::OpenaiApi),
            "gemini-api" => Some(ProviderId::GeminiApi),
            "mistral-api" => Some(ProviderId::MistralApi),
            _ => None,
        }
    }
}

/// How the provider is reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    Cli,
    Api,
}

/// Static metadata describing a provider.
///
/// `binary` is set for CLIs, `env` is set for APIs. Exactly one of the two
/// is `Some`; this is enforced by construction (the `REGISTRY` below).
#[derive(Debug, Clone)]
pub struct Provider {
    pub id: ProviderId,
    pub kind: ProviderKind,
    /// Human-readable provider name (e.g. "anthropic", "openai").
    pub provider: &'static str,
    /// For CLIs: binary name to look up on PATH.
    pub binary: Option<&'static str>,
    /// For APIs: env var holding the API key.
    pub env: Option<&'static str>,
    /// Default model identifier reported in detect output; callers can override.
    pub model_default: &'static str,
}

/// Hard-coded registry. Order here defines the default fallback order used
/// by `llm-here run --auto` (CLIs first, then APIs, matching the caloron-
/// settled chain). Callers can override via `--provider` or by providing
/// their own order.
pub const REGISTRY: &[Provider] = &[
    Provider {
        id: ProviderId::ClaudeCli,
        kind: ProviderKind::Cli,
        provider: "anthropic",
        binary: Some("claude"),
        env: None,
        model_default: "claude-desktop",
    },
    Provider {
        id: ProviderId::GeminiCli,
        kind: ProviderKind::Cli,
        provider: "google",
        binary: Some("gemini"),
        env: None,
        model_default: "gemini-desktop",
    },
    Provider {
        id: ProviderId::CursorCli,
        kind: ProviderKind::Cli,
        provider: "cursor",
        binary: Some("cursor-agent"),
        env: None,
        model_default: "cursor-desktop",
    },
    Provider {
        id: ProviderId::Opencode,
        kind: ProviderKind::Cli,
        provider: "opencode",
        binary: Some("opencode"),
        env: None,
        model_default: "opencode-desktop",
    },
    Provider {
        id: ProviderId::AnthropicApi,
        kind: ProviderKind::Api,
        provider: "anthropic",
        binary: None,
        env: Some("ANTHROPIC_API_KEY"),
        model_default: "claude-sonnet-4-5",
    },
    Provider {
        id: ProviderId::OpenaiApi,
        kind: ProviderKind::Api,
        provider: "openai",
        binary: None,
        env: Some("OPENAI_API_KEY"),
        model_default: "gpt-4o",
    },
    Provider {
        id: ProviderId::GeminiApi,
        kind: ProviderKind::Api,
        provider: "google",
        binary: None,
        env: Some("GOOGLE_API_KEY"),
        model_default: "gemini-1.5-pro",
    },
    Provider {
        id: ProviderId::MistralApi,
        kind: ProviderKind::Api,
        provider: "mistral",
        binary: None,
        env: Some("MISTRAL_API_KEY"),
        model_default: "mistral-large-latest",
    },
];

/// Look up a provider by id in the static registry.
pub fn get(id: ProviderId) -> &'static Provider {
    REGISTRY
        .iter()
        .find(|p| p.id == id)
        .expect("ProviderId variants are covered by REGISTRY by construction")
}
