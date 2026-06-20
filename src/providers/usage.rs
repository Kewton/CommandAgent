use crate::config::Provider;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ModelUsage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cached_input_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<u64>,
    pub request_count: u64,
    pub retry_count: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<String>,
    pub estimated: bool,
}

impl ModelUsage {
    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            request_count: 1,
            unavailable_reason: Some(reason.into()),
            ..Self::default()
        }
    }

    pub fn with_latency(mut self, latency_ms: u64) -> Self {
        self.latency_ms = Some(latency_ms);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsageRecord {
    pub schema_version: String,
    pub run_id: String,
    pub job_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempt_id: Option<String>,
    pub provider: String,
    pub model: String,
    pub role: RequestRole,
    pub usage: ModelUsage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestRole {
    Planner,
    Worker,
    Repair,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CostRecord {
    pub schema_version: String,
    pub token_record_id: String,
    pub currency: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_cost_microusd: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_cost_microusd: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cached_input_cost_microusd: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_cost_microusd: Option<u64>,
    pub pricing_version: String,
    pub unavailable: bool,
}

pub fn extract_usage(provider: Provider, response_json: &Value) -> ModelUsage {
    match provider {
        Provider::Gemini => extract_gemini_usage(response_json),
        Provider::OpenAi => extract_openai_usage(response_json),
        Provider::Ollama => extract_ollama_usage(response_json),
    }
}

pub fn extract_gemini_usage(value: &Value) -> ModelUsage {
    let Some(usage) = value.get("usageMetadata") else {
        return ModelUsage::unavailable("gemini_usage_metadata_missing");
    };
    ModelUsage {
        input_tokens: get_u64(usage, "promptTokenCount"),
        cached_input_tokens: get_u64(usage, "cachedContentTokenCount"),
        output_tokens: get_u64(usage, "candidatesTokenCount"),
        reasoning_tokens: get_u64(usage, "thoughtsTokenCount"),
        total_tokens: get_u64(usage, "totalTokenCount"),
        request_count: 1,
        retry_count: 0,
        latency_ms: None,
        unavailable_reason: None,
        estimated: false,
    }
}

pub fn extract_openai_usage(value: &Value) -> ModelUsage {
    let Some(usage) = value.get("usage") else {
        return ModelUsage::unavailable("openai_usage_missing");
    };
    ModelUsage {
        input_tokens: get_u64(usage, "input_tokens"),
        cached_input_tokens: usage
            .get("input_tokens_details")
            .and_then(|details| get_u64(details, "cached_tokens")),
        output_tokens: get_u64(usage, "output_tokens"),
        reasoning_tokens: usage
            .get("output_tokens_details")
            .and_then(|details| get_u64(details, "reasoning_tokens")),
        total_tokens: get_u64(usage, "total_tokens"),
        request_count: 1,
        retry_count: 0,
        latency_ms: None,
        unavailable_reason: None,
        estimated: false,
    }
}

pub fn extract_ollama_usage(value: &Value) -> ModelUsage {
    let input = get_u64(value, "prompt_eval_count");
    let output = get_u64(value, "eval_count");
    if input.is_none() && output.is_none() {
        return ModelUsage::unavailable("ollama_usage_missing");
    }
    ModelUsage {
        input_tokens: input,
        cached_input_tokens: None,
        output_tokens: output,
        reasoning_tokens: None,
        total_tokens: input.zip(output).map(|(input, output)| input + output),
        request_count: 1,
        retry_count: 0,
        latency_ms: None,
        unavailable_reason: None,
        estimated: true,
    }
}

fn get_u64(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(Value::as_u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn maps_gemini_usage_metadata() {
        let usage = extract_gemini_usage(&json!({
            "usageMetadata": {
                "promptTokenCount": 10,
                "cachedContentTokenCount": 4,
                "candidatesTokenCount": 3,
                "thoughtsTokenCount": 2,
                "totalTokenCount": 15
            }
        }));

        assert_eq!(usage.input_tokens, Some(10));
        assert_eq!(usage.cached_input_tokens, Some(4));
        assert_eq!(usage.output_tokens, Some(3));
        assert_eq!(usage.reasoning_tokens, Some(2));
        assert_eq!(usage.total_tokens, Some(15));
        assert!(!usage.estimated);
    }

    #[test]
    fn maps_openai_response_usage() {
        let usage = extract_openai_usage(&json!({
            "usage": {
                "input_tokens": 10,
                "output_tokens": 8,
                "total_tokens": 18,
                "input_tokens_details": {"cached_tokens": 6},
                "output_tokens_details": {"reasoning_tokens": 2}
            }
        }));

        assert_eq!(usage.cached_input_tokens, Some(6));
        assert_eq!(usage.reasoning_tokens, Some(2));
        assert_eq!(usage.total_tokens, Some(18));
    }

    #[test]
    fn ollama_usage_is_estimated_when_counts_exist() {
        let usage = extract_ollama_usage(&json!({
            "prompt_eval_count": 7,
            "eval_count": 5
        }));

        assert_eq!(usage.total_tokens, Some(12));
        assert!(usage.estimated);
    }

    #[test]
    fn missing_usage_is_explicitly_unavailable() {
        let usage = extract_openai_usage(&json!({}));

        assert_eq!(
            usage.unavailable_reason.as_deref(),
            Some("openai_usage_missing")
        );
    }
}
