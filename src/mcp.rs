/// MCP (Model Context Protocol) server for costcut.
///
/// Exposes costcut functionality as slash commands in Claude Code, OpenCode, etc.
/// Tools: status, recommend_model, switch_instructions

use rmcp::{
    model::ServerInfo,
    tool, ServerHandler,
};
use serde::{Deserialize, Serialize};

use crate::analyzer::{analyze, AnalysisResult};
use crate::config::{AvailableModel, WitcherConfig};
use crate::selector::{select_model, SelectionResult};
use crate::setup;

/// Response for the status tool
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct StatusResponse {
    pub strategy: String,
    pub interactive_model: Option<String>,
    pub configured_providers: Vec<String>,
    pub available_models: Vec<ModelInfo>,
    pub shell_wrappers_active: bool,
    pub banned_models: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ModelInfo {
    pub id: String,
    pub provider: String,
    pub display_name: String,
    pub tier: String,
    pub free: bool,
    pub context: u64,
    pub cost_per_1m_input: f64,
    pub cost_per_1m_output: f64,
}

impl From<&AvailableModel> for ModelInfo {
    fn from(m: &AvailableModel) -> Self {
        ModelInfo {
            id: m.id.clone(),
            provider: m.provider_id.clone(),
            display_name: m.display_name.clone(),
            tier: m.tier.clone(),
            free: m.free,
            context: m.context,
            cost_per_1m_input: m.cost_per_1m_input,
            cost_per_1m_output: m.cost_per_1m_output,
        }
    }
}

/// Response for the recommend tool
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RecommendResponse {
    pub complexity_score: f64,
    pub complexity_tier: String,
    pub estimated_tokens: usize,
    pub use_cases: Vec<String>,
    pub needs_vision: bool,
    pub selection: Option<SelectionInfo>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SelectionInfo {
    pub model_id: String,
    pub display_name: String,
    pub provider: String,
    pub tier: String,
    pub free: bool,
    pub reasoning: Vec<String>,
    pub estimated_cost_input: f64,
    pub estimated_cost_output: f64,
}

impl SelectionInfo {
    fn from_result(sel: &SelectionResult) -> Self {
        SelectionInfo {
            model_id: sel.model.id.clone(),
            display_name: sel.model.display_name.clone(),
            provider: sel.model.provider_id.clone(),
            tier: sel.model.tier.clone(),
            free: sel.model.free,
            reasoning: sel.reasoning.clone(),
            estimated_cost_input: sel.estimated_cost_input,
            estimated_cost_output: sel.estimated_cost_output,
        }
    }
}

/// Response for the switch tool
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SwitchResponse {
    pub success: bool,
    pub instruction: String,
    pub message: String,
    pub model_info: Option<ModelInfo>,
}

/// Parameter for recommend_model tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RecommendParams {
    #[schemars(description = "The task or prompt to analyze for model recommendation")]
    pub task: String,
}

/// Parameter for switch_instructions tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SwitchParams {
    #[schemars(description = "The model ID to switch to (e.g., 'deepseek-v4-pro', 'claude-sonnet-4-5-20250929')")]
    pub model: String,
}

/// The costcut MCP server
#[derive(Clone)]
pub struct CostcutMcpServer;

#[tool(tool_box)]
impl CostcutMcpServer {
    pub fn new() -> Self {
        Self
    }

    /// Get current costcut configuration status
    #[tool(description = "Show current costcut configuration: strategy, available models, credentials")]
    async fn status(&self) -> String {
        let config = WitcherConfig::load().unwrap_or_default();
        let available = config.available_models();

        let response = StatusResponse {
            strategy: match config.strategy {
                crate::config::Strategy::FreeFirst => "free-first".into(),
                crate::config::Strategy::CostConscious => "cost-conscious".into(),
                crate::config::Strategy::BestQuality => "best-quality".into(),
            },
            interactive_model: config.interactive_model.clone(),
            configured_providers: config.credentials.keys().cloned().collect(),
            available_models: available.iter().map(ModelInfo::from).collect(),
            shell_wrappers_active: setup::find_rc_file()
                .map_or(false, |p| setup::has_costcut_block(&p)),
            banned_models: config.banned_models.clone(),
        };

        format_status(&response)
    }

    /// Analyze task complexity and recommend optimal model
    #[tool(description = "Analyze a task description and recommend the optimal AI model based on complexity")]
    async fn recommend_model(
        &self,
        #[tool(aggr)] RecommendParams { task }: RecommendParams,
    ) -> String {
        if task.trim().is_empty() {
            return "Error: No task provided. Please provide a task description to analyze.".into();
        }

        let config = WitcherConfig::load().unwrap_or_default();
        let available = config.available_models();

        if available.is_empty() {
            return "Error: No models available. Add credentials with 'costcut add-credential <provider>'.".into();
        }

        let analysis = analyze(&task);
        let selection = select_model(&analysis, &config, &available);

        let response = RecommendResponse {
            complexity_score: (analysis.complexity_score * 100.0).round(),
            complexity_tier: analysis.tier.as_str().to_string(),
            estimated_tokens: analysis.estimated_tokens,
            use_cases: analysis.use_cases.clone(),
            needs_vision: analysis.needs_vision,
            selection: selection.as_ref().map(SelectionInfo::from_result),
        };

        format_recommendation(&response, &analysis)
    }

    /// Get instructions to switch to a different model
    #[tool(description = "Get instructions to switch to a specific AI model mid-session")]
    async fn switch_instructions(
        &self,
        #[tool(aggr)] SwitchParams { model }: SwitchParams,
    ) -> String {
        let config = WitcherConfig::load().unwrap_or_default();
        let available = config.available_models();

        let model_info = available.iter().find(|m| m.id == model);

        let response = match model_info {
            Some(m) => SwitchResponse {
                success: true,
                instruction: format!("/model {}", model),
                message: format!(
                    "To switch to {}, run the /model command in Claude Code.\n\
                     Note: costcut cannot change models programmatically.",
                    m.display_name
                ),
                model_info: Some(ModelInfo::from(m)),
            },
            None => {
                let available_list = available
                    .iter()
                    .map(|m| format!("  - {}", m.id))
                    .collect::<Vec<_>>()
                    .join("\n");

                SwitchResponse {
                    success: false,
                    instruction: String::new(),
                    message: format!(
                        "Model '{}' not found.\n\nAvailable models:\n{}",
                        model, available_list
                    ),
                    model_info: None,
                }
            }
        };

        format_switch(&response)
    }
}

#[tool(tool_box)]
impl ServerHandler for CostcutMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some("Costcut - intelligent model switcher for AI coding assistants".into()),
            ..Default::default()
        }
    }
}

fn format_status(status: &StatusResponse) -> String {
    let mut lines = vec![
        "=== Costcut Status ===".to_string(),
        String::new(),
        format!("Strategy: {} ({})", status.strategy, strategy_description(&status.strategy)),
    ];

    if let Some(ref im) = status.interactive_model {
        lines.push(format!("Interactive model: {}", im));
    }

    lines.push(String::new());
    lines.push("Configured Providers:".to_string());
    if status.configured_providers.is_empty() {
        lines.push("  (none — add with 'costcut add-credential <provider>')".to_string());
    } else {
        for provider in &status.configured_providers {
            lines.push(format!("  ✓ {}", provider));
        }
    }

    lines.push(String::new());
    lines.push("Available Models:".to_string());
    if status.available_models.is_empty() {
        lines.push("  (none — add credentials first)".to_string());
    } else {
        for m in &status.available_models {
            let cost = if m.free {
                "FREE".to_string()
            } else {
                format!("${:.2}/M in", m.cost_per_1m_input)
            };
            lines.push(format!(
                "  {} {} {}K ctx — {}",
                m.id,
                cost,
                m.context / 1000,
                m.provider
            ));
        }
    }

    lines.push(String::new());
    lines.push(format!(
        "Shell wrappers: {}",
        if status.shell_wrappers_active {
            "active"
        } else {
            "inactive"
        }
    ));

    if !status.banned_models.is_empty() {
        lines.push(String::new());
        lines.push("Banned models:".to_string());
        for bm in &status.banned_models {
            lines.push(format!("  ✗ {}", bm));
        }
    }

    lines.join("\n")
}

fn strategy_description(strategy: &str) -> &str {
    match strategy {
        "free-first" => "prefer free models",
        "cost-conscious" => "balance cost and quality",
        "best-quality" => "use most capable",
        _ => "",
    }
}

fn format_recommendation(response: &RecommendResponse, analysis: &AnalysisResult) -> String {
    let mut lines = vec![
        "=== Request Analysis ===".to_string(),
        String::new(),
        format!(
            "Complexity: {:.0}% — {}",
            response.complexity_score, response.complexity_tier
        ),
        format!("Tokens: ~{}", response.estimated_tokens),
        format!("Use cases: {}", response.use_cases.join(", ")),
    ];

    if response.needs_vision {
        lines.push("Vision: Yes — vision-capable model recommended".to_string());
    }

    lines.push(String::new());
    lines.push("Factor Breakdown:".to_string());
    for factor in &analysis.breakdown {
        let bar_len = (factor.score * 10.0) as usize;
        let bar = "█".repeat(bar_len) + &"░".repeat(10 - bar_len);
        lines.push(format!(
            "  {:>18}: [{}] {:.0}% {}",
            factor.name,
            bar,
            factor.score * 100.0,
            factor.detail
        ));
    }

    lines.push(String::new());
    lines.push("=== Recommendation ===".to_string());
    lines.push(String::new());

    match &response.selection {
        Some(sel) => {
            lines.push(format!("Model: {}", sel.display_name));
            lines.push(format!("Provider: {}", sel.provider));
            lines.push(format!("Tier: {}", sel.tier));

            if sel.free {
                lines.push("Cost: FREE".to_string());
            } else {
                lines.push(format!(
                    "Est. cost: ${:.6} input + ${:.6} output",
                    sel.estimated_cost_input, sel.estimated_cost_output
                ));
            }

            lines.push(String::new());
            lines.push("Why:".to_string());
            for reason in &sel.reasoning {
                lines.push(format!("  • {}", reason));
            }
        }
        None => {
            lines.push(
                "No suitable model found. Add credentials with 'costcut add-credential'.".to_string(),
            );
        }
    }

    lines.join("\n")
}

fn format_switch(response: &SwitchResponse) -> String {
    let mut lines = Vec::new();

    if response.success {
        lines.push(format!(
            "To switch to {}:",
            response
                .model_info
                .as_ref()
                .map(|m| m.display_name.as_str())
                .unwrap_or("the model")
        ));
        lines.push(String::new());
        lines.push(format!("  Run: {}", response.instruction));
        lines.push(String::new());
        lines.push(
            "Note: costcut cannot change Claude Code's model programmatically.".to_string(),
        );
        lines.push("The /model command must be run manually.".to_string());

        if let Some(ref m) = response.model_info {
            lines.push(String::new());
            lines.push("Model Info:".to_string());
            lines.push(format!("  Provider: {}", m.provider));
            lines.push(format!("  Context: {}K tokens", m.context / 1000));
            if m.free {
                lines.push("  Cost: FREE".to_string());
            } else {
                lines.push(format!(
                    "  Cost: ${:.2}/M input | ${:.2}/M output",
                    m.cost_per_1m_input, m.cost_per_1m_output
                ));
            }
        }
    } else {
        lines.push(response.message.clone());
    }

    lines.join("\n")
}

/// Run the MCP server with stdio transport
pub async fn run_server() -> Result<(), Box<dyn std::error::Error>> {
    use rmcp::ServiceExt;
    use tokio::io::{stdin, stdout};

    // Initialize tracing for debugging (to stderr so it doesn't interfere with stdio)
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::WARN.into()),
        )
        .with_writer(std::io::stderr)
        .init();

    let server = CostcutMcpServer::new();
    let transport = (stdin(), stdout());
    let service = server.serve(transport).await?;
    service.waiting().await?;

    Ok(())
}
