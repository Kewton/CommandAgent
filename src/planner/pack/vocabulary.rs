use std::fmt;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer};

macro_rules! closed_id {
    ($visibility:vis enum $name:ident { $($variant:ident => $wire:expr),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        $visibility enum $name {
            $($variant),+
        }

        impl $name {
            pub fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $wire),+
                }
            }

            pub fn parse(value: &str) -> Option<Self> {
                $(if value == $wire {
                    return Some(Self::$variant);
                })+
                None
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::parse(&value).ok_or_else(|| {
                    D::Error::custom(format!("unregistered {} id `{value}`", stringify!($name)))
                })
            }
        }
    };
}

closed_id! {
    pub enum PackProfile {
        Data => "data",
        PythonCli => "python-cli",
        Ingest => "ingest",
        Nextjs => crate::planner::profiles::nextjs::PROFILE_ID,
    }
}

closed_id! {
    pub enum PackIntent {
        Create => "create",
        Fix => "fix",
        Investigate => "investigate",
    }
}

closed_id! {
    pub enum InjectionPoint {
        DataInspection => "data-inspection",
        DataCleaning => "data-cleaning",
        DataAggregation => "data-aggregation",
        DataReporting => "data-reporting",
        DataValidation => "data-validation",
        CliScaffold => "cli-scaffold",
        CliImplementation => "cli-implementation",
        CliValidation => "cli-validation",
        IngestImplement => "ingest-implement",
        IngestRun => "ingest-run",
        IngestStructuralGate => "ingest-structural-gate",
        DeclareIngestInspection => "declare-ingest-inspection",
        ImplementIngestDelivery => "implement-ingest-delivery",
        ProjectSetup => "project-setup",
        CoreImplementation => "core-implementation",
        ContractWiring => "contract-wiring",
        BuildVerification => "build-verification",
        ReproduceCandidate => "reproduce-candidate",
        Diagnose => "diagnose",
        BindVerify => "bind-verify",
        ReproduceBefore => "reproduce-before",
        IsolateCause => "isolate-cause",
        ImplementFix => "implement-fix",
        Repair => "repair",
        VerifyAfter => "verify-after",
        VerifyRegressions => "verify-regressions",
    }
}

closed_id! {
    pub enum AssistSource {
        IngestSnapshotStructure => "ingest_snapshot_structure_injected",
        IngestCandidateIds => "ingest_candidate_ids_injected",
        ReproducerOutput => "R_output",
        InvestigationWorkspaceFiles => "investigation_workspace_files",
        FixFailureOutput => "R_failure_output",
        VerifiedDiagnosis => "verified_diagnosis",
        CliProbe => "cli_probe",
        C3Binding => "c3_binding",
        DataInspectionSchema => "data_inspection_schema",
        BrowserInteraction => "browser_interaction",
        HumanDirective => "human_directive",
    }
}

closed_id! {
    pub enum VocabularySource {
        RequiredDelivery => "required_delivery_vocabulary",
        IngestCandidateIds => "ingest_candidate_ids_injected",
        InvestigationWorkspaceFiles => "investigation_workspace_files",
    }
}

closed_id! {
    pub enum ExtractionId {
        NumericClaims => "claims_binding.extract_numeric_claims",
        DateLabelSpans => "claims_binding.DateLabelSpans",
        CliUsageCase => "argv_probe.extract_usage_case",
        CliOutputExamples => "argv_probe.extract_output_examples",
        HelpOptions => "help_binding.extract_options",
        DiagnosisBinding => "investigation_binding.bind_diagnosis",
        CandidateEnumeration => "accounting.enumerate",
        SourceValues => "source_binding.source_values",
    }
}

closed_id! {
    pub enum NormalizerId {
        Identity => "identity",
        JapaneseDateToIso => "japanese_date_to_iso",
        DocumentYearContext => "document_year_context",
        NumberCanonical => "number_canonical",
        Time24h => "time24h",
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CheckId(String);

impl CheckId {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn registered(value: &str) -> bool {
        const PROFILE_IDS: &[&str] = &[
            "pipeline_probe",
            "data_inspection_schema",
            "data_results_schema",
            "data_reconciliation",
            "data_claims_binding",
            "data_rerun_consistency",
            "cli_probe",
            "help_binding",
            "cli_output_claims",
            "cli_rerun_consistency",
            "ingest_source_binding",
            "ingest_candidate_accounting",
            "ingest_format_schema",
            "ingest_rerun_consistency",
        ];
        let profile_registered = PROFILE_IDS.contains(&value)
            && crate::planner::capability_catalog::registry()
                .iter()
                .any(|spec| spec.id == value);
        let intent_registered = ["fix", "investigate"].into_iter().any(|intent| {
            crate::planner::adjudication::contract::intent_contract(intent).is_some_and(
                |contract| {
                    contract
                        .requirements
                        .iter()
                        .any(|requirement| requirement.id == value)
                },
            )
        });
        profile_registered || intent_registered
    }
}

impl<'de> Deserialize<'de> for CheckId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        if Self::registered(&value) {
            Ok(Self(value))
        } else {
            Err(D::Error::custom(format!(
                "unregistered check/gate id `{value}`"
            )))
        }
    }
}
