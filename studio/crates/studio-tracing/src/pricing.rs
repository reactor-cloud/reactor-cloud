use crate::types::{ModelPricing, StepCost, TokenUsage};

pub fn get_model_pricing(model_id: &str) -> Option<ModelPricing> {
    let (input_per_m, output_per_m, cache_write, cache_read) = match model_id {
        // Claude models (via OpenRouter)
        m if m.contains("claude-3-5-sonnet") || m.contains("claude-sonnet-4") => (3.0, 15.0, Some(3.75), Some(0.30)),
        m if m.contains("claude-3-opus") || m.contains("claude-opus-4") => (15.0, 75.0, Some(18.75), Some(1.50)),
        m if m.contains("claude-3-5-haiku") || m.contains("claude-haiku") => (0.80, 4.0, Some(1.0), Some(0.08)),
        m if m.contains("claude-3-haiku") => (0.25, 1.25, Some(0.30), Some(0.03)),

        // GPT models
        m if m.contains("gpt-4o") => (2.50, 10.0, None, None),
        m if m.contains("gpt-4-turbo") => (10.0, 30.0, None, None),
        m if m.contains("gpt-4") => (30.0, 60.0, None, None),
        m if m.contains("gpt-3.5-turbo") => (0.50, 1.50, None, None),
        m if m.contains("o1-preview") => (15.0, 60.0, None, None),
        m if m.contains("o1-mini") => (3.0, 12.0, None, None),

        // Gemini models
        m if m.contains("gemini-2.0-flash") => (0.10, 0.40, None, None),
        m if m.contains("gemini-1.5-pro") => (1.25, 5.0, Some(0.315), Some(0.315)),
        m if m.contains("gemini-1.5-flash") => (0.075, 0.30, Some(0.01875), Some(0.01875)),

        // DeepSeek
        m if m.contains("deepseek-chat") || m.contains("deepseek-v3") => (0.14, 0.28, Some(0.014), None),
        m if m.contains("deepseek-reasoner") => (0.55, 2.19, Some(0.14), None),

        // Llama models
        m if m.contains("llama-3.3-70b") => (0.40, 0.40, None, None),
        m if m.contains("llama-3.1-405b") => (2.0, 2.0, None, None),

        // Qwen models
        m if m.contains("qwen-2.5-coder-32b") => (0.15, 0.60, None, None),
        m if m.contains("qwen-2.5-72b") => (0.35, 0.40, None, None),

        _ => return None,
    };

    Some(ModelPricing {
        model_id: model_id.to_string(),
        input_per_m_tok: input_per_m,
        output_per_m_tok: output_per_m,
        cache_write_per_m_tok: cache_write,
        cache_read_per_m_tok: cache_read,
        fetched_at: chrono::Utc::now().timestamp_millis(),
    })
}

pub fn calculate_cost(model_id: &str, usage: &TokenUsage) -> Option<StepCost> {
    let pricing = get_model_pricing(model_id)?;

    let input_cost = (usage.input_tokens as f64 / 1_000_000.0) * pricing.input_per_m_tok;
    let output_cost = (usage.output_tokens as f64 / 1_000_000.0) * pricing.output_per_m_tok;

    let mut total_cost = input_cost + output_cost;

    // Add cache costs if applicable
    if let (Some(cache_write_price), Some(cache_tokens)) =
        (pricing.cache_write_per_m_tok, usage.cache_creation_tokens)
    {
        total_cost += (cache_tokens as f64 / 1_000_000.0) * cache_write_price;
    }

    if let (Some(cache_read_price), Some(cache_tokens)) =
        (pricing.cache_read_per_m_tok, usage.cache_read_tokens)
    {
        total_cost += (cache_tokens as f64 / 1_000_000.0) * cache_read_price;
    }

    Some(StepCost {
        input_cost,
        output_cost,
        total_cost,
        pricing,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claude_pricing() {
        let pricing = get_model_pricing("anthropic/claude-3-5-sonnet").unwrap();
        assert_eq!(pricing.input_per_m_tok, 3.0);
        assert_eq!(pricing.output_per_m_tok, 15.0);
    }

    #[test]
    fn test_cost_calculation() {
        let usage = TokenUsage {
            input_tokens: 1000,
            output_tokens: 500,
            cache_creation_tokens: None,
            cache_read_tokens: None,
        };

        let cost = calculate_cost("anthropic/claude-3-5-sonnet", &usage).unwrap();
        assert!((cost.input_cost - 0.003).abs() < 0.0001);
        assert!((cost.output_cost - 0.0075).abs() < 0.0001);
    }
}
