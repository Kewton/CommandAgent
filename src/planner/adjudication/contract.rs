use serde::{Deserialize, Serialize};

pub const FIX_CONTRACT_VERSION: &str = "v0";
pub const FIX_CONTRACT_REF: &str = "docs/fix-intent-contract.md";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentId {
    Create,
    Fix,
}

impl IntentId {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Fix => "fix",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceStage {
    Unstaged,
    Before,
    After,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedOutcome {
    Success,
    Failure,
    Observation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeOutcome {
    Success,
    Failure,
    Inconclusive,
    Unavailable,
    NotExecuted,
}

impl ProbeOutcome {
    pub const fn was_executed(self) -> bool {
        matches!(self, Self::Success | Self::Failure)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionRule {
    MustExecute,
    StaticAllowed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequirementImpact {
    Blocking,
    Degradable,
    FullOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequirementBinding {
    Reproducer,
    ProfileRegressionSet,
    ExistingCreateAdapter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineageRule {
    None,
    Required,
    SameAs(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvidenceRequirement {
    pub id: &'static str,
    pub binding: RequirementBinding,
    pub stage: EvidenceStage,
    pub expected: ExpectedOutcome,
    pub execution: ExecutionRule,
    pub impact: RequirementImpact,
    pub lineage: LineageRule,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhaseRole {
    pub id: &'static str,
    pub entry_requirement: Option<&'static str>,
    pub exit_requirement: Option<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanSkeleton {
    pub roles: &'static [PhaseRole],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssurancePolicy {
    ExistingCreateAdapter,
    FixV0,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileHookId {
    ExistingCreateHooks,
    FixRegressionBindings,
    FixRegressionProbe,
    Admission,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntentContract {
    pub id: IntentId,
    pub version: &'static str,
    pub contract_ref: &'static str,
    pub requirements: &'static [EvidenceRequirement],
    pub plan: PlanSkeleton,
    pub assurance: AssurancePolicy,
    pub required_profile_hooks: &'static [ProfileHookId],
}

const CREATE_REQUIREMENTS: [EvidenceRequirement; 1] = [EvidenceRequirement {
    id: "create_acceptance",
    binding: RequirementBinding::ExistingCreateAdapter,
    stage: EvidenceStage::Unstaged,
    expected: ExpectedOutcome::Observation,
    execution: ExecutionRule::MustExecute,
    impact: RequirementImpact::Blocking,
    lineage: LineageRule::None,
}];

const CREATE_PHASES: [PhaseRole; 1] = [PhaseRole {
    id: "existing_create_plan",
    entry_requirement: None,
    exit_requirement: Some("create_acceptance"),
}];

const CREATE_HOOKS: [ProfileHookId; 2] =
    [ProfileHookId::ExistingCreateHooks, ProfileHookId::Admission];

const CREATE_CONTRACT: IntentContract = IntentContract {
    id: IntentId::Create,
    version: "current",
    contract_ref: "docs/intent-skeleton.md#create-intent",
    requirements: &CREATE_REQUIREMENTS,
    plan: PlanSkeleton {
        roles: &CREATE_PHASES,
    },
    assurance: AssurancePolicy::ExistingCreateAdapter,
    required_profile_hooks: &CREATE_HOOKS,
};

const FIX_REQUIREMENTS: [EvidenceRequirement; 3] = [
    EvidenceRequirement {
        id: "before_fails",
        binding: RequirementBinding::Reproducer,
        stage: EvidenceStage::Before,
        expected: ExpectedOutcome::Failure,
        execution: ExecutionRule::MustExecute,
        impact: RequirementImpact::Blocking,
        lineage: LineageRule::Required,
    },
    EvidenceRequirement {
        id: "after_passes",
        binding: RequirementBinding::Reproducer,
        stage: EvidenceStage::After,
        expected: ExpectedOutcome::Success,
        execution: ExecutionRule::MustExecute,
        impact: RequirementImpact::Blocking,
        lineage: LineageRule::SameAs("before_fails"),
    },
    EvidenceRequirement {
        id: "no_regression",
        binding: RequirementBinding::ProfileRegressionSet,
        stage: EvidenceStage::After,
        expected: ExpectedOutcome::Success,
        execution: ExecutionRule::MustExecute,
        impact: RequirementImpact::Blocking,
        lineage: LineageRule::None,
    },
];

const FIX_PHASES: [PhaseRole; 4] = [
    PhaseRole {
        id: "reproducer_before",
        entry_requirement: None,
        exit_requirement: Some("before_fails"),
    },
    PhaseRole {
        id: "repair",
        entry_requirement: Some("before_fails"),
        exit_requirement: None,
    },
    PhaseRole {
        id: "reproducer_after",
        entry_requirement: Some("before_fails"),
        exit_requirement: Some("after_passes"),
    },
    PhaseRole {
        id: "regression",
        entry_requirement: Some("after_passes"),
        exit_requirement: Some("no_regression"),
    },
];

const FIX_HOOKS: [ProfileHookId; 3] = [
    ProfileHookId::FixRegressionBindings,
    ProfileHookId::FixRegressionProbe,
    ProfileHookId::Admission,
];

const FIX_CONTRACT: IntentContract = IntentContract {
    id: IntentId::Fix,
    version: FIX_CONTRACT_VERSION,
    contract_ref: FIX_CONTRACT_REF,
    requirements: &FIX_REQUIREMENTS,
    plan: PlanSkeleton { roles: &FIX_PHASES },
    assurance: AssurancePolicy::FixV0,
    required_profile_hooks: &FIX_HOOKS,
};

pub fn intent_contract(id: &str) -> Option<&'static IntentContract> {
    match id.trim().to_ascii_lowercase().as_str() {
        "create" => Some(&CREATE_CONTRACT),
        "fix" => Some(&FIX_CONTRACT),
        _ => None,
    }
}

pub fn is_fix_intent(id: &str) -> bool {
    intent_contract(id).is_some_and(|contract| contract.id == IntentId::Fix)
}

pub fn registered_intents() -> &'static [&'static str] {
    &["create", "fix"]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_create_and_fix_but_fails_closed_for_unknown() {
        assert_eq!(registered_intents(), ["create", "fix"]);
        assert_eq!(intent_contract("create").unwrap().id, IntentId::Create);
        assert_eq!(intent_contract("fix").unwrap().id, IntentId::Fix);
        assert!(is_fix_intent(" FIX "));
        assert!(intent_contract("research").is_none());
    }

    #[test]
    fn fix_v0_declares_the_three_fixed_requirements() {
        let contract = intent_contract("fix").unwrap();
        assert_eq!(contract.version, "v0");
        assert_eq!(contract.contract_ref, "docs/fix-intent-contract.md");
        assert_eq!(
            contract
                .requirements
                .iter()
                .map(|requirement| requirement.id)
                .collect::<Vec<_>>(),
            ["before_fails", "after_passes", "no_regression"]
        );
        assert_eq!(
            contract.requirements[1].lineage,
            LineageRule::SameAs("before_fails")
        );
        assert_eq!(contract.requirements[2].impact, RequirementImpact::Blocking);
    }
}
