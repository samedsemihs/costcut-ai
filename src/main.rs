mod analyzer;
mod config;
mod launcher;
mod mcp;
mod providers;
mod selector;
mod setup;

use std::io::{self, Read};

use clap::{Parser, Subcommand};
use colored::Colorize;

use analyzer::analyze;
use config::WitcherConfig;
use launcher::{detect_available_tools, exec_tool, launch_tool, print_launch_info, Tool};
use selector::select_model;

#[derive(Parser)]
#[command(
    name = "costcut",
    version,
    about = "Intelligent model switcher — analyzes request complexity and routes to the best AI model",
    long_about = "costcut analyzes your prompt, scores its complexity, and automatically selects \
                  the best model (Anthropic or DeepSeek). Seamlessly integrates with Claude Code, \
                  OpenCode, and Pi via shell wrappers."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Analyze a prompt and launch the best model with the selected tool
    Run {
        /// The prompt / request to analyze
        prompt: Vec<String>,

        /// Target tool: claude_code (default), opencode, or pi
        #[arg(short = 't', long, default_value = "claude_code")]
        tool: String,

        /// Extra arguments to pass to the tool
        #[arg(last = true)]
        extra_args: Vec<String>,

        /// Read prompt from stdin
        #[arg(short = 's', long, default_value = "false")]
        stdin: bool,

        /// Don't execute, just show what would happen
        #[arg(short = 'n', long = "dry-run")]
        dry_run: bool,
    },

    /// Called by shell wrappers — silent analysis + exec (internal use)
    Exec {
        /// Target tool name (claude, opencode, pi)
        #[arg(short = 't', long)]
        tool: String,

        /// The prompt to analyze
        prompt: Vec<String>,
    },

    /// Analyze a prompt and show the recommendation without launching
    Recommend {
        /// The prompt to analyze
        prompt: Vec<String>,

        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Read prompt from stdin
        #[arg(short = 's', long, default_value = "false")]
        stdin: bool,
    },

    /// Show detailed complexity analysis of a prompt
    Analyze {
        /// The prompt to analyze
        prompt: Vec<String>,

        /// Read prompt from stdin
        #[arg(short = 's', long, default_value = "false")]
        stdin: bool,
    },

    /// Initialize costcut config + shell wrappers
    Init {
        /// Overwrite existing config
        #[arg(long)]
        force: bool,

        /// Skip shell wrapper injection
        #[arg(long)]
        no_shell: bool,
    },

    /// Remove shell wrappers from your rc file
    Uninit {
        /// Also remove config and credentials
        #[arg(long)]
        purge: bool,
    },

    /// Add an API credential for a provider
    AddCredential {
        /// Provider ID (anthropic, deepseek, zai, minimax, moonshot, openrouter)
        provider: String,

        /// API key
        #[arg(short = 'k', long)]
        key: Option<String>,
    },

    /// Remove a credential
    RemoveCredential {
        /// Provider ID
        provider: String,
    },

    /// List configured credentials and available models
    Status {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Set the selection strategy
    Strategy {
        /// Strategy: free-first, cost-conscious, best-quality
        strategy: String,
    },

    /// Ban a model from selection
    Ban {
        /// Model ID to ban
        model: String,
    },

    /// Unban a previously banned model
    Unban {
        /// Model ID to unban
        model: String,
    },

    /// List supported providers and their models
    Providers {
        /// Provider ID for details
        provider: Option<String>,
    },

    /// Detect available tools on the system
    Detect,

    /// Run as MCP server (for Claude Code slash commands)
    McpServer,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run {
            prompt,
            tool,
            extra_args,
            stdin,
            dry_run,
        } => cmd_run(prompt, tool, extra_args, stdin, dry_run),

        Commands::Exec { tool, prompt } => cmd_exec(tool, prompt),

        Commands::Recommend { prompt, json, stdin } => cmd_recommend(prompt, json, stdin),

        Commands::Analyze { prompt, stdin } => cmd_analyze(prompt, stdin),

        Commands::Init { force, no_shell } => cmd_init(force, no_shell),

        Commands::Uninit { purge } => cmd_uninit(purge),

        Commands::AddCredential { provider, key } => cmd_add_credential(provider, key),

        Commands::RemoveCredential { provider } => cmd_remove_credential(provider),

        Commands::Status { json } => cmd_status(json),

        Commands::Strategy { strategy } => cmd_strategy(strategy),

        Commands::Ban { model } => cmd_ban(model),

        Commands::Unban { model } => cmd_unban(model),

        Commands::Providers { provider } => cmd_providers(provider),

        Commands::Detect => cmd_detect(),

        Commands::McpServer => cmd_mcp_server(),
    }
}

// ─── Run ──────────────────────────────────────────────────────────────────────

fn cmd_run(
    prompt: Vec<String>,
    tool: String,
    extra_args: Vec<String>,
    stdin: bool,
    dry_run: bool,
) {
    let prompt_text = resolve_prompt(prompt, stdin);
    if prompt_text.is_empty() {
        eprintln!("{} No prompt provided. Use --stdin or pass prompt as arguments.", "Error:".red());
        std::process::exit(1);
    }

    let tool = match Tool::from_str(&tool) {
        Some(t) => t,
        None => {
            eprintln!("{} Unknown tool '{}'. Available: claude_code, opencode, pi", "Error:".red(), tool);
            std::process::exit(1);
        }
    };

    let config = load_config();
    let available = check_available_models(&config);
    let analysis = analyze(&prompt_text);
    let selection = select_model(&analysis, &config, &available);

    match selection {
        Some(sel) => {
            println!();
            println!("{} ===== Analysis =====", "".bold());
            println!("  Complexity: {:.0}% ({})", (analysis.complexity_score * 100.0).round(), analysis.tier.as_str());
            println!("  Use cases:  {}", analysis.use_cases.join(", "));
            println!("  Tokens:     ~{}", analysis.estimated_tokens);
            println!();
            println!("{} ===== Selection =====", "".bold());
            println!("  Selected:   {}", sel.model.display_name.bright_green());
            for reason in &sel.reasoning {
                println!("  • {}", reason);
            }
            println!("  Est. cost:  ${:.6} input + ~${:.6} output", sel.estimated_cost_input, sel.estimated_cost_output);

            if dry_run {
                print_launch_info(tool, &sel.model, &config);
            } else {
                match launch_tool(tool, &sel.model, &config, &prompt_text, &extra_args) {
                    Ok(code) => {
                        if code != 0 {
                            std::process::exit(code);
                        }
                    }
                    Err(e) => {
                        eprintln!("{} Failed to launch tool: {}", "Error:".red(), e);
                        std::process::exit(1);
                    }
                }
            }
        }
        None => {
            eprintln!("{} Could not select a suitable model.", "Error:".red());
            std::process::exit(1);
        }
    }
}

// ─── Exec (called by shell wrappers) ──────────────────────────────────────────

fn cmd_exec(tool: String, prompt: Vec<String>) {
    let prompt_text = prompt.join(" ");
    if prompt_text.is_empty() {
        eprintln!("{} No prompt provided.", "Error:".red());
        std::process::exit(1);
    }

    let tool = match Tool::from_str(&tool) {
        Some(t) => t,
        None => {
            eprintln!("{} Unknown tool '{}'.", "Error:".red(), tool);
            std::process::exit(1);
        }
    };

    let config = load_config();
    let available = check_available_models(&config);
    let analysis = analyze(&prompt_text);
    let selection = select_model(&analysis, &config, &available);

    match selection {
        Some(sel) => {
            // Silent: no banner, just exec the real tool
            match exec_tool(tool, &sel.model, &config, &prompt_text, &[]) {
                Ok(code) => std::process::exit(code),
                Err(e) => {
                    eprintln!("{} Failed to exec: {}", "Error:".red(), e);
                    std::process::exit(1);
                }
            }
        }
        None => {
            eprintln!("{} Could not select a suitable model.", "Error:".red());
            std::process::exit(1);
        }
    }
}

// ─── Recommend ────────────────────────────────────────────────────────────────

fn cmd_recommend(prompt: Vec<String>, json: bool, stdin: bool) {
    let prompt_text = resolve_prompt(prompt, stdin);
    if prompt_text.is_empty() {
        eprintln!("{} No prompt provided.", "Error:".red());
        std::process::exit(1);
    }

    let config = load_config();
    let available = config.available_models();
    let analysis = analyze(&prompt_text);
    let selection = select_model(&analysis, &config, &available);

    if json {
        let output = serde_json::json!({
            "complexity_score": (analysis.complexity_score * 100.0).round(),
            "complexity_tier": analysis.tier.as_str(),
            "estimated_tokens": analysis.estimated_tokens,
            "use_cases": analysis.use_cases,
            "needs_vision": analysis.needs_vision,
            "selection": selection.map(|s| serde_json::json!({
                "model_id": s.model.id,
                "display_name": s.model.display_name,
                "provider": s.model.provider_id,
                "tier": s.model.tier,
                "free": s.model.free,
                "context": s.model.context,
                "reasoning": s.reasoning,
                "estimated_cost_input": s.estimated_cost_input,
                "estimated_cost_output": s.estimated_cost_output,
            })),
            "alternatives": available.iter().map(|m| {
                serde_json::json!({
                    "id": m.id, "display_name": m.display_name, "provider": m.provider_id,
                    "tier": m.tier, "free": m.free, "context": m.context,
                })
            }).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        println!();
        println!("{} ===== Request Analysis =====", "".bold());
        println!("  Complexity: {:.0}% — {}", (analysis.complexity_score * 100.0).round(), tier_color(&analysis.tier));
        println!("  Tokens:     ~{}", analysis.estimated_tokens);
        println!("  Use cases:  {}", analysis.use_cases.join(", "));
        if analysis.needs_vision {
            println!("  Vision:     {}", "Yes — vision-capable model recommended".yellow());
        }
        for factor in &analysis.breakdown {
            let bar = "█".repeat((factor.score * 20.0) as usize);
            println!("  {:>20}: {} ({:.0}%) {}", factor.name, bar, factor.score * 100.0, factor.detail.dimmed());
        }
        println!();
        println!("{} ===== Top Recommendation =====", "".bold());
        match selection {
            Some(sel) => {
                println!("  Model:    {}", sel.model.display_name.bright_green());
                println!("  Provider: {}", sel.model.provider_id);
                println!("  Tier:     {}", sel.model.tier);
                if sel.model.free { println!("  Cost:     FREE"); }
                else { println!("  Cost:     ${:.2}/M input | ${:.2}/M output", sel.model.cost_per_1m_input, sel.model.cost_per_1m_output); }
                println!();
                println!("{} Why:", "".bold());
                for reason in &sel.reasoning { println!("  • {}", reason); }
            }
            None => { println!("  No suitable model found. Add credentials with 'costcut add-credential'."); }
        }

        if !available.is_empty() {
            let run_hint = if prompt_text.len() > 60 {
                String::new()
            } else {
                format!("\"{}\" ", prompt_text)
            };
            println!("{} Run: costcut run {}<prompt>", "  #".bright_yellow().dimmed(), run_hint);
        }
    }
}

// ─── Analyze ──────────────────────────────────────────────────────────────────

fn cmd_analyze(prompt: Vec<String>, stdin: bool) {
    let prompt_text = resolve_prompt(prompt, stdin);
    if prompt_text.is_empty() {
        eprintln!("{} No prompt provided.", "Error:".red());
        std::process::exit(1);
    }
    let analysis = analyze(&prompt_text);
    println!();
    println!("{} ===== Complexity Analysis =====", "".bold());
    println!();
    println!("  Overall score: {:.0}%", (analysis.complexity_score * 100.0).round());
    println!("  Tier:          {}", tier_color(&analysis.tier));
    println!("  Est. tokens:   ~{}", analysis.estimated_tokens);
    println!("  Min context:   {}K", 1.max(analysis.min_context_needed / 1000));
    println!("  Use cases:     {}", analysis.use_cases.join(", "));
    println!("  Needs vision:  {}", if analysis.needs_vision { "Yes" } else { "No" });
    println!();
    println!("  {} Breakdown:", "Factor".bold());
    for factor in &analysis.breakdown {
        let bar_len = (factor.score * 30.0) as usize;
        let bar = "█".repeat(bar_len) + &"░".repeat(30 - bar_len);
        println!("    {:>18} [{}] {:.0}%  {}", factor.name, bar, factor.score * 100.0, factor.detail.dimmed());
    }
    println!();
}

// ─── Init ─────────────────────────────────────────────────────────────────────

fn cmd_init(force: bool, no_shell: bool) {
    let path = WitcherConfig::default_path();
    if path.exists() && !force {
        eprintln!("{} Config already exists at {}. Use --force to overwrite.", "Warning:".yellow(), path.display());
        std::process::exit(0);
    }

    let config = WitcherConfig::default();
    match config.save() {
        Ok(()) => {
            println!("{} Config:    {}", "✓".green(), path.display());

            // Shell wrapper injection
            if !no_shell {
                match setup::find_rc_file() {
                    Some(rc_path) => {
                        let bin_name = std::env::current_exe()
                            .ok()
                            .and_then(|p| p.to_str().map(|s| s.to_string()))
                            .unwrap_or_else(|| "costcut".to_string());

                        let interactive = config.interactive_model.as_deref();

                        match setup::inject_wrappers(&rc_path, &bin_name, interactive) {
                            Ok(()) => {
                                println!("{} Shell:     {} updated (costcut wrappers active)", "✓".green(), rc_path.display());
                                println!("  → Run: source {}", rc_path.display());
                                println!("  → Now just type: claude -p \"fix the bug\"");
                            }
                            Err(e) => {
                                eprintln!("{} Failed to inject shell wrappers: {}", "Warning:".yellow(), e);
                            }
                        }
                    }
                    None => {
                        eprintln!("{} Could not find .zshrc or .bashrc to inject wrappers.", "Warning:".yellow());
                        eprintln!("  Add manually from: costcut init --no-shell");
                    }
                }
            }

            // Check available tools
            let tools = detect_available_tools();
            if !tools.is_empty() {
                println!();
                println!("{} Detected tools:", "".bold());
                for t in &tools {
                    println!("  {} ✓ {}", t.name().bright_cyan(), "available");
                }
            } else {
                println!();
                println!("{} No supported tools detected. Install one of:", "Warning:".yellow());
                println!("  - Claude Code: `npm install -g @anthropic-ai/claude-code`");
                println!("  - OpenCode:    see https://opencode.ai");
                println!("  - Pi:          see pi.ai/cli");
            }

            println!();
            println!("{} Add your API keys:", "Next steps:".bold());
            println!("  costcut add-credential anthropic    # Your Anthropic API key");
            println!("  costcut add-credential deepseek     # DeepSeek (~10x cheaper)");
            println!();
            println!("Anthropic key auto-detected? {}", if std::env::var("ANTHROPIC_API_KEY").is_ok() { "YES".green() } else { "no".dimmed() });
            println!();
        }
        Err(e) => {
            eprintln!("{} Failed to create config: {}", "Error:".red(), e);
            std::process::exit(1);
        }
    }
}

// ─── Uninit ───────────────────────────────────────────────────────────────────

fn cmd_uninit(purge: bool) {
    if let Some(rc_path) = setup::find_rc_file() {
        if setup::has_costcut_block(&rc_path) {
            match setup::remove_wrappers(&rc_path) {
                Ok(()) => println!("{} Removed shell wrappers from {}", "✓".green(), rc_path.display()),
                Err(e) => eprintln!("{} Failed to remove wrappers: {}", "Error:".red(), e),
            }
        } else {
            println!("{} No costcut wrappers found in {}", "•".dimmed(), rc_path.display());
        }
    } else {
        println!("{} No rc file found.", "•".dimmed());
    }

    if purge {
        let dir = WitcherConfig::dir();
        if dir.exists() {
            match std::fs::remove_dir_all(&dir) {
                Ok(()) => println!("{} Removed config directory: {}", "✓".green(), dir.display()),
                Err(e) => eprintln!("{} Failed to remove config: {}", "Error:".red(), e),
            }
        }
    }
}

// ─── Add Credential ───────────────────────────────────────────────────────────

fn cmd_add_credential(provider: String, key: Option<String>) {
    let mut config = WitcherConfig::load().unwrap_or_default();
    let catalog = providers::bundled_catalog();
    let provider_def = catalog.providers.iter().find(|p| p.id == provider);

    if provider_def.is_none() {
        eprintln!("{} Unknown provider '{}'. Known providers: {}",
            "Error:".red(), provider,
            catalog.providers.iter().map(|p| p.id.as_str()).collect::<Vec<_>>().join(", "));
        std::process::exit(1);
    }

    let provider_def = provider_def.unwrap();
    let api_key = match key {
        Some(k) => k,
        None => {
            println!("{} Provider: {}", "→".bright_blue(), provider_def.display_name.bright_cyan());
            println!("  Get API key: {}", provider_def.auth.key_url.bright_cyan());
            println!();
            let mut input = String::new();
            print!("  Paste API key: ");
            use std::io::Write;
            let _ = io::stdout().flush();
            io::stdin().read_line(&mut input).expect("Failed to read input");
            let key = input.trim().to_string();
            if key.is_empty() {
                eprintln!("{} No API key provided.", "Error:".red());
                std::process::exit(1);
            }
            key
        }
    };

    config.credentials.insert(provider.clone(), config::Credential {
        key: api_key,
        base_url: provider_def.base_url.clone(),
    });

    match config.save() {
        Ok(()) => {
            println!("{} Credential saved for provider: {}", "✓".green(), provider.bright_green());
            let available = config.available_models();
            let provider_models: Vec<_> = available.iter().filter(|m| m.provider_id == provider).collect();
            if !provider_models.is_empty() {
                println!();
                println!("  Available models:");
                for m in provider_models {
                    let tier_color = if m.free { m.tier.green() } else { m.tier.yellow() };
                    println!("    {} ({}, {}K ctx)", m.id.bright_green(), tier_color, m.context / 1000);
                }
            }
        }
        Err(e) => {
            eprintln!("{} Failed to save config: {}", "Error:".red(), e);
            std::process::exit(1);
        }
    }
}

// ─── Remove Credential ────────────────────────────────────────────────────────

fn cmd_remove_credential(provider: String) {
    let mut config = WitcherConfig::load().unwrap_or_default();
    if config.credentials.remove(&provider).is_some() {
        config.save().ok();
        println!("{} Removed credentials for: {}", "✓".green(), provider.bright_green());
    } else {
        eprintln!("{} No credentials found for provider: {}", "Warning:".yellow(), provider);
    }
}

// ─── Status ───────────────────────────────────────────────────────────────────

fn cmd_status(json: bool) {
    let config = WitcherConfig::load().unwrap_or_default();
    let available = config.available_models();

    if json {
        let output = serde_json::json!({
            "strategy": match config.strategy {
                config::Strategy::FreeFirst => "free-first",
                config::Strategy::CostConscious => "cost-conscious",
                config::Strategy::BestQuality => "best-quality",
            },
            "interactive_model": config.interactive_model,
            "configured_providers": config.credentials.keys().collect::<Vec<_>>(),
            "available_models": available.iter().map(|m| serde_json::json!({
                "id": m.id, "provider": m.provider_id, "display_name": m.display_name,
                "tier": m.tier, "free": m.free, "role": m.role, "context": m.context,
                "use_cases": m.use_cases, "cost_per_1m_input": m.cost_per_1m_input,
                "cost_per_1m_output": m.cost_per_1m_output,
            })).collect::<Vec<_>>(),
            "shell_wrappers_active": setup::find_rc_file().map_or(false, |p| setup::has_costcut_block(&p)),
            "tools": config.tools.iter().map(|t| serde_json::json!({ "name": t.name, "enabled": t.enabled, "binary": t.binary })).collect::<Vec<_>>(),
            "banned_models": config.banned_models,
        });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        println!();
        println!("{} ===== Costcut Status =====", "".bold());
        println!();
        println!("  Strategy:  {}", match config.strategy {
            config::Strategy::FreeFirst => "Free-first (prefer free models)".green(),
            config::Strategy::CostConscious => "Cost-conscious (balance)".yellow(),
            config::Strategy::BestQuality => "Best quality (use most capable)".red(),
        });

        // Shell wrapper status
        if let Some(rc) = setup::find_rc_file() {
            if setup::has_costcut_block(&rc) {
                println!("  Shell:     {} (wrappers in {})", "active".green(), rc.display());
            } else {
                println!("  Shell:     {} — run 'costcut init' to activate", "inactive".dimmed());
            }
        }

        println!();
        println!("  {} Configured Providers:", "Credentials".bold());
        if config.credentials.is_empty() {
            println!("    (none — add with 'costcut add-credential <provider>')");
        } else {
            for (provider, cred) in &config.credentials {
                let key_preview = if cred.key.len() > 12 {
                    format!("{}...{}", &cred.key[..4], &cred.key[cred.key.len()-4..])
                } else { "***".to_string() };
                println!("    {} {} (key: {})", "✓".green(), provider.bright_green(), key_preview.dimmed());
            }
        }

        println!();
        println!("  {} Available Models:", "Models".bold());
        if available.is_empty() {
            println!("    (none — add credentials first)");
        } else {
            for m in &available {
                let tier_indicator = match m.tier.as_str() {
                    "free" => "FREE".green(), "trial" => "TRIAL".truecolor(255, 165, 0),
                    "paid" => "PAID".yellow(), _ => m.tier.normal(),
                };
                let role_indicator = if m.role == "main" { "main".bold() } else { "small".dimmed() };
                println!("    {} {:>6} {:>6} {}K ctx — {}", m.id.bright_green(), tier_indicator, role_indicator, m.context / 1000, m.provider_id.dimmed());
            }
        }

        println!();
        println!("  {} Tools:", "Tools".bold());
        let detected = detect_available_tools();
        for tc in &config.tools {
            let available = detected.iter().any(|t| t.name() == tc.name);
            let status = if tc.enabled && available { "enabled + detected".green() }
                else if tc.enabled { "enabled (not in PATH)".yellow() }
                else { "disabled".dimmed() };
            println!("    {} {:>12} {}", if available && tc.enabled { "✓".green() } else { "✗".red() }, tc.name.bright_cyan(), status);
        }

        if !config.banned_models.is_empty() {
            println!();
            println!("  {} Banned Models:", "Banned".bold());
            for bm in &config.banned_models { println!("    ✗ {}", bm.dimmed()); }
        }
        println!();
    }
}

// ─── Strategy ─────────────────────────────────────────────────────────────────

fn cmd_strategy(strategy: String) {
    let mut config = WitcherConfig::load().unwrap_or_default();
    let new_strategy = match strategy.to_lowercase().as_str() {
        "free-first" | "freefirst" | "free" => config::Strategy::FreeFirst,
        "cost-conscious" | "costconscious" | "cost" => config::Strategy::CostConscious,
        "best-quality" | "bestquality" | "best" | "quality" => config::Strategy::BestQuality,
        _ => {
            eprintln!("{} Unknown strategy '{}'. Use: free-first, cost-conscious, best-quality", "Error:".red(), strategy);
            std::process::exit(1);
        }
    };
    config.strategy = new_strategy;
    config.save().ok();
    println!("{} Strategy set to: {}", "✓".green(), match config.strategy {
        config::Strategy::FreeFirst => "free-first".green(),
        config::Strategy::CostConscious => "cost-conscious".yellow(),
        config::Strategy::BestQuality => "best-quality".red(),
    });
}

// ─── Ban / Unban ──────────────────────────────────────────────────────────────

fn cmd_ban(model: String) {
    let mut config = WitcherConfig::load().unwrap_or_default();
    if config.banned_models.contains(&model) {
        println!("{} Model already banned: {}", "⚠".yellow(), model);
    } else {
        config.banned_models.push(model.clone());
        config.save().ok();
        println!("{} Banned model: {}", "✓".green(), model.bright_green());
    }
}

fn cmd_unban(model: String) {
    let mut config = WitcherConfig::load().unwrap_or_default();
    if let Some(pos) = config.banned_models.iter().position(|m| m == &model) {
        config.banned_models.remove(pos);
        config.save().ok();
        println!("{} Unbanned model: {}", "✓".green(), model.bright_green());
    } else {
        println!("{} Model not in banned list: {}", "⚠".yellow(), model);
    }
}

// ─── Providers ────────────────────────────────────────────────────────────────

fn cmd_providers(provider: Option<String>) {
    let catalog = providers::bundled_catalog();
    if let Some(ref pid) = provider {
        let prov = catalog.providers.iter().find(|p| p.id == *pid);
        match prov {
            Some(p) => {
                println!();
                println!("{} {} ({})", "".bold(), p.display_name, p.id);
                println!("  Tier:     {:?}", p.tier);
                println!("  Base URL: {}", p.base_url.as_deref().unwrap_or("(default)"));
                println!("  Auth:     {} (get key: {})", p.auth.env_var, p.auth.key_url);
                if let Some(end) = &p.trial_ends_at { println!("  Trial ends: {}", end); }
                if let Some(note) = p.notes.get("en") { println!("  Note:     {}", note); }
                println!();
                println!("  Models:");
                for m in &p.models {
                    let cost = match &m.pricing {
                        Some(pricing) if pricing.input_per_1m > 0.0 =>
                            format!(" — ${:.2}/M in | ${:.2}/M out", pricing.input_per_1m, pricing.output_per_1m),
                        Some(pricing) if pricing.trial => " — FREE (trial)".to_string(),
                        _ => " — FREE".to_string(),
                    };
                    println!("    {} {:>7} {:>5}K ctx{}", m.id.bright_green(),
                        if m.free { "FREE".green() } else { "PAID".yellow() }, m.context / 1000, cost.dimmed());
                    if !m.note.is_empty() { println!("      {}", m.note.dimmed()); }
                    println!("      Use cases: {}", m.use_cases.iter().map(|u| u.dimmed().to_string()).collect::<Vec<_>>().join(", "));
                }
            }
            None => {
                eprintln!("{} Unknown provider '{}'. Try: costcut providers", "Error:".red(), pid);
                std::process::exit(1);
            }
        }
    } else {
        println!();
        println!("{} ===== Available Providers =====", "".bold());
        println!();
        for p in &catalog.providers {
            let tier_str = match p.tier {
                providers::Tier::Free => "FREE".green(),
                providers::Tier::Trial => "TRIAL".truecolor(255, 165, 0),
                providers::Tier::Paid => "PAID".yellow(),
            };
            println!("  {} {} — {} ({} models)", p.id.bright_green(), tier_str, p.display_name, p.models.len());
            if let Some(note) = p.notes.get("en") { println!("    {}", note.dimmed()); }
            println!();
        }
        println!("{} For details: costcut providers <id>", "".dimmed());
        println!();
    }
}

// ─── Detect ───────────────────────────────────────────────────────────────────

fn cmd_detect() {
    let tools = detect_available_tools();
    println!();
    println!("{} ===== Tool Detection =====", "".bold());
    println!();
    for tool in &[Tool::ClaudeCode, Tool::OpenCode, Tool::Pi] {
        let found = tools.contains(tool);
        println!("  {} {} — {}", if found { "✓".green() } else { "✗".red() }, tool.name().bright_cyan(),
            if found { format!("found ({})", tool.binary_name()).green() }
            else { format!("not found (looking for '{}')", tool.binary_name()).dimmed() });
    }
    println!();
}

// ─── MCP Server ───────────────────────────────────────────────────────────────

fn cmd_mcp_server() {
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    rt.block_on(async {
        if let Err(e) = mcp::run_server().await {
            eprintln!("MCP server error: {}", e);
            std::process::exit(1);
        }
    });
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn resolve_prompt(args: Vec<String>, from_stdin: bool) -> String {
    if from_stdin {
        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer).unwrap_or_default();
        return buffer.trim().to_string();
    }
    if !args.is_empty() {
        return args.join(" ");
    }
    let mut buffer = String::new();
    if io::stdin().read_to_string(&mut buffer).is_ok() && !buffer.is_empty() {
        return buffer.trim().to_string();
    }
    String::new()
}

fn load_config() -> WitcherConfig {
    WitcherConfig::load().unwrap_or_else(|e| {
        eprintln!("{} Failed to load config: {}", "Warning:".yellow(), e);
        eprintln!("{} Run 'costcut init' first.", "Hint:".dimmed());
        WitcherConfig::default()
    })
}

fn check_available_models(config: &WitcherConfig) -> Vec<config::AvailableModel> {
    let available = config.available_models();
    if available.is_empty() {
        eprintln!("{} No models configured. Run 'costcut add-credential <provider>' to add API keys.", "Error:".red());
        std::process::exit(1);
    }
    available
}

fn tier_color(tier: &analyzer::ComplexityTier) -> colored::ColoredString {
    match tier {
        analyzer::ComplexityTier::Low => "Low".green(),
        analyzer::ComplexityTier::Medium => "Medium".yellow(),
        analyzer::ComplexityTier::High => "High".truecolor(255, 165, 0),
        analyzer::ComplexityTier::Extreme => "Extreme".red(),
    }
}
