use crate::analyzer::{AnalysisResult, ComplexityTier};
use crate::config::{AvailableModel, Strategy, WitcherConfig};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SelectionResult {
    pub model: AvailableModel,
    pub reasoning: Vec<String>,
    pub complexity_tier: String,
    pub estimated_cost_input: f64,
    pub estimated_cost_output: f64,
}

/// Select the best available model for the given request.
pub fn select_model(
    analysis: &AnalysisResult,
    config: &WitcherConfig,
    available: &[AvailableModel],
) -> Option<SelectionResult> {
    if available.is_empty() {
        return None;
    }

    // Step 1: Check overrides
    // Use-case override
    for uc in &analysis.use_cases {
        if let Some(forced_model) = config.overrides.use_case.get(uc) {
            if let Some(m) = available.iter().find(|m| m.id == *forced_model) {
                return Some(score_model(m, analysis, config, vec![
                    format!("Forced by use-case override for '{}'", uc),
                ]));
            }
        }
    }

    // Complexity tier override
    if let Some(forced_model) = config.overrides.complexity_tier.get(analysis.tier.as_str()) {
        if let Some(m) = available.iter().find(|m| m.id == *forced_model) {
            return Some(score_model(m, analysis, config, vec![
                format!("Forced by complexity tier override for '{}'", analysis.tier.as_str()),
            ]));
        }
    }

    // Default override
    if let Some(forced_model) = &config.overrides.default_model {
        if let Some(m) = available.iter().find(|m| m.id == *forced_model) {
            return Some(score_model(m, analysis, config, vec![
                "Forced by default model override".into(),
            ]));
        }
    }

    // Step 2: Filter models that can handle the context
    let mut viable: Vec<&AvailableModel> = available
        .iter()
        .filter(|m| m.context >= analysis.min_context_needed)
        .collect();

    if viable.is_empty() {
        // If no model can handle the context, pick the one with largest context
        viable = available.iter().collect();
    }

    // Step 3: Filter by vision needs
    if analysis.needs_vision {
        let vision_models: Vec<&AvailableModel> = viable
            .iter()
            .filter(|m| m.use_cases.contains(&"vision".to_string()))
            .copied()
            .collect();
        if !vision_models.is_empty() {
            viable = vision_models;
        }
        // If no vision-specific models, proceed with all (may still work)
    }

    // Step 4: Score and rank
    let mut scored: Vec<(&AvailableModel, f64, Vec<String>)> = viable
        .iter()
        .map(|m| {
            let (score, reasons) = score_model_raw(m, analysis, config);
            (*m, score, reasons)
        })
        .collect();

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    if let Some((best_model, _score, reasons)) = scored.first() {
        Some(score_model(best_model, analysis, config, reasons.clone()))
    } else {
        // Fallback to first available
        available.first().map(|m| score_model(m, analysis, config, vec!["Fallback selection".into()]))
    }
}

fn score_model_raw(
    model: &AvailableModel,
    analysis: &AnalysisResult,
    config: &WitcherConfig,
) -> (f64, Vec<String>) {
    let mut score = 50.0; // base score
    let mut reasons = Vec::new();

    // ── Capability fit ───────────────────────────────────────
    let use_case_match = analysis
        .use_cases
        .iter()
        .filter(|uc| model.use_cases.contains(uc))
        .count();

    let uc_ratio = if analysis.use_cases.is_empty() {
        0.5
    } else {
        use_case_match as f64 / analysis.use_cases.len() as f64
    };

    score += uc_ratio * 25.0;

    if use_case_match > 0 {
        reasons.push(format!(
            "Matches {}/{} detected use cases ({})",
            use_case_match,
            analysis.use_cases.len(),
            analysis
                .use_cases
                .iter()
                .filter(|uc| model.use_cases.contains(uc))
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    // ── Context window adequacy ──────────────────────────────
    let context_ratio = model.context as f64 / analysis.min_context_needed as f64;
    if context_ratio >= 3.0 {
        score += 10.0;
        reasons.push(format!("Ample context window ({}K, need {}K)", model.context / 1000, analysis.min_context_needed / 1000));
    } else if context_ratio >= 1.5 {
        score += 5.0;
        reasons.push(format!("Adequate context window ({}K)", model.context / 1000));
    } else if context_ratio < 1.0 {
        score -= 15.0;
        reasons.push(format!("TIGHT context: {}K available, {}K needed", model.context / 1000, analysis.min_context_needed / 1000));
    }

    // ── Role bonus ───────────────────────────────────────────
    if model.role == "main" {
        score += 8.0;
        reasons.push("Main model (better reasoning)".into());
    }

    // ── Cost / tier considerations ──────────────────────────
    match config.strategy {
        Strategy::FreeFirst => {
            if model.tier == "free" {
                score += 20.0;
                reasons.push("Free model — preferred by FreeFirst strategy".into());
            } else if model.tier == "trial" {
                score += 15.0;
                reasons.push("Trial model — free during trial period".into());
            } else if model.cost_per_1m_input < 1.0 {
                score += 5.0;
                reasons.push(format!("Cheap paid ($0.{:.0}/M input)", model.cost_per_1m_input * 100.0));
            } else {
                score -= 10.0;
                reasons.push(format!("Paid model (${:.2}/M input) — FreeFirst may prefer cheaper", model.cost_per_1m_input));
            }
        }
        Strategy::CostConscious => {
            if model.tier == "free" && analysis.tier != ComplexityTier::Extreme {
                score += 15.0;
                reasons.push("Free model sufficient for this complexity".into());
            } else if model.cost_per_1m_input < 2.0 {
                score += 8.0;
                reasons.push(format!("Cost-effective (${:.2}/M input)", model.cost_per_1m_input));
            }
        }
        Strategy::BestQuality => {
            if !model.free {
                score += 12.0;
                reasons.push("Paid model preferred by BestQuality strategy".into());
            }
            if model.cost_per_1m_input > 2.0 {
                score += 5.0;
                reasons.push("High-end model (premium capability)".into());
            }
        }
    }

    // ── Complexity-appropriateness ──────────────────────────
    match analysis.tier {
        ComplexityTier::Low => {
            // Simple tasks — prefer small/fast/cheap
            if model.role == "small" {
                score += 10.0;
                reasons.push("Small model ideal for low complexity".into());
            }
            if model.free {
                score += 5.0;
                reasons.push("Free model ideal for simple request".into());
            }
        }
        ComplexityTier::Medium => {
            if model.role == "main" {
                score += 6.0;
                reasons.push("Main model for medium complexity".into());
            }
        }
        ComplexityTier::High => {
            if model.role == "main" {
                score += 12.0;
                reasons.push("Main model needed for high complexity".into());
            }
            if model.role == "small" {
                score -= 8.0;
                reasons.push("Small model may struggle with high complexity".into());
            }
            if !model.free && model.cost_per_1m_input > 1.0 {
                score += 5.0;
                reasons.push("Premium model better for complex tasks".into());
            }
        }
        ComplexityTier::Extreme => {
            if model.role == "main" {
                score += 15.0;
                reasons.push("Main model required for extreme complexity".into());
            }
            if model.role == "small" {
                score -= 20.0;
                reasons.push("Small model may fail on extreme complexity".into());
            }
            if !model.free && model.cost_per_1m_input > 2.0 {
                score += 8.0;
                reasons.push("Premium model for extreme complexity".into());
            }
        }
    }

    // ── Vision handling ──────────────────────────────────────
    if analysis.needs_vision && model.use_cases.contains(&"vision".to_string()) {
        score += 10.0;
        reasons.push("Vision-capable model".into());
    }

    // ── Provider reputation ──────────────────────────────────
    match model.provider_id.as_str() {
        "anthropic" => {
            score += 3.0;
            reasons.push("Official Anthropic — most reliable".into());
        }
        "deepseek" | "moonshot" => {
            score += 2.0;
            reasons.push(format!("Proven provider: {}", model.provider_id));
        }
        _ => {}
    }

    (score, reasons)
}

fn score_model(
    model: &AvailableModel,
    analysis: &AnalysisResult,
    _config: &WitcherConfig,
    reasons: Vec<String>,
) -> SelectionResult {
    let tokens_in = analysis.estimated_tokens as f64 / 1_000_000.0;
    let tokens_out_est = tokens_in * 2.0; // rough output estimate

    SelectionResult {
        model: model.clone(),
        reasoning: reasons,
        complexity_tier: analysis.tier.as_str().to_string(),
        estimated_cost_input: tokens_in * model.cost_per_1m_input,
        estimated_cost_output: tokens_out_est * model.cost_per_1m_output,
    }
}
