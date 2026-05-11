use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCatalog {
    pub schema_version: u32,
    pub last_updated: String,
    pub providers: Vec<ProviderDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderDefinition {
    pub id: String,
    pub display_name: String,
    pub tier: Tier,
    pub base_url: Option<String>,
    pub env_style: EnvStyle,
    pub auth: AuthConfig,
    pub models: Vec<ModelDefinition>,
    pub defaults: DefaultModels,
    pub notes: HashMap<String, String>,
    pub trial_ends_at: Option<String>,
    pub last_verified: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Free,
    Trial,
    Paid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EnvStyle {
    Anthropic,
    OpenAI,
    Generic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub env_var: String,
    pub key_url: String,
    pub needs_key: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDefinition {
    pub id: String,
    pub role: ModelRole,
    pub free: bool,
    pub context: u64,
    #[serde(default)]
    pub note: String,
    pub use_cases: Vec<String>,
    pub pricing: Option<ModelPricing>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ModelRole {
    Main,
    Small,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricing {
    #[serde(default)]
    pub input_per_1m: f64,
    #[serde(default)]
    pub output_per_1m: f64,
    #[serde(default)]
    pub trial: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultModels {
    pub main: Option<String>,
    pub small: Option<String>,
}

pub fn bundled_catalog() -> ProviderCatalog {
    ProviderCatalog {
        schema_version: 1,
        last_updated: "2026-05-01".into(),
        providers: vec![
            ProviderDefinition {
                id: "anthropic".into(),
                display_name: "Anthropic (Official)".into(),
                tier: Tier::Paid,
                base_url: None,
                env_style: EnvStyle::Anthropic,
                auth: AuthConfig {
                    env_var: "ANTHROPIC_AUTH_TOKEN".into(),
                    key_url: "https://console.anthropic.com/settings/keys".into(),
                    needs_key: false,
                },
                models: vec![
                    ModelDefinition {
                        id: "claude-sonnet-4-5-20250929".into(),
                        role: ModelRole::Main,
                        free: false,
                        context: 200000,
                        note: "Best balance of speed and capability".into(),
                        use_cases: vec!["refactor".into(), "coding".into(), "coding-fast".into(), "multi-agent".into(), "architecture".into(), "code-review".into(), "testing".into(), "debugging".into(), "optimization".into(), "documentation".into()],
                        pricing: Some(ModelPricing { input_per_1m: 3.0, output_per_1m: 15.0, trial: false }),
                    },
                    ModelDefinition {
                        id: "claude-opus-4-7-20251201".into(),
                        role: ModelRole::Main,
                        free: false,
                        context: 200000,
                        note: "Maximum capability for complex reasoning".into(),
                        use_cases: vec!["refactor".into(), "long-context".into(), "multi-agent".into(), "architecture".into()],
                        pricing: Some(ModelPricing { input_per_1m: 15.0, output_per_1m: 75.0, trial: false }),
                    },
                    ModelDefinition {
                        id: "claude-haiku-4-5-20250929".into(),
                        role: ModelRole::Small,
                        free: false,
                        context: 200000,
                        note: "Fast and cheap for simple tasks".into(),
                        use_cases: vec!["coding-fast".into(), "cheap-batch".into()],
                        pricing: Some(ModelPricing { input_per_1m: 0.8, output_per_1m: 4.0, trial: false }),
                    },
                ],
                defaults: DefaultModels { main: Some("claude-sonnet-4-5-20250929".into()), small: Some("claude-haiku-4-5-20250929".into()) },
                notes: HashMap::from([
                    ("en".into(), "Official Anthropic endpoint. For users with separate API accounts.".into()),
                ]),
                trial_ends_at: None,
                last_verified: "2026-05-01".into(),
            },
            ProviderDefinition {
                id: "zai".into(),
                display_name: "Z.ai (Zhipu) — FREE FOREVER".into(),
                tier: Tier::Free,
                base_url: Some("https://api.z.ai/api/anthropic".into()),
                env_style: EnvStyle::Anthropic,
                auth: AuthConfig {
                    env_var: "ANTHROPIC_AUTH_TOKEN".into(),
                    key_url: "https://z.ai/manage-apikey/apikey-list".into(),
                    needs_key: true,
                },
                models: vec![
                    ModelDefinition {
                        id: "glm-4.7-flash".into(),
                        role: ModelRole::Main,
                        free: true,
                        context: 203000,
                        note: "MoE 30B active 3B — strong coding, free forever".into(),
                        use_cases: vec!["coding-fast".into(), "free-trial".into(), "long-context".into()],
                        pricing: None,
                    },
                    ModelDefinition {
                        id: "glm-4.5-flash".into(),
                        role: ModelRole::Small,
                        free: true,
                        context: 131000,
                        note: "Lightweight fast cousin".into(),
                        use_cases: vec!["coding-fast".into(), "free-trial".into(), "cheap-batch".into()],
                        pricing: None,
                    },
                ],
                defaults: DefaultModels { main: Some("glm-4.7-flash".into()), small: Some("glm-4.5-flash".into()) },
                notes: HashMap::from([
                    ("en".into(), "Strongest free option. Policy may change long-term.".into()),
                ]),
                trial_ends_at: None,
                last_verified: "2026-05-01".into(),
            },
            ProviderDefinition {
                id: "minimax".into(),
                display_name: "MiniMax — FREE TRIAL (until Nov 7 2026)".into(),
                tier: Tier::Trial,
                base_url: Some("https://api.minimax.io/anthropic".into()),
                env_style: EnvStyle::Anthropic,
                auth: AuthConfig {
                    env_var: "ANTHROPIC_AUTH_TOKEN".into(),
                    key_url: "https://platform.minimax.io/user-center/basic-information/interface-key".into(),
                    needs_key: true,
                },
                models: vec![
                    ModelDefinition {
                        id: "MiniMax-M2.7".into(),
                        role: ModelRole::Main,
                        free: true,
                        context: 204800,
                        note: "60 tps — matches Claude Opus 4.6 level (56% SWE-Pro)".into(),
                        use_cases: vec!["refactor".into(), "long-context".into(), "multi-agent".into(), "free-trial".into()],
                        pricing: Some(ModelPricing { input_per_1m: 0.0, output_per_1m: 0.0, trial: true }),
                    },
                    ModelDefinition {
                        id: "MiniMax-M2.7-highspeed".into(),
                        role: ModelRole::Small,
                        free: true,
                        context: 204800,
                        note: "100 tps highspeed variant".into(),
                        use_cases: vec!["coding-fast".into(), "cheap-agent".into(), "cheap-batch".into(), "free-trial".into()],
                        pricing: Some(ModelPricing { input_per_1m: 0.0, output_per_1m: 0.0, trial: true }),
                    },
                ],
                defaults: DefaultModels { main: Some("MiniMax-M2.7".into()), small: Some("MiniMax-M2.7-highspeed".into()) },
                notes: HashMap::from([
                    ("en".into(), "Strongest trial-free model. Switches to paid after trial ends.".into()),
                ]),
                trial_ends_at: Some("2026-11-07".into()),
                last_verified: "2026-05-01".into(),
            },
            ProviderDefinition {
                id: "deepseek".into(),
                display_name: "DeepSeek — Cheap".into(),
                tier: Tier::Paid,
                base_url: Some("https://api.deepseek.com/anthropic".into()),
                env_style: EnvStyle::Anthropic,
                auth: AuthConfig {
                    env_var: "ANTHROPIC_AUTH_TOKEN".into(),
                    key_url: "https://platform.deepseek.com/api_keys".into(),
                    needs_key: true,
                },
                models: vec![
                    ModelDefinition {
                        id: "deepseek-v4-pro".into(),
                        role: ModelRole::Main,
                        free: false,
                        context: 128000,
                        note: "Main model for heavy coding ~10x cheaper than Sonnet".into(),
                        use_cases: vec!["refactor".into(), "cheap-batch".into()],
                        pricing: Some(ModelPricing { input_per_1m: 0.27, output_per_1m: 1.10, trial: false }),
                    },
                    ModelDefinition {
                        id: "deepseek-v4-flash".into(),
                        role: ModelRole::Small,
                        free: false,
                        context: 128000,
                        note: "Background/cheap tasks".into(),
                        use_cases: vec!["coding-fast".into(), "cheap-agent".into(), "cheap-batch".into()],
                        pricing: Some(ModelPricing { input_per_1m: 0.07, output_per_1m: 0.27, trial: false }),
                    },
                ],
                defaults: DefaultModels { main: Some("deepseek-v4-pro".into()), small: Some("deepseek-v4-flash".into()) },
                notes: HashMap::from([
                    ("en".into(), "Paid but cheap. 10x cheaper on cache hits. Most cost-effective for serious coding.".into()),
                ]),
                trial_ends_at: None,
                last_verified: "2026-05-01".into(),
            },
            ProviderDefinition {
                id: "moonshot".into(),
                display_name: "Moonshot Kimi".into(),
                tier: Tier::Paid,
                base_url: Some("https://api.moonshot.ai/anthropic".into()),
                env_style: EnvStyle::Anthropic,
                auth: AuthConfig {
                    env_var: "ANTHROPIC_AUTH_TOKEN".into(),
                    key_url: "https://platform.moonshot.ai/console/api-keys".into(),
                    needs_key: true,
                },
                models: vec![
                    ModelDefinition {
                        id: "kimi-k2.5".into(),
                        role: ModelRole::Main,
                        free: false,
                        context: 200000,
                        note: "1T param MoE — optimized for long context".into(),
                        use_cases: vec!["long-context".into(), "refactor".into()],
                        pricing: Some(ModelPricing { input_per_1m: 0.6, output_per_1m: 2.5, trial: false }),
                    },
                ],
                defaults: DefaultModels { main: Some("kimi-k2.5".into()), small: Some("kimi-k2.5".into()) },
                notes: HashMap::from([
                    ("en".into(), "Strong on long-context (>200K) work.".into()),
                ]),
                trial_ends_at: None,
                last_verified: "2026-05-01".into(),
            },
            ProviderDefinition {
                id: "openrouter".into(),
                display_name: "OpenRouter — 32 free models".into(),
                tier: Tier::Free,
                base_url: Some("https://openrouter.ai/api".into()),
                env_style: EnvStyle::Anthropic,
                auth: AuthConfig {
                    env_var: "ANTHROPIC_AUTH_TOKEN".into(),
                    key_url: "https://openrouter.ai/settings/keys".into(),
                    needs_key: true,
                },
                models: vec![
                    ModelDefinition {
                        id: "openrouter/qwen3-coder:free".into(),
                        role: ModelRole::Main,
                        free: true,
                        context: 262000,
                        note: "MOST POWERFUL free coder — Qwen3-Coder 480B".into(),
                        use_cases: vec!["coding-fast".into(), "free-trial".into(), "refactor".into()],
                        pricing: None,
                    },
                    ModelDefinition {
                        id: "openrouter/z-ai/glm-4.5-air:free".into(),
                        role: ModelRole::Small,
                        free: true,
                        context: 131000,
                        note: "Light GLM, fast".into(),
                        use_cases: vec!["coding-fast".into(), "free-trial".into(), "cheap-batch".into()],
                        pricing: None,
                    },
                    ModelDefinition {
                        id: "openrouter/openai/gpt-oss-120b:free".into(),
                        role: ModelRole::Main,
                        free: true,
                        context: 131000,
                        note: "OpenAI open-source — strong reasoning".into(),
                        use_cases: vec!["refactor".into(), "multi-agent".into(), "free-trial".into()],
                        pricing: None,
                    },
                    ModelDefinition {
                        id: "openrouter/google/gemma-4-31b-it:free".into(),
                        role: ModelRole::Main,
                        free: true,
                        context: 262000,
                        note: "Vision + tool use".into(),
                        use_cases: vec!["vision".into(), "coding-fast".into(), "free-trial".into()],
                        pricing: None,
                    },
                    ModelDefinition {
                        id: "openrouter/anthropic/claude-sonnet-4.5".into(),
                        role: ModelRole::Main,
                        free: false,
                        context: 200000,
                        note: "Official Sonnet 4.5 via OpenRouter — paid".into(),
                        use_cases: vec!["refactor".into(), "multi-agent".into()],
                        pricing: Some(ModelPricing { input_per_1m: 3.0, output_per_1m: 15.0, trial: false }),
                    },
                    ModelDefinition {
                        id: "openrouter/anthropic/claude-opus-4.7".into(),
                        role: ModelRole::Main,
                        free: false,
                        context: 200000,
                        note: "Official Opus 4.7 via OpenRouter — paid".into(),
                        use_cases: vec!["refactor".into(), "multi-agent".into(), "long-context".into()],
                        pricing: Some(ModelPricing { input_per_1m: 15.0, output_per_1m: 75.0, trial: false }),
                    },
                ],
                defaults: DefaultModels { main: Some("openrouter/qwen3-coder:free".into()), small: Some("openrouter/z-ai/glm-4.5-air:free".into()) },
                notes: HashMap::from([
                    ("en".into(), "One key, many providers. 20 RPM / 200 req daily per free model.".into()),
                ]),
                trial_ends_at: None,
                last_verified: "2026-05-01".into(),
            },
        ],
    }
}

/// Resolve a user-facing short model ID to its canonical form.
/// e.g. "qwen3-coder:free" → "openrouter/qwen3-coder:free"
/// e.g. "claude-sonnet-4-5-20250929" → "claude-sonnet-4-5-20250929" (anthropic, no prefix)
#[allow(dead_code)]
pub fn resolve_model_id(short: &str, catalog: &ProviderCatalog) -> Option<String> {
    // Direct match first
    for p in &catalog.providers {
        for m in &p.models {
            if m.id == short {
                return Some(m.id.clone());
            }
        }
    }

    // Try prefixing with provider ID
    for p in &catalog.providers {
        let prefixed = format!("{}/{}", p.id, short);
        for m in &p.models {
            if m.id == prefixed {
                return Some(prefixed);
            }
        }
    }

    // Try known short names
    let short_names: HashMap<String, &str> = HashMap::from([
        ("qwen3-coder:free".to_string(), "openrouter/qwen3-coder:free"),
        ("glm-4.5-air:free".to_string(), "openrouter/z-ai/glm-4.5-air:free"),
        ("gpt-oss-120b:free".to_string(), "openrouter/openai/gpt-oss-120b:free"),
        ("gemma-4-31b-it:free".to_string(), "openrouter/google/gemma-4-31b-it:free"),
        ("sonnet-4.5".to_string(), "openrouter/anthropic/claude-sonnet-4.5"),
        ("opus-4.7".to_string(), "openrouter/anthropic/claude-opus-4.7"),
    ]);

    if let Some(resolved) = short_names.get(short) {
        return Some(resolved.to_string());
    }

    None
}
