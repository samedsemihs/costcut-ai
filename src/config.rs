use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;

use crate::providers::bundled_catalog;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitcherConfig {
    pub schema_version: u32,

    /// User's model choice strategy
    #[serde(default)]
    pub strategy: Strategy,

    /// Default model for interactive/REPL sessions (no prompt to analyze)
    #[serde(default)]
    pub interactive_model: Option<String>,

    /// Available tools to launch (claude_code, opencode, pi)
    #[serde(default)]
    pub tools: Vec<ToolConfig>,

    /// User-configured API keys per provider
    #[serde(default)]
    pub credentials: HashMap<String, Credential>,

    /// Explicit model overrides (per use-case, per complexity tier)
    #[serde(default)]
    pub overrides: ModelOverrides,

    /// Custom models the user wants to add
    #[serde(default)]
    pub custom_models: Vec<CustomModel>,

    /// Models the user has banned from selection
    #[serde(default)]
    pub banned_models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Strategy {
    /// Always use free models when possible
    #[default]
    FreeFirst,
    /// Balance cost and quality
    CostConscious,
    /// Always use the most capable model
    BestQuality,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfig {
    /// Tool name: "claude_code", "opencode", "pi"
    pub name: String,
    /// Path to the binary (auto-detected if absent)
    pub binary: Option<String>,
    /// Whether this tool is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credential {
    /// API key
    pub key: String,
    /// Override base URL (if different from provider default)
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelOverrides {
    /// Force specific model for given use-case
    pub use_case: HashMap<String, String>,
    /// Force specific model for complexity tier: "low", "medium", "high", "extreme"
    pub complexity_tier: HashMap<String, String>,
    /// Default model to use when no rule matches
    pub default_model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomModel {
    pub id: String,
    pub display_name: String,
    pub provider_id: String,
    pub base_url: String,
    pub tier: String,
    pub context: u64,
    pub free: bool,
    pub use_cases: Vec<String>,
    pub pricing_input_per_1m: Option<f64>,
    pub pricing_output_per_1m: Option<f64>,
}

impl WitcherConfig {
    pub fn default_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("costcut")
            .join("config.toml")
    }

    pub fn dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("costcut")
    }

    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let path = Self::default_path();
        if path.exists() {
            let content = fs::read_to_string(&path)?;
            let config: WitcherConfig = toml::from_str(&content)?;
            Ok(config)
        } else {
            Ok(WitcherConfig::default())
        }
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let dir = Self::dir();
        fs::create_dir_all(&dir)?;
        let path = Self::default_path();
        let content = toml::to_string_pretty(self)?;
        fs::write(&path, content)?;

        // Set permissions on config dir (Unix only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
        }

        Ok(())
    }

    /// List all models the user has access to (credentials configured)
    pub fn available_models(&self) -> Vec<AvailableModel> {
        let catalog = bundled_catalog();
        let mut models = Vec::new();

        // Anthropic is available if the user has the ANTHROPIC_API_KEY env var
        // or has explicitly added credentials via `costcut add-credential anthropic`
        let anthropic_available = self.credentials.contains_key("anthropic")
            || env::var("ANTHROPIC_API_KEY").is_ok()
            || env::var("ANTHROPIC_AUTH_TOKEN").is_ok();

        for provider in &catalog.providers {
            let has_creds = self.credentials.contains_key(&provider.id)
                || (provider.id == "anthropic" && anthropic_available);

            if !has_creds {
                continue;
            }

            for model in &provider.models {
                // Skip banned models
                if self.banned_models.contains(&model.id) {
                    continue;
                }

                let cred = self.credentials.get(&provider.id);

                models.push(AvailableModel {
                    id: model.id.clone(),
                    provider_id: provider.id.clone(),
                    display_name: format!("{}/{}", provider.display_name, model.id),
                    tier: if model.free {
                        "free".into()
                    } else {
                        match provider.tier {
                            crate::providers::Tier::Free => "free",
                            crate::providers::Tier::Trial => "trial",
                            crate::providers::Tier::Paid => "paid",
                        }
                        .into()
                    },
                    context: model.context,
                    free: model.free,
                    use_cases: model.use_cases.clone(),
                    role: match model.role {
                        crate::providers::ModelRole::Main => "main",
                        crate::providers::ModelRole::Small => "small",
                    }
                    .into(),
                    cost_per_1m_input: model
                        .pricing
                        .as_ref()
                        .map(|p| p.input_per_1m)
                        .unwrap_or(0.0),
                    cost_per_1m_output: model
                        .pricing
                        .as_ref()
                        .map(|p| p.output_per_1m)
                        .unwrap_or(0.0),
                    base_url: cred
                        .and_then(|c| c.base_url.clone())
                        .or_else(|| provider.base_url.clone()),
                });
            }
        }

        // Add custom models
        for cm in &self.custom_models {
            let cred = self.credentials.get(&cm.provider_id);
            models.push(AvailableModel {
                id: cm.id.clone(),
                provider_id: cm.provider_id.clone(),
                display_name: format!("{}/{}", cm.display_name, cm.id),
                tier: cm.tier.clone(),
                context: cm.context,
                free: cm.free,
                use_cases: cm.use_cases.clone(),
                role: "main".into(),
                cost_per_1m_input: cm.pricing_input_per_1m.unwrap_or(0.0),
                cost_per_1m_output: cm.pricing_output_per_1m.unwrap_or(0.0),
                base_url: cred
                    .and_then(|c| c.base_url.clone())
                    .or(Some(cm.base_url.clone())),
            });
        }

        models
    }
}

impl Default for WitcherConfig {
    fn default() -> Self {
        WitcherConfig {
            schema_version: 1,
            strategy: Strategy::CostConscious,
            interactive_model: None,
            tools: vec![
                ToolConfig {
                    name: "claude_code".into(),
                    binary: None,
                    enabled: true,
                },
                ToolConfig {
                    name: "opencode".into(),
                    binary: None,
                    enabled: true,
                },
                ToolConfig {
                    name: "pi".into(),
                    binary: None,
                    enabled: true,
                },
            ],
            credentials: HashMap::new(),
            overrides: ModelOverrides::default(),
            custom_models: Vec::new(),
            banned_models: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailableModel {
    pub id: String,
    pub provider_id: String,
    pub display_name: String,
    pub tier: String,
    pub context: u64,
    pub free: bool,
    pub use_cases: Vec<String>,
    pub role: String,
    pub cost_per_1m_input: f64,
    pub cost_per_1m_output: f64,
    pub base_url: Option<String>,
}
