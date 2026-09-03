"use client";

import Link from "next/link";
import { useEffect, useRef } from "react";

import type { TrialRunState } from "../hooks/use-trial-run";
import { trialRoutePath } from "../lib/base-path";
import { byteLabel, trialGateLabel } from "../lib/format";
import type { PolledSession } from "../lib/types";
import { DocumentViewer } from "./document-viewer";
import { TrialFailureExplanation } from "./trial-failure-explanation";
import { TrialPhaseTiming } from "./trial-phase-timing";
import { TrialRunIdentity } from "./trial-run-identity";
import { TrialTaskProgress } from "./trial-task-progress";
import {
  hasFailureDiagnostics,
  hasVerificationResults,
  TrialFailureDiagnostics,
} from "./trial-failure-diagnostics";

export function TrialTerminal({ run }: { run: TrialRunState }) {
  const {
    artifacts, busy, confirmDirective, confirmRecoveryRun, created, directive, directiveText,
    evidenceAnnouncement, evidenceDocument, evidenceError, evidenceLoading, evidenceOpen,
    persistDirective, proposeRecoveryRun, readArtifact, readEvents, readRecoveryDocument,
    recoveryRun, recoveryRunAcknowledged, session, setDirective, setDirectiveText,
    setRecoveryRunAcknowledged, setStage, stage, startNewRun, terminalRef,
  } = run;
  const evidenceViewerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (evidenceDocument === null || evidenceLoading) return;
    const frame = window.requestAnimationFrame(() => {
      const target = evidenceViewerRef.current;
      if (target === null) return;
      target.focus({ preventScroll: true });
      target.scrollIntoView({ behavior: "smooth", block: "start" });
    });
    return () => window.cancelAnimationFrame(frame);
  }, [evidenceDocument, evidenceLoading]);

  return (
    <>
      {stage === "terminal" && session !== null && (
        <section
          className="terminal-grid"
          data-testid="terminal-gate"
          ref={terminalRef}
          tabIndex={-1}
        >
          <article className="panel verdict-card">
            <span className="panel-index">
              実行結果 / {session.gate === "gate_3" ? "Gate 3" : "Gate 4"}
            </span>
            <h2 data-testid="terminal-result-heading">{terminalHeading(session)}</h2>
            <p className="terminal-gate-explanation">{gateExplanation(session.gate)}</p>
            <dl className="terminal-result-summary" data-testid="terminal-result-summary">
              <div>
                <dt>結果</dt>
                <dd data-testid="terminal-verdict-summary">{resultSummary(session)}</dd>
              </div>
              <div>
                <dt>{session.gate === "gate_3" ? "判定理由" : "原因"}</dt>
                <dd data-testid="terminal-assurance-summary">{reasonSummary(session)}</dd>
              </div>
              <div>
                <dt>次の一手</dt>
                <dd data-testid="terminal-status-summary">{nextActionSummary(session)}</dd>
              </div>
            </dl>
            {session.gate === "gate_4" && session.failure_explanation != null && (
              <TrialFailureExplanation
                busy={busy}
                evidenceLoading={evidenceLoading}
                explanation={session.failure_explanation}
                onConfirmRecoveryRun={confirmRecoveryRun}
                onApplyToContinuation={(value) => {
                  setDirectiveText(value);
                  setDirective(null);
                }}
                onOpenArtifact={readArtifact}
                onOpenEvents={readEvents}
                onOpenRecoveryDocument={readRecoveryDocument}
                onProposeRecoveryRun={proposeRecoveryRun}
                recoveryRun={recoveryRun}
                recoveryRunAcknowledged={recoveryRunAcknowledged}
                setRecoveryRunAcknowledged={setRecoveryRunAcknowledged}
              />
            )}
            {(session.gate === "gate_4" && session.failure_explanation == null &&
              (session.status === "failed" ||
              hasFailureDiagnostics(session.failure_diagnostics, session.stop_reason))) && (
              <TrialFailureDiagnostics
                diagnostics={session.failure_diagnostics}
                fallbackStopReason={session.stop_reason}
                testId="terminal-failure-diagnostics"
              />
            )}
            {session.gate === "gate_3" &&
              hasVerificationResults(session.failure_diagnostics) && (
                <TrialFailureDiagnostics
                  diagnostics={session.failure_diagnostics}
                  mode="verification"
                  testId="terminal-verification-results"
                />
            )}
            <TrialRunIdentity
              identity={session.identity}
              recovery={session.recovery_auto_run}
            />
            <TrialPhaseTiming
              phases={session.phases}
              totalProcessingDurationMs={session.total_processing_duration_ms ?? null}
            />
            <TrialTaskProgress
              evidenceLoading={evidenceLoading}
              onOpenEvents={readEvents}
              progress={session.task_progress}
              terminal
            />
            <Link
              className="terminal-history-link"
              data-testid="terminal-session-history-link"
              href={`${trialRoutePath("history")}#trial-session-${session.id}`}
            >
              このセッションをトライアル実行履歴で確認
            </Link>
            <details className="acceptance-sheet-details" data-testid="terminal-acceptance-details">
              <summary>受入シートの詳細を表示</summary>
              <pre>
                {session.acceptance_sheet ?? "実行結果の証跡が不足しているため、受入シートは生成されていません。"}
              </pre>
            </details>
          </article>
          <aside className="panel next-action-card">
            <span className="panel-index">任意の次の操作</span>
            <h2>追加の依頼を入力</h2>
            <p>保存前に認証情報を除去し、実行前に内容をもう一度確認します。確定済みの必須チェックは変更できません。</p>
            <label htmlFor="directive-input">追加の依頼</label>
            <textarea
              data-testid="directive-input"
              id="directive-input"
              onChange={(event) => {
                setDirectiveText(event.target.value);
                setDirective(null);
              }}
              placeholder="実行結果を踏まえた追加の依頼を入力…"
              rows={4}
              value={directiveText}
            />
            <button
              className="secondary-action"
              disabled={busy || directive !== null || directiveText.trim() === ""}
              onClick={() => void persistDirective()}
              type="button"
            >
              追加の依頼を確認用に準備
            </button>
            {directive !== null && (
              <div className="directive-receipt" data-testid="directive-receipt">
                <strong>{directive.scrubbed_directive}</strong>
                <code>{directive.directive_hash}</code>
                <small>{trialGateLabel(directive.issued_gate)} · 追加依頼 {directive.directive_round}</small>
                <button
                  className="primary-action"
                  disabled={busy}
                  onClick={() => void confirmDirective()}
                  type="button"
                >
                  確認して追加の依頼を実行
                </button>
              </div>
            )}
            <button
              className="close-action"
              data-testid="close-session"
              onClick={() => setStage("closed")}
              type="button"
            >
              追加実行せず終了
            </button>
          </aside>
        </section>
      )}

      {evidenceOpen && created !== null && (stage === "gate_2" || stage === "terminal") && (
        <section className="panel session-files-panel" data-testid="trial-session-files">
          <header className="panel-heading">
            <div>
              <span className="panel-index">読み取り専用セッションファイル</span>
              <h2>セッションファイル</h2>
            </div>
            <span>{evidenceLoading ? "読み込み中…" : `${artifacts.length} 件`}</span>
          </header>
          {evidenceError !== null && <p className="trial-error" role="alert">{evidenceError}</p>}
          <div className="session-files-workbench">
            <aside className="session-file-list" aria-label="セッションファイル">
              <button
                className={evidenceDocument?.path === "events.jsonl" ? "active" : ""}
                data-testid="trial-events-open"
                disabled={evidenceLoading}
                onClick={() => void readEvents()}
                type="button"
              >
                <span>events.jsonl</span>
                <small>直近 200 行</small>
              </button>
              {artifacts.filter((artifact) => artifact.path !== "events.jsonl").map((artifact) => (
                <button
                  className={evidenceDocument?.path === artifact.path ? "active" : ""}
                  data-testid={artifact.path === "summary.md" ? "trial-summary-open" : undefined}
                  disabled={evidenceLoading}
                  key={artifact.path}
                  onClick={() => void readArtifact(artifact.path)}
                  type="button"
                >
                  <span>{artifact.id}</span>
                  <small>{byteLabel(artifact.size_bytes)}</small>
                </button>
              ))}
            </aside>
            <div
              aria-label={evidenceDocument === null
                ? "セッション文書ビューアー"
                : `${evidenceDocument.id} 文書ビューアー`}
              className="session-file-document"
              data-testid="trial-file-viewer"
              ref={evidenceViewerRef}
              role="region"
              tabIndex={-1}
            >
              <p
                aria-atomic="true"
                aria-live="polite"
                className="trial-copy-announcement"
                data-testid="trial-document-open-announcement"
                role="status"
              >
                {evidenceAnnouncement ?? ""}
              </p>
              <DocumentViewer
                document={evidenceDocument}
                empty="イベント、サマリー、または受入成果物を選択すると、ここに表示します。"
              />
            </div>
          </div>
        </section>
      )}

      {stage === "closed" && (
        <section className="panel closed-card" data-testid="closed-session" tabIndex={-1}>
          <span>セッション終了</span>
          <h2>追加の操作は実行されていません。</h2>
          <button
            className="primary-action"
            data-testid="start-new-run"
            onClick={startNewRun}
            type="button"
          >
            新しい実行を開始
          </button>
        </section>
      )}
    </>
  );
}

export function terminalHeading(session: PolledSession): string {
  return session.gate === "gate_3"
    ? "すべての必須チェックに合格しました"
    : "実行結果と次の一手を確認してください";
}

function gateExplanation(gate: string): string {
  return gate === "gate_3"
    ? "Gate 3 は、固定された必須チェックをすべて満たした実行結果です。"
    : "Gate 4 は、未達または不十分な必須チェックがあり、証跡と次の操作を確認する結果です。";
}

function resultSummary(session: PolledSession): string {
  return `${statusSummary(session.status)} ${verdictSummary(session.verdict)}`;
}

function verdictSummary(verdict: string | null): string {
  if (verdict === null) return "最終受け入れは記録されていません。";
  if (["static", "partial", "none", "reduced"].includes(verdict)) {
    return "最終受け入れは記録されていません。";
  }
  return ["pass", "passed", "full", "full_success", "completed"].includes(verdict)
    ? "最終受け入れは合格として記録されています。"
    : "最終受け入れは不合格として記録されています。";
}

function reasonSummary(session: PolledSession): string {
  if (session.gate === "gate_3") {
    return "独立検証を含む必須チェックがすべて合格しました。";
  }
  const assurance = assuranceSummary(session.assurance);
  const assuranceReason = translateAssuranceReason(session.assurance_reason);
  const stopReason = translateStopReason(session.stop_reason);
  if (stopReason !== null && assuranceReason !== null) {
    return `${stopReason} ${assurance} ${assuranceReason}`;
  }
  if (stopReason !== null) return `${stopReason} ${assurance}`;
  if (assuranceReason !== null) return `${assurance} ${assuranceReason}`;
  return `${assurance} 原因は記録されていません。`;
}

function assuranceSummary(assurance: string | null): string {
  switch (assurance) {
    case "full": return "必要な実行証跡がすべて記録されています。";
    case "partial": return "必要な実行証跡の一部だけが記録されています。";
    case "static": return "実行検証は完了しておらず、静的な証跡だけが記録されています。";
    case "failed": return "記録された証跡に、必須チェックの不合格があります。";
    case null: return "保証水準は記録されていません。";
    default: return "詳しい保証水準は下の受入シートで確認してください。";
  }
}

function translateAssuranceReason(reason: string | null | undefined): string | null {
  if (reason === null || reason === undefined || reason.trim() === "") return null;
  switch (reason) {
    case "cli_probe_not_run":
      return "独立した CLI 動作プローブは実行されていません。";
    case "data_profile_probe_not_run":
      return "独立したデータ動作プローブは実行されていません。";
    case "investigation_probe_not_run":
      return "独立した調査プローブは実行されていません。";
    case "profile_not_admitted":
      return "未承認プロファイルのため、保証上限は static です。";
    case "acceptance_not_full_success":
      return "最終受け入れが full success に到達していません。";
    case "generic profile — no capability contract, no behavioral verification":
      return "汎用プロファイルには能力契約と動作検証がありません。";
    default:
      if (reason.startsWith("missing_required_evidence:")) {
        return `必須証跡が不足しています（${reason.slice("missing_required_evidence:".length)}）。`;
      }
      return `記録された保証理由: ${reason}`;
  }
}

function translateStopReason(reason: string | null | undefined): string | null {
  if (reason === null || reason === undefined || reason.trim() === "" || reason === "completed") {
    return null;
  }
  if (reason === "interrupted by user") return "ユーザー操作により中断されました。";
  return `記録された停止理由: ${reason.split("\n", 1)[0]}`;
}

function nextActionSummary(session: PolledSession): string {
  const action = session.next_action?.trim();
  if (action === undefined || action === "" || action === "none") {
    return session.gate === "gate_3"
      ? "追加操作はありません。"
      : "次の一手は記録されていません。";
  }
  switch (action) {
    case "fix_command_failure": return "コマンドの失敗を修正してから再実行します。";
    case "repair_release_gate_failure": return "リリースゲートの不合格を修正して再検証します。";
    case "resume_or_rerun_command": return "コマンドを再開または再実行します。";
    case "inspect_summary_and_resume_or_rerun":
      return "summary.md を確認してから再開または再実行します。";
    case "run_setup_interaction_probe_to_enable_interaction_release_checks":
      return "interaction probe を準備して再検証します。";
    case "elevated_model": return "より高性能なモデルを選び、Gate 1 から再確認します。";
    default: return `記録された次の一手: ${action}`;
  }
}

function statusSummary(status: string): string {
  switch (status) {
    case "completed": return "実行は完了しました。";
    case "failed": return "実行は失敗として終了しました。";
    case "interrupted": return "実行は中断されました。";
    case "starting": return "実行を開始しています。";
    case "running": return "実行中です。";
    default: return "詳しい状態は下の受入シートで確認してください。";
  }
}
