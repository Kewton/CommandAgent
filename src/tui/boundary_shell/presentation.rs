use anyhow::{Context, bail};

use crate::planner::pack::catalog::PackLocator;

use super::acceptance::{NextAction, TerminalPresentation};
use super::confirmation::{ConfirmationIdentity, PackSelection};
use super::pack_catalog;

pub fn render_gate_one(
    identity: &ConfirmationIdentity,
    pack_locator: &PackLocator,
) -> anyhow::Result<String> {
    validate_complete_identity_with_locator(identity, pack_locator)?;
    let card_hash = identity.card_hash()?;
    let mut lines = vec![
        "# Gate 1 — 実行前の確認".to_string(),
        String::new(),
        "## 実行する内容".to_string(),
        String::new(),
        format!("- 依頼: {}", inline(&identity.request)),
        format!("- 作業内容: {}", work_type(identity)),
        format!(
            "- この作業として判定した根拠: {}",
            identity.route_bases.join("; ")
        ),
        format!("- 契約の参照先: {}", identity.contract_ref),
        String::new(),
        "## 必須チェック".to_string(),
        String::new(),
    ];
    if let Some(manifest) = identity.draft_manifest.as_ref() {
        let mut profile_lines = vec![
            format!(
                "- プロファイル: {}（draft / 未承認 / 保証上限 {}）",
                identity.profile, manifest.assurance_ceiling
            ),
            format!("- manifest: {}", manifest.hash),
        ];
        if let Some(base) = manifest.base_profile.as_ref() {
            profile_lines.push(format!(
                "- overlay: {} / base: {}（admitted） / source: {} / {}",
                identity.profile, base, manifest.source, manifest.hash
            ));
        }
        lines.splice(5..5, profile_lines);
    }
    lines.extend(identity.contract_checks.iter().map(|check| {
        format!(
            "- {}",
            contract_check_description(&identity.profile, check, &identity.contract_ref)
        )
    }));
    lines.extend([
        String::new(),
        "## 類似実行の結果".to_string(),
        String::new(),
        format!(
            "- 全必須チェックに合格した実行: {}件中{}件 ({})",
            identity.band_denominator, identity.band_full, identity.band_rate
        ),
        format!(
            "- 比較対象: {}; {} までの証跡",
            identity.band_arm, identity.band_measurement
        ),
        format!("- 証跡の参照先: {}", identity.band_source),
        format!("- 合格の条件: {}", full_meaning(identity)),
        String::new(),
        "## ファイルへのアクセス".to_string(),
        String::new(),
        format!("- 変更可能な範囲: {}", identity.workspace),
        String::new(),
        "## モデルとプリセット".to_string(),
        String::new(),
        format!(
            "- 計画モデル: {} / {}",
            identity.pins.planner_provider, identity.pins.planner_model
        ),
        format!(
            "- 実行モデル: {} / {}",
            identity.pins.executor_provider, identity.pins.executor_model
        ),
        format!("- 計画プリセット: {}", identity.pins.preset),
    ]);
    render_pack(identity, pack_locator, &mut lines)?;
    let candidates = pack_catalog::compatible(&identity.profile, &identity.intent);
    lines.push(if candidates.is_empty() {
        "- ほかに利用可能な検証パック: なし".to_string()
    } else {
        format!(
            "- ほかに利用可能な検証パック: {}",
            candidates
                .iter()
                .map(|pack| format!("{}@{} / {}", pack.id, pack.version, pack.hash))
                .collect::<Vec<_>>()
                .join("; ")
        )
    });
    lines.extend([
        String::new(),
        "## 確認".to_string(),
        String::new(),
        format!("- 確認 ID (内容が1つでも変わると ID も変わります): {card_hash}"),
        "これは提案であり、実行結果ではありません。".to_string(),
        format!("実行前にこの ID と完全一致する内容を確認してください。CLI では /confirm {card_hash} を使用します。"),
    ]);
    Ok(lines.join("\n"))
}

fn inline(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn work_type(identity: &ConfirmationIdentity) -> String {
    let profile = match identity.profile.as_str() {
        "python-cli" => "Python CLI ツール",
        "nextjs" => "Web アプリ",
        "data" => "データ処理パイプライン",
        "ingest" => "データ取り込みパイプライン",
        _ => identity.profile.as_str(),
    };
    let intent = match identity.intent.as_str() {
        "create" => "新しい機能を作成",
        "fix" => "既存機能を修正",
        "investigate" => "問題を調査",
        _ => identity.intent.as_str(),
    };
    let family = match identity.task_family.as_str() {
        "filter" => "絞り込み",
        "stats" => "集計",
        "aggregation" => "集約",
        "timeseries" => "時系列",
        "list" => "一覧",
        "table" => "表",
        _ => identity.task_family.as_str(),
    };
    format!(
        "{intent} ({}): {profile} ({}) / {family} ({})",
        identity.intent, identity.profile, identity.task_family
    )
}

fn contract_check_description(profile: &str, check: &str, contract_ref: &str) -> String {
    let description = match (profile, check) {
        ("python-cli", "C1") => "実行動作: 通常のコマンドは成功し、不正な入力はエラーになる",
        ("python-cli", "C2") => {
            "ヘルプの正確さ: --help と実際に受け付けるオプション・引数が一致する"
        }
        ("python-cli", "C3") => "出力の正確さ: README の例や説明が実際のコマンド出力と一致する",
        ("python-cli", "C4") => "再現性: 同じケースを再実行しても同じ結果になる",
        ("data", "E1") => "行の勘定: すべての入力行が採用または除外として説明される",
        ("data", "E2") => "記述の正確さ: レポート値が実行結果で観測した値と一致する",
        ("data", "E3") => "再現性: パイプラインを再実行しても同じ結果になる",
        ("data", "E4") => "スキーマの正確さ: 生成データが必須スキーマと一致する",
        ("ingest", "N1") => "パイプライン実行: 有界な取り込みコマンドが正常に完了する",
        ("ingest", "N2") => "入力元の正確さ: すべての出力値が選択した入力レコードに結び付く",
        ("ingest", "N3") => "候補の勘定: すべての検出候補が採用または明示的な除外になる",
        ("ingest", "N4") => "形式の正確さ: 出力フィールドと型が必須スキーマと一致する",
        ("ingest", "N5") => "再現性: 取り込みを再実行しても同じ結果になる",
        ("nextjs", "build") => "ビルド: アプリケーションが正常にコンパイルできる",
        ("nextjs", "browser_route") => "ブラウザ表示: 必須ページが実ブラウザで表示される",
        ("nextjs", "interaction_state") => "操作: 必須のユーザー操作で確認可能な状態変化が起きる",
        ("nextjs", "T1_testimony") => {
            "記述の正確さ: ユーザー向け説明がブラウザで観測した動作と一致する"
        }
        _ => {
            return format!("{check} — 必須証跡の内容は契約 {contract_ref} で定義されています");
        }
    };
    format!("{check} — {description}")
}

fn full_meaning(identity: &ConfirmationIdentity) -> &str {
    match (identity.profile.as_str(), identity.intent.as_str()) {
        ("python-cli", "create") => {
            "上記4項目がすべて合格し、README の出力説明も実際のコマンド出力と一致すること"
        }
        ("data", "create") => {
            "パイプラインが実行され、観測した成果物に基づいて上記4項目がすべて合格すること"
        }
        ("ingest", "create") => {
            "入力元に結び付いた値と完全な候補勘定を含め、上記5項目がすべて合格すること"
        }
        ("nextjs", "create") => {
            "ビルド、実ブラウザ表示、操作、状態変化、記述の各チェックがすべて合格すること"
        }
        (_, "fix") => "修正前の問題が再現し、修正後のチェックが合格し、回帰が残っていないこと",
        (_, "investigate") => "失敗を再現する検証を実行し、調査報告の記述が観測証跡と一致すること",
        _ => identity.full_meaning.as_str(),
    }
}

pub fn render_gate_three(
    identity: &ConfirmationIdentity,
    terminal: &TerminalPresentation,
) -> anyhow::Result<String> {
    validate_terminal_identity(identity, terminal)?;
    if !terminal.full {
        bail!("non-full terminals must use Gate 4");
    }
    Ok(format!(
        "# Gate 3 — Acceptance\n\nConfirmed Full meaning: {}\n\n{}\n\n## Continued modification\n\n- human_directive: available — enter `/directive <instruction>`; the full check set is frozen before confirmed dispatch",
        identity.full_meaning, terminal.acceptance_sheet
    ))
}

pub fn render_gate_four(
    identity: &ConfirmationIdentity,
    terminal: &TerminalPresentation,
    actions: &[(NextAction, bool, &str)],
) -> anyhow::Result<String> {
    validate_terminal_identity(identity, terminal)?;
    if terminal.full {
        bail!("full terminals must use Gate 3");
    }
    let section5 = terminal
        .section5
        .as_deref()
        .context("Gate 4 requires section 5")?;
    let action_lines = actions
        .iter()
        .map(|(action, enabled, reason)| {
            format!(
                "- {}: {} — {}",
                action.as_str(),
                if *enabled { "available" } else { "unavailable" },
                reason
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let summary = gate_four_summary(identity, terminal, actions).join("\n");
    Ok(format!(
        "# Gate 4 — Failure and next action\n\n{}\n\nConfirmed Full meaning: {}\n\n{}\n\n## Section 5\n\n{}\n\n## Typed next actions\n\n{}",
        summary, identity.full_meaning, terminal.acceptance_sheet, section5, action_lines
    ))
}

fn gate_four_summary(
    identity: &ConfirmationIdentity,
    terminal: &TerminalPresentation,
    actions: &[(NextAction, bool, &str)],
) -> [String; 3] {
    let passed = [
        (
            "コマンド",
            sheet_value(&terminal.acceptance_sheet, "Command succeeded") == Some("true"),
        ),
        (
            "実行時受入",
            sheet_value(&terminal.acceptance_sheet, "Runtime acceptance") == Some("pass"),
        ),
        (
            "最終受入",
            sheet_value(&terminal.acceptance_sheet, "Final acceptance")
                .is_some_and(|value| matches!(value, "full" | "full_success" | "completed")),
        ),
        (
            "リリースゲート",
            sheet_value(&terminal.acceptance_sheet, "Release gate") == Some("pass"),
        ),
    ]
    .into_iter()
    .filter_map(|(label, passed)| passed.then_some(label))
    .collect::<Vec<_>>();
    let passed = if passed.is_empty() {
        "なし（失敗内容は Section 5）".to_string()
    } else {
        passed.join("、")
    };

    let assurance = sheet_value(&terminal.acceptance_sheet, "Assurance").unwrap_or("unknown");
    let assurance_level = assurance
        .split_once(" (")
        .map(|(level, _)| level)
        .unwrap_or(assurance);
    let not_run = if assurance_level == "static" {
        if matches!(identity.profile.as_str(), "python-cli" | "cli") {
            "CLI 動作プローブ C1–C4（未実行のため保証は static）".to_string()
        } else {
            "動作検証（未実行のため保証は static）".to_string()
        }
    } else {
        let not_run = [
            ("実行時受入", "Runtime acceptance"),
            ("最終受入", "Final acceptance"),
            ("リリースゲート", "Release gate"),
        ]
        .into_iter()
        .filter_map(|(label, field)| {
            sheet_value(&terminal.acceptance_sheet, field)
                .is_some_and(|value| matches!(value, "not_checked" | "not_recorded"))
                .then_some(label)
        })
        .collect::<Vec<_>>();
        if not_run.is_empty() {
            "なし（失敗内容は Section 5）".to_string()
        } else {
            not_run.join("、")
        }
    };
    let next_action = recommended_gate_four_action(actions, assurance_level);

    [
        format!("- 通過: {passed}"),
        format!("- 未実行: {not_run}"),
        format!(
            "- 次の一手: `{}` — {}",
            next_action.as_str(),
            next_action_guidance(next_action)
        ),
    ]
}

fn sheet_value<'a>(sheet: &'a str, label: &str) -> Option<&'a str> {
    let prefix = format!("- {label}: ");
    sheet
        .lines()
        .find_map(|line| line.trim().strip_prefix(&prefix))
}

fn recommended_gate_four_action(
    actions: &[(NextAction, bool, &str)],
    assurance_level: &str,
) -> NextAction {
    if assurance_level == "static"
        && actions
            .iter()
            .any(|(action, enabled, _)| *action == NextAction::ElevatedModel && *enabled)
    {
        return NextAction::ElevatedModel;
    }
    actions
        .iter()
        .find_map(|(action, enabled, _)| enabled.then_some(*action))
        .unwrap_or(NextAction::Close)
}

fn next_action_guidance(action: NextAction) -> &'static str {
    match action {
        NextAction::Retry => "同じ構成で再実行し、Gate 1 で再確認",
        NextAction::RecoveryCircle => "失敗証拠を引き継ぐ回復フローを Gate 1 で再確認",
        NextAction::ElevatedModel => "上位モデルで再実行し、Gate 1 で再確認",
        NextAction::PackChange => "互換 pack を選び、Gate 1 で再確認",
        NextAction::HumanDirective => "追加指示を保存し、継続前に再確認",
        NextAction::Close => "証拠を保存したまま終了",
    }
}

fn render_pack(
    identity: &ConfirmationIdentity,
    pack_locator: &PackLocator,
    lines: &mut Vec<String>,
) -> anyhow::Result<()> {
    match &identity.pack {
        PackSelection::None => {
            lines.push("- 追加の検証パック: 選択なし".to_string());
            lines.push("- 検証パックの完全一致 ID: なし".to_string());
        }
        PackSelection::Pinned {
            id,
            version,
            hash,
            point,
            source,
        } => {
            lines.push(format!("- 追加の検証パック: {id}@{version}"));
            lines.push(format!("- 検証パックの完全一致 ID: {hash}"));
            lines.push(format!("- 検証パックの供給元: {}", source.japanese_label()));
            lines.push(format!("- 検証箇所: {point}"));
            let observed = pack_catalog::observed_pin(pack_locator, &identity.pack)?;
            if observed.as_deref() != Some(hash) {
                lines.push(format!(
                    "- 警告: 検証パックが変更されています (確認済み {hash}, 現在 {})",
                    observed.as_deref().unwrap_or("missing")
                ));
            } else {
                lines.push("- 検証パックの状態: バイト単位で一致".to_string());
            }
        }
    }
    Ok(())
}

fn validate_identity_fields(identity: &ConfirmationIdentity) -> anyhow::Result<()> {
    let required = [
        ("request", identity.request.as_str()),
        ("workspace", identity.workspace.as_str()),
        ("profile", identity.profile.as_str()),
        ("intent", identity.intent.as_str()),
        ("task_family", identity.task_family.as_str()),
        ("contract_ref", identity.contract_ref.as_str()),
        ("band_rate", identity.band_rate.as_str()),
        ("band_arm", identity.band_arm.as_str()),
        ("band_measurement", identity.band_measurement.as_str()),
        ("band_source", identity.band_source.as_str()),
        ("full_meaning", identity.full_meaning.as_str()),
        ("planner_provider", identity.pins.planner_provider.as_str()),
        ("planner_model", identity.pins.planner_model.as_str()),
        (
            "executor_provider",
            identity.pins.executor_provider.as_str(),
        ),
        ("executor_model", identity.pins.executor_model.as_str()),
        ("preset", identity.pins.preset.as_str()),
    ];
    let missing = required
        .into_iter()
        .filter_map(|(name, value)| value.trim().is_empty().then_some(name))
        .collect::<Vec<_>>();
    let draft_complete = identity.draft_manifest.as_ref().is_none_or(|manifest| {
        !manifest.source.trim().is_empty()
            && !manifest.path.trim().is_empty()
            && !manifest.hash.trim().is_empty()
            && manifest.assurance_ceiling == "static"
    });
    if !missing.is_empty()
        || identity.route_bases.is_empty()
        || identity.contract_checks.is_empty()
        || (identity.band_denominator == 0 && identity.draft_manifest.is_none())
        || !draft_complete
    {
        bail!(
            "Gate 1 refuses conversational omission: missing={}, bases={}, checks={}, denominator={}",
            missing.join(","),
            identity.route_bases.len(),
            identity.contract_checks.len(),
            identity.band_denominator
        );
    }
    Ok(())
}

fn validate_complete_identity(identity: &ConfirmationIdentity) -> anyhow::Result<()> {
    validate_identity_fields(identity)?;
    pack_catalog::validate_selection(&identity.profile, &identity.intent, &identity.pack)
}

fn validate_complete_identity_with_locator(
    identity: &ConfirmationIdentity,
    locator: &PackLocator,
) -> anyhow::Result<()> {
    validate_identity_fields(identity)?;
    pack_catalog::validate_selection_with_locator(
        &identity.profile,
        &identity.intent,
        &identity.pack,
        locator,
    )
}

fn validate_terminal_identity(
    identity: &ConfirmationIdentity,
    terminal: &TerminalPresentation,
) -> anyhow::Result<()> {
    validate_complete_identity(identity)?;
    if identity.card_hash()? != terminal.card_hash {
        bail!("terminal sheet identity differs from the confirmed Gate 1 card");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::planner::adjudication::contract::IntentId;
    use crate::planner::pack::catalog::{ADMITTED_PACKS, PackLocator, PackSource};
    use crate::planner::profile::ProfileId;
    use crate::tui::boundary_shell::band_catalog::value_for;
    use crate::tui::boundary_shell::family_catalog::TaskFamilyId;
    use crate::tui::boundary_shell::route::{RouteBasis, RouteCandidate};

    use super::*;
    use crate::tui::boundary_shell::confirmation::ExecutionPins;

    fn identity(pack: PackSelection) -> ConfirmationIdentity {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let route = RouteCandidate {
            profile: ProfileId::PythonCli,
            intent: IntentId::Create,
            family: TaskFamilyId::Stats,
            bases: vec![RouteBasis {
                rule: "fixture",
                observation: "stats".to_string(),
            }],
            contract_ref: "docs/cli-profile-contract.md",
        };
        ConfirmationIdentity::new(
            "create a CLI".to_string(),
            root,
            &route,
            value_for("python-cli", IntentId::Create, TaskFamilyId::Stats).unwrap(),
            ExecutionPins {
                planner_provider: "ollama".to_string(),
                planner_model: "planner".to_string(),
                executor_provider: "ollama".to_string(),
                executor_model: "executor".to_string(),
                preset: "profile".to_string(),
            },
            pack,
        )
        .unwrap()
    }

    #[test]
    fn no_pack_card_explains_python_cli_checks_value_and_confirmation() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let identity = identity(PackSelection::None);
        let rendered = render_gate_one(&identity, &PackLocator::new(root)).unwrap();
        for required in [
            "全必須チェックに合格した実行: 3件中0件 (0%)",
            "C1 — 実行動作: 通常のコマンドは成功",
            "C2 — ヘルプの正確さ: --help と実際",
            "C3 — 出力の正確さ: README の例",
            "C4 — 再現性: 同じケースを再実行",
            "追加の検証パック: 選択なし",
            "検証パックの完全一致 ID: なし",
            "確認 ID (内容が1つでも変わると ID も変わります): sha256:",
            "CLI では /confirm sha256:",
        ] {
            assert!(rendered.contains(required), "{rendered}");
        }
        for internal_label in ["Card hash:", "Route:", "Checks: C1", "Value tag:"] {
            assert!(!rendered.contains(internal_label), "{rendered}");
        }
        assert!(!rendered.contains("検証パックの供給元:"), "{rendered}");
    }

    #[test]
    fn missing_value_or_pack_pin_is_a_fixture_failure_not_a_shorter_card() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut missing_value = identity(PackSelection::None);
        missing_value.band_denominator = 0;
        let locator = PackLocator::new(root);
        assert!(render_gate_one(&missing_value, &locator).is_err());

        let mut missing_pin = identity(PackSelection::None);
        missing_pin.pack = PackSelection::Pinned {
            id: "cli-assist".to_string(),
            version: "1.1.0".to_string(),
            hash: String::new(),
            point: "cli-validation".to_string(),
            source: PackSource::Admitted,
        };
        assert!(render_gate_one(&missing_pin, &locator).is_err());
    }

    #[test]
    fn stale_pack_pin_is_warned_without_silently_rebinding_the_card() {
        let repository_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let identity = identity(PackSelection::Pinned {
            id: "cli-assist".to_string(),
            version: "1.1.0".to_string(),
            hash: ADMITTED_PACKS[1].hash.to_string(),
            point: "cli-validation".to_string(),
            source: PackSource::Admitted,
        });
        let temp = tempfile::tempdir().unwrap();
        let destination = temp.path().join("packs/cli-assist/1.1.0");
        std::fs::create_dir_all(&destination).unwrap();
        let mut bytes =
            std::fs::read(repository_root.join("packs/cli-assist/1.1.0/assist.yaml")).unwrap();
        bytes.extend_from_slice(b"\n# stale bytes\n");
        std::fs::write(destination.join("assist.yaml"), bytes).unwrap();

        let rendered = render_gate_one(&identity, &PackLocator::new(temp.path())).unwrap();
        assert!(
            rendered.contains("警告: 検証パックが変更されています"),
            "{rendered}"
        );
        assert!(rendered.contains(ADMITTED_PACKS[1].hash), "{rendered}");
        assert!(rendered.contains("検証パックの供給元: 承認済み"));
    }

    #[test]
    fn full_sheet_is_included_verbatim_and_cannot_cross_card_identity() {
        let identity = identity(PackSelection::None);
        let sheet = "# Acceptance sheet\n\nverdict: full".to_string();
        let terminal =
            TerminalPresentation::new(identity.card_hash().unwrap(), sheet.clone(), true, None)
                .unwrap();
        let rendered = render_gate_three(&identity, &terminal).unwrap();
        assert!(rendered.contains(&sheet));
        assert!(rendered.contains("human_directive: available"));

        let other =
            TerminalPresentation::new("sha256:other".to_string(), sheet, true, None).unwrap();
        assert!(render_gate_three(&identity, &other).is_err());
    }

    #[test]
    fn static_gate_four_leads_with_three_lines_explaining_the_result() {
        let identity = identity(PackSelection::None);
        let sheet = "# Acceptance sheet\n\n\
- Command succeeded: true\n\
- Status: completed\n\
- Assurance: static (cli_probe_not_run)\n\
- Runtime acceptance: pass\n\
- Final acceptance: full_success\n\
- Release gate: pass"
            .to_string();
        let terminal = TerminalPresentation::new(
            identity.card_hash().unwrap(),
            sheet.clone(),
            false,
            Some("cli_probe_not_run".to_string()),
        )
        .unwrap();
        let actions = [
            (NextAction::Retry, true, "human confirmation required"),
            (
                NextAction::ElevatedModel,
                true,
                "returns to Gate 1 with a new model pin",
            ),
            (NextAction::Close, true, "records no further action"),
        ];

        let rendered = render_gate_four(&identity, &terminal, &actions).unwrap();
        let summary = rendered
            .split_once("\n\nConfirmed Full meaning:")
            .unwrap()
            .0;
        let summary_lines = summary
            .lines()
            .skip(1)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>();

        assert_eq!(
            summary_lines,
            vec![
                "- 通過: コマンド、実行時受入、最終受入、リリースゲート",
                "- 未実行: CLI 動作プローブ C1–C4（未実行のため保証は static）",
                "- 次の一手: `elevated_model` — 上位モデルで再実行し、Gate 1 で再確認",
            ]
        );
        assert!(rendered.contains(&sheet));
        assert!(rendered.contains("## Section 5\n\ncli_probe_not_run"));
        assert!(rendered.contains("## Typed next actions"));
    }

    #[test]
    fn failed_gate_four_summary_does_not_claim_unearned_passes() {
        let identity = identity(PackSelection::None);
        let terminal = TerminalPresentation::new(
            identity.card_hash().unwrap(),
            "# Acceptance sheet\n\n\
- Command succeeded: false\n\
- Assurance: failed (cli_assurance_failed)\n\
- Runtime acceptance: failed\n\
- Final acceptance: failed\n\
- Release gate: failed"
                .to_string(),
            false,
            Some("C1 polarity violation".to_string()),
        )
        .unwrap();
        let actions = [(NextAction::Retry, true, "human confirmation required")];

        let rendered = render_gate_four(&identity, &terminal, &actions).unwrap();

        assert!(rendered.contains("- 通過: なし（失敗内容は Section 5）"));
        assert!(rendered.contains("- 未実行: なし（失敗内容は Section 5）"));
        assert!(rendered.contains("- 次の一手: `retry` — 同じ構成で再実行し、Gate 1 で再確認"));
    }
}
