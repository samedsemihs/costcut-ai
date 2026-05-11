# costcut

> Intelligent model switcher — analyzes request complexity and routes to Anthropic or DeepSeek. Seamlessly integrated with Claude Code, OpenCode, and Pi.

**costcut** automatically picks the best model for each request. It analyzes your prompt's complexity, checks which models you have API keys for, and selects the optimal one — balancing capability and cost. No manual switching. No fixed aliases. Just type `claude -p "fix the bug"` and costcut does the rest.

## Two Integration Modes

### 1. Shell Wrapper Mode (Transparent)
Intercepts `claude`, `opencode`, or `pi` commands at the shell level — you don't change how you work.

### 2. MCP Server Mode (In-REPL)
Run `/costcut` slash commands directly inside Claude Code or OpenCode REPL for on-demand model recommendations.

---

## How it works

```
You type:                 Shell wrapper            costcut engine
─────────                ─────────────            ──────────────
claude -p "refactor      claude() function        analyze prompt →
  auth across 15          intercepts call          complexity: 59% HIGH
  microservices"          calls: costcut exec      select: deepseek-v4-pro
                          --tool claude            set env vars → exec claude

Result: DeepSeek-V4-Pro handles the complex refactor. You save ~10x vs Sonnet.
```

### Shell wrapper integration

`costcut init` injects shell functions into your `~/.zshrc` or `~/.bashrc` that shadow the real binaries. When you type `claude`, `opencode`, or `pi`, the wrapper intercepts the call:

| What you type | What happens |
|---|---|
| `claude -p "simple question"` | costcut picks cheap model (DeepSeek Flash) |
| `claude -p "complex refactor..."` | costcut picks strong model (DeepSeek Pro or Sonnet) |
| `claude` (interactive REPL) | passes through to real claude |
| `opencode "build a dashboard"` | costcut analyzes, picks model, execs opencode |
| `pi "explain this code"` | same pattern |

### Complexity analysis

costcut scores your prompt on 7 dimensions:

| Factor | Weight | What it measures |
|--------|--------|-----------------|
| `prompt_length` | 20% | Character count, estimated tokens |
| `task_complexity` | 40% | Detected use-cases (refactor, architecture, multi-agent, etc.) |
| `file_references` | 10% | File extensions, directories, config files mentioned |
| `technology_depth` | 8% | Technology/framework references |
| `constraints` | 10% | Security, scale, backward compat, edge-case requirements |
| `vision` | 5% | Screenshot/image/diagram mentions |
| `context_size` | 7% | Long-context indicators, prompt length |

### Selection strategies

| Strategy | Behavior |
|----------|----------|
| `cost-conscious` *(default)* | DeepSeek Flash for simple, DeepSeek Pro for medium, Sonnet for high, Opus for extreme |
| `free-first` | Prefer free models when possible, fallback to cheapest paid |
| `best-quality` | Haiku for low, Sonnet for medium, Opus for high/extreme — cost ignored |

---

## Supported CLI Tools

costcut integrates with the following AI coding assistants:

| Tool | Description | Binary | Website |
|------|-------------|--------|---------|
| **Claude Code** | Anthropic's official CLI for Claude | `claude` | [claude.ai/code](https://claude.ai/code) |
| **OpenCode** | Open-source AI coding assistant | `opencode` | [opencode.ai](https://opencode.ai) |
| **Pi** | Inflection AI's coding assistant | `pi` | [pi.ai](https://pi.ai) |

All three tools support the Anthropic API format, allowing costcut to seamlessly switch between providers (Anthropic, DeepSeek, etc.) by setting environment variables before launch.

---

## Quick start

### Prerequisites

- [Rust](https://rustup.rs) toolchain
- At least one of: Claude Code, OpenCode, or Pi

```bash
# Build and install
cargo build --release
cargo install --path .
```

### Setup

```bash
# One command: creates config + injects shell wrappers
costcut init

# Source your rc file
source ~/.zshrc   # or ~/.bashrc

# Add API keys
costcut add-credential anthropic    # Your Anthropic API key
costcut add-credential deepseek     # DeepSeek API key (~10x cheaper)

# Anthropic key auto-detected if ANTHROPIC_API_KEY env var is set
```

### That's it. Start using it:

```bash
# costcut picks the best model from the full pool automatically
claude -p "what is a Rust trait?"
# → selects deepseek-v4-flash (cheapest, simple question)

claude -p "fix the login bug in auth module"
# → selects claude-haiku-4-5 (low complexity, cheap Anthropic)

claude -p "add OAuth2 middleware to the Rust backend"
# → selects deepseek-v4-pro (medium complexity, best value)

claude -p "refactor the entire auth system across 15 microservices"
# → selects deepseek-v4-pro or claude-sonnet-4-5 (high complexity)

claude -p "design a multi-region distributed database with CQRS"
# → selects claude-opus-4-7 (extreme complexity, max reasoning)

claude
# → interactive REPL, passes through to real claude
```

---

## MCP Server Integration

costcut can run as an MCP (Model Context Protocol) server, exposing slash commands directly inside Claude Code or OpenCode REPL. This lets you get model recommendations without leaving your coding session.

### Setup MCP Server

```bash
# Option 1: Register with Claude Code CLI
claude mcp add costcut -- costcut mcp-server

# Option 2: Add to your project's .mcp.json
{
  "mcpServers": {
    "costcut": {
      "type": "stdio",
      "command": "costcut",
      "args": ["mcp-server"]
    }
  }
}

# Option 3: Add to global Claude Code config (~/.claude.json)
```

### Available MCP Tools

Once registered, use these tools inside the REPL:

| Tool | Description | Example |
|------|-------------|---------|
| `status` | Show current config, strategy, available models | `/mcp__costcut__status` |
| `recommend_model` | Analyze a task and get model recommendation | `/mcp__costcut__recommend_model "refactor auth"` |
| `switch_instructions` | Get instructions to switch to a specific model | `/mcp__costcut__switch_instructions deepseek-v4-pro` |

### Example MCP Usage

```
You: /mcp__costcut__status

=== Costcut Status ===

Strategy: cost-conscious (balance cost and quality)

Configured Providers:
  ✓ anthropic
  ✓ deepseek

Available Models:
  claude-sonnet-4-5-20250929 $3.00/M in 200K ctx — anthropic
  deepseek-v4-pro $0.27/M in 128K ctx — deepseek
  deepseek-v4-flash $0.07/M in 128K ctx — deepseek

Shell wrappers: active
```

```
You: /mcp__costcut__recommend_model "refactor the authentication system to use OAuth2 PKCE"

=== Request Analysis ===

Complexity: 45% — Medium
Tokens: ~15
Use cases: refactor, coding

Factor Breakdown:
     prompt_length: [██░░░░░░░░] 15% 67 chars (~15 tokens)
   task_complexity: [██████░░░░] 60% Detected: refactor, coding
   ...

=== Recommendation ===

Model: DeepSeek/deepseek-v4-pro
Provider: deepseek
Tier: paid
Est. cost: $0.000004 input + $0.000016 output

Why:
  • Matches 2/2 detected use cases (refactor, coding)
  • Cost-effective ($0.27/M input)
  • Main model for medium complexity
```

```
You: /mcp__costcut__switch_instructions deepseek-v4-pro

To switch to DeepSeek/deepseek-v4-pro:

  Run: /model deepseek-v4-pro

Note: costcut cannot change Claude Code's model programmatically.
The /model command must be run manually.

Model Info:
  Provider: deepseek
  Context: 128K tokens
  Cost: $0.27/M input | $1.10/M output
```

---

## Commands

| Command | Description |
|---------|-------------|
| `costcut init` | Create config + inject shell wrappers into rc file |
| `costcut uninit` | Remove shell wrappers from rc file |
| `costcut run <prompt>` | Analyze, select model, launch tool (with banner) |
| `costcut exec --tool <name> <prompt>` | Silent analysis + launch (used by shell wrappers) |
| `costcut recommend <prompt>` | Analyze and show top recommendation (no execution) |
| `costcut analyze <prompt>` | Show detailed complexity breakdown |
| `costcut add-credential <provider>` | Add API key for a provider |
| `costcut remove-credential <provider>` | Remove stored API key |
| `costcut status` | Show configured providers, models, wrappers, tools |
| `costcut strategy <strategy>` | Set: free-first / cost-conscious / best-quality |
| `costcut ban <model>` | Exclude a model from selection |
| `costcut unban <model>` | Re-enable a banned model |
| `costcut providers [<id>]` | List provider catalog |
| `costcut detect` | Check which tools are available on PATH |
| `costcut mcp-server` | Run as MCP server (for Claude Code slash commands) |

---

## Configuration

Config lives at `~/.config/costcut/config.toml`:

```toml
schema_version = 1
strategy = "cost-conscious"

# Default model for interactive/REPL sessions
interactive_model = "deepseek-v4-pro"

[overrides]
# Force model for specific use-case
# use_case = { refactor = "deepseek-v4-pro" }
# Force model for complexity tier
# complexity_tier = { high = "claude-sonnet-4-5-20250929" }

banned_models = []

[credentials.anthropic]
key = "sk-ant-..."

[credentials.deepseek]
key = "sk-..."
base_url = "https://api.deepseek.com/anthropic"
```

### Custom models

```toml
[[custom_models]]
id = "my-custom-model"
display_name = "My Custom Model"
provider_id = "custom"
base_url = "https://api.example.com/anthropic"
tier = "free"
context = 131000
free = true
use_cases = ["coding-fast", "refactor"]
```

---

## Model lineup

costcut selects from every model you have API access to. With both Anthropic and DeepSeek keys configured, the full pool is:

| Model | Provider | Role | Context | Cost (in/out per 1M) | Best for |
|-------|----------|------|---------|----------------------|----------|
| `claude-opus-4-7` | Anthropic | main | 200K | $15/$75 | Extreme complexity, architecture, multi-agent |
| `claude-sonnet-4-5` | Anthropic | main | 200K | $3/$15 | High complexity, refactoring, code review |
| `claude-haiku-4-5` | Anthropic | small | 200K | $0.80/$4 | Low complexity, simple queries, cheap batch |
| `deepseek-v4-pro` | DeepSeek | main | 128K | $0.27/$1.10 | High complexity, refactoring (~10x cheaper than Sonnet) |
| `deepseek-v4-flash` | DeepSeek | small | 128K | $0.07/$0.27 | Simple tasks, fastest/cheapest option |

### How costcut picks

| Complexity | Tier | Cost-conscious (default) | Best-quality |
|------------|------|--------------------------|-------------|
| 0–25% | Low | `deepseek-v4-flash` (cheapest) | `claude-haiku-4-5` |
| 25–55% | Medium | `deepseek-v4-pro` (best value) | `claude-sonnet-4-5` |
| 55–80% | High | `deepseek-v4-pro` or `claude-sonnet-4-5` | `claude-sonnet-4-5` |
| 80–100% | Extreme | `claude-sonnet-4-5` or `claude-opus-4-7` | `claude-opus-4-7` |

### Additional providers

Also available: Z.ai (free), MiniMax (trial), Moonshot, OpenRouter. Add keys with `costcut add-credential <provider>`.

---

## Example: complexity scoring in action

```
$ costcut recommend "what is rust"
Complexity: 5% — Low
→ Recommends: deepseek-v4-flash (cheapest, simple question)

$ costcut recommend "fix the login bug in auth.rs"
Complexity: 15% — Low
→ Recommends: claude-haiku-4-5 or deepseek-v4-flash (simple debugging)

$ costcut recommend "add JWT authentication middleware to the Rust backend"
Complexity: 32% — Medium
→ Recommends: deepseek-v4-pro (solid capability, ~10x cheaper than Sonnet)

$ costcut recommend "refactor the entire auth system across 15 microservices
  in Rust and TypeScript, migrate from JWT to OAuth2 PKCE..."
Complexity: 59% — High
→ Recommends: deepseek-v4-pro or claude-sonnet-4-5 (complex refactor)

$ costcut recommend "design a distributed event-sourcing architecture
  with CQRS, multi-region replication, and exactly-once semantics"
Complexity: 82% — Extreme
→ Recommends: claude-opus-4-7 (maximum reasoning capability)
```

---

## Architecture

```
src/
├── main.rs          # CLI entry point (clap) — 15 commands
├── mcp.rs           # MCP server for in-REPL slash commands
├── setup.rs         # Shell wrapper injection/removal
├── analyzer.rs      # Prompt complexity scoring (7-factor analysis)
├── selector.rs      # Model selection algorithm (scoring + ranking)
├── config.rs        # User config, credential management
├── providers.rs     # Built-in provider/model catalog
└── launcher.rs      # Tool integration (Claude Code, OpenCode, Pi)
```

### Module responsibilities

| Module | Purpose |
|--------|---------|
| `main.rs` | CLI dispatcher, command handlers, user interaction |
| `mcp.rs` | MCP server with `status`, `recommend_model`, `switch_instructions` tools |
| `analyzer.rs` | 7-factor complexity scoring, returns `AnalysisResult` |
| `selector.rs` | Model ranking based on strategy, complexity, use-cases |
| `config.rs` | TOML config loading/saving, credential management |
| `providers.rs` | Hardcoded provider/model catalog with pricing |
| `setup.rs` | Shell wrapper generation and rc file injection |
| `launcher.rs` | Environment setup and tool binary execution |

---

## Environment Variables

costcut sets these environment variables when launching tools:

| Variable | Purpose |
|----------|---------|
| `ANTHROPIC_MODEL` | Selected model ID |
| `ANTHROPIC_AUTH_TOKEN` | API key for the selected provider |
| `ANTHROPIC_BASE_URL` | API endpoint (for non-Anthropic providers) |
| `OPENAI_API_KEY` | Set for OpenRouter compatibility |
| `OPENAI_BASE_URL` | Set for OpenRouter compatibility |

costcut also reads:
- `ANTHROPIC_API_KEY` / `ANTHROPIC_AUTH_TOKEN` — Auto-detect Anthropic credentials
- `SHELL` / `ZSH_VERSION` — Detect shell for rc file selection

---

## License

MIT

## Disclaimer

costcut is an independent open-source tool. Not affiliated with Anthropic, DeepSeek, or any other provider. Compliance with each provider's terms of service is your responsibility.
