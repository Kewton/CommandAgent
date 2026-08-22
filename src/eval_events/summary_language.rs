use std::borrow::Cow;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum SummaryLanguage {
    #[default]
    English,
    Japanese,
}

impl SummaryLanguage {
    #[allow(dead_code)] // Staged API for the follow-up CLI `--lang` consumer.
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "en" => Ok(Self::English),
            "ja" => Ok(Self::Japanese),
            _ => Err(format!(
                "unsupported summary language `{value}`; use `en` or `ja`"
            )),
        }
    }

    pub(crate) fn from_process_locale() -> Self {
        ["LC_ALL", "LC_MESSAGES", "LANG"]
            .into_iter()
            .find_map(|name| {
                std::env::var(name)
                    .ok()
                    .filter(|value| !value.trim().is_empty())
            })
            .map_or(Self::English, |locale| Self::from_locale(&locale))
    }

    pub(crate) fn from_locale(locale: &str) -> Self {
        let locale = locale.trim().to_ascii_lowercase().replace('-', "_");
        if locale == "ja" || locale.starts_with("ja_") {
            Self::Japanese
        } else {
            Self::English
        }
    }

    pub(crate) fn heading(self) -> &'static str {
        match self {
            Self::English => "Run result",
            Self::Japanese => "実行結果",
        }
    }

    pub(crate) fn result_label(self) -> &'static str {
        match self {
            Self::English => "Result",
            Self::Japanese => "結果",
        }
    }

    pub(crate) fn assurance_label(self) -> &'static str {
        match self {
            Self::English => "Assurance",
            Self::Japanese => "保証",
        }
    }

    pub(crate) fn gate_label(self) -> &'static str {
        match self {
            Self::English => "Gate",
            Self::Japanese => "ゲート",
        }
    }

    pub(crate) fn stop_reason_label(self) -> &'static str {
        match self {
            Self::English => "Stop reason",
            Self::Japanese => "停止理由",
        }
    }

    pub(crate) fn next_action_label(self) -> &'static str {
        match self {
            Self::English => "Next action",
            Self::Japanese => "次の一手",
        }
    }

    pub(crate) fn changed_files_label(self) -> &'static str {
        match self {
            Self::English => "Changed files",
            Self::Japanese => "変更ファイル",
        }
    }

    pub(crate) fn verification_label(self) -> &'static str {
        match self {
            Self::English => "Verification",
            Self::Japanese => "検証",
        }
    }

    pub(crate) fn verification_commands_heading(self) -> &'static str {
        match self {
            Self::English => "Verification commands",
            Self::Japanese => "検証コマンド",
        }
    }

    pub(crate) fn exit_code_label(self) -> &'static str {
        match self {
            Self::English => "Exit code",
            Self::Japanese => "終了コード",
        }
    }

    pub(crate) fn none(self) -> &'static str {
        match self {
            Self::English => "none",
            Self::Japanese => "なし",
        }
    }

    pub(crate) fn unavailable(self) -> &'static str {
        match self {
            Self::English => "unavailable",
            Self::Japanese => "取得不可",
        }
    }

    pub(crate) fn status(self, value: &str) -> Cow<'_, str> {
        self.closed_value(
            value,
            &[
                ("completed", "完了"),
                ("complete", "完了"),
                ("running", "実行中"),
                ("failed", "失敗"),
                ("incomplete", "未完了"),
                ("interrupted", "中断"),
                ("aborted", "中止"),
                ("partial", "一部完了"),
            ],
        )
    }

    pub(crate) fn gate(self, value: &str) -> Cow<'_, str> {
        self.closed_value(
            value,
            &[
                ("pass", "合格"),
                ("passed", "合格"),
                ("failed", "不合格"),
                ("partial", "一部合格"),
                ("not_checked", "未確認"),
                ("not_recorded", "未記録"),
                ("not_applicable", "対象外"),
            ],
        )
    }

    pub(crate) fn stop_reason(self, value: &str) -> Cow<'_, str> {
        self.closed_value(
            value,
            &[
                ("completed", "完了"),
                ("interrupted by user", "ユーザーにより中断"),
            ],
        )
    }

    pub(crate) fn next_action(self, value: &str) -> Cow<'_, str> {
        self.closed_value(
            value,
            &[
                ("none", "追加操作なし"),
                ("fix_command_failure", "コマンドの失敗を修正する"),
                ("resume_or_rerun_command", "コマンドを再開または再実行する"),
                (
                    "inspect_summary_and_resume_or_rerun",
                    "summary.md を確認して再開または再実行する",
                ),
                (
                    "run_setup_interaction_probe_to_enable_interaction_release_checks",
                    "interaction probe を準備して再検証する",
                ),
            ],
        )
    }

    fn closed_value<'a>(
        self,
        value: &'a str,
        translations: &[(&'static str, &'static str)],
    ) -> Cow<'a, str> {
        if self == Self::English {
            return Cow::Borrowed(value);
        }
        translations
            .iter()
            .find_map(|(source, translated)| (*source == value).then_some(*translated))
            .map_or_else(|| Cow::Borrowed(value), Cow::Borrowed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_language_is_closed_to_en_and_ja() {
        assert_eq!(SummaryLanguage::parse("en"), Ok(SummaryLanguage::English));
        assert_eq!(SummaryLanguage::parse("JA"), Ok(SummaryLanguage::Japanese));
        assert!(SummaryLanguage::parse("fr").is_err());
    }

    #[test]
    fn locale_projection_defaults_to_english_and_recognizes_japanese() {
        assert_eq!(
            SummaryLanguage::from_locale("ja_JP.UTF-8"),
            SummaryLanguage::Japanese
        );
        assert_eq!(
            SummaryLanguage::from_locale("ja-JP"),
            SummaryLanguage::Japanese
        );
        assert_eq!(
            SummaryLanguage::from_locale("C.UTF-8"),
            SummaryLanguage::English
        );
        assert_eq!(
            SummaryLanguage::from_locale("en_US.UTF-8"),
            SummaryLanguage::English
        );
    }

    #[test]
    fn japanese_projection_translates_closed_guidance_but_preserves_diagnostics() {
        let language = SummaryLanguage::Japanese;
        assert_eq!(language.status("completed"), "完了");
        assert_eq!(language.gate("partial"), "一部合格");
        assert_eq!(
            language.next_action("fix_command_failure"),
            "コマンドの失敗を修正する"
        );
        assert_eq!(
            language.next_action("custom_recovery:keep_this_code"),
            "custom_recovery:keep_this_code"
        );
    }
}
