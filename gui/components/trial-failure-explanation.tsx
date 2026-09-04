"use client";

import { useState } from "react";

import type {
  BoundedText,
  BoundedTextList,
  FailureExplanation,
  RecoveryRunProposal,
} from "../lib/types";

type TrialFailureExplanationProps = {
  busy: boolean;
  evidenceLoading: boolean;
  explanation: FailureExplanation;
  onConfirmRecoveryRun: () => Promise<void>;
  onApplyToContinuation: (value: string) => void;
  onOpenArtifact: (path: string) => Promise<void>;
  onOpenEvents: () => Promise<void>;
  onOpenRecoveryDocument: (path: string) => Promise<void>;
  onProposeRecoveryRun: () => Promise<void>;
  recoveryRun: RecoveryRunProposal | null;
  recoveryRunAcknowledged: boolean;
  setRecoveryRunAcknowledged: (value: boolean) => void;
};

export function TrialFailureExplanation({
  busy,
  evidenceLoading,
  explanation,
  onConfirmRecoveryRun,
  onApplyToContinuation,
  onOpenArtifact,
  onOpenEvents,
  onOpenRecoveryDocument,
  onProposeRecoveryRun,
  recoveryRun,
  recoveryRunAcknowledged,
  setRecoveryRunAcknowledged,
}: TrialFailureExplanationProps) {
  const [announcement, setAnnouncement] = useState("");
  const { evidence, location, primary, progress, recovery } = explanation;
  const treatmentBlocksPlan = recovery.resolution.treatment_promotion_status === "rejected" ||
    recovery.resolution.treatment_promotion_status === "pending";

  async function copy(label: string, value: BoundedText) {
    if (value.truncated) {
      setAnnouncement(`${label}は上限で省略されているためコピーできません。events.jsonl で完全な記録を確認してください。`);
      return;
    }
    try {
      if (navigator.clipboard?.writeText === undefined) {
        throw new Error("Clipboard API is unavailable");
      }
      await navigator.clipboard.writeText(value.value);
      setAnnouncement(`${label}をクリップボードにコピーしました。自動実行はしていません。`);
    } catch {
      setAnnouncement(`${label}をコピーできませんでした。表示内容を選択して手動でコピーしてください。`);
    }
  }

  function applyToContinuation() {
    if (!recovery.continuation_eligible) return;
    onApplyToContinuation(continuationDraft(explanation));
    setAnnouncement("推奨内容を追加の依頼欄へ反映しました。まだ保存、確認、実行はしていません。");
    window.requestAnimationFrame(() => document.getElementById("directive-input")?.focus());
  }

  return (
    <section
      aria-labelledby="failure-explanation-heading"
      className="trial-failure-explanation"
        data-category={explanation.category}
        data-projection-status={explanation.projection_status}
        data-treatment-promotion-status={recovery.resolution.treatment_promotion_status}
      data-testid="terminal-failure-explanation"
    >
      <header>
        <span>FAILED / 構造化された結果</span>
        <h3 id="failure-explanation-heading">失敗した場所、原因、次の操作</h3>
        <p>{categoryExplanation(explanation.category)}</p>
      </header>

      <ol className="failure-explanation-sections">
        <li>
          <h4>1. 失敗した場所</h4>
          <dl className="failure-location-grid" data-testid="failure-location">
            <div><dt>実行区間</dt><dd>{location.interval_index}</dd></div>
            <div>
              <dt>Plan 実行</dt>
              <dd>{location.plan_execution_id?.value ?? "記録なし"}</dd>
            </div>
            <div>
              <dt>フェーズ</dt>
              <dd>{phaseLabel(location.phase)}</dd>
            </div>
            <div>
              <dt>タスク</dt>
              <dd>{stepLabel(location.step)}</dd>
            </div>
          </dl>
        </li>

        <li>
          <h4>2. 原因</h4>
          <p className="failure-primary-summary" data-testid="failure-primary-summary">
            {primary.summary.value}
            {primary.summary.truncated && <TruncatedLabel />}
          </p>
        </li>

        <li>
          <h4>3. 根拠</h4>
          <div className="failure-evidence-grid" data-testid="failure-evidence">
            <EvidenceStatus label="検証" value={evidence.verification_status} />
            <EvidenceStatus label="受け入れ" value={evidence.acceptance_status} />
            <EvidenceStatus label="リリースゲート" value={evidence.release_gate_status} />
            {evidence.command !== null && (
              <div className="failure-command-evidence">
                <strong>失敗したコマンド</strong>
                <code>{evidence.command.value}</code>
                {evidence.command.truncated && <TruncatedLabel />}
                {evidence.exit_code !== null && <small>exit code: {evidence.exit_code}</small>}
              </div>
            )}
            <OutputEvidence label="stdout" value={evidence.stdout} />
            <OutputEvidence label="stderr" value={evidence.stderr} />
            {evidence.observations.length > 0 && (
              <div>
                <strong>検証・プローブ所見</strong>
                <ul>
                  {evidence.observations.map((observation, index) => (
                    <li key={`${observation.kind.value}-${index}`}>
                      <code>{observation.kind.value}</code>
                      {observation.status !== null && <span> — {observation.status.value}</span>}
                      {observation.detail !== null && <small>{observation.detail.value}</small>}
                      {observation.path !== null && <small>証跡: {observation.path.value}</small>}
                    </li>
                  ))}
                </ul>
                {evidence.observations_truncated && (
                  <small>{evidence.observation_count} 件中、上限内の所見だけを表示しています。</small>
                )}
              </div>
            )}
            <EvidenceList label="欠落パス" list={evidence.missing_paths} />
            <EvidenceList label="変更を観測したパス" list={evidence.changed_paths} />
            <div className="failure-evidence-actions">
              <button
                className="inline-action"
                disabled={evidenceLoading}
                onClick={() => void onOpenArtifact("summary.md")}
                type="button"
              >
                summary.md を開く
              </button>
              <button
                className="inline-action"
                disabled={evidenceLoading}
                onClick={() => void onOpenEvents()}
                type="button"
              >
                events.jsonl を開く
              </button>
            </div>
          </div>
        </li>

        <li>
          <h4>4. 完了範囲と部分成果物</h4>
          <dl className="failure-progress-grid" data-testid="failure-progress">
            <div>
              <dt>完了フェーズ</dt>
              <dd>{progress.completed_phases} / {progress.total_phases || "未記録"}</dd>
            </div>
            <div>
              <dt>完了タスク</dt>
              <dd>{progress.completed_tasks} / {progress.total_tasks || "未記録"}</dd>
            </div>
            <div><dt>修復試行</dt><dd>{progress.repair_attempts} 回</dd></div>
            <div><dt>作業ディレクトリ</dt><dd>{workspaceLabel(progress.workspace_state)}</dd></div>
          </dl>
          <p className="failure-artifact-state">{artifactLabel(progress.partial_artifact_state)}</p>
          <p className="source-note">
            作業ディレクトリの絶対パスとコピーボタンは、この結果カードの上にある
            「セッションの作業ディレクトリ」で確認できます。
          </p>
        </li>

        <li>
          <h4>5. 推奨アクション</h4>
          <p>{recoveryExplanation(explanation)}</p>
          {recovery.resolution.treatment_promotion_status !== "not_attempted" && (
            <RecoveryResolutionNotice explanation={explanation} />
          )}
          <EvidenceList label="実行可能な修復方針" list={recovery.viable_actions} translate />
          <RecoveryDocumentAction
            disabled={evidenceLoading}
            label="repair prompt を開く"
            onOpen={onOpenRecoveryDocument}
            path={recovery.repair_prompt_path}
            testId="open-repair-prompt"
          />
          <RecoveryDocumentAction
            disabled={evidenceLoading}
            label="Recovery Plan を開く"
            onOpen={onOpenRecoveryDocument}
            path={recovery.recovery_plan_path}
            testId="open-recovery-plan"
          />
          <button
            className="secondary-action"
            data-testid="propose-recovery-run"
            disabled={
              busy || treatmentBlocksPlan || recovery.recovery_plan_path === null ||
              recovery.recovery_plan_path.truncated
            }
            onClick={() => void onProposeRecoveryRun()}
            type="button"
          >
            Recovery Plan を実行する
          </button>
          {recoveryRun !== null && (
            <section
              aria-labelledby="recovery-run-confirmation-heading"
              className="recovery-run-confirmation"
              data-testid="recovery-run-confirmation"
            >
              <h5 id="recovery-run-confirmation-heading">Recovery Run の確認</h5>
              <dl>
                <div><dt>解決済みプラン</dt><dd><code>{recoveryRun.source_plan_path}</code></dd></div>
                <div><dt>凍結プラン</dt><dd><code>{recoveryRun.frozen_plan_path}</code></dd></div>
                <div><dt>exact-byte hash</dt><dd><code>{recoveryRun.plan_hash}</code></dd></div>
                <div><dt>実行フェーズ</dt><dd>{recoveryRun.execution_phases.join(" → ")}</dd></div>
                <div><dt>許可ポリシー</dt><dd><code>{recoveryRun.permission_policy}</code></dd></div>
                <div>
                  <dt>自動実行予算</dt>
                  <dd>{recoveryRun.automatic_run_budget} 回（明示実行後の Recovery 上限）</dd>
                </div>
              </dl>
              <label className="recovery-run-acknowledgement">
                <input
                  checked={recoveryRunAcknowledged}
                  data-testid="recovery-run-acknowledgement"
                  onChange={(event) => setRecoveryRunAcknowledged(event.target.checked)}
                  type="checkbox"
                />
                この path、hash、フェーズ、許可、自動実行予算で凍結プランを実行します
              </label>
              <button
                className="primary-action"
                data-testid="confirm-recovery-run"
                disabled={busy || !recoveryRunAcknowledged}
                onClick={() => void onConfirmRecoveryRun()}
                type="button"
              >
                確認して Recovery Plan を実行
              </button>
            </section>
          )}
          <RecoveryCommand
            label="推奨コマンド"
            onCopy={copy}
            testId="copy-recovery-command"
            value={recovery.suggested_command}
          />
          <RecoveryCommand
            label="推奨 YAML コマンド"
            onCopy={copy}
            testId="copy-recovery-yaml-command"
            value={recovery.suggested_yaml_command}
          />
          <button
            className="secondary-action"
            data-testid="apply-recovery-to-continuation"
            disabled={!recovery.continuation_eligible}
            onClick={applyToContinuation}
            type="button"
          >
            推奨内容を追加の依頼欄へ反映
          </button>
          {!recovery.continuation_eligible && (
            <p className="source-note">
              継続不可: {continuationReason(recovery.continuation_reason.value)}
            </p>
          )}
          <p className="recovery-no-autorun">
            この画面はリカバリーを自動実行しません。Recovery Plan と追加の依頼は別々の
            確認操作で、どちらも専用の確認を完了するまで実行されません。
          </p>
        </li>
      </ol>

      <details className="failure-technical-details" data-testid="failure-technical-details">
        <summary>技術詳細（machine code と根拠イベント）</summary>
        <dl>
          <div><dt>category</dt><dd><code>{explanation.category}</code></dd></div>
          <div><dt>projection</dt><dd><code>{explanation.projection_status}</code></dd></div>
          <div>
            <dt>failure kind</dt>
            <dd><code>{primary.failure_kind?.value ?? "not_recorded"}</code></dd>
          </div>
          <div>
            <dt>reason code</dt>
            <dd><code>{primary.reason_code?.value ?? "not_recorded"}</code></dd>
          </div>
          <div>
            <dt>source codes</dt>
            <dd>{explanation.technical.machine_codes.items.map((item) => (
              <code key={item.value}>{item.value}</code>
            ))}</dd>
          </div>
        </dl>
      </details>

      <p
        aria-atomic="true"
        aria-live="polite"
        className="trial-copy-announcement"
        data-testid="failure-action-announcement"
        role="status"
      >
        {announcement}
      </p>
    </section>
  );
}

function RecoveryResolutionNotice({ explanation }: { explanation: FailureExplanation }) {
  const resolution = explanation.recovery.resolution;
  const rejected = resolution.treatment_promotion_status === "rejected";
  const pending = resolution.treatment_promotion_status === "pending";
  const reason = resolution.treatment_rejection_reason?.value;
  return (
    <div className="recovery-resolution-notice" data-testid="recovery-resolution-notice">
      <strong>自動 Recovery の採用結果</strong>
      <p>
        {rejected
          ? "直前の treatment は採用されず、元の control 成果物を保持しています。"
          : pending
            ? "treatment の採用判定が完了していないため、元の control 成果物を保持しています。"
            : "treatment を採用し、現在の成果物として使用しています。"}
      </p>
      {reason !== undefined && <p>拒否理由: {promotionReason(reason)}</p>}
      {(rejected || pending) && (
        <p>同じ Recovery Plan は再実行できません。追加の依頼から新しい計画を作成してください。</p>
      )}
    </div>
  );
}

function promotionReason(reason: string): string {
  if (reason === "no_registered_post_recovery_observation") {
    return "成功判定用の登録済み検証がないため、変更を採用できませんでした。";
  }
  return reason;
}

function EvidenceStatus({ label, value }: { label: string; value: BoundedText | null }) {
  if (value === null) return null;
  return <p><strong>{label}:</strong> <code>{value.value}</code></p>;
}

function OutputEvidence({ label, value }: { label: string; value: BoundedText | null }) {
  if (value === null) return null;
  return (
    <div className="failure-output-evidence">
      <strong>{label}</strong>
      <pre>{value.value}</pre>
      {value.truncated && <TruncatedLabel />}
    </div>
  );
}

function EvidenceList({
  label,
  list,
  translate = false,
}: {
  label: string;
  list: BoundedTextList;
  translate?: boolean;
}) {
  if (list.items.length === 0) return null;
  return (
    <div>
      <strong>{label}</strong>
      <ul>{list.items.map((item) => (
        <li key={item.value}>
          <code>{item.value}</code>
          {translate && <small>{viableActionLabel(item.value)}</small>}
          {item.truncated && <TruncatedLabel />}
        </li>
      ))}</ul>
      {list.truncated && <small>{list.total_count} 件中、上限内の項目だけを表示しています。</small>}
    </div>
  );
}

function RecoveryDocumentAction({
  disabled,
  label,
  onOpen,
  path,
  testId,
}: {
  disabled: boolean;
  label: string;
  onOpen: (path: string) => Promise<void>;
  path: BoundedText | null;
  testId: string;
}) {
  if (path === null) return null;
  return (
    <div className="recovery-document-action">
      <code>{path.value}</code>
      <button
        className="inline-action"
        data-testid={testId}
        disabled={disabled || path.truncated}
        onClick={() => void onOpen(path.value)}
        type="button"
      >
        {label}
      </button>
      {path.truncated && <TruncatedLabel />}
    </div>
  );
}

function RecoveryCommand({
  label,
  onCopy,
  testId,
  value,
}: {
  label: string;
  onCopy: (label: string, value: BoundedText) => Promise<void>;
  testId: string;
  value: BoundedText | null;
}) {
  if (value === null) return null;
  return (
    <div className="recovery-command">
      <strong>{label}</strong>
      <code>{value.value}</code>
      <button
        aria-label={`${label}をコピー（自動実行しません）`}
        className="inline-action"
        data-testid={testId}
        disabled={value.truncated}
        onClick={() => void onCopy(label, value)}
        type="button"
      >
        コピー
      </button>
      {value.truncated && <TruncatedLabel />}
    </div>
  );
}

function TruncatedLabel() {
  return <small className="bounded-truncation">上限により省略</small>;
}

function phaseLabel(phase: FailureExplanation["location"]["phase"]): string {
  if (phase === null) return "記録なし";
  return phase.index === null
    ? phase.id.value
    : `${phase.id.value}（${phase.index} / ${phase.total ?? "?"}）`;
}

function stepLabel(step: FailureExplanation["location"]["step"]): string {
  if (step === null) return "記録なし";
  return `${step.id.value}（${step.kind.value}、${step.index} / ${step.total}）`;
}

function categoryExplanation(category: FailureExplanation["category"]): string {
  switch (category) {
    case "planning": return "実行計画の作成またはフェーズ計画で停止しました。";
    case "execution": return "計画済みタスクの実行中に停止しました。";
    case "verification": return "検証または bounded repair 後の再検証に合格できませんでした。";
    case "release_gate": return "成果物はありますが、リリースゲートの必須証跡に合格できませんでした。";
    case "infrastructure": return "spawn、preflight、または実行基盤の準備で停止しました。";
    case "interrupted": return "実行は完了判定の前に中断されました。";
    case "unknown": return "旧形式または不完全な記録のため、失敗分類を推測していません。";
  }
}

function workspaceLabel(state: FailureExplanation["progress"]["workspace_state"]): string {
  switch (state) {
    case "available": return "利用可能";
    case "missing": return "削除済み";
    case "unknown": return "確認不能";
  }
}

function artifactLabel(state: FailureExplanation["progress"]["partial_artifact_state"]): string {
  switch (state) {
    case "observed": return "変更パスが記録され、作業ディレクトリも利用可能です。部分成果物を確認できます。";
    case "workspace_available":
      return "作業ディレクトリは利用可能です。変更件数だけで成果物不在とは判断せず、内容を確認してください。";
    case "workspace_missing": return "作業ディレクトリは削除済みです。記録上の変更パスと実在する成果物を区別してください。";
    case "unknown": return "作業ディレクトリ状態を確認できないため、成果物の有無を推測していません。";
  }
}

function recoveryExplanation(explanation: FailureExplanation): string {
  const recovery = explanation.recovery;
  if (recovery.repair_prompt_path !== null || recovery.recovery_plan_path !== null) {
    return "保存済みの repair prompt と Recovery Plan を確認し、必要な内容だけを既存の確認付き追加依頼へ反映してください。";
  }
  if (recovery.viable_actions.items.length > 0) {
    return "記録された実行可能な修復方針を確認し、既存の確認付き追加依頼から適用してください。";
  }
  if (explanation.category === "release_gate") {
    return "リリースゲートの根拠を修復し、同じ必須チェックで再検証してください。";
  }
  return "summary.md と events.jsonl を確認してから、継続または再実行を判断してください。";
}

function viableActionLabel(action: string): string {
  switch (action) {
    case "edit_source_artifact": return "生成済みソースを修復する";
    case "rerun_verification": return "同じ検証を再実行する";
    case "install_dependency": return "確認済みの依存関係を準備する";
    case "repair_completion_contract": return "completion contract の境界違反を修復する";
    default: return "記録された修復方針";
  }
}

function continuationReason(reason: string): string {
  switch (reason) {
    case "workspace_missing": return "作業ディレクトリが削除済みです。";
    case "workspace_state_unknown": return "作業ディレクトリ状態を確認できません。";
    case "recovery_artifacts_invalid": return "保存済みリカバリー成果物の検証が不合格です。";
    case "interrupted_run_requires_review": return "中断後の状態確認が必要です。";
    case "infrastructure_recovery_not_continuable": return "実行基盤を先に修復する必要があります。";
    case "no_structured_recovery": return "構造化されたリカバリー情報がありません。";
    default: return "この失敗記録から安全な継続可否を確認できません。";
  }
}

function continuationDraft(explanation: FailureExplanation): string {
  const { primary, recovery } = explanation;
  const lines = [
    "記録済みの失敗原因と Recovery Plan に基づいて修復してください。",
    `失敗カテゴリ: ${explanation.category}`,
    `原因: ${primary.summary.value}`,
  ];
  if (recovery.viable_actions.items.length > 0) {
    lines.push(`実行可能な修復方針: ${recovery.viable_actions.items.map((item) => item.value).join(", ")}`);
  }
  if (recovery.repair_prompt_path !== null && !recovery.repair_prompt_path.truncated) {
    lines.push(`repair prompt: ${recovery.repair_prompt_path.value}`);
  }
  if (recovery.recovery_plan_path !== null && !recovery.recovery_plan_path.truncated) {
    lines.push(`Recovery Plan: ${recovery.recovery_plan_path.value}`);
  }
  if (recovery.suggested_command !== null && !recovery.suggested_command.truncated) {
    lines.push(`記録された推奨コマンド（自動実行しない）: ${recovery.suggested_command.value}`);
  }
  if (recovery.suggested_yaml_command !== null && !recovery.suggested_yaml_command.truncated) {
    lines.push(`記録された推奨 YAML コマンド（自動実行しない）: ${recovery.suggested_yaml_command.value}`);
  }
  return lines.join("\n");
}
