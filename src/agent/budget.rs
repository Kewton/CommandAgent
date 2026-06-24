use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetScope {
    ModelRequest,
    WorkerSession,
    PlannerSession,
    Step,
    Phase,
    Job,
    ToolResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetAction {
    Stop,
    DeterministicCompaction,
    ShrinkToolResult,
    ReplanRequest,
    ApprovalRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BudgetContract {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_input_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tool_result_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_iterations: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_model_requests: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_cumulative_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_estimated_cost_microusd: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BudgetUsage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub tool_result_tokens: u64,
    #[serde(default)]
    pub iterations: u64,
    #[serde(default)]
    pub model_requests: u64,
    #[serde(default)]
    pub cumulative_tokens: u64,
    #[serde(default)]
    pub estimated_cost_microusd: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum BudgetDecision {
    WithinBudget,
    Exceeded {
        scope: BudgetScope,
        action: BudgetAction,
        reason: String,
    },
}

impl BudgetContract {
    pub fn check(&self, scope: BudgetScope, usage: &BudgetUsage) -> BudgetDecision {
        if exceeds(self.max_input_tokens, usage.input_tokens) {
            return exceeded(scope, BudgetAction::Stop, "max_input_tokens");
        }
        if exceeds(self.max_output_tokens, usage.output_tokens) {
            return exceeded(scope, BudgetAction::Stop, "max_output_tokens");
        }
        if exceeds(self.max_tool_result_tokens, usage.tool_result_tokens) {
            return exceeded(
                scope,
                BudgetAction::ShrinkToolResult,
                "max_tool_result_tokens",
            );
        }
        if exceeds(self.max_iterations, usage.iterations) {
            return exceeded(scope, BudgetAction::Stop, "max_iterations");
        }
        if exceeds(self.max_model_requests, usage.model_requests) {
            return exceeded(scope, BudgetAction::Stop, "max_model_requests");
        }
        if exceeds(self.max_cumulative_tokens, usage.cumulative_tokens) {
            return exceeded(
                scope,
                BudgetAction::DeterministicCompaction,
                "max_cumulative_tokens",
            );
        }
        if exceeds(
            self.max_estimated_cost_microusd,
            usage.estimated_cost_microusd,
        ) {
            return exceeded(
                scope,
                BudgetAction::ApprovalRequired,
                "max_estimated_cost_microusd",
            );
        }
        BudgetDecision::WithinBudget
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolResultBudget {
    pub max_output_chars: usize,
}

impl Default for ToolResultBudget {
    fn default() -> Self {
        Self {
            max_output_chars: 64 * 1024,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TruncationRecord {
    pub truncated: bool,
    pub original_chars: usize,
    pub returned_chars: usize,
    pub reason: String,
}

pub fn enforce_tool_result_budget(
    output: String,
    budget: ToolResultBudget,
) -> (String, TruncationRecord) {
    let original_chars = output.chars().count();
    if original_chars <= budget.max_output_chars {
        return (
            output,
            TruncationRecord {
                truncated: false,
                original_chars,
                returned_chars: original_chars,
                reason: "within_budget".to_string(),
            },
        );
    }

    let mut bounded = output
        .chars()
        .take(budget.max_output_chars)
        .collect::<String>();
    bounded.push_str("\n[commandagent: tool result truncated by budget]");
    let returned_chars = bounded.chars().count();
    (
        bounded,
        TruncationRecord {
            truncated: true,
            original_chars,
            returned_chars,
            reason: "max_output_chars".to_string(),
        },
    )
}

fn exceeds(limit: Option<u64>, observed: u64) -> bool {
    limit.is_some_and(|limit| observed > limit)
}

fn exceeded(scope: BudgetScope, action: BudgetAction, reason: &str) -> BudgetDecision {
    BudgetDecision::Exceeded {
        scope,
        action,
        reason: reason.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_check_stops_on_model_request_limit() {
        let contract = BudgetContract {
            max_model_requests: Some(2),
            ..BudgetContract::default()
        };
        let usage = BudgetUsage {
            model_requests: 3,
            ..BudgetUsage::default()
        };

        assert_eq!(
            contract.check(BudgetScope::Step, &usage),
            BudgetDecision::Exceeded {
                scope: BudgetScope::Step,
                action: BudgetAction::Stop,
                reason: "max_model_requests".to_string(),
            }
        );
    }

    #[test]
    fn tool_result_budget_records_truncation() {
        let (output, record) = enforce_tool_result_budget(
            "abcdef".to_string(),
            ToolResultBudget {
                max_output_chars: 3,
            },
        );

        assert!(record.truncated);
        assert_eq!(record.original_chars, 6);
        assert!(output.starts_with("abc"));
        assert!(output.contains("tool result truncated"));
    }
}
