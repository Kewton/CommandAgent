use anyhow::bail;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NextAction {
    Retry,
    RecoveryCircle,
    ElevatedModel,
    PackChange,
    Close,
}

impl NextAction {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Retry => "retry",
            Self::RecoveryCircle => "recovery_circle",
            Self::ElevatedModel => "elevated_model",
            Self::PackChange => "pack_change",
            Self::Close => "close",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalPresentation {
    pub card_hash: String,
    pub acceptance_sheet: String,
    pub full: bool,
    pub section5: Option<String>,
}

impl TerminalPresentation {
    pub fn new(
        card_hash: String,
        acceptance_sheet: String,
        full: bool,
        section5: Option<String>,
    ) -> anyhow::Result<Self> {
        if acceptance_sheet.trim().is_empty() {
            bail!("Gate 3/4 requires the full generated acceptance sheet");
        }
        if !full && section5.as_deref().is_none_or(str::is_empty) {
            bail!("Gate 4 requires the acceptance-sheet section 5 stop reason");
        }
        Ok(Self {
            card_hash,
            acceptance_sheet,
            full,
            section5,
        })
    }
}
