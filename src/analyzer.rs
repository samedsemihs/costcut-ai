/// Request complexity analysis.
///
/// Scores a user prompt on multiple dimensions to determine
/// which model tier is appropriate.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    /// Overall complexity score 0.0–1.0
    pub complexity_score: f64,

    /// Estimated token count of the prompt
    pub estimated_tokens: usize,

    /// Detected use cases
    pub use_cases: Vec<String>,

    /// Complexity tier
    pub tier: ComplexityTier,

    /// Detailed breakdown
    pub breakdown: Vec<ScoreFactor>,

    /// Whether the request involves vision (images mentioned)
    pub needs_vision: bool,

    /// Minimum context window needed
    pub min_context_needed: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ComplexityTier {
    Low,
    Medium,
    High,
    Extreme,
}

impl ComplexityTier {
    pub fn as_str(&self) -> &str {
        match self {
            ComplexityTier::Low => "low",
            ComplexityTier::Medium => "medium",
            ComplexityTier::High => "high",
            ComplexityTier::Extreme => "extreme",
        }
    }

    pub fn from_score(score: f64) -> Self {
        if score >= 0.8 {
            ComplexityTier::Extreme
        } else if score >= 0.55 {
            ComplexityTier::High
        } else if score >= 0.25 {
            ComplexityTier::Medium
        } else {
            ComplexityTier::Low
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreFactor {
    pub name: String,
    pub score: f64,
    pub weight: f64,
    pub detail: String,
}

pub fn analyze(prompt: &str) -> AnalysisResult {
    let mut breakdown = Vec::new();
    let prompt_lower = prompt.to_lowercase();

    // ── Factor 1: Prompt length ──────────────────────────────
    let char_count = prompt.len();
    let estimated_tokens = char_count / 4; // rough estimate
    let length_score = if char_count < 200 {
        0.05
    } else if char_count < 1000 {
        0.15
    } else if char_count < 5000 {
        0.35
    } else if char_count < 20000 {
        0.6
    } else if char_count < 100000 {
        0.8
    } else {
        1.0
    };

    breakdown.push(ScoreFactor {
        name: "prompt_length".into(),
        score: length_score,
        weight: 0.20,
        detail: format!("{} chars (~{} tokens)", char_count, estimated_tokens),
    });

    // ── Factor 2: Task type detection ────────────────────────
    let mut use_cases = Vec::new();
    let mut task_score = 0.0;

    // Architecture / system design
    if contains_any(&prompt_lower, &["architecture", "design pattern", "system design", "microservice", "distributed", "scalable", "fault tolerance", "circuit breaker", "consistency", "partition", "cap theorem"]) {
        task_score += 0.35;
        use_cases.push("architecture".into());
    }

    // Refactoring
    if contains_any(&prompt_lower, &["refactor", "rewrite", "restructure", "migrate", "reorganize", "clean up", "rework", "overhaul"]) {
        task_score += 0.30;
        use_cases.push("refactor".into());
    }

    // Multi-file / codebase-wide
    if contains_any(&prompt_lower, &["entire codebase", "whole project", "all files", "across the codebase", "every file"]) {
        task_score += 0.30;
        use_cases.push("multi-file".into());
    }

    // Agentic / multi-step
    if contains_any(&prompt_lower, &["autonomous", "agent", "multi-step", "pipeline", "workflow", "orchestrate", "chain"]) {
        task_score += 0.30;
        use_cases.push("multi-agent".into());
    }

    // General coding
    if contains_any(&prompt_lower, &["create", "build", "implement", "write a", "develop", "generate code"]) {
        task_score += 0.20;
        if !use_cases.contains(&"coding".to_string()) {
            use_cases.push("coding".into());
        }
    }

    // Debugging
    if contains_any(&prompt_lower, &["fix", "debug", "error", "bug", "issue", "broken", "failing"]) {
        task_score += 0.20;
        use_cases.push("debugging".into());
    }

    // Code review
    if contains_any(&prompt_lower, &["review", "audit", "check this code", "code review", "analyze this code"]) {
        task_score += 0.20;
        use_cases.push("code-review".into());
    }

    // Optimization
    if contains_any(&prompt_lower, &["optimize", "performance", "faster", "slow", "bottleneck", "latency"]) {
        task_score += 0.25;
        use_cases.push("optimization".into());
    }

    // Simple query / explanation
    if contains_any(&prompt_lower, &["what is", "what does", "explain", "how do i", "show me", "tell me about"]) {
        task_score += 0.10;
        use_cases.push("question".into());
    }

    // Translation / conversion
    if contains_any(&prompt_lower, &["translate", "convert to", "port to", "migrate from"]) {
        task_score += 0.25;
        use_cases.push("migration".into());
    }

    // Testing
    if contains_any(&prompt_lower, &["test", "unit test", "integration test", "test case", "coverage"]) {
        task_score += 0.15;
        use_cases.push("testing".into());
    }

    // Documentation
    if contains_any(&prompt_lower, &["document", "readme", "docstring", "comment", "jsdoc"]) {
        task_score += 0.10;
        use_cases.push("documentation".into());
    }

    let task_score = (task_score / 1.2f64).min(1.0f64);

    breakdown.push(ScoreFactor {
        name: "task_complexity".into(),
        score: task_score,
        weight: 0.40,
        detail: format!("Detected: {}", use_cases.join(", ")),
    });

    // ── Factor 3: File/code references ───────────────────────
    let file_ref_count = count_patterns(&prompt_lower, &[
        ".rs", ".ts", ".js", ".py", ".go", ".java", ".cpp", ".c", ".h",
        ".tsx", ".jsx", ".vue", ".svelte", ".rb", ".php", ".swift", ".kt",
        ".toml", ".yaml", ".yml", ".json", ".xml", ".sql", ".sh", ".bash",
        "src/", "lib/", "app/", "test/", "tests/",
        "mod.rs", "main.rs", "Cargo.toml",
        "package.json", "tsconfig.json", "dockerfile", "makefile",
        "directory", "file", "module",
    ]);

    let file_score = (file_ref_count as f64 / 8.0).min(1.0);

    breakdown.push(ScoreFactor {
        name: "file_references".into(),
        score: file_score,
        weight: 0.10,
        detail: format!("{} file/code references found", file_ref_count),
    });

    // ── Factor 4: Language/technology mentions ────────────────
    let tech_mentions = count_patterns(&prompt_lower, &[
        "rust", "python", "javascript", "typescript", "go", "golang",
        "react", "vue", "angular", "svelte", "next", "nuxt",
        "docker", "kubernetes", "aws", "gcp", "azure",
        "postgres", "mysql", "mongodb", "redis", "sqlite",
        "graphql", "rest", "grpc", "websocket",
        "llm", "transformer", "neural", "ml", "training",
        "microservice", "oauth", "jwt", "token", "api",
        "rabbitmq", "kafka", "nginx", "load balancer",
        "terraform", "ansible", "ci/cd", "jenkins", "github action",
        "linux", "macos", "windows", "android", "ios",
        "embedded", "firmware", "driver", "kernel",
        "css", "html", "tailwind", "bootstrap", "webpack", "vite",
        "prisma", "drizzle", "orm", "sql", "nosql",
        "c++", "c#", "java", "kotlin", "swift", "dart", "flutter",
        "s3", "lambda", "ec2", "cloudflare", "vercel",
        "prometheus", "grafana", "elk", "datadog",
        "cuda", "opencl", "webgpu", "wasm",
    ]);

    let tech_score = (tech_mentions as f64 / 3.0).min(1.0) * 0.7;

    breakdown.push(ScoreFactor {
        name: "technology_depth".into(),
        score: tech_score,
        weight: 0.08,
        detail: format!("{} technology references", tech_mentions),
    });

    // ── Factor 5: Constraints / requirements ─────────────────
    let constraint_count = count_patterns(&prompt_lower, &[
        "must", "should", "required", "need to", "important",
        "security", "auth", "encrypt", "production", "scale",
        "backward compat", "breaking change", "deprecat",
        "edge case", "handle", "ensure", "verify", "validate",
        "contract", "compliance", "regulation", "gdpr", "hipaa",
        "latency", "throughput", "high availability", "reliab",
        "transaction", "acid", "consistency", "atomic",
    ]);

    let constraint_score = (constraint_count as f64 / 3.0).min(1.0) * 0.7;

    breakdown.push(ScoreFactor {
        name: "constraints".into(),
        score: constraint_score,
        weight: 0.10,
        detail: format!("{} constraints/requirements detected", constraint_count),
    });

    // ── Factor 6: Vision needs ───────────────────────────────
    let has_vision = contains_any(&prompt_lower, &[
        "screenshot", "image", "picture", "photo", "diagram",
        "chart", "graph", "ui mockup", "design mock", "figma",
    ]);

    breakdown.push(ScoreFactor {
        name: "vision".into(),
        score: if has_vision { 0.7 } else { 0.0 },
        weight: 0.05,
        detail: if has_vision {
            "Vision/image content detected".into()
        } else {
            "No vision needs".into()
        },
    });

    // ── Factor 7: Long-context indicators ────────────────────
    let long_context = contains_any(&prompt_lower, &[
        "long file", "entire file", "full source", "complete code",
        "all the code", "entire function", "whole class",
    ]);

    breakdown.push(ScoreFactor {
        name: "context_size".into(),
        score: if long_context { 0.6 } else { length_score * 0.5 },
        weight: 0.07,
        detail: if long_context {
            "Long context explicitly requested".into()
        } else {
            format!("Context need based on length: {:.0}%", length_score * 100.0)
        },
    });

    // ── Calculate weighted total ─────────────────────────────
    let total_weight: f64 = breakdown.iter().map(|f| f.weight).sum();
    let weighted_sum: f64 = breakdown.iter().map(|f| f.score * f.weight).sum();
    let complexity_score = (weighted_sum / total_weight).clamp(0.0, 1.0);

    let tier = ComplexityTier::from_score(complexity_score);

    // Min context window based on estimated tokens with 2x safety margin
    let min_context_needed = ((estimated_tokens as f64) * 2.5).ceil() as u64;

    // Clean up use cases
    if use_cases.is_empty() {
        if estimated_tokens < 200 {
            use_cases.push("coding-fast".into());
        } else {
            use_cases.push("coding".into());
        }
    }
    use_cases.sort();
    use_cases.dedup();

    AnalysisResult {
        complexity_score,
        estimated_tokens,
        use_cases,
        tier,
        breakdown,
        needs_vision: has_vision,
        min_context_needed,
    }
}

fn contains_any(text: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|&p| text.contains(p))
}

fn count_patterns(text: &str, patterns: &[&str]) -> usize {
    patterns.iter().filter(|&&p| text.contains(p)).count()
}
