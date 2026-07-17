#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RepairTargetSelectionReason {
    DiagnosisMapped,
    TracebackMapped,
    EvidenceMapped,
    ContractAttribute,
    RepairChanged,
    RequiredPath,
    Fallback,
}

impl RepairTargetSelectionReason {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::DiagnosisMapped => "diagnosis_mapped",
            Self::TracebackMapped => "traceback_mapped",
            Self::EvidenceMapped => "evidence_mapped",
            Self::ContractAttribute => "contract_attribute",
            Self::RepairChanged => "repair_changed",
            Self::RequiredPath => "required_path",
            Self::Fallback => "fallback",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RepairTargetSelection {
    pub(crate) selected_targets: Vec<String>,
    pub(crate) selection_reason: RepairTargetSelectionReason,
}

impl RepairTargetSelection {
    pub(crate) fn primary_target(&self) -> Option<&str> {
        self.selected_targets.first().map(String::as_str)
    }
}
