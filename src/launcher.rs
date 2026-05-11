use std::env;
use std::process::{Command, Stdio};

use crate::config::{AvailableModel, WitcherConfig};

/// Tool type enum
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tool {
    ClaudeCode,
    OpenCode,
    Pi,
}

impl Tool {
    pub fn name(&self) -> &str {
        match self {
            Tool::ClaudeCode => "claude_code",
            Tool::OpenCode => "opencode",
            Tool::Pi => "pi",
        }
    }

    pub fn binary_name(&self) -> &str {
        match self {
            Tool::ClaudeCode => "claude",
            Tool::OpenCode => "opencode",
            Tool::Pi => "pi",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "claude_code" | "claude" | "claude-code" => Some(Tool::ClaudeCode),
            "opencode" => Some(Tool::OpenCode),
            "pi" => Some(Tool::Pi),
            _ => None,
        }
    }

    /// Environmental variables the tool uses for API configuration
    pub fn env_vars_for_model(&self, model: &AvailableModel, config: &WitcherConfig) -> Vec<(&str, String)> {
        let cred = config.credentials.get(&model.provider_id);
        let api_key = cred.map(|c| c.key.clone()).unwrap_or_default();
        let base_url = model
            .base_url
            .clone()
            .or_else(|| cred.and_then(|c| c.base_url.clone()));

        match self {
            Tool::ClaudeCode => {
                let mut vars = vec![
                    ("ANTHROPIC_MODEL", model.id.clone()),
                ];

                if let Some(url) = base_url {
                    vars.push(("ANTHROPIC_BASE_URL", url));
                }

                if !api_key.is_empty() {
                    vars.push(("ANTHROPIC_AUTH_TOKEN", api_key));
                }

                // Also set OPENAI-compatible vars for codex/openrouter
                if model.provider_id == "openrouter" {
                    vars.push(("OPENAI_API_KEY", cred.map(|c| c.key.clone()).unwrap_or_default()));
                    if let Some(url) = &model.base_url {
                        vars.push(("OPENAI_BASE_URL", url.clone()));
                    }
                }

                vars
            }
            Tool::OpenCode => {
                // OpenCode uses the same env vars as Claude Code
                let mut vars = vec![
                    ("ANTHROPIC_MODEL", model.id.clone()),
                ];

                if let Some(url) = base_url {
                    vars.push(("ANTHROPIC_BASE_URL", url));
                }

                if !api_key.is_empty() {
                    vars.push(("ANTHROPIC_AUTH_TOKEN", api_key));
                }

                vars
            }
            Tool::Pi => {
                // Pi also uses Anthropic-compatible env vars
                let mut vars = vec![
                    ("ANTHROPIC_MODEL", model.id.clone()),
                ];

                if let Some(url) = base_url {
                    vars.push(("ANTHROPIC_BASE_URL", url));
                }

                if !api_key.is_empty() {
                    vars.push(("ANTHROPIC_AUTH_TOKEN", api_key));
                }

                vars
            }
        }
    }
}

/// Launch a tool with the selected model.
/// On success, returns the exit code. On failure, returns an error.
pub fn launch_tool(
    tool: Tool,
    model: &AvailableModel,
    config: &WitcherConfig,
    prompt: &str,
    extra_args: &[String],
) -> Result<i32, Box<dyn std::error::Error>> {
    let tool_config = config.tools.iter().find(|t| {
        Tool::from_str(&t.name).map(|tt| tt == tool).unwrap_or(false)
    });

    if let Some(tc) = tool_config {
        if !tc.enabled {
            return Err(format!("Tool '{}' is disabled in config", tool.name()).into());
        }
    }

    let binary = resolve_binary(tool, config);

    let env_vars = tool.env_vars_for_model(model, config);

    let mut cmd = Command::new(&binary);

    // Set all env vars
    for (key, val) in &env_vars {
        cmd.env(key, val);
    }

    // Forward relevant env vars not explicitly set
    for (key, val) in env::vars() {
        if !env_vars.iter().any(|(k, _)| k == &key) {
            // Only forward safe vars
            if key.starts_with("PATH")
                || key.starts_with("HOME")
                || key.starts_with("USER")
                || key.starts_with("LANG")
                || key.starts_with("TERM")
                || key.starts_with("SHELL")
                || key.starts_with("SSH_")
                || key.starts_with("DISPLAY")
                || key == "COLORTERM"
                || key == "NO_COLOR"
            {
                cmd.env(key, val);
            }
        }
    }

    // Add tool-specific args
    match tool {
        Tool::ClaudeCode => {
            cmd.arg("-p");
            cmd.arg(prompt);
        }
        Tool::OpenCode => {
            // OpenCode can take a prompt inline
            cmd.arg(prompt);
        }
        Tool::Pi => {
            cmd.arg(prompt);
        }
    }

    // Add any extra args
    for arg in extra_args {
        cmd.arg(arg);
    }

    // Print summary
    println!(
        "{} Launching {} with model: {}",
        colored::Colorize::bright_blue("→"),
        tool.name(),
        colored::Colorize::bright_green(model.display_name.as_str())
    );
    println!(
        "{}   Provider: {} | Tier: {} | Context: {}K",
        colored::Colorize::dimmed(" "),
        model.provider_id,
        model.tier,
        model.context / 1000
    );

    if model.cost_per_1m_input > 0.0 {
        println!(
            "{}   Cost: ${:.2}/M input | ${:.2}/M output",
            colored::Colorize::dimmed(" "),
            model.cost_per_1m_input,
            model.cost_per_1m_output
        );
    } else if model.free {
        println!("{}   Cost: FREE", colored::Colorize::dimmed(" "));
    }

    let env_display: Vec<String> = env_vars.iter().map(|(k, v)| {
        if k.contains("KEY") || k.contains("TOKEN") || k.contains("SECRET") {
            format!("{}=***redacted***", k)
        } else {
            format!("{}={}", k, v)
        }
    }).collect();

    println!(
        "{}   Env: {}",
        colored::Colorize::dimmed(" "),
        env_display.join(" ")
    );
    println!("{}   Binary: {}", colored::Colorize::dimmed(" "), binary);
    println!();

    let status = cmd
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;

    Ok(status.code().unwrap_or(0))
}

/// Exec a tool with the selected model — silent, for shell wrapper use.
/// Replaces the current process on Unix; spawns on Windows.
pub fn exec_tool(
    tool: Tool,
    model: &AvailableModel,
    config: &WitcherConfig,
    prompt: &str,
    extra_args: &[String],
) -> Result<i32, Box<dyn std::error::Error>> {
    let binary = resolve_binary(tool, config);
    let env_vars = tool.env_vars_for_model(model, config);

    let mut cmd = Command::new(&binary);

    for (key, val) in &env_vars {
        cmd.env(key, val);
    }

    // Forward safe env vars
    for (key, val) in env::vars() {
        if !env_vars.iter().any(|(k, _)| k == &key) {
            if key.starts_with("PATH")
                || key.starts_with("HOME")
                || key.starts_with("USER")
                || key.starts_with("LANG")
                || key.starts_with("TERM")
                || key.starts_with("SHELL")
                || key.starts_with("SSH_")
                || key.starts_with("DISPLAY")
                || key == "COLORTERM"
                || key == "NO_COLOR"
            {
                cmd.env(key, val);
            }
        }
    }

    match tool {
        Tool::ClaudeCode => {
            cmd.arg("-p");
            cmd.arg(prompt);
        }
        Tool::OpenCode => {
            cmd.arg(prompt);
        }
        Tool::Pi => {
            cmd.arg(prompt);
        }
    }

    for arg in extra_args {
        cmd.arg(arg);
    }

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = cmd.exec();
        Err(Box::new(err))
    }

    #[cfg(not(unix))]
    {
        let status = cmd
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()?;
        Ok(status.code().unwrap_or(0))
    }
}

/// Resolve the binary path for a tool.
fn resolve_binary(tool: Tool, config: &WitcherConfig) -> String {
    // Check user configuration first
    if let Some(tc) = config.tools.iter().find(|t| {
        Tool::from_str(&t.name).map(|tt| tt == tool).unwrap_or(false)
    }) {
        if let Some(ref bin) = tc.binary {
            if !bin.is_empty() {
                return bin.clone();
            }
        }
    }

    // Fall back to default binary name (resolve via PATH)
    tool.binary_name().to_string()
}

/// Detect which tools are available on the system.
pub fn detect_available_tools() -> Vec<Tool> {
    let mut available = Vec::new();

    for tool in &[Tool::ClaudeCode, Tool::OpenCode, Tool::Pi] {
        if which::which(tool.binary_name()).is_ok() {
            available.push(*tool);
        }
    }

    available
}

/// Print a launch summary without executing.
pub fn print_launch_info(
    tool: Tool,
    model: &AvailableModel,
    config: &WitcherConfig,
) {
    println!();
    println!("{} ===== Launch Summary =====", colored::Colorize::bold(""));
    println!(
        "  Tool:     {} ({})",
        colored::Colorize::bright_cyan(tool.name()),
        resolve_binary(tool, config)
    );
    println!(
        "  Model:    {}",
        colored::Colorize::bright_green(model.display_name.as_str())
    );
    println!("  Provider: {}", model.provider_id);
    println!("  Tier:     {}", model.tier);
    println!("  Context:  {}K tokens", model.context / 1000);

    if model.free {
        println!("  Cost:     FREE");
    } else {
        println!(
            "  Cost:     ${:.2}/M input | ${:.2}/M output",
            model.cost_per_1m_input,
            model.cost_per_1m_output
        );
    }

    let env_vars = tool.env_vars_for_model(model, config);
    println!("  Env vars:");
    for (key, val) in &env_vars {
        if key.contains("KEY") || key.contains("TOKEN") || key.contains("SECRET") {
            println!("    {}=***redacted***", key);
        } else {
            println!("    {}={}", key, val);
        }
    }

    println!();
    println!(
        "{} Run: {} {} \"<your prompt>\"",
        colored::Colorize::bright_yellow("  #"),
        resolve_binary(tool, config),
        if tool == Tool::ClaudeCode { "-p" } else { "" }
    );
    println!();
}
